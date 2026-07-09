//! Abstraction over how [`DenseStateGraph`](super::DenseStateGraph) deduplicates
//! markings it has already discovered against the graph node that stores them,
//! isolating the decision-diagram-backed storage strategy from the exploration
//! logic in [`DenseStateGraphExplorer`](super::DenseStateGraphExplorer), which
//! only ever needs this exact surface.

#![allow(dead_code)]

use crate::core::marking::IdxMarking;
use crate::core::state_space::TokenOps;
use ahash::HashMap;
use oxidd::Manager;
use oxidd::ManagerRef;
use oxidd::PseudoBooleanFunction;
use oxidd::mtbdd::terminal::I64;
use oxidd::mtbdd::{MTBDDFunction, MTBDDManagerRef};
use oxidd::util::AllocResult;
use petgraph::graph::NodeIndex;
use crate::core::net::PlaceIdx;

/// Dedup/lookup index from a marking to the graph node that stores it.
pub trait MarkingMap<T: TokenOps> {
    /// If `marking` exists in the map, return the [`NodeIndex`]
    /// it is associated with; otherwise return `None`.
    fn get(&self, marking: &IdxMarking<T>) -> Option<NodeIndex>;

    /// Associates `marking` with `idx`, overwriting any previous association.
    fn insert(&mut self, marking: IdxMarking<T>, idx: NodeIndex);

    /// Whether `marking` exists in the map.
    fn contains_key(&self, marking: &IdxMarking<T>) -> bool {
        self.get(marking).is_some()
    }
}

impl<T: TokenOps> MarkingMap<T> for HashMap<IdxMarking<T>, NodeIndex> {
    fn get(&self, marking: &IdxMarking<T>) -> Option<NodeIndex> {
        HashMap::get(self, marking).copied()
    }

    fn insert(&mut self, marking: IdxMarking<T>, idx: NodeIndex) {
        HashMap::insert(self, marking, idx);
    }

    fn contains_key(&self, marking: &IdxMarking<T>) -> bool {
        HashMap::contains_key(self, marking)
    }
}

/// The only per-token-type knowledge a [`MarkingDecisionDiagram`] needs: how
/// many manager variables a value currently requires, and the literal
/// (true/false) for each of them.
///
/// Keeping this trait to exactly these two methods is what lets the
/// Omega-aware coverability encoding (an extra "unbounded" flag bit per
/// place, on top of the finite-count bits) share every other line of
/// `MarkingDecisionDiagram` -- manager/capacity handling, lazy bit growth,
/// the narrowing that growth requires, cube construction, `ite`-based
/// overwrite, regrow-and-replay -- with the plain `u32` reachability
/// encoding. Only `bits_needed`/`bit` differ between them.
pub trait MtbddEncode: TokenOps {
    /// Number of manager variables needed to represent `self` exactly.
    fn bits_needed(&self) -> usize;
    /// The literal for bit `idx` (`0` = least significant), given
    /// `idx < self.bits_needed()`.
    fn bit_at(&self, idx: usize) -> bool;
}

impl MtbddEncode for u32 {
    fn bits_needed(&self) -> usize {
        (u32::BITS - self.leading_zeros()) as usize
    }

    fn bit_at(&self, idx: usize) -> bool {
        (self >> idx) & 1 == 1
    }
}

/// [`MarkingMap<T>`] backed by a multi-terminal binary decision diagram (MTBDD)
/// from the `oxidd` crate.
///
/// Decision diagrams are a more memory-efficient alternative to a hash map for
/// state spaces with a high degree of similarity between markings, which is
/// common in Petri nets.
///
/// See the `oxidd_marking_poc` example for the underlying encoding scheme
/// (lazy per-place bit growth, arithmetic-cube point updates via `ite`) and
/// the reasoning behind the design choices below:
///
/// - `oxidd`'s manager has a *fixed* node capacity chosen at construction, no
///   live resize. `Err(OutOfMemory)` from it only ever means "this
///   self-imposed capacity is full" -- never genuine process-level OOM (that
///   would abort before any `Result` of ours could be inspected, since
///   `oxidd` uses ordinary infallible allocation internally). So it's treated
///   as an internal, fully recoverable condition: rebuild a bigger manager
///   and replay every `(marking, NodeIndex)` pair seen so far. Nothing about
///   that belongs in this trait's public surface.
/// - `max_capacity` is a safety valve, not part of the recovery logic: an
///   unbounded net (or a bug) could legitimately grow forever, and it's
///   better to panic with a clear diagnostic than silently balloon memory
///   until the OS kills the process.
pub struct MarkingDecisionDiagram<T: MtbddEncode> {
    /// The MTBDD manager that owns the nodes and terminals of `seen`.
    manager_ref: MTBDDManagerRef<I64>,
    /// Current node capacity of `manager_ref`, which is doubled on each regrow.
    /// Note that this is *not* the maximum number of markings that can be stored,
    /// but rather shared subgraphs: many markings can share the same nodes in the MTBDD.
    capacity: usize,
    /// Optional maximum node capacity, beyond which `grow()` panics instead of
    /// silently allocating more memory.
    max_capacity: Option<usize>,
    /// The number of bits allocated for each place so far, in place-index order.
    /// This grows lazily as the number of tokens on each place exceeds the current
    /// bit capacity (powers of two) for that place.
    place_bits: Vec<Vec<u32>>,
    /// The MTBDD that encodes the mapping from markings to graph node indices.
    /// The terminal value is `0` for unseen markings, and `NodeIndex.index() + 1`
    /// for seen markings.
    seen: MTBDDFunction<I64>,
    /// Everything inserted so far, replayed into a new manager on regrow.
    /// This does *not* duplicate memory in the real integration: it's
    /// exactly `petgraph::Graph::node_weights()` zipped with their indices,
    /// which the exploration already keeps around for other reasons.
    known: Vec<(IdxMarking<T>, NodeIndex)>,
}

impl<T: MtbddEncode> MarkingDecisionDiagram<T> {
    /// Creates a new MTBDD-backed marking map with the given initial and maximum node capacities.
    pub fn new(initial_capacity: usize, max_capacity: Option<usize>) -> Self {
        let manager_ref = oxidd::mtbdd::new_manager(initial_capacity, initial_capacity, initial_capacity, 1);
        let seen = manager_ref
            .with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(0)))
            .expect("a new manager must be able to hold its own zero constant");
        Self {
            manager_ref,
            capacity: initial_capacity,
            max_capacity,
            place_bits: Vec::new(),
            seen,
            known: Vec::new(),
        }
    }

    fn ensure_bits(&mut self, place: PlaceIdx, value: &T) -> AllocResult<()> {
        if place >= self.place_bits.len() {
            self.place_bits.resize(place + 1, Vec::new());
        }
        let needed = value.bits_needed();
        let have = self.place_bits[place].len();
        if have < needed {
            let additional = (needed - have) as u32;
            let new_vars: Vec<u32> = self
                .manager_ref
                .with_manager_exclusive(|manager| manager.add_vars(additional).collect());
            for &var in &new_vars {
                self.narrow_new_bit(var)?;
            }
            self.place_bits[place].extend(new_vars);
        }
        Ok(())
    }

    /// Force a newly allocated bit's "1" branch to mean "not present".
    ///
    /// Without this, every marking already recorded before this bit existed
    /// would silently keep matching *both* values of the new bit once it's
    /// added -- its original cube was built over the variables that existed
    /// at the time, which never included this one, so it's still an
    /// unconstrained don't-care over it. That aliases points that were never
    /// actually inserted onto whichever old marking happens to match on
    /// every bit that did exist back then.
    fn narrow_new_bit(&mut self, var: u32) -> AllocResult<()> {
        self.seen = self.manager_ref.with_manager_shared(|m| {
            let v = MTBDDFunction::var(m, var)?;
            let zero = MTBDDFunction::constant(m, I64::Num(0))?;
            v.ite(&zero, &self.seen)
        })?;
        Ok(())
    }

    fn indicator(
        &self,
        marking: &IdxMarking<T>,
    ) -> AllocResult<MTBDDFunction<I64>> {
        self.manager_ref.with_manager_shared(|m| {
            let one = MTBDDFunction::constant(m, I64::Num(1))?;
            let mut acc = one.clone();
            for (place, count) in marking.enumerate() {
                for (bit_idx, &varno) in self.place_bits[place].iter().enumerate() {
                    let v = MTBDDFunction::var(m, varno)?;
                    let literal = if count.bit_at(bit_idx) { v } else { one.sub(&v)? };
                    acc = acc.mul(&literal)?;
                }
            }
            Ok(acc)
        })
    }

    fn overwrite(
        &self,
        indicator: &MTBDDFunction<I64>,
        value: usize,
    ) -> AllocResult<MTBDDFunction<I64>> {
        let value = i64::try_from(value).expect("sorry, that's too many nodes for an MTBDD terminal");
        let value = self.manager_ref.with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(value)))?;
        indicator.ite(&value, &self.seen)
    }

    fn grow(&mut self) {
        self.capacity *= 2;
        if let Some(max) = self.max_capacity {
            assert!(
                self.capacity <= max,
                "MarkingDecisionDiagram exceeded its configured max_capacity ({new} > {max})", new = self.capacity, max = max
            );
        }

        self.manager_ref = oxidd::mtbdd::new_manager(self.capacity, self.capacity, self.capacity, 1);
        self.seen = self
            .manager_ref
            .with_manager_shared(|m| MTBDDFunction::constant(m, I64::Num(0)))
            .expect("a new manager must be able to hold its own zero constant");
        for bits in &mut self.place_bits {
            bits.clear();
        }

        let known = std::mem::take(&mut self.known);
        for (marking, idx) in known {
            self.insert(marking, idx); // may recurse into `grow()` again
        }
    }

    /// Bit allocation (and the narrowing it requires), cube construction,
    /// and the overwrite itself, as one fallible unit: if any step runs out
    /// of nodes, `insert` rebuilds bigger and replays everything instead of
    /// retrying this one call directly.
    fn try_insert(&mut self, marking: &IdxMarking<T>, idx: NodeIndex) -> AllocResult<MTBDDFunction<I64>> {
        for (place, count) in marking.enumerate() {
            self.ensure_bits(place, count)?;
        }
        let value = idx.index() + 1;
        self.overwrite(&self.indicator(marking)?, value)
    }
}

impl<T: MtbddEncode> MarkingMap<T> for MarkingDecisionDiagram<T> {
    fn get(&self, marking: &IdxMarking<T>) -> Option<NodeIndex> {
        for (place, count) in marking.iter().enumerate() {
            if count.bits_needed() > self.place_bits.get(place).map_or(0, Vec::len) {
                return None; // never allocated enough bits to represent this
            }
        }
        let args = marking.iter().enumerate().flat_map(|(place, count)| {
            self.place_bits
                .get(place)
                .into_iter()
                .flatten()
                .enumerate()
                .map(move |(bit, &varno)| (varno, count.bit_at(bit)))
        });
        match self.seen.eval(args) {
            I64::Num(0) => None,
            I64::Num(n) => Some(NodeIndex::new((n - 1) as usize)),
            other => panic!("unexpected terminal in seen-markings MTBDD: {other:?}"),
        }
    }

    fn insert(&mut self, marking: IdxMarking<T>, idx: NodeIndex) {
        self.known.push((marking.clone(), idx));
        match self.try_insert(&marking, idx) {
            Ok(new_seen) => self.seen = new_seen,
            Err(_out_of_memory) => self.grow(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ahash::HashMapExt;

    fn exercise(mut seen: impl MarkingMap<u32>) {
        let m0: IdxMarking<u32> = [0, 0].into();
        let m1: IdxMarking<u32> = [1, 2].into();
        let m2: IdxMarking<u32> = [3, 0].into();

        assert_eq!(seen.get(&m0), None);
        assert!(!seen.contains_key(&m1));

        seen.insert(m0.clone(), NodeIndex::new(0));
        seen.insert(m1.clone(), NodeIndex::new(1));
        seen.insert(m2.clone(), NodeIndex::new(2));

        assert_eq!(seen.get(&m0), Some(NodeIndex::new(0)));
        assert_eq!(seen.get(&m1), Some(NodeIndex::new(1)));
        assert_eq!(seen.get(&m2), Some(NodeIndex::new(2)));
        assert!(seen.contains_key(&m1));
        assert_eq!(seen.get(&[2, 1].into()), None);
    }

    #[test]
    fn hash_map_impl() {
        exercise(HashMap::new());
    }

    #[test]
    fn mtbdd_impl() {
        exercise(MarkingDecisionDiagram::new(1024, None));
    }

    #[test]
    fn mtbdd_impl_regrows_past_a_tiny_initial_capacity() {
        // Deliberately tiny, to force several capacity regrows partway
        // through -- see `MarkingDecisionDiagram::grow`.
        let mut seen = MarkingDecisionDiagram::new(8, Some(4096));
        let markings: Vec<IdxMarking<u32>> = (0..14u32).map(|i| [i, (i * 3) % 4].into()).collect();
        for (idx, marking) in markings.iter().enumerate() {
            seen.insert(marking.clone(), NodeIndex::new(idx));
        }
        for (idx, marking) in markings.iter().enumerate() {
            assert_eq!(seen.get(marking), Some(NodeIndex::new(idx)));
        }
        assert!(seen.capacity > 8, "expected at least one regrow, got capacity {}", seen.capacity);
        assert_eq!(seen.get(&[100, 0].into()), None);
    }

    #[test]
    #[should_panic(expected = "exceeded its configured max_capacity")]
    fn mtbdd_impl_respects_max_capacity() {
        let mut seen = MarkingDecisionDiagram::new(4, Some(8));
        for i in 0..50u32 {
            seen.insert([i, i * 7].into(), NodeIndex::new(i as usize));
        }
    }
}
