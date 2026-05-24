//! Bidirectional mapping between public [`Place`] / [`Transition`] handles and dense
//! ranks (`0 .. n`) for a single built [`Net`].

use std::collections::HashMap;

use crate::core::marking::IdxMarking;
use crate::core::net::{PlaceIdx, TransitionIdx};
use crate::core::state_space::TokenOps;
use crate::{Marking, Place, Transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseMapping {
    place_to_dense: HashMap<Place, PlaceIdx>,
    transition_to_dense: HashMap<Transition, TransitionIdx>,
    ordered_places: Box<[Place]>,
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

    /// Dense rank for `place` in this built net, if it is a member of this snapshot.
    #[must_use]
    pub fn place_idx(&self, place: Place) -> Option<PlaceIdx> {
        self.place_to_dense.get(&place).copied()
    }

    /// Public handle for place dense rank `idx`.
    #[must_use]
    pub fn place(&self, idx: PlaceIdx) -> Place {
        self.ordered_places[idx]
    }

    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.ordered_places.iter().copied()
    }

    /// Number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> u32 {
        u32::try_from(self.ordered_places.len()).expect("cannot be built with more than u32::MAX places")
    }

    /// Dense rank for `transition` in this built net, if it is a member of this snapshot.
    #[must_use]
    pub fn transition_idx(&self, transition: Transition) -> Option<TransitionIdx> {
        self.transition_to_dense.get(&transition).copied()
    }

    /// Public handle for transition dense rank `idx`.
    #[must_use]
    pub fn transition(&self, idx: TransitionIdx) -> Transition {
        self.ordered_transitions[idx]
    }

    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.ordered_transitions.iter().copied()
    }

    /// Convert an internal index marking to a public marking.
    pub fn marking<T: TokenOps>(&self, idx_marking: IdxMarking<T>) -> Marking<T> {
        self.places().zip(idx_marking).collect()
    }

    /// Convert a public marking to an internal index marking.
    /// If the marking contains places not in the net, they are ignored.
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
