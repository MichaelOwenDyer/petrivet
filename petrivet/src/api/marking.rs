//! Markings: the state of a Petri net.
//!
//! A marking assigns a token count to each place. The default token type is
//! `u32`. For coverability analysis, [`Omega`] extends token counts with an
//! unbounded symbol ω, and [`OmegaMarking`] is a type alias for `Marking<Omega>`.

use crate::core::state_space::TokenOps;
use crate::core::unique_sorted_slice::UniqueSortedSlice;
use crate::Place;

/// A mapping from [`Place`] to tokens of type `T`.
///
/// The canonical way to create a marking is to use the [`From`] trait on an array of `(Place, T)`:
/// ```
/// use petrivet::{Marking, Net};
/// let mut b = Net::builder();
/// let [p0, p1] = b.add_places();
/// let t0 = b.add_transition();
/// b.add_arcs((p0, t0, p1));
/// let net = b.build().expect("valid net");
///
/// let marking: Marking<u32> = [(p0, 1)].into();
/// let petri_net = net.with_initial_marking(marking);
/// ```
///
/// You can also collect an iterator of `(Place, T)` into a marking:
/// ```
/// use std::collections::HashSet;
/// use petrivet::{Marking, Net};
/// let mut b = Net::builder();
/// let [p0, p1] = b.add_places();
/// let t0 = b.add_transition();
/// b.add_arcs((p0, t0, p1));
/// let net = b.build().expect("valid net");
///
/// let mut marking = HashSet::new();
/// marking.insert((p0, 1));
/// let marking: Marking<u32> = marking.into_iter().collect();
/// let petri_net = net.with_initial_marking(marking);
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Marking<T> {
    /// The support of the marking.
    ///
    /// Only places with non-zero token counts are stored here,
    /// and a token count of zero is implicitly assigned to all other places.
    pub(crate) support: UniqueSortedSlice<(Place, T)>,
}

impl<T> Marking<T> {
    pub fn support(&self) -> impl Iterator<Item = &(Place, T)> {
        self.support.iter()
    }
}

impl<T> Default for Marking<T> {
    /// Creates an empty marking with zero tokens on all places.
    fn default() -> Self {
        Marking {
            support: UniqueSortedSlice::default(),
        }
    }
}

impl<T: TokenOps> FromIterator<(Place, T)> for Marking<T> {
    fn from_iter<I: IntoIterator<Item = (Place, T)>>(iter: I) -> Self {
        let mut vec: Vec<(Place, T)> = iter.into_iter()
            .filter(|(_, t)| *t != T::ZERO)
            .collect();
        vec.sort_unstable_by_key(|elem| elem.0.0);
        vec.dedup_by_key(|elem| elem.0.0);
        let support = UniqueSortedSlice::from_sorted_unique(vec);
        Marking { support }
    }
}

impl<T: TokenOps, const N: usize> From<[(Place, T); N]> for Marking<T> {
    fn from(array: [(Place, T); N]) -> Self {
        Marking::from_iter(array)
    }
}

impl<T: TokenOps> Marking<T> {
    /// Returns the number of tokens this marking assigns to the provided [`Place`].
    #[must_use]
    pub fn get(&self, place: Place) -> T {
        self.support.iter()
            .find(|(p, _)| *p == place)
            .map_or(T::ZERO, |(_, t)| *t)
    }

    /// Returns the maximum token count assigned to any place in this marking.
    #[must_use]
    pub fn max_token_count(&self) -> T {
        self.support.iter()
            .map(|(_, t)| *t)
            .max()
            .unwrap_or(T::ZERO)
    }

    /// Returns the total number of tokens across all places in this marking.
    #[must_use]
    pub fn total_tokens(&self) -> T {
        self.support.iter()
            .map(|(_, t)| *t)
            .sum()
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = (Place, T);
    type IntoIter = std::vec::IntoIter<(Place, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.support.into_iter()
    }
}