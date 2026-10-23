//! Markings: the state of a Petri net.
//!
//! A marking assigns a `u32` token count to each place.

use crate::core::state_space::TokenOps;
use crate::net::Place;

/// A mapping from [`Place`] to tokens of type `T`.
///
/// The canonical way to create a marking is to use the [`From`] trait on an array of `(Place, T)`:
/// ```
/// use petrivet::net::Net;
/// use petrivet::system::PetriNet;
/// use petrivet::system::marking::Marking;
/// let mut b = Net::builder();
/// let [p0, p1] = b.add_places();
/// let t0 = b.add_transition();
/// b.add_arcs((p0, t0, p1));
/// let net = b.build().expect("valid net");
///
/// let marking: Marking<u32> = [(p0, 1)].into();
/// let petri_net = PetriNet::new(&net, marking);
/// ```
///
/// You can also collect an iterator of `(Place, T)` into a marking:
/// ```
/// use std::collections::HashSet;
/// use petrivet::system::marking::Marking;
/// use petrivet::net::Net;
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
#[derive(Debug, Clone, Default, Eq, PartialEq, Hash)]
pub struct Marking<T> {
    /// The support of the marking.
    ///
    /// Only places with non-zero token counts are stored here,
    /// and a token count of zero is implicitly assigned to all other places.
    pub(crate) support: Vec<(Place, T)>,
}

impl<T> Marking<T> {
    pub fn support(&self) -> impl Iterator<Item=Place> + '_ {
        self.support.iter().map(|(place, _)| *place)
    }
}

impl<T: TokenOps> FromIterator<(Place, T)> for Marking<T> {
    fn from_iter<I: IntoIterator<Item=(Place, T)>>(iter: I) -> Self {
        let mut vec: Vec<(Place, T)> = iter.into_iter().filter(|(_, t)| *t != T::ZERO).collect();
        vec.sort_unstable_by_key(|elem| elem.0.0);
        vec.dedup_by_key(|elem| elem.0.0);
        Marking { support: vec }
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
        self.support
            .iter()
            .find(|(p, _)| *p == place)
            .map_or(T::ZERO, |(_, t)| *t)
    }

    /// Returns the maximum token count assigned to any place in this marking.
    #[must_use]
    pub fn max_token_count(&self) -> T {
        self.support
            .iter()
            .map(|(_, t)| *t)
            .max()
            .unwrap_or(T::ZERO)
    }

    /// Returns the total number of tokens across all places in this marking.
    #[must_use]
    pub fn total_tokens(&self) -> T {
        self.support.iter().map(|(_, t)| *t).sum()
    }

    /// Whether this marking assigns at least as many tokens as `other` to every place.
    #[must_use]
    pub fn covers(&self, other: &Marking<T>) -> bool {
        other.support.iter().all(|(place, tokens)| self.get(*place) >= *tokens)
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = (Place, T);
    type IntoIter = std::vec::IntoIter<(Place, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.support.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::net::builder::NetBuilder;
    use crate::system::marking::Marking;

    #[test]
    fn markings_are_canonical_so_equality_ignores_zeros_and_order() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let a: Marking<u32> = [(p2, 1), (p0, 3), (p1, 0)].into_iter().collect();
        let same: Marking<u32> = [(p0, 3), (p2, 1)].into_iter().collect();
        assert_eq!(a, same);
        assert_ne!(a, [(p0, 3)].into_iter().collect());
    }

    #[test]
    fn covers_compares_every_place_of_the_other_marking() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let big: Marking<u32> = [(p0, 3), (p1, 1)].into_iter().collect();
        assert!(big.covers(&[(p0, 2)].into_iter().collect()));
        assert!(big.covers(&big));
        assert!(big.covers(&Marking::default()));
        assert!(!big.covers(&[(p0, 4)].into_iter().collect()));
        // a place the marking does not mention holds no tokens
        assert!(!big.covers(&[(p2, 1)].into_iter().collect()));
    }
}
