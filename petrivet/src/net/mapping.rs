//! Bidirectional mapping between public [`Place`] / [`Transition`] handles and dense
//! ranks (`0 .. n`) for a single built [`super::Net`].

use std::collections::HashMap;

use super::idx::{PlaceIdx, TransitionIdx};
use super::nodes::{Place, Transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseMapping {
    place_to_dense: HashMap<Place, PlaceIdx>,
    transition_to_dense: HashMap<Transition, TransitionIdx>,
    ordered_places: Box<[Place]>,
    ordered_transitions: Box<[Transition]>,
}

impl DenseMapping {
    pub(crate) fn new(
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
    pub(crate) fn place_idx(&self, place: Place) -> Option<PlaceIdx> {
        self.place_to_dense.get(&place).copied()
    }

    /// Public handle for place dense rank `idx`.
    #[must_use]
    pub(crate) fn place_key(&self, idx: PlaceIdx) -> Place {
        self.ordered_places[idx]
    }

    /// Dense rank for `transition` in this built net, if it is a member of this snapshot.
    #[must_use]
    pub(crate) fn transition_idx(&self, transition: Transition) -> Option<TransitionIdx> {
        self.transition_to_dense.get(&transition).copied()
    }

    /// Public handle for transition dense rank `idx`.
    #[must_use]
    pub(crate) fn transition_key(&self, idx: TransitionIdx) -> Transition {
        self.ordered_transitions[idx]
    }

    pub(crate) fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.ordered_places.iter().copied()
    }

    pub(crate) fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.ordered_transitions.iter().copied()
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
        assert_eq!(m.place_key(0), p0);
        assert_eq!(m.place_key(1), p1);
        assert_eq!(m.transition_idx(t0), Some(0));
        assert_eq!(m.transition_key(0), t0);
    }
}
