//! Markings: the state of a Petri net.
//!
//! A marking assigns a token count to each place. The default token type is
//! `u32`. For coverability analysis, [`Omega`] extends token counts with an
//! unbounded symbol ω, and [`OmegaMarking`] is a type alias for `Marking<Omega>`.

use crate::api::net::Place;
use crate::core::unique_sorted_slice::UniqueSortedSlice;
use std::iter::Sum;
use std::ops::Index;
use std::{iter, vec};

/// A marking of a Petri net.
///
/// This is a mapping from `Place` to token counts of type `T`.
#[derive(Debug, Clone)]
pub struct Marking<T> {
    /// The support of the marking.
    ///
    /// Only places with non-default token counts are stored here,
    /// so the default token count is implicitly assigned to all other places.
    pub(crate) support: UniqueSortedSlice<(Place, T)>,
}

impl<T: Default + PartialEq> FromIterator<(Place, T)> for Marking<T> {
    fn from_iter<I: IntoIterator<Item = (Place, T)>>(iter: I) -> Self {
        let mut vec: Vec<(Place, T)> = iter.into_iter()
            .filter(|(_, t)| *t != T::default())
            .collect();
        vec.sort_unstable_by_key(|elem| elem.0.0);
        vec.dedup_by_key(|elem| elem.0.0);
        let support = UniqueSortedSlice::from_sorted_unique(vec);
        Marking { support }
    }
}

impl<T: Default + PartialEq, const N: usize> From<[(Place, T); N]> for Marking<T> {
    fn from(array: [(Place, T); N]) -> Self {
        Marking::from_iter(array)
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = (Place, T);
    type IntoIter = vec::IntoIter<(Place, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.support.into_iter()
    }
}

impl<T: Default + Copy> Index<Place> for Marking<T> {
    type Output = T;
    fn index(&self, place: Place) -> &Self::Output {
        self.support.iter()
            .find(|(p, _)| *p == place)
            .map(|(_, t)| t)
            .unwrap()
    }
}

impl<T: Copy + Sum> Marking<T> {
    #[must_use]
    pub fn total_tokens(&self) -> T {
        self.support.iter()
            .map(|(_, t)| *t)
            .sum()
    }
}

impl<T: Default + Copy> Marking<T> {
    /// Gets the number of tokens this marking assigns to the provided place.
    /// Returns `T::default()` if the place is not present in the marking.
    #[must_use]
    pub fn get(&self, place: Place) -> T {
        self.support.iter()
            .find(|(p, _)| *p == place)
            .map(|(_, t)| t)
            .copied()
            .unwrap_or_default()
    }
}

impl<T> Marking<T> {
    pub fn iter(&self) -> impl Iterator<Item = (&Place, &T)> {
        self.support.iter().map(|(p, t)| (p, t))
    }
    pub fn support(&self) -> impl Iterator<Item = Place> {
        self.support.iter().map(|(p, _)| *p)
    }
}

impl<T: PartialEq> PartialEq for Marking<T> {
    fn eq(&self, other: &Self) -> bool {
        // this impl assumes that the supports are sorted and unique,
        // as guaranteed by the constructor.
        self.support.len() == other.support.len() && {
            iter::zip(self.support.iter(), other.support.iter())
                .all(|((p1, t1), (p2, t2))| {
                    p1 == p2 && t1 == t2
                })
        }
    }
}