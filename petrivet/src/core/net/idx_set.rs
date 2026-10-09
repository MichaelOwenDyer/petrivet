use crate::core::net::{PlaceIdx, TransitionIdx};
use fixedbitset::FixedBitSet;
use std::ops::Index;

macro_rules! define_idx_set {
    (
        $struct_name:ident,
        $idx_type:ty,
    ) => {
        /// A compact index set, implemented as a fixed-size bitset.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $struct_name(pub FixedBitSet);

        impl $struct_name {
            /// Creates a new empty set of indices with the given size.
            pub fn none_of(size: usize) -> Self {
                Self(FixedBitSet::with_capacity(size))
            }

            /// Creates a new set of indices containing all indices from 0 to size-1.
            #[allow(unused)]
            pub fn all_of(size: usize) -> Self {
                let mut set = FixedBitSet::with_capacity(size);
                set.insert_range(..size);
                Self(set)
            }

            /// Returns the number of indices in the set.
            #[allow(unused)]
            pub fn count(&self) -> usize {
                self.0.count_ones(..)
            }

            /// Returns an iterator over the indices in the set.
            pub fn iter(&self) -> impl Iterator<Item = $idx_type> + '_ {
                self.0.ones()
            }

            /// Returns the complement of the set, i.e., all indices not in the set.
            #[allow(unused)]
            pub fn complement(&self) -> Self {
                let mut complement = self.0.clone();
                complement.toggle_range(..);
                Self(complement)
            }

            /// Returns true if the set is empty.
            pub fn is_empty(&self) -> bool {
                self.0.is_clear()
            }

            /// Inserts the index into the set if value is true, or removes it if value is false.
            pub fn set(&mut self, idx: $idx_type, value: bool) {
                self.0.set(idx, value);
            }

            /// Sets all indices in the set to the given value.
            #[allow(unused)]
            pub fn set_all(&mut self, value: bool) {
                if value {
                    self.0.insert_range(..);
                } else {
                    self.0.clear();
                }
            }

            /// Adds an index into the set.
            pub fn add(&mut self, idx: $idx_type) {
                self.0.insert(idx);
            }

            /// Removes an index from the set.
            #[allow(unused)]
            pub fn remove(&mut self, idx: $idx_type) {
                self.0.remove(idx)
            }

            /// Removes all indices in the other set from this set.
            #[allow(unused)]
            pub fn remove_all(&mut self, other: &Self) {
                self.0.difference_with(&other.0);
            }

            /// Removes and returns the first index in the set, if any.
            #[allow(unused)]
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

            /// Returns true if the set of indices is disjoint from another set of indices.
            #[allow(unused)]
            pub fn is_disjoint(&self, other: &Self) -> bool {
                self.0.is_disjoint(&other.0)
            }

            /// Returns the intersection of this set with another set of indices.
            #[allow(unused)]
            pub fn intersection(self, other: &Self) -> Self {
                Self(self.0.intersection(&other.0).collect())
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

        impl IntoIterator for $struct_name {
            type Item = $idx_type;
            type IntoIter = fixedbitset::IntoOnes;

            fn into_iter(self) -> Self::IntoIter {
                self.0.into_ones()
            }
        }

        impl<'a> IntoIterator for &'a $struct_name {
            type Item = $idx_type;
            type IntoIter = fixedbitset::Ones<'a>;

            fn into_iter(self) -> Self::IntoIter {
                self.0.ones()
            }
        }
    };
}

define_idx_set!(
    PlaceIdxSet,
    PlaceIdx,
);

define_idx_set!(
    TransitionIdxSet,
    TransitionIdx,
);