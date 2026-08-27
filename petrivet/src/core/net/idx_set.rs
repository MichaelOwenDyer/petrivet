use crate::core::net::{PlaceIdx, TransitionIdx};
use fixedbitset::FixedBitSet;
use std::ops::Index;

macro_rules! define_idx_set {
    (
        $struct_name:ident,
        $idx_type:ty,
        $iter_name:ident,
        $into_iter_name:ident,
        $complement_iter_name:ident
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $struct_name(pub FixedBitSet);

        impl $struct_name {
            /// Creates a new empty set of indices with the given size.
            pub fn none_of(size: usize) -> Self {
                Self(FixedBitSet::with_capacity(size))
            }

            /// Creates a new full set of indices with the given size.
            pub fn all_of(size: usize) -> Self {
                let mut set = FixedBitSet::with_capacity(size);
                set.insert_range(..);
                Self(set)
            }

            /// Returns an iterator over the indices in the set.
            pub fn $iter_name(&self) -> impl Iterator<Item = $idx_type> + '_ {
                self.0.ones()
            }

            /// Consumes the set and returns an iterator over the indices in the set.
            pub fn $into_iter_name(self) -> impl Iterator<Item = $idx_type> {
                self.0.into_ones()
            }

            /// Returns an iterator over the indices not in the set.
            pub fn $complement_iter_name(&self) -> impl Iterator<Item = $idx_type> + '_ {
                self.0.zeroes()
            }

            /// Returns the number of items in the set.
            pub fn size(&self) -> usize {
                self.0.count_ones(..)
            }

            /// Returns true if the set is empty.
            pub fn is_empty(&self) -> bool {
                self.0.is_clear()
            }

            /// Inserts the index into the set if value is true, or removes it if value is false.
            pub fn set(&mut self, idx: $idx_type, value: bool) {
                self.0.set(idx, value);
            }

            /// Adds an index into the set.
            pub fn add(&mut self, idx: $idx_type) {
                self.0.insert(idx);
            }

            /// Removes an index from the set.
            pub fn remove(&mut self, idx: $idx_type) {
                self.0.remove(idx)
            }

            /// Removes and returns the first index in the set, if any.
            pub fn remove_first(&mut self) -> Option<$idx_type> {
                if let Some(idx) = self.0.ones().next() {
                    self.0.remove(idx);
                    Some(idx)
                } else {
                    None
                }
            }

            /// Returns true if the set contains the given index.
            pub fn contains(&self, idx: $idx_type) -> bool {
                self.0.contains(idx)
            }

            /// Returns true if this set contains only indices that are also in the other set.
            pub fn is_subset(&self, other: &Self) -> bool {
                self.0.is_subset(&other.0)
            }

            /// Returns true if the set of indices is disjoint from another set of indices.
            pub fn is_disjoint(&self, other: &Self) -> bool {
                self.0.is_disjoint(&other.0)
            }
        }

        impl Index<$idx_type> for $struct_name {
            type Output = bool;

            fn index(&self, index: $idx_type) -> &Self::Output {
                if self.0.contains(index) {
                    &true
                } else {
                    &false
                }
            }
        }
    };
}

define_idx_set!(
    PlaceIdxSet,
    PlaceIdx,
    place_indices,
    into_place_indices,
    complement_place_indices
);

define_idx_set!(
    TransitionIdxSet,
    TransitionIdx,
    transition_indices,
    into_transition_indices,
    complement_transition_indices
);