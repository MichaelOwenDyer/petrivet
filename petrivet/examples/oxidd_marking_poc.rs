//! Proof of concept: using an MTBDD as a growable `HashMap<Marking, NodeIndex>`
//! replacement.
//!
//! A "marking" here is just `[u32; PLACES]`. Each place is binary-encoded
//! into as many manager variables as its largest-seen value needs so far
//! (LSB first) -- allocated lazily, one `add_vars(1)` at a time, the first
//! time a place's value needs another bit.
//!
//! There is no native "set a single point" (ITE) operation for MTBDDs in
//! oxidd 0.11 (the `MTBDDOp::Ite` enum variant exists but is unused/dead in
//! the current implementation). So insertion is done the classic ADD way:
//! build a 0/1-valued "indicator" function for the exact marking (the
//! product of `var` / `1 - var` literals, i.e. the arithmetic form of a
//! cube), then
//!
//!     new_map = old_map * (1 - indicator) + value * indicator
//!
//! which leaves every other point untouched and overwrites exactly the one
//! point the indicator singles out. `0` is reserved to mean "not present";
//! stored values are `NodeIndex + 1`.
//!
//! `oxidd`'s manager has a *fixed* node capacity chosen at construction --
//! there is no live resize API. `MarkingMap::grow()` demonstrates the
//! pragmatic workaround: on `OutOfMemory`, build a new, bigger manager and
//! replay every `(marking, value)` pair recorded so far. In the real
//! integration that replay list doesn't need separate bookkeeping -- it's
//! exactly `petgraph::Graph::node_weights()`, which the exploration already
//! keeps around for other reasons (path reconstruction, deadlock scans).

use oxidd::Function;
use oxidd::Manager;
use oxidd::ManagerRef;
use oxidd::PseudoBooleanFunction;
use oxidd::mtbdd::terminal::I64;
use oxidd::mtbdd::{MTBDDFunction, MTBDDManagerRef};
use oxidd::util::AllocResult;
use oxidd_dump::dot::dump_all;

const PLACES: usize = 2;

/// Number of bits needed to represent `value` (0 for `value == 0`, i.e. an
/// all-zero place costs no variables at all until it needs to be told apart
/// from something nonzero).
fn bits_for(value: u32) -> usize {
    (u32::BITS - value.leading_zeros()) as usize
}

/// `HashMap<[u32; PLACES], NodeIndex>`, backed by one MTBDD.
struct MarkingMap {
    manager_ref: MTBDDManagerRef<I64>,
    capacity: usize,
    /// VarNo of each allocated bit per place, LSB first. Grows lazily.
    place_bits: Vec<Vec<u32>>,
    seen: MTBDDFunction<I64>,
    /// Stand-in for `petgraph::Graph::node_weights()`: replayed into a fresh,
    /// bigger manager whenever the current one runs out of capacity.
    known: Vec<([u32; PLACES], i64)>,
}

impl MarkingMap {
    fn new(initial_capacity: usize) -> AllocResult<Self> {
        let manager_ref: MTBDDManagerRef<I64> =
            oxidd::mtbdd::new_manager(initial_capacity, initial_capacity, initial_capacity, 1);
        let seen = manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(0)))?;
        Ok(Self {
            manager_ref,
            capacity: initial_capacity,
            place_bits: vec![Vec::new(); PLACES],
            seen,
            known: Vec::new(),
        })
    }

    /// Lateral growth: add manager variables so that `place` can represent
    /// `value`, if it can't already.
    fn ensure_bits(&mut self, place: usize, value: u32) {
        let needed = bits_for(value);
        let have = self.place_bits[place].len();
        if have < needed {
            let additional = (needed - have) as u32;
            let new_vars: Vec<u32> = self
                .manager_ref
                .with_manager_exclusive(|manager| manager.add_vars(additional).collect());
            println!(
                "  place {place}: grew from {have} to {needed} bits (value {value})"
            );
            self.place_bits[place].extend(new_vars);
        }
    }

    /// The arithmetic cube: 1 at exactly `marking`, 0 everywhere else.
    fn indicator(&self, marking: &[u32; PLACES]) -> AllocResult<MTBDDFunction<I64>> {
        self.manager_ref.with_manager_shared(|m| {
            let one = MTBDDFunction::constant(m, I64::Num(1))?;
            let mut acc = one.clone();
            for place in 0..PLACES {
                for (bit, &varno) in self.place_bits[place].iter().enumerate() {
                    let v = MTBDDFunction::var(m, varno)?;
                    let bit_is_set = (marking[place] >> bit) & 1 == 1;
                    let literal = if bit_is_set { v } else { one.sub(&v)? };
                    acc = acc.mul(&literal)?;
                }
            }
            Ok(acc)
        })
    }

    /// `seen = seen * (1 - indicator) + value * indicator`.
    fn overwrite(&self, indicator: &MTBDDFunction<I64>, value: i64) -> AllocResult<MTBDDFunction<I64>> {
        let one = self.manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(1)))?;
        let value = self.manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(value)))?;
        let keep_old = one.sub(indicator)?;
        self.seen.mul(&keep_old)?.add(&indicator.mul(&value)?)
    }

    fn insert(&mut self, marking: [u32; PLACES], value: i64) -> AllocResult<()> {
        for place in 0..PLACES {
            self.ensure_bits(place, marking[place]);
        }
        self.known.push((marking, value));
        // Both cube construction and the arithmetic overwrite can run out of
        // nodes -- either failure means "rebuild bigger and replay
        // everything", `known` (this call included).
        match self.indicator(&marking).and_then(|ind| self.overwrite(&ind, value)) {
            Ok(new_seen) => {
                self.seen = new_seen;
                Ok(())
            }
            Err(_out_of_memory) => self.grow(),
        }
    }

    fn grow(&mut self) -> AllocResult<()> {
        let old_capacity = self.capacity;
        self.capacity *= 2;
        println!("-- capacity {old_capacity} exhausted; rebuilding at {} --", self.capacity);

        self.manager_ref = oxidd::mtbdd::new_manager(self.capacity, self.capacity, self.capacity, 1);
        self.seen = self.manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(0)))?;
        for bits in &mut self.place_bits {
            bits.clear();
        }

        let known = std::mem::take(&mut self.known);
        for (marking, value) in known {
            self.insert(marking, value)?; // may recurse into `grow()` again
        }
        Ok(())
    }

    fn lookup(&self, marking: &[u32; PLACES]) -> Option<u32> {
        // If this marking needs more bits than we've ever allocated for some
        // place, it was never inserted -- don't evaluate with truncated bits,
        // which could alias onto an unrelated, already-stored marking.
        for place in 0..PLACES {
            if bits_for(marking[place]) > self.place_bits[place].len() {
                return None;
            }
        }
        let args = (0..PLACES).flat_map(|place| {
            self.place_bits[place]
                .iter()
                .enumerate()
                .map(move |(bit, &varno)| (varno, (marking[place] >> bit) & 1 == 1))
        });
        match self.seen.eval(args) {
            I64::Num(0) => None,
            I64::Num(n) => Some((n - 1) as u32),
            other => panic!("unexpected terminal: {other:?}"),
        }
    }
}

fn main() -> AllocResult<()> {
    // Deliberately tiny, to force at least one capacity regrow partway
    // through -- see MarkingMap::grow().
    let mut map = MarkingMap::new(8)?;

    // Place 0 climbs 0..=13 (needs up to 4 bits), place 1 cycles 0..=3
    // (needs up to 2 bits) -- asymmetric growth on purpose.
    let markings: Vec<[u32; PLACES]> = (0..14u32).map(|i| [i, (i * 3) % 4]).collect();

    for (idx, &marking) in markings.iter().enumerate() {
        map.insert(marking, idx as i64 + 1)?;
    }

    for (idx, &marking) in markings.iter().enumerate() {
        assert_eq!(map.lookup(&marking), Some(idx as u32));
    }
    println!("all {} markings round-tripped correctly", markings.len());

    let never_seen = [100, 0]; // needs more bits than place 0 has ever used
    assert_eq!(map.lookup(&never_seen), None);
    println!("{never_seen:?} correctly reports as not present");

    let bit_widths: Vec<usize> = map.place_bits.iter().map(Vec::len).collect();
    println!("final per-place bit widths: {bit_widths:?}");
    println!("final capacity: {}, node_count(seen) = {}", map.capacity, map.seen.node_count());

    map.manager_ref.with_manager_shared(|manager| {
        manager.gc();
        let file = std::fs::File::create("oxidd_marking_poc.dot")
            .expect("could not create oxidd_marking_poc.dot");
        dump_all(file, manager, [(&map.seen, "seen: marking -> NodeIndex+1")])
            .expect("dot export failed");
    });
    println!("wrote oxidd_marking_poc.dot");

    // Interactive visualization is opt-in (it blocks waiting for a browser
    // to connect, which would hang a non-interactive run):
    //   OXIDD_VISUALIZE=1 cargo run -p petrivet --example oxidd_marking_poc
    // Then open https://oxidd.net/vis in your browser -- NOT
    // http://localhost:4000 itself, that URL only serves the raw JSON data
    // the oxidd.net/vis page polls for.
    if std::env::var_os("OXIDD_VISUALIZE").is_some() {
        map.manager_ref.with_manager_shared(|manager| {
            oxidd_dump::Visualizer::new()
                .add("seen", manager, [&map.seen])
                .serve()
                .inspect_err(|e| eprintln!("Visualizer error: {e}"))
                .ok();
        });
    }

    Ok(())
}
