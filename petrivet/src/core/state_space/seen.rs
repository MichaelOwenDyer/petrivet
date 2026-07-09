//! Storage strategies for previously encountered markings during state space
//! exploration. The fundamental operation is a mapping from an [`IdxMarking`]
//! to a [`NodeIndex`], allowing the exploration logic to quickly determine
//! whether a marking has already been seen and, if so, which node in the
//! state space graph represents it.
//!
//! A naive implementation of this mapping could be a simple [`HashMap`], but
//! this comes with some disadvantages:
//! - Cache locality is poor.
//! - Every marking is stored in whole, even if they are very similar to each
//!   other (e.g., differing by only a few tokens in a few places).
//!
//! A more sophisticated implementation uses a [multi-terminal binary decision
//! diagram](https://en.wikipedia.org/wiki/Binary_decision_diagram).
//! Such a data structure essentially encodes a marking as a path through a
//! binary tree, where each level of the tree corresponds to a bit in the
//! marking's representation. Similar markings will share common paths in the
//! tree, leading to significant memory savings when many similar markings are
//! stored. The terminal nodes of the decision diagram can store the associated
//! [`NodeIndex`] for each unique marking, allowing for efficient retrieval.

#![allow(dead_code)]

use crate::core::marking::IdxMarking;
use crate::core::net::PlaceIdx;
use crate::core::state_space::coverability::Omega;
use crate::core::state_space::TokenOps;
use ahash::HashMap;
use oxidd::mtbdd::terminal::I64;
use oxidd::mtbdd::{MTBDDFunction, MTBDDManagerRef};
use oxidd::util::{AllocResult, Borrowed};
use oxidd::{
    Edge as _, Function, HasLevel, InnerNode, LevelNo, Manager, ManagerRef, Node, PseudoBooleanFunction, VarNo,
};
use petgraph::graph::NodeIndex;
use std::borrow::Borrow;
use std::marker::PhantomData;

/// A mapping from [`IdxMarking`] to [`NodeIndex`],
/// used to track which markings have already been seen during state space exploration.
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

/// [`MarkingMap<T>`] backed by a [`HashMap`].
// `ahash::HashMap` is this crate's one deliberate hash map choice throughout,
// not something meant to be generic over the hasher.
#[allow(clippy::implicit_hasher)]
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

/// Defines how a token type `T` is encoded into the bits of a [`MarkingDecisionDiagram`],
/// and how to reconstruct a value of type `T` from such an encoding.
pub trait MtbddEncode: TokenOps {
    /// Number of bits variables needed to represent `self` exactly.
    ///
    /// We must ensure that there are sufficient manager variables
    /// allocated for each place to represent the largest value that
    /// can be stored there.
    fn bits_needed(&self) -> usize;
    /// Returns the literal for bit `idx` of this value
    /// (`0` = least significant), given `idx < self.bits_needed()`.
    fn bit_at(&self, idx: usize) -> bool;
    /// Reconstructs a value from the literal at each bit index
    /// (`0` = least significant first), given exactly the bits
    /// currently allocated for the place it came from.
    ///
    /// This is the inverse of [`MtbddEncode::bit_at`].
    fn from_bits(bits: impl Iterator<Item = bool>) -> Self;
}

impl MtbddEncode for u32 {
    fn bits_needed(&self) -> usize {
        (u32::BITS - self.leading_zeros()) as usize
    }

    fn bit_at(&self, idx: usize) -> bool {
        (self >> idx) & 1 == 1
    }

    fn from_bits(bits: impl Iterator<Item = bool>) -> Self {
        bits.enumerate().fold(0, |value, (idx, bit)| value | (u32::from(bit) << idx))
    }
}

/// Encodes a place's bit 0 as an "unbounded" flag, and the following bits
/// as the finite count if the flag is false. This is a compact representation
/// that allows the MTBDD to share a single decision node for the unbounded
/// case across all markings, while still allowing finite counts to be represented
/// using as many bits as needed.
impl MtbddEncode for Omega {
    fn bits_needed(&self) -> usize {
        match self {
            Omega::Finite(count) => 1 + count.bits_needed(),
            Omega::Unbounded => 1,
        }
    }

    fn bit_at(&self, idx: usize) -> bool {
        match (idx, self) {
            // Bit 0 is the "unbounded" flag: true if this is an unbounded value, false otherwise.
            (0, _) => self.is_unbounded(),
            // `idx != 0` on an unbounded value is always false,
            // because the count's bits don't exist at all.
            (_, Omega::Unbounded) => false,
            // `idx >= 1` on a finite value: the count's own bit at `idx - 1`.
            (idx, Omega::Finite(count)) => count.bit_at(idx - 1),
        }
    }

    fn from_bits(mut bits: impl Iterator<Item = bool>) -> Self {
        match bits.next() {
            Some(true) => Omega::Unbounded,
            _ => Omega::Finite(u32::from_bits(bits)),
        }
    }
}

/// [`MarkingMap<T>`] backed by a multi-terminal binary decision diagram (MTBDD)
/// from the `oxidd` crate.
///
/// Decision diagrams are a more memory-efficient alternative to a hash map for
/// state spaces with a high degree of similarity between markings, which is
/// common in Petri nets.
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
    /// The manager variable allocated for each bit of each place so far, in
    /// place-index order and least-significant-bit first. Grows lazily as
    /// values needing more bits are inserted.
    place_bits: Vec<Vec<VarNo>>,
    /// The MTBDD that encodes the mapping from markings to graph node indices.
    /// The terminal value is `0` for unseen markings, and `NodeIndex.index() + 1`
    /// for seen markings.
    seen: MTBDDFunction<I64>,
    /// Nowhere in this struct are values of type `T` actually stored, since they
    /// are all encoded into the MTBDD. This is just to keep the compiler happy
    /// about the generic parameter.
    marking_type: PhantomData<T>,
}

impl<T: MtbddEncode> MarkingDecisionDiagram<T> {
    /// Creates a new MTBDD-backed marking map with the given initial and maximum node capacities.
    pub fn new(initial_capacity: usize, max_capacity: Option<usize>) -> Self {
        let manager_ref = oxidd::mtbdd::new_manager(initial_capacity, initial_capacity, initial_capacity, 1);
        let seen = manager_ref
            .with_manager_shared(|manager| MTBDDFunction::constant(manager, I64::Num(0)))
            .expect("a new manager must be able to hold its own zero constant");
        Self {
            manager_ref,
            capacity: initial_capacity,
            max_capacity,
            place_bits: Vec::new(),
            seen,
            marking_type: PhantomData,
        }
    }

    /// Lateral growth: add manager variables so that `place` can represent
    /// `value`, if it can't already.
    fn ensure_bits(&mut self, place: PlaceIdx, value: &T) -> AllocResult<()> {
        if place >= self.place_bits.len() {
            self.place_bits.resize(place + 1, Vec::new());
        }
        let bits_needed = value.bits_needed();
        let bits_allocated = self.place_bits[place].len();
        if bits_allocated < bits_needed {
            let additional_bits = u32::try_from(bits_needed - bits_allocated)
                .expect("a place cannot plausibly need this many bits");
            let new_manager_vars: Vec<VarNo> = self
                .manager_ref
                .with_manager_exclusive(|manager| manager.add_vars(additional_bits).collect());
            for &manager_var in &new_manager_vars {
                self.narrow_new_bit(manager_var)?;
            }
            self.place_bits[place].extend(new_manager_vars);
        }
        Ok(())
    }

    /// Force a newly allocated bit's "1" branch to mean "not present".
    ///
    /// Without this, every marking already recorded before this bit existed
    /// would silently keep matching *both* values of the new bit once added.
    /// An existing marking's cube was built over the variables that existed
    /// at the time it was inserted, so the new bit is effectively a
    /// "don't care" for it. That means that some future marking that differs
    /// from an existing one only in the new bit would be indistinguishable from
    /// the existing one, and would be recorded as a duplicate of it.
    fn narrow_new_bit(&mut self, manager_var: VarNo) -> AllocResult<()> {
        self.seen = self.manager_ref.with_manager_shared(|manager| {
            let bit_is_set = MTBDDFunction::var(manager, manager_var)?;
            let not_present = MTBDDFunction::constant(manager, I64::Num(0))?;
            bit_is_set.ite(&not_present, &self.seen)
        })?;
        Ok(())
    }

    /// Returns the arithmetic cube of the given marking:
    /// `1` at exactly `marking`, `0` everywhere else.
    fn indicator(&self, marking: &IdxMarking<T>) -> AllocResult<MTBDDFunction<I64>> {
        self.manager_ref.with_manager_shared(|manager| {
            let one = MTBDDFunction::constant(manager, I64::Num(1))?;
            let mut cube = one.clone();
            for (place, count) in marking.enumerate() {
                for (bit_idx, &manager_var) in self.place_bits[place].iter().enumerate() {
                    let bit_is_set = MTBDDFunction::var(manager, manager_var)?;
                    let literal = if count.bit_at(bit_idx) { bit_is_set } else { one.sub(&bit_is_set)? };
                    cube = cube.mul(&literal)?;
                }
            }
            Ok(cube)
        })
    }

    /// Returns a new [`MTBDDFunction`] where the given `indicator` points to `terminal_value`,
    /// and all other paths are unchanged.
    fn overlay(&self, indicator: &MTBDDFunction<I64>, terminal_value: i64) -> AllocResult<MTBDDFunction<I64>> {
        let terminal_value = self.manager_ref.with_manager_shared(|manager| {
            MTBDDFunction::constant(manager, I64::Num(terminal_value))
        })?;
        indicator.ite(&terminal_value, &self.seen)
    }

    /// Bit allocation (and the narrowing it requires), cube construction,
    /// and the overwrite itself, as one fallible unit: if any step runs out
    /// of nodes, `insert` rebuilds bigger and replays everything instead of
    /// retrying this one call directly.
    fn try_insert(&mut self, marking: &IdxMarking<T>, idx: NodeIndex) -> AllocResult<()> {
        for (place, count) in marking.enumerate() {
            self.ensure_bits(place, count)?;
        }
        let indicator = &self.indicator(marking)?;
        let terminal_value = i64::try_from(idx.index() + 1)
            .expect("sorry, that's too many nodes for an MTBDD terminal");
        self.overlay(indicator, terminal_value).map(|new_seen| self.seen = new_seen)
    }

    /// Rebuilds `seen` in a bigger manager, and replays into it every
    /// marking `seen` currently records (found by walking `seen` itself --
    /// see `enumerate` -- not from a separately maintained list, which would
    /// permanently duplicate whatever `petgraph::Graph::node_weights()`
    /// already holds in the real integration).
    fn grow(&mut self) {
        let new_capacity = (self.capacity * 2).max(1); // `.max(1)`: capacity 0 must still be able to grow
        if let Some(max_capacity) = self.max_capacity {
            assert!(
                new_capacity <= max_capacity,
                "MarkingDecisionDiagram exceeded its configured max_capacity ({new_capacity} > {max_capacity})",
            );
        }

        // Must happen before replacing `manager_ref`/`seen`/`place_bits`: it
        // walks the diagram we're about to discard.
        let recorded_markings = self.enumerate();

        self.capacity = new_capacity;
        self.manager_ref = oxidd::mtbdd::new_manager(self.capacity, self.capacity, self.capacity, 1);
        self.seen = self
            .manager_ref
            .with_manager_shared(|manager| MTBDDFunction::constant(manager, I64::Num(0)))
            .expect("a new manager must be able to hold its own zero constant");
        for bits in &mut self.place_bits {
            bits.clear();
        }

        for (marking, idx) in recorded_markings {
            self.insert(marking, idx); // may recurse into `grow()` again
        }
    }

    /// Reconstructs every `(marking, NodeIndex)` pair currently recorded in
    /// `seen`, by walking its structure directly: a memoization-free
    /// recursive descent, following both children of every inner node
    /// reached, canonicalizing any level skipped by reduction to `false`
    /// (sound because a skipped level cannot affect which terminal a path
    /// reaches, so any value is consistent with it). Every currently
    /// recorded marking's cube constrains every bit that existed when it was
    /// last touched (see `narrow_new_bit`), so each one corresponds to
    /// exactly one deterministic root-to-terminal path of length
    /// `manager.num_vars()`.
    ///
    /// This is *not* memoized against shared subgraphs, so a diagram with
    /// heavy re-convergent sharing could in principle revisit the same node
    /// from multiple root-to-terminal paths. In exchange it needs no
    /// permanent bookkeeping at all, and only runs on the (amortized, rare)
    /// regrow path -- if profiling ever shows this walk itself dominates,
    /// memoizing per-node partial results (as `Function::node_count()` does
    /// for plain node counting) would bound it by diagram size instead.
    fn enumerate(&self) -> Vec<(IdxMarking<T>, NodeIndex)> {
        self.seen.with_manager_shared(|manager, edge| {
            let mut bit_assignment = vec![false; manager.num_vars() as usize];
            let mut recorded = Vec::new();
            collect_recorded_markings(manager, edge.borrowed(), 0, &mut bit_assignment, &self.place_bits, &mut recorded);
            recorded
        })
    }
}

/// See [`MarkingDecisionDiagram::enumerate`].
///
/// `edge` is taken by value (a `Borrowed<'_, Edge>`, not a reference to one)
/// to match the shape `InnerNode::child()` hands back on each recursive
/// call, mirroring how oxidd's own `PseudoBooleanFunction::eval_edge` walks
/// a diagram recursively.
#[allow(clippy::needless_pass_by_value)]
fn collect_recorded_markings<M, T>(
    manager: &M,
    edge: Borrowed<'_, M::Edge>,
    level: LevelNo,
    bit_assignment: &mut [bool],
    place_bits: &[Vec<VarNo>],
    recorded: &mut Vec<(IdxMarking<T>, NodeIndex)>,
) where
    M: Manager<Terminal = I64>,
    M::InnerNode: HasLevel,
    T: MtbddEncode,
{
    match manager.get_node(&edge) {
        Node::Inner(node) => {
            let node_level = node.level();
            for skipped_level in level..node_level {
                bit_assignment[manager.level_to_var(skipped_level) as usize] = false;
            }
            let manager_var = manager.level_to_var(node_level);
            // `child(0)` is the "then"/true branch, `child(1)` the "else"/false one.
            for (child_index, bit_value) in [(0, true), (1, false)] {
                bit_assignment[manager_var as usize] = bit_value;
                collect_recorded_markings(
                    manager,
                    node.child(child_index),
                    node_level + 1,
                    bit_assignment,
                    place_bits,
                    recorded,
                );
            }
        }
        Node::Terminal(terminal) => {
            for skipped_level in level..manager.num_levels() {
                bit_assignment[manager.level_to_var(skipped_level) as usize] = false;
            }
            if let I64::Num(terminal_value) = *terminal.borrow()
                && terminal_value != 0
            {
                let marking: IdxMarking<T> = place_bits
                    .iter()
                    .map(|bits| T::from_bits(bits.iter().map(|&manager_var| bit_assignment[manager_var as usize])))
                    .collect();
                let node_index = usize::try_from(terminal_value - 1)
                    .expect("seen-markings MTBDD terminal values are always positive");
                recorded.push((marking, NodeIndex::new(node_index)));
            }
        }
    }
}

impl<T: MtbddEncode> MarkingMap<T> for MarkingDecisionDiagram<T> {
    fn get(&self, marking: &IdxMarking<T>) -> Option<NodeIndex> {
        for (place, count) in marking.enumerate() {
            if count.bits_needed() > self.place_bits.get(place).map_or(0, Vec::len) {
                return None; // never allocated enough bits to represent this
            }
        }
        let args = marking.enumerate().flat_map(|(place, count)| {
            self.place_bits
                .get(place)
                .into_iter()
                .flatten()
                .enumerate()
                .map(move |(bit_idx, &manager_var)| (manager_var, count.bit_at(bit_idx)))
        });
        match self.seen.eval(args) {
            I64::Num(0) => None,
            I64::Num(terminal_value) => {
                let node_index = usize::try_from(terminal_value - 1)
                    .expect("seen-markings MTBDD terminal values are always positive");
                Some(NodeIndex::new(node_index))
            }
            other => panic!("unexpected terminal in seen-markings MTBDD: {other:?}"),
        }
    }

    fn insert(&mut self, marking: IdxMarking<T>, idx: NodeIndex) {
        match self.try_insert(&marking, idx) {
            Ok(()) => {},
            Err(_out_of_memory) => {
                // `enumerate()` inside `grow()` only sees markings already
                // successfully folded into `seen`, so the one that triggered
                // the regrow must be re-inserted.
                self.grow();
                self.insert(marking, idx);
            }
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

    /// The marking that actually *triggers* a regrow must survive it -- easy
    /// to lose if `grow()`'s replay only covers what was already recorded in
    /// `seen` before the failing insert.
    #[test]
    fn the_marking_that_triggers_a_regrow_is_not_lost() {
        let mut seen = MarkingDecisionDiagram::new(4, None);
        let capacity_before = seen.capacity;
        // Grows place 0 by many bits at once, in a manager barely big enough
        // to hold anything -- likely to trigger a regrow exactly here.
        seen.insert([0, 0].into(), NodeIndex::new(0));
        seen.insert([1000, 0].into(), NodeIndex::new(1));
        assert!(seen.capacity > capacity_before, "test setup didn't actually trigger a regrow");
        assert_eq!(seen.get(&[0, 0].into()), Some(NodeIndex::new(0)));
        assert_eq!(seen.get(&[1000, 0].into()), Some(NodeIndex::new(1)));
    }

    /// A single insert can need more than one new bit for the same place at
    /// once (jumping straight from 0 to 1000, say) -- `ensure_bits` narrows
    /// each newly allocated bit in a loop, one at a time; this exercises
    /// more than one iteration of that loop in a single call.
    #[test]
    fn a_single_insert_can_jump_several_bits_at_once() {
        let mut seen = MarkingDecisionDiagram::new(1024, None);
        seen.insert([0, 0].into(), NodeIndex::new(0));
        seen.insert([1000, 5].into(), NodeIndex::new(1));
        assert_eq!(seen.get(&[0, 0].into()), Some(NodeIndex::new(0)));
        assert_eq!(seen.get(&[1000, 5].into()), Some(NodeIndex::new(1)));
        assert_eq!(seen.get(&[999, 5].into()), None);
        assert_eq!(seen.get(&[0, 1].into()), None);
    }

    #[test]
    fn enumerate_matches_what_was_inserted() {
        let mut seen = MarkingDecisionDiagram::new(8, Some(4096)); // tiny: forces regrows, so enumerate() is exercised for real
        let markings: Vec<IdxMarking<u32>> = (0..20u32).map(|i| [i, (i * 5) % 7].into()).collect();
        for (idx, marking) in markings.iter().enumerate() {
            seen.insert(marking.clone(), NodeIndex::new(idx));
        }
        let mut recorded = seen.enumerate();
        recorded.sort_by_key(|(_, idx)| idx.index());
        let expected: Vec<_> =
            markings.iter().cloned().zip((0..markings.len()).map(NodeIndex::new)).collect();
        assert_eq!(recorded, expected);
    }

    fn exercise_omega(mut seen: impl MarkingMap<Omega>) {
        let m0: IdxMarking<Omega> = [Omega::Finite(0), Omega::Finite(0)].into();
        let m1: IdxMarking<Omega> = [Omega::Finite(3), Omega::Unbounded].into();
        let m2: IdxMarking<Omega> = [Omega::Unbounded, Omega::Finite(0)].into();

        seen.insert(m0.clone(), NodeIndex::new(0));
        seen.insert(m1.clone(), NodeIndex::new(1));
        seen.insert(m2.clone(), NodeIndex::new(2));

        assert_eq!(seen.get(&m0), Some(NodeIndex::new(0)));
        assert_eq!(seen.get(&m1), Some(NodeIndex::new(1)));
        assert_eq!(seen.get(&m2), Some(NodeIndex::new(2)));
        // Same finite bit pattern as m1's first place, but Unbounded instead
        // of Finite(3) -- must not alias, that's exactly what the fixed flag
        // bit exists to keep apart.
        assert_eq!(seen.get(&[Omega::Unbounded, Omega::Unbounded].into()), None);
        assert_eq!(seen.get(&[Omega::Finite(3), Omega::Finite(0)].into()), None);
    }

    #[test]
    fn mtbdd_impl_omega() {
        exercise_omega(MarkingDecisionDiagram::new(1024, None));
    }

    #[test]
    fn mtbdd_impl_omega_enumerate_roundtrip() {
        let mut seen = MarkingDecisionDiagram::new(8, Some(4096));
        let markings: Vec<IdxMarking<Omega>> = (0..16u32)
            .map(|i| {
                if i % 5 == 0 {
                    [Omega::Unbounded, Omega::Finite(i)].into()
                } else {
                    [Omega::Finite(i), Omega::Finite(i * 2)].into()
                }
            })
            .collect();
        for (idx, marking) in markings.iter().enumerate() {
            seen.insert(marking.clone(), NodeIndex::new(idx));
        }
        for (idx, marking) in markings.iter().enumerate() {
            assert_eq!(seen.get(marking), Some(NodeIndex::new(idx)));
        }
        let mut recorded = seen.enumerate();
        recorded.sort_by_key(|(_, idx)| idx.index());
        let expected: Vec<_> =
            markings.iter().cloned().zip((0..markings.len()).map(NodeIndex::new)).collect();
        assert_eq!(recorded, expected);
    }
}
