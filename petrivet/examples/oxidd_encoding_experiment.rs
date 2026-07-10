//! Experiment: does variable ordering / per-place encoding scheme explain
//! the MTBDD backend's poor size (and therefore poor time/memory) on
//! `state_machine_sc`, compared to `HashMap`?
//!
//! Current `seen.rs` encoding: binary, LSB-first, one contiguous block of
//! variables per place, blocks allocated in place order ("place-major").
//! Two independent axes are varied here:
//!
//! - Encoding: binary (current) vs. unary/thermometer (bit `i` means
//!   "value > i", i.e. "value >= i+1" -- monotonic in value, so
//!   incrementing/decrementing a place's count flips exactly one bit,
//!   unlike binary where a carry can flip many).
//! - Ordering: place-major (current) vs. interleaved-by-bit-index (bit 0 of
//!   every place, then bit 1 of every place, ...) -- the classic BDD trick
//!   for functions that compare/relate values across several independent
//!   variables of the same "rank", which is exactly what a conserved-token
//!   sum is.
//!
//! All four combinations are built the same way as `MarkingDecisionDiagram`
//! (indicator cube via mul/sub, single ite overlay per marking), with every
//! place's variables allocated upfront (bounded by the known token total,
//! so no incremental narrowing needed) so the comparison isolates encoding
//! + ordering, not insertion-order artifacts already ruled out separately.
//!
//! Run with: `cargo run -p petrivet --release --example oxidd_encoding_experiment`

use oxidd::bdd::{BDDFunction, BDDManagerRef};
use oxidd::mtbdd::terminal::I64;
use oxidd::mtbdd::{MTBDDFunction, MTBDDManagerRef};
use oxidd::util::AllocResult;
use oxidd::{BooleanFunction, Function, Manager, ManagerRef, PseudoBooleanFunction};
use petrivet::prelude::*;
use petrivet::state_space::{ExplorationOrder, StateGraphExplorer};
use std::time::Instant;

const PLACES: usize = 9;
const MAX_TOKENS: u32 = 8; // conserved total for state_machine_sc(8)

fn state_machine_sc(m: u32) -> (PetriNet<Net>, Vec<Place>) {
    let mut b = NetBuilder::new();
    let center = b.add_place();
    let mut places = vec![center];
    for _ in 0..m {
        let t_enter = b.add_transition();
        let arm = b.add_place();
        let t_exit = b.add_transition();
        b.add_arcs((center, t_enter, arm, t_exit, center));
        places.push(arm);
    }
    let net = b.build().unwrap();
    (PetriNet::new(net, [(center, m)]), places)
}

fn real_markings(count: usize) -> Vec<Vec<u32>> {
    let (sys, places) = state_machine_sc(MAX_TOKENS);
    let explorer = StateGraphExplorer::<u32>::new(&sys, ExplorationOrder::BreadthFirst);
    let graph = explorer.build_graph();
    let mut markings: Vec<Vec<u32>> =
        graph.markings().map(|m| places.iter().map(|&p| m.get(p)).collect()).collect();
    markings.truncate(count);
    markings
}

#[derive(Clone, Copy)]
enum Encoding {
    Binary,
    Unary,
}

impl Encoding {
    fn bits_needed(self, max_value: u32) -> usize {
        match self {
            Encoding::Binary => (u32::BITS - max_value.leading_zeros()) as usize,
            Encoding::Unary => max_value as usize,
        }
    }

    /// Bit `i` (LSB-first for binary; threshold-first for unary).
    fn bit_at(self, value: u32, i: usize) -> bool {
        match self {
            Encoding::Binary => (value >> i) & 1 == 1,
            Encoding::Unary => value > i as u32,
        }
    }
}

#[derive(Clone, Copy)]
enum VarOrder {
    PlaceMajor,
    Interleaved,
}

/// Allocates all variables upfront and returns, per place, the manager
/// variables assigned to its bits (in bit-index order), laid out according
/// to `order`.
fn allocate_vars(
    manager_ref: &MTBDDManagerRef<I64>,
    encoding: Encoding,
    order: VarOrder,
) -> Vec<Vec<u32>> {
    let bits_per_place: Vec<usize> = (0..PLACES).map(|_| encoding.bits_needed(MAX_TOKENS)).collect();
    let total_bits: u32 = bits_per_place.iter().sum::<usize>() as u32;
    let vars: Vec<u32> =
        manager_ref.with_manager_exclusive(|manager| manager.add_vars(total_bits).collect());

    let mut place_bits: Vec<Vec<u32>> = vec![Vec::new(); PLACES];
    match order {
        VarOrder::PlaceMajor => {
            let mut idx = 0;
            for place in 0..PLACES {
                for _ in 0..bits_per_place[place] {
                    place_bits[place].push(vars[idx]);
                    idx += 1;
                }
            }
        }
        VarOrder::Interleaved => {
            let max_bits = bits_per_place.iter().copied().max().unwrap_or(0);
            let mut idx = 0;
            for bit in 0..max_bits {
                for place in 0..PLACES {
                    if bit < bits_per_place[place] {
                        place_bits[place].push(vars[idx]);
                        idx += 1;
                    }
                }
            }
        }
    }
    place_bits
}

fn indicator(
    manager_ref: &MTBDDManagerRef<I64>,
    encoding: Encoding,
    place_bits: &[Vec<u32>],
    marking: &[u32],
) -> AllocResult<MTBDDFunction<I64>> {
    manager_ref.with_manager_shared(|m| {
        let one = MTBDDFunction::constant(m, I64::Num(1))?;
        let mut acc = one.clone();
        for place in 0..PLACES {
            for (bit, &varno) in place_bits[place].iter().enumerate() {
                let v = MTBDDFunction::var(m, varno)?;
                let literal = if encoding.bit_at(marking[place], bit) { v } else { one.sub(&v)? };
                acc = acc.mul(&literal)?;
            }
        }
        Ok(acc)
    })
}

fn build_and_measure(markings: &[Vec<u32>], encoding: Encoding, order: VarOrder) -> AllocResult<(usize, u128)> {
    let manager_ref: MTBDDManagerRef<I64> = oxidd::mtbdd::new_manager(1 << 23, 1 << 23, 1 << 23, 1);
    let place_bits = allocate_vars(&manager_ref, encoding, order);
    let mut seen = manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(0)))?;

    let start = Instant::now();
    for (idx, marking) in markings.iter().enumerate() {
        let ind = indicator(&manager_ref, encoding, &place_bits, marking)?;
        let value_fn =
            manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(idx as i64 + 1)))?;
        seen = ind.ite(&value_fn, &seen)?;
    }
    let elapsed = start.elapsed().as_millis();

    Ok((seen.node_count(), elapsed))
}

/// Same encoding/ordering scheme, but as a **plain boolean set** ("is this
/// marking in the seen set?") rather than a map to a distinct value per
/// marking -- isolates whether the poor compression is really about
/// encoding/ordering, or inherent to storing a unique integer per point
/// (which forces N distinct terminals, giving reduction nothing to merge on
/// beyond shared *prefixes*).
fn allocate_vars_bdd(
    manager_ref: &BDDManagerRef,
    encoding: Encoding,
    order: VarOrder,
) -> Vec<Vec<u32>> {
    let bits_per_place: Vec<usize> = (0..PLACES).map(|_| encoding.bits_needed(MAX_TOKENS)).collect();
    let total_bits: u32 = bits_per_place.iter().sum::<usize>() as u32;
    let vars: Vec<u32> =
        manager_ref.with_manager_exclusive(|manager| manager.add_vars(total_bits).collect());

    let mut place_bits: Vec<Vec<u32>> = vec![Vec::new(); PLACES];
    match order {
        VarOrder::PlaceMajor => {
            let mut idx = 0;
            for place in 0..PLACES {
                for _ in 0..bits_per_place[place] {
                    place_bits[place].push(vars[idx]);
                    idx += 1;
                }
            }
        }
        VarOrder::Interleaved => {
            let max_bits = bits_per_place.iter().copied().max().unwrap_or(0);
            let mut idx = 0;
            for bit in 0..max_bits {
                for place in 0..PLACES {
                    if bit < bits_per_place[place] {
                        place_bits[place].push(vars[idx]);
                        idx += 1;
                    }
                }
            }
        }
    }
    place_bits
}

fn indicator_bdd(
    manager_ref: &BDDManagerRef,
    encoding: Encoding,
    place_bits: &[Vec<u32>],
    marking: &[u32],
) -> AllocResult<BDDFunction> {
    manager_ref.with_manager_shared(|m| {
        let mut acc = BDDFunction::t(m);
        for place in 0..PLACES {
            for (bit, &varno) in place_bits[place].iter().enumerate() {
                let v = BDDFunction::var(m, varno)?;
                let literal = if encoding.bit_at(marking[place], bit) { v } else { v.not()? };
                acc = acc.and(&literal)?;
            }
        }
        Ok(acc)
    })
}

fn build_and_measure_bdd_set(
    markings: &[Vec<u32>],
    encoding: Encoding,
    order: VarOrder,
) -> AllocResult<(usize, u128)> {
    let manager_ref: BDDManagerRef = oxidd::bdd::new_manager(1 << 23, 1 << 23, 1);
    let place_bits = allocate_vars_bdd(&manager_ref, encoding, order);
    let mut seen = manager_ref.with_manager_shared(BDDFunction::f);

    let start = Instant::now();
    for marking in markings {
        let ind = indicator_bdd(&manager_ref, encoding, &place_bits, marking)?;
        seen = seen.or(&ind)?;
    }
    let elapsed = start.elapsed().as_millis();

    Ok((seen.node_count(), elapsed))
}

fn main() -> AllocResult<()> {
    let num_markings: usize =
        std::env::var("NUM_MARKINGS").ok().and_then(|s| s.parse().ok()).unwrap_or(2000);
    let markings = real_markings(num_markings);
    println!("using {} real markings from state_machine_sc({MAX_TOKENS})", markings.len());
    println!();
    println!("--- MTBDD: marking -> distinct NodeIndex (current design) ---");
    println!("{:<12} {:<14} {:>12} {:>12}", "encoding", "ordering", "node_count", "build_ms");
    for (encoding, enc_name) in [(Encoding::Binary, "binary"), (Encoding::Unary, "unary")] {
        for (order, order_name) in [(VarOrder::PlaceMajor, "place-major"), (VarOrder::Interleaved, "interleaved")] {
            let (node_count, ms) = build_and_measure(&markings, encoding, order)?;
            println!("{enc_name:<12} {order_name:<14} {node_count:>12} {ms:>12}");
        }
    }

    println!();
    println!("--- plain BDD: marking in seen-set? (boolean, no per-marking value) ---");
    println!("{:<12} {:<14} {:>12} {:>12}", "encoding", "ordering", "node_count", "build_ms");
    for (encoding, enc_name) in [(Encoding::Binary, "binary"), (Encoding::Unary, "unary")] {
        for (order, order_name) in [(VarOrder::PlaceMajor, "place-major"), (VarOrder::Interleaved, "interleaved")] {
            let (node_count, ms) = build_and_measure_bdd_set(&markings, encoding, order)?;
            println!("{enc_name:<12} {order_name:<14} {node_count:>12} {ms:>12}");
        }
    }

    Ok(())
}
