//! Compact, self-contained encoding for a single marking, used to store
//! [`DenseStateGraph`](super::DenseStateGraph) node weights as a
//! [`FixedBitSet`] instead of a full [`IdxMarking`] vector.
//!
//! # Why this exists
//!
//! [`IdxMarking<T>`](IdxMarking) stores one `T` (4 bytes, for `u32`) per
//! place, unconditionally -- a place that's almost always `0` or `1` still
//! costs 4 whole bytes, in every one of the (potentially millions of)
//! markings a state-space exploration discovers. `seen`
//! ([`MarkingDecisionDiagram`](super::seen::MarkingDecisionDiagram)) already
//! solves this for the *dedup index*, via structural sharing across similar
//! markings -- but that sharing is a property of the decision diagram, not
//! of `petgraph`, so it does nothing for the graph's own node weights, which
//! are stored flatly with no sharing between nodes at all. Left unaddressed,
//! the graph itself becomes the dominant memory cost of exploration,
//! quietly undoing much of what the decision-diagram-backed `seen` bought.
//! This module is what closes that gap for the graph's own storage.
//!
//! # The encoding
//!
//! Each place's value is written one after another, in place-index order,
//! via [Rice coding](https://en.wikipedia.org/wiki/Golomb_coding) -- the
//! standard, well-studied encoding for "many small values, exponentially
//! rarer as they grow" data (used, e.g., by JPL/NASA for spacecraft
//! telemetry) -- with a bounded escape hatch added for outliers, since a
//! Petri net place *can* hold an occasional very large count and pure Rice
//! coding has no defense against that (its cost is unbounded in the value
//! itself). Concretely, for a value `n` and parameter `k`
//! ([`RiceParams`]):
//!
//! - **common case** -- if the quotient `n >> k` is less than
//!   [`ESCAPE_AFTER`]: write that many one-bits, then a zero stop bit
//!   (unary-encoding the quotient), then the low `k` bits of `n` (the
//!   remainder). Larger `k` gives more values this flat, escape-free
//!   treatment, at the cost of `k` wasted bits even for `n == 0`; there is
//!   no single best `k` for every net, so it's exposed as a parameter (see
//!   `explore_k` in this module's tests to compare a given net's real
//!   numbers against different `k`).
//! - **escape** -- otherwise: [`ESCAPE_AFTER`] one-bits with *no* stop bit.
//!   This is unambiguous on decode: the common case can only ever produce
//!   *fewer than* `ESCAPE_AFTER` ones before its stop bit, so seeing that
//!   many in a row with no zero in between can only mean escape, never a
//!   coincidence. Follow with [`ESCAPE_LENGTH_BITS`] bits giving how many
//!   bits `n` itself needs, then `n` written directly as that many bits.
//!
//! [`Omega`] reuses this unchanged for its finite case, preceded by one flag
//! bit ("is this place unbounded?") -- mirroring how
//! [`MtbddEncode`] handles the same distinction for the decision-diagram
//! encoding.
//!
//! Both the common and escape cases cost `O(1)` *decode* work per place (a
//! unary run capped at [`ESCAPE_AFTER`], then one or two fixed-width reads)
//! -- unlike a plain, uncapped unary or Elias-gamma code, whose decode cost
//! scales with the value itself. [`BitWriter`]/[`BitReader`] read and write
//! a full machine word at a time via a wide accumulator, rather than one
//! [`FixedBitSet::set`]/[`FixedBitSet::contains`] call per bit, since this
//! is on what's expected to be a very hot path (`marking_at`-style lookups,
//! called constantly during exploration).
//!
//! Not wired into `DenseStateGraph` yet -- exercised only by this module's
//! own tests for now, hence the blanket `dead_code` allow below.

#![allow(dead_code)]

use crate::core::marking::IdxMarking;
use crate::core::state_space::coverability::Omega;
use crate::core::state_space::seen::MtbddEncode;
use fixedbitset::{Block, FixedBitSet};

/// Max unary quotient bits before switching to the escape encoding.
const ESCAPE_AFTER: u32 = 3;
/// Width of the escape length field. 6 bits covers any `bits_needed()` up to
/// 63, comfortably more than `Omega`'s maximum of 33 (32-bit count + flag).
const ESCAPE_LENGTH_BITS: u32 = 6;

/// Rice coding parameters. `k` is the remainder width: larger `k` gives more
/// values a flat, escape-free cost, at a flat cost of `k` bits even for `0`.
#[derive(Debug, Clone, Copy)]
pub struct RiceParams {
    pub k: u32,
}

/// Appends bits to a growing [`FixedBitSet`], a full machine word at a time
/// via a wide accumulator, rather than one [`FixedBitSet::set`] call per bit.
pub struct BitWriter {
    blocks: Vec<Block>,
    acc: u128,
    acc_len: u32,
    bits_written: usize,
}

impl BitWriter {
    #[must_use]
    pub const fn new() -> Self {
        Self { blocks: Vec::new(), acc: 0, acc_len: 0, bits_written: 0 }
    }

    /// Appends the low `width` bits of `value` (LSB first). `width` must be
    /// at most 64.
    ///
    /// The flush loop below always leaves `acc_len < Block::BITS` (at most
    /// 63 bits, on any realistic platform where `Block` is 32 or 64 bits)
    /// by the time this is called again, so `acc_len + width` never exceeds
    /// `63 + 64 = 127` -- always fitting in the 128-bit accumulator with no
    /// overflow risk, regardless of `Block`'s actual width.
    pub fn write_bits(&mut self, value: u64, width: u32) {
        debug_assert!(width <= u64::BITS);
        if width == 0 {
            return;
        }
        let masked = if width == u64::BITS { value } else { value & ((1u64 << width) - 1) };
        self.acc |= u128::from(masked) << self.acc_len;
        self.acc_len += width;
        self.bits_written += width as usize;
        while self.acc_len >= Block::BITS {
            #[allow(clippy::cast_possible_truncation)]
            self.blocks.push(self.acc as Block);
            self.acc >>= Block::BITS;
            self.acc_len -= Block::BITS;
        }
    }

    /// Appends a single bit.
    pub fn write_bit(&mut self, bit: bool) {
        self.write_bits(u64::from(bit), 1);
    }

    /// Appends `count` one-bits.
    pub fn write_ones(&mut self, count: u32) {
        let mut remaining = count;
        while remaining > 0 {
            let chunk = remaining.min(u64::BITS);
            let value = if chunk == u64::BITS { u64::MAX } else { (1u64 << chunk) - 1 };
            self.write_bits(value, chunk);
            remaining -= chunk;
        }
    }

    /// Consumes the writer, producing the final [`FixedBitSet`], sized to
    /// exactly the number of bits written (no trailing padding beyond what
    /// [`FixedBitSet`] itself requires internally).
    #[must_use]
    pub fn finish(mut self) -> FixedBitSet {
        if self.acc_len > 0 {
            #[allow(clippy::cast_possible_truncation)]
            self.blocks.push(self.acc as Block);
        }
        FixedBitSet::with_capacity_and_blocks(self.bits_written, self.blocks)
    }
}

impl Default for BitWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads bits back out of a block slice (e.g. [`FixedBitSet::as_slice`]),
/// mirroring [`BitWriter`].
pub struct BitReader<'a> {
    blocks: &'a [Block],
    next_block: usize,
    acc: u128,
    acc_len: u32,
}

impl<'a> BitReader<'a> {
    #[must_use]
    pub const fn new(blocks: &'a [Block]) -> Self {
        Self { blocks, next_block: 0, acc: 0, acc_len: 0 }
    }

    fn refill(&mut self) {
        while self.acc_len + Block::BITS <= u128::BITS && self.next_block < self.blocks.len() {
            // `Block = usize` is at most 64 bits on any realistic target, so
            // this widening cast to `u128` is always lossless.
            self.acc |= (self.blocks[self.next_block] as u128) << self.acc_len;
            self.acc_len += Block::BITS;
            self.next_block += 1;
        }
    }

    /// Reads and consumes `width` (at most 64) bits, LSB first.
    ///
    /// Panics (in debug builds) if fewer than `width` bits remain -- callers
    /// are expected to know their own format's shape (e.g. via
    /// [`MtbddEncode::bits_needed`]) and never read past it.
    #[allow(clippy::cast_possible_truncation)]
    pub fn read_bits(&mut self, width: u32) -> u64 {
        debug_assert!(width <= u64::BITS);
        if width == 0 {
            return 0;
        }
        self.refill();
        debug_assert!(self.acc_len >= width, "read past the end of the bit stream");
        let mask = if width == u64::BITS { u64::MAX } else { (1u64 << width) - 1 };
        let value = self.acc as u64 & mask;
        self.acc >>= width;
        self.acc_len -= width;
        value
    }

    /// Counts consecutive one-bits, up to `cap`.
    ///
    /// Returns `(count, true)` if `cap` ones were read with no intervening
    /// zero-bit (the escape case) -- the zero that would normally terminate
    /// the run is *not* consumed, because there isn't one to consume yet.
    /// Returns `(count, false)` if a terminating zero-bit was found first
    /// (and consumed) after fewer than `cap` ones.
    pub fn read_capped_unary(&mut self, cap: u32) -> (u32, bool) {
        let mut count = 0;
        while count < cap {
            self.refill();
            debug_assert!(self.acc_len > 0, "read past the end of the bit stream");
            let bit_is_one = self.acc & 1 == 1;
            self.acc >>= 1;
            self.acc_len -= 1;
            if !bit_is_one {
                return (count, false);
            }
            count += 1;
        }
        (count, true)
    }
}

/// Per-token-type Rice(k)+escape encoding, composed on top of
/// [`MtbddEncode`] (whose `bits_needed`/`bit_at`/`from_bits` already handle
/// the escape fallback and, for [`Omega`], the unbounded flag).
pub trait RiceEncode: Sized {
    fn rice_encode(&self, writer: &mut BitWriter, params: RiceParams);
    fn rice_decode(reader: &mut BitReader<'_>, params: RiceParams) -> Self;
}

impl RiceEncode for u32 {
    fn rice_encode(&self, writer: &mut BitWriter, params: RiceParams) {
        rice_encode_u32(writer, *self, params);
    }

    fn rice_decode(reader: &mut BitReader<'_>, params: RiceParams) -> Self {
        rice_decode_u32(reader, params)
    }
}

/// `Omega` is encoded as one flag bit (unbounded?), then -- only if finite
/// -- the count Rice-coded exactly as a plain `u32` would be. This mirrors
/// how `MtbddEncode for Omega` is built on top of `MtbddEncode for u32`.
impl RiceEncode for Omega {
    fn rice_encode(&self, writer: &mut BitWriter, params: RiceParams) {
        writer.write_bit(self.is_unbounded());
        if let Omega::Finite(count) = self {
            rice_encode_u32(writer, *count, params);
        }
    }

    fn rice_decode(reader: &mut BitReader<'_>, params: RiceParams) -> Self {
        if reader.read_bits(1) == 1 {
            Omega::Unbounded
        } else {
            Omega::Finite(rice_decode_u32(reader, params))
        }
    }
}

fn rice_encode_u32(writer: &mut BitWriter, value: u32, params: RiceParams) {
    let quotient = u64::from(value) >> params.k;
    if quotient < u64::from(ESCAPE_AFTER) {
        let quotient = u32::try_from(quotient).expect("quotient < ESCAPE_AFTER, which fits comfortably in u32");
        writer.write_ones(quotient);
        writer.write_bit(false);
        writer.write_bits(u64::from(value), params.k);
    } else {
        writer.write_ones(ESCAPE_AFTER);
        // `bits_needed()` is reused from `MtbddEncode` to avoid a second
        // "how many bits does this value need" formula living in two
        // places, but the value itself is written directly as one native
        // integer -- `value < 2^bits_needed(value)` always holds by that
        // method's own definition, so this round-trips exactly. No need to
        // go through `bit_at()` bit by bit here (that exists for `Omega`,
        // where a value isn't always a plain machine integer).
        let length = u32::try_from(value.bits_needed()).expect("u32::bits_needed() is at most 32");
        writer.write_bits(u64::from(length), ESCAPE_LENGTH_BITS);
        writer.write_bits(u64::from(value), length);
    }
}

fn rice_decode_u32(reader: &mut BitReader<'_>, params: RiceParams) -> u32 {
    let (quotient, hit_cap) = reader.read_capped_unary(ESCAPE_AFTER);
    if hit_cap {
        let length = u32::try_from(reader.read_bits(ESCAPE_LENGTH_BITS))
            .expect("a 6-bit length field always fits in u32");
        u32::try_from(reader.read_bits(length)).expect("value fits in u32 by construction")
    } else {
        let remainder = u32::try_from(reader.read_bits(params.k)).expect("remainder fits in u32 by construction");
        (quotient << params.k) | remainder
    }
}

/// Encodes an entire marking as a [`FixedBitSet`], one place after another
/// in place-index order, each via [`RiceEncode`].
pub fn encode_marking<T: RiceEncode>(marking: &IdxMarking<T>, params: RiceParams) -> FixedBitSet {
    let mut writer = BitWriter::new();
    for value in marking.iter() {
        value.rice_encode(&mut writer, params);
    }
    writer.finish()
}

/// Decodes a marking with `place_count` places back out of `bits`, the
/// inverse of [`encode_marking`]. `place_count` is a static fact about the
/// net, not something read from `bits` itself.
pub fn decode_marking<T: RiceEncode>(
    bits: &FixedBitSet,
    place_count: usize,
    params: RiceParams,
) -> IdxMarking<T> {
    let mut reader = BitReader::new(bits.as_slice());
    (0..place_count).map(|_| T::rice_decode(&mut reader, params)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_writer_reader_round_trip_mixed_widths() {
        // Deliberately odd, block-boundary-straddling widths.
        let fields: &[(u64, u32)] =
            &[(0, 1), (1, 1), (0b101, 3), (0xFF, 8), (0, 0), (u64::MAX, 64), (0x1234_5678, 32), (1, 1)];

        let mut writer = BitWriter::new();
        for &(value, width) in fields {
            writer.write_bits(value, width);
        }
        let bits = writer.finish();

        let mut reader = BitReader::new(bits.as_slice());
        for &(value, width) in fields {
            let expected = if width == 0 { 0 } else if width == 64 { value } else { value & ((1 << width) - 1) };
            assert_eq!(reader.read_bits(width), expected, "width {width}");
        }
    }

    #[test]
    fn capped_unary_below_cap() {
        let mut writer = BitWriter::new();
        writer.write_ones(2);
        writer.write_bit(false);
        writer.write_bits(0b101, 3); // sentinel to check position after
        let bits = writer.finish();

        let mut reader = BitReader::new(bits.as_slice());
        assert_eq!(reader.read_capped_unary(5), (2, false));
        assert_eq!(reader.read_bits(3), 0b101);
    }

    #[test]
    fn capped_unary_hits_cap() {
        let mut writer = BitWriter::new();
        writer.write_ones(3); // no stop bit -- exactly the escape shape
        writer.write_bits(0b110, 3);
        let bits = writer.finish();

        let mut reader = BitReader::new(bits.as_slice());
        assert_eq!(reader.read_capped_unary(3), (3, true));
        assert_eq!(reader.read_bits(3), 0b110);
    }

    fn rice_round_trip_u32(k: u32) {
        let params = RiceParams { k };
        let mut interesting: Vec<u32> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 15, 16, 17, 28, 31, 32, 63, 100, 1_000_000, u32::MAX];
        // Values right at this k's escape boundary, since off-by-ones love
        // to hide exactly there.
        let boundary = ESCAPE_AFTER << k;
        interesting.extend([boundary.saturating_sub(1), boundary, boundary + 1]);

        for value in interesting {
            let mut writer = BitWriter::new();
            rice_encode_u32(&mut writer, value, params);
            let bits = writer.finish();
            let mut reader = BitReader::new(bits.as_slice());
            assert_eq!(rice_decode_u32(&mut reader, params), value, "k={k}, value={value}");
        }
    }

    #[test]
    fn rice_round_trip_all_k() {
        for k in 0..8 {
            rice_round_trip_u32(k);
        }
    }

    #[test]
    fn rice_round_trip_omega() {
        for k in 0..5 {
            let params = RiceParams { k };
            let values = [Omega::Finite(0), Omega::Finite(1), Omega::Finite(1000), Omega::Unbounded];
            for value in values {
                let mut writer = BitWriter::new();
                value.rice_encode(&mut writer, params);
                let bits = writer.finish();
                let mut reader = BitReader::new(bits.as_slice());
                assert_eq!(Omega::rice_decode(&mut reader, params), value, "k={k}, value={value:?}");
            }
        }
    }

    #[test]
    fn marking_round_trip_u32() {
        let params = RiceParams { k: 2 };
        let marking: IdxMarking<u32> = [0, 1, 1_000_000, 5, 0, 17].into();
        let bits = encode_marking(&marking, params);
        let decoded: IdxMarking<u32> = decode_marking(&bits, marking.place_count(), params);
        assert_eq!(decoded, marking);
    }

    #[test]
    fn marking_round_trip_omega() {
        let params = RiceParams { k: 1 };
        let marking: IdxMarking<Omega> =
            [Omega::Finite(0), Omega::Unbounded, Omega::Finite(28), Omega::Finite(0)].into();
        let bits = encode_marking(&marking, params);
        let decoded: IdxMarking<Omega> = decode_marking(&bits, marking.place_count(), params);
        assert_eq!(decoded, marking);
    }

    /// Not a correctness check (round-trip tests above cover that) -- prints
    /// real, measured encoded sizes across a range of `k`, for the same
    /// scenarios discussed by hand earlier, so `k` can be explored against
    /// actual code instead of arithmetic on paper. Run with
    /// `cargo test -p petrivet --lib bitpack::tests::explore_k -- --nocapture`.
    #[test]
    fn explore_k() {
        let scenarios: &[(&str, &[u32])] = &[
            ("1. mutex-style", &[1, 0, 0, 1, 0, 0, 1]),
            ("2. bounded counter", &[1, 0, 17, 0, 1]),
            ("3. growing counter", &[1, 0, 28]),
            ("4. adversarial", &[0, 0, 1_000_000]),
        ];

        println!("\n{:<20} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}", "scenario", "k=0", "k=1", "k=2", "k=3", "k=4", "k=5");
        for (name, values) in scenarios {
            let marking: IdxMarking<u32> = values.iter().copied().collect();
            let costs: Vec<usize> = (0..=5)
                .map(|k| encode_marking(&marking, RiceParams { k }).len())
                .collect();
            println!(
                "{name:<20} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
                costs[0], costs[1], costs[2], costs[3], costs[4], costs[5]
            );
        }
    }
}
