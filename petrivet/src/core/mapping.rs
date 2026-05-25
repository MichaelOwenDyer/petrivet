//! Bidirectional mapping between public [`Place`] / [`Transition`] handles
//! and dense indices (`0 .. n`) for a single built [`Net`].

use crate::core::marking::IdxMarking;
use crate::core::net::{PlaceIdx, TransitionIdx};
use crate::core::state_space::TokenOps;
use crate::{Marking, Place, Transition};
use std::collections::HashMap;

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
        u32::try_from(self.ordered_places.len()).expect("cannot be built with more than u32::MAX places")
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

    /// Convert an internal index marking to a public marking.
    pub fn marking<T: TokenOps>(&self, idx_marking: IdxMarking<T>) -> Marking<T> {
        self.places().zip(idx_marking).collect()
    }

    /// Convert a public marking to an internal index marking.
    ///
    /// If the provided marking contains places that do not exist in this net, those places will be ignored.
    pub fn idx_marking<T: TokenOps>(&self, marking: Marking<T>) -> IdxMarking<T> {
        let mut idx_marking = IdxMarking::zeros(self.place_count());
        marking.into_iter().for_each(|(place, count)| {
            if let Some(dense) = self.place_idx(place) {
                idx_marking[dense] = count;
            }
        });
        idx_marking
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
