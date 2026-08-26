//! Public <=> Internal mapping at the API boundary.
//!
//! In order to efficiently store and manipulate markings and other data associated with places and
//! transitions, petrivet internally assigns places and transitions dense 0-based indices.
//! This enables the use of compact, cache friendly data structures such as `Vec` and `Box<[T]>`
//! for storing data associated with places and transitions, advantageous for performance and memory usage.
//!
//! This decoupling allows [`NetBuilder`](crate::net::NetBuilder) to offer a flexible API with
//! arbitrary insertion and removal, while optimizing the internal representation for analysis algorithms.
//!
//! This module provides a bidirectional mapping between the public handles and the internal dense indices.

use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::observe::IdxCegarEvent as IdxCegarEvent;
use crate::core::marking::IdxMarking;
use crate::core::net::{PlaceIdx, TransitionIdx};
use crate::core::parikh::IdxParikhVector;
use crate::core::state_space::TokenOps;
use crate::net::invariant::PInvariant;
use crate::parikh_vector::ParikhVector;
use crate::prelude::{Marking, Place, Transition};
use crate::system::lemma::Lemma;
use crate::system::observe::CegarEvent;
use ahash::{HashMap, HashSet};
use fixedbitset::FixedBitSet;
use crate::core::cegar::CegarResult;
use crate::system::coverability::CoverabilityResult;
use crate::system::reachability::Reachability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseMapping {
    /// Mapping from public place handles to internal dense indices.
    place_to_dense: HashMap<Place, PlaceIdx>,
    /// Mapping from public transition handles to internal dense indices.
    transition_to_dense: HashMap<Transition, TransitionIdx>,
    /// Ordered list of places in the net, indexed by their dense indices.
    ordered_places: Box<[Place]>,
    /// Ordered list of transitions in the net, indexed by their dense indices.
    ordered_transitions: Box<[Transition]>,
}

impl DenseMapping {
    pub fn new(
        place_to_dense: HashMap<Place, PlaceIdx>,
        transition_to_dense: HashMap<Transition, TransitionIdx>,
        ordered_places: Box<[Place]>,
        ordered_transitions: Box<[Transition]>,
    ) -> Self {
        debug_assert_eq!(place_to_dense.len(), ordered_places.len());
        debug_assert_eq!(transition_to_dense.len(), ordered_transitions.len());
        for (i, &p) in ordered_places.iter().enumerate() {
            debug_assert_eq!(place_to_dense.get(&p).copied(), Some(i));
        }
        for (i, &t) in ordered_transitions.iter().enumerate() {
            debug_assert_eq!(transition_to_dense.get(&t).copied(), Some(i));
        }
        Self {
            place_to_dense,
            transition_to_dense,
            ordered_places,
            ordered_transitions,
        }
    }

    /// Returns the dense index for `place` if it exists in this net, or `None` if it does not.
    #[must_use]
    pub fn place_idx(&self, place: Place) -> Option<PlaceIdx> {
        self.place_to_dense.get(&place).copied()
    }

    /// Returns the [`Place`] at dense index `p_idx`.
    #[must_use]
    pub fn place(&self, p_idx: PlaceIdx) -> Place {
        self.ordered_places[p_idx]
    }

    /// Returns an iterator over all [`Places`](Place) in the net in their dense index order.
    ///
    /// This is useful for zipping places together with internal dense place data.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.ordered_places.iter().copied()
    }

    /// Returns the number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> u32 {
        u32::try_from(self.ordered_places.len())
            .expect("cannot be built with more than u32::MAX places")
    }

    /// Returns the dense index for `transition` if it exists in this net, or `None` if it does not.
    #[must_use]
    pub fn transition_idx(&self, transition: Transition) -> Option<TransitionIdx> {
        self.transition_to_dense.get(&transition).copied()
    }

    /// Returns the [`Transition`] at dense index `t_idx`.
    #[must_use]
    pub fn transition(&self, t_idx: TransitionIdx) -> Transition {
        self.ordered_transitions[t_idx]
    }

    /// Returns an iterator over all [`Transitions`](Transition) in the net in their dense index order.
    ///
    /// This is useful for zipping transitions together with internal dense transition data.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.ordered_transitions.iter().copied()
    }

    /// Returns the number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> u32 {
        u32::try_from(self.ordered_transitions.len())
            .expect("cannot be built with more than u32::MAX transitions")
    }

    /// Convert an internal index marking to a public marking.
    pub fn encode<T: TokenOps>(&self, idx_marking: IdxMarking<T>) -> Marking<T> {
        self.places().zip(idx_marking).collect()
    }

    /// Convert a public marking to an internal index marking.
    ///
    /// If the provided marking contains places that do not exist in this net, those places will be ignored.
    ///
    /// todo: accept any `IntoIterator<Item=(Place, T)>` instead of a `Marking<T>` to avoid unnecessary
    ///  intermediate allocations when the caller already has an iterator over the marking's support.
    pub fn decode<T: TokenOps>(&self, marking: Marking<T>) -> IdxMarking<T> {
        let mut idx_marking = IdxMarking::zeros(self.place_count());
        marking.into_iter().for_each(|(place, count)| {
            if let Some(dense) = self.place_idx(place) {
                idx_marking[dense] = count;
            }
        });
        idx_marking
    }

    /// Convert a set of internal indices to a HashSet<Place>.
    pub fn place_set(&self, places: &FixedBitSet) -> HashSet<Place> {
        places.ones().map(|p_idx| self.place(p_idx)).collect()
    }

    /// Convert a set of internal indices to a HashSet<Transition>.
    pub fn transition_set(&self, transitions: &FixedBitSet) -> HashSet<Transition> {
        transitions.ones().map(|p_idx| self.transition(p_idx)).collect()
    }

    /// Convert a sequence of internal transition indices to a Vec<Transition>.
    pub fn firing_sequence(&self, transitions: Vec<TransitionIdx>) -> Vec<Transition> {
        transitions.into_iter().map(|t_idx| self.transition(t_idx)).collect()
    }

    /// Convert an internal Parikh vector to a public one.
    pub fn parikh_vector(&self, parikh_vector: IdxParikhVector<u32>) -> ParikhVector<u32> {
        parikh_vector
            .into_inner()
            .into_iter()
            .enumerate()
            .map(|(t_idx, count)| (self.transition(t_idx), count))
            .collect()
    }

    /// Translates an internal dense-indexed [`IdxCegarEvent`](IdxCegarEvent) to its public equivalent.
    pub fn cegar_event(&self, event: IdxCegarEvent) -> CegarEvent {
        CegarEvent {
            spurious_marking: event.spurious_marking.map(|m| self.encode(m)),
            spurious_parikh_vector: event.spurious_parikh_vector.map(|pv| self.parikh_vector(pv)),
            lemma: self.lemma(event.lemma),
        }
    }

    pub fn reachability_result(&self, result: CegarResult) -> Reachability {
        match result {
            CegarResult::Satisfiable { marking: _, firing_sequence } => Reachability::Reachable {
                firing_sequence: self.firing_sequence(firing_sequence),
            },
            CegarResult::Unsatisfiable { contradiction } => Reachability::Unreachable {
                contradiction: contradiction.into_iter().map(|lemma| self.lemma(lemma)).collect(),
            },
        }
    }

    pub fn coverability_result(&self, result: CegarResult) -> CoverabilityResult {
        match result {
            CegarResult::Satisfiable { marking, firing_sequence } => CoverabilityResult::Coverable {
                marking: self.encode(marking),
                firing_sequence: self.firing_sequence(firing_sequence),
            },
            CegarResult::Unsatisfiable { contradiction } => CoverabilityResult::Uncoverable {
                contradiction: contradiction.into_iter().map(|lemma| self.lemma(lemma)).collect(),
            },
        }
    }

    /// Convert an internal lemma to a public lemma.
    pub fn lemma(&self, lemma: IdxLemma) -> Lemma {
        match lemma {
            IdxLemma::PInvariant(p_inv) => Lemma::PInvariant(PInvariant {
                weights: p_inv.weights.into_iter()
                    .map(|(p_idx, w)| (self.place(p_idx), w))
                    .collect(),
                value: p_inv.value,
            }),
            IdxLemma::InitiallyMarkedTrap(trap) => Lemma::InitiallyMarkedTrap(
                self.place_set(&trap)
            ),
            IdxLemma::MarkingEquation {
                place, initial_marking, net_effects,
            } => Lemma::MarkingEquation {
                place: self.place(place),
                initial_marking,
                net_effects: net_effects.into_iter()
                    .map(|(t_idx, effect)| (self.transition(t_idx), effect))
                    .collect(),
            },
            IdxLemma::TrapBecomesMarked { feeder, trap } => Lemma::TrapBecomesMarked {
                feeder: self.transition(feeder),
                trap: self.place_set(&trap),
            },
            IdxLemma::CausalOrdering { t_idx, p_idx, feeders } => Lemma::CausalOrdering {
                transition: self.transition(t_idx),
                place: self.place(p_idx),
                feeders: feeders.into_iter().map(|t_idx| self.transition(t_idx)).collect(),
            },
            IdxLemma::Increment {
                component_places,
                component_transitions,
                firing_sequence,
            } => Lemma::Increment {
                component_places: self.place_set(&component_places),
                component_transitions: self.transition_set(&component_transitions),
                firing_sequence: self.firing_sequence(firing_sequence),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    #[test]
    fn trivial_mapping_roundtrip_invariants() {
        let p0 = Place(NonZeroU32::new(1).unwrap());
        let p1 = Place(NonZeroU32::new(2).unwrap());
        let t0 = Transition(NonZeroU32::new(1).unwrap());
        let place_to_dense = vec![(p0, 0usize), (p1, 1)].into_iter().collect();
        let transition_to_dense = vec![(t0, 0usize)].into_iter().collect();
        let ordered_places = vec![p0, p1].into_boxed_slice();
        let ordered_transitions = vec![t0].into_boxed_slice();
        let m = DenseMapping::new(
            place_to_dense,
            transition_to_dense,
            ordered_places,
            ordered_transitions,
        );
        assert_eq!(m.place_idx(p0), Some(0));
        assert_eq!(m.place_idx(p1), Some(1));
        assert_eq!(m.place(0), p0);
        assert_eq!(m.place(1), p1);
        assert_eq!(m.transition_idx(t0), Some(0));
        assert_eq!(m.transition(0), t0);
    }
}
