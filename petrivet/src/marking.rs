//! Markings: the state of a Petri net.
//!
//! A marking assigns a token count to each place. The default token type is
//! `u32`. For coverability analysis, [`Omega`] extends token counts with an
//! unbounded symbol ω, and [`IdxOmegaMarking`] is a type alias for `Marking<Omega>`.
//!
//! ```
//! use petrivet::marking::IdxMarking;
//! let m: IdxMarking = [1, 0, 3].into();
//! ```

use crate::net::PlaceIdx;
use crate::Place;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::Sum;
use std::ops::{Index, IndexMut};
use std::{iter, vec};

/// The public-facing marking type. Contains a slice of (Place, Token) pairs.
/// A place does not appear in the list iff it has `T::default()` tokens.
/// Places are sorted ascending by place ID.
#[derive(Debug, Clone)]
pub struct ApiMarking<T = u32>(Box<[(Place, T)]>);
pub type ApiOmegaMarking = ApiMarking<Omega>;

impl<T> AsRef<[(Place, T)]> for ApiMarking<T> {
    fn as_ref(&self) -> &[(Place, T)] {
        &self.0
    }
}

impl<T: Default + Eq + Hash> FromIterator<(Place, T)> for ApiMarking<T> {
    fn from_iter<I: IntoIterator<Item = (Place, T)>>(iter: I) -> Self {
        let mut x = iter
            .into_iter()
            .filter(|(_, t)| *t != T::default())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Box<_>>();
        x.sort_unstable_by_key(|(p, _)| p.into_raw());
        ApiMarking(x)
    }
}

impl<T: Default + Eq + Hash, const N: usize> From<[(Place, T); N]> for ApiMarking<T> {
    fn from(array: [(Place, T); N]) -> Self {
        ApiMarking::from_iter(array)
    }
}

impl<T> IntoIterator for ApiMarking<T> {
    type Item = (Place, T);
    type IntoIter = vec::IntoIter<(Place, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Clone + Sum> ApiMarking<T> {
    pub fn total_tokens(&self) -> T {
        self.0.iter().map(|(_, t)| t.clone()).sum()
    }
    pub fn support(&self) -> impl Iterator<Item = Place> {
        self.0.iter().map(|(p, _)| *p)
    }
}

impl<T> Index<Place> for ApiMarking<T> {
    type Output = T;
    fn index(&self, place: Place) -> &Self::Output {
        self.get(place).unwrap()
    }
}

impl<T: PartialEq> PartialEq for ApiMarking<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && iter::zip(self.0.iter(), other.0.iter())
            .all(|((p1, t1), (p2, t2))| p1 == p2 && t1 == t2)
    }
}

impl<T> ApiMarking<T> {
    pub fn get(&self, place: Place) -> Option<&T> {
        self.0.iter().find(|(p, _)| *p == place).map(|(_, t)| t)
    }
}

/// A marking: one value of type `T` per place, indexed by [`PlaceIdx`].
///
/// The default token type is `u32`. For coverability analysis, use
/// [`IdxOmegaMarking`] (alias for `Marking<Omega>`).
///
/// Create from arrays or vectors:
/// ```
/// use petrivet::marking::IdxMarking;
/// let m: IdxMarking = [1, 0, 3].into();
/// let m = IdxMarking::from([1, 0, 3]);
/// let m: IdxMarking = vec![1, 0, 3].into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct IdxMarking<T = u32>(Box<[T]>);

/// An ω-marking: a marking where token counts can be finite or "infinity" (ω).
/// Used to construct the Karp-Miller coverability tree, where ω represents unbounded growth of tokens.
pub(crate) type IdxOmegaMarking = IdxMarking<Omega>;

/// A marking can be viewed as a simple slice of T values, indexed by place index.
impl<T> AsRef<[T]> for IdxMarking<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}

impl<T> IdxMarking<T> {
    /// Number of places in this marking.
    #[must_use]
    pub fn place_count(&self) -> usize {
        self.0.len()
    }

    /// Iterator over token counts in place-index order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Mutable iterator over token counts in place-index order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

impl<T> IntoIterator for IdxMarking<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Default + Clone> IdxMarking<T> {
    /// Creates a marking with the default value for each place.
    /// For `u32` this is 0; for `Omega` this is `Omega::Finite(0)`.
    #[must_use]
    pub(crate) fn zeros(n_places: u32) -> Self {
        Self(vec![T::default(); n_places as usize].into_boxed_slice())
    }
}

impl<T: PartialEq> PartialEq<IdxMarking<T>> for &IdxMarking<T> {
    fn eq(&self, other: &IdxMarking<T>) -> bool {
        *self == other
    }
}

impl<T: PartialEq> PartialEq<&IdxMarking<T>> for IdxMarking<T> {
    fn eq(&self, other: &&IdxMarking<T>) -> bool {
        self == *other
    }
}

impl<T> Index<PlaceIdx> for IdxMarking<T> {
    type Output = T;
    fn index(&self, p: PlaceIdx) -> &T {
        &self.0[p]
    }
}

impl<T> IndexMut<PlaceIdx> for IdxMarking<T> {
    fn index_mut(&mut self, p: PlaceIdx) -> &mut T {
        &mut self.0[p]
    }
}

impl<T> From<Vec<T>> for IdxMarking<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v.into_boxed_slice())
    }
}

impl<T, const N: usize> From<[T; N]> for IdxMarking<T> {
    fn from(a: [T; N]) -> Self {
        Self(Box::new(a))
    }
}

impl<T> FromIterator<T> for IdxMarking<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Merges two orderings in the context of element-wise comparison of markings.
/// If either is `Equal`, returns the other. If both are `Less` or both are `Greater`, returns that.
/// Otherwise, returns `None` (incomparable).
fn merge_ordering(acc: Ordering, next: Ordering) -> Option<Ordering> {
    match (acc, next) {
        (Ordering::Equal, o) | (o, Ordering::Equal) => Some(o),
        (Ordering::Less, Ordering::Less) => Some(Ordering::Less),
        (Ordering::Greater, Ordering::Greater) => Some(Ordering::Greater),
        _ => None,
    }
}

/// Covering relation on markings:
/// M1 >= M2 iff M1(p) >= M2(p) for all places p.
/// Two markings may be incomparable if some places are greater and others are lesser.
impl<T: Ord> PartialOrd for IdxMarking<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        debug_assert_eq!(self.place_count(), other.place_count());
        iter::zip(self.0.iter(), other.0.iter())
            .map(|(a, b)| a.cmp(b))
            .try_fold(Ordering::Equal, merge_ordering)
    }
}

/// A token count that is either finite or ω (unbounded).
///
/// "Omega" as the name of this enum is a slight misnomer,
/// since ω represents unboundedness but this enum
/// represents either boundedness or unboundedness.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Omega {
    /// A concrete finite token count.
    Finite(u32),
    /// An unbounded token count (ω). Greater than any finite value.
    Unbounded,
}

impl Omega {
    /// Returns `true` if this is a finite value.
    #[must_use]
    pub fn is_finite(self) -> bool {
        matches!(self, Omega::Finite(_))
    }

    /// Returns `true` if this value is unbounded (omega).
    #[must_use]
    pub fn is_unbounded(self) -> bool {
        matches!(self, Omega::Unbounded)
    }

    /// Returns true if this is a finite value less than or equal to `b`.
    #[must_use]
    pub fn is_b_bounded(self, b: u32) -> bool {
        matches!(self, Omega::Finite(bound) if bound <= b)
    }

    /// Returns the finite value, or `None` if unbounded.
    #[must_use]
    pub fn finite(self) -> Option<u32> {
        match self {
            Omega::Finite(n) => Some(n),
            Omega::Unbounded => None,
        }
    }
}

impl Default for Omega {
    fn default() -> Self {
        Omega::Finite(0)
    }
}

impl From<u32> for Omega {
    fn from(n: u32) -> Self {
        Omega::Finite(n)
    }
}

impl Ord for Omega {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Omega::Finite(a), Omega::Finite(b)) => a.cmp(b),
            (Omega::Finite(_), Omega::Unbounded) => Ordering::Less,
            (Omega::Unbounded, Omega::Finite(_)) => Ordering::Greater,
            (Omega::Unbounded, Omega::Unbounded) => Ordering::Equal,
        }
    }
}

impl PartialOrd for Omega {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl IdxMarking<Omega> {
    /// Returns `true` if all components are finite (no ω).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|o| o.is_finite())
    }
}

impl From<&IdxMarking<u32>> for IdxMarking<Omega> {
    fn from(marking: &IdxMarking<u32>) -> Self {
        IdxMarking(marking.iter().map(|&n| Omega::Finite(n)).collect())
    }
}

impl From<IdxMarking<u32>> for IdxMarking<Omega> {
    fn from(marking: IdxMarking<u32>) -> Self {
        IdxMarking(marking.into_iter().map(Omega::Finite).collect())
    }
}

impl TryFrom<IdxMarking<Omega>> for IdxMarking<u32> {
    type Error = ();
    fn try_from(omega_marking: IdxMarking<Omega>) -> Result<Self, ()> {
        omega_marking.into_iter()
            .map(|o| o.finite().ok_or(()))
            .collect()
    }
}

impl PartialEq<IdxMarking<Omega>> for IdxMarking<u32> {
    fn eq(&self, other: &IdxMarking<Omega>) -> bool {
        self.place_count() == other.place_count() && iter::zip(self.0.iter(), other.0.iter())
            .all(|(&t, &o)| o == Omega::Finite(t))
    }
}

impl PartialEq<IdxMarking<u32>> for IdxMarking<Omega> {
    fn eq(&self, other: &IdxMarking<u32>) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<IdxMarking<Omega>> for IdxMarking<u32> {
    fn partial_cmp(&self, other: &IdxMarking<Omega>) -> Option<Ordering> {
        debug_assert_eq!(self.place_count(), other.place_count());
        iter::zip(self.0.iter(), other.0.iter())
            .map(|(&n, o)| Omega::Finite(n).cmp(o))
            .try_fold(Ordering::Equal, merge_ordering)
    }
}

impl PartialOrd<IdxMarking<u32>> for IdxMarking<Omega> {
    fn partial_cmp(&self, other: &IdxMarking<u32>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_array() {
        let m: IdxMarking = [1, 0, 3].into();
        assert_eq!(m[0], 1);
        assert_eq!(m[1], 0);
        assert_eq!(m[2], 3);
    }

    #[test]
    fn partial_order() {
        let m0: IdxMarking = [1, 3, 0].into();
        let m1: IdxMarking = [2, 3, 0].into();
        let m2: IdxMarking = [1, 4, 0].into();
        assert!(m1 > m0);
        assert!(m2 > m0);
        assert!(m1.partial_cmp(&m2).is_none());
    }

    #[test]
    fn omega_ordering() {
        assert!(Omega::Finite(100) < Omega::Unbounded);
        assert!(Omega::Unbounded > Omega::Finite(u32::MAX));
        assert_eq!(Omega::Finite(5), Omega::Finite(5));
    }

    #[test]
    fn omega_marking_from_array() {
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert_eq!(om[0], Omega::Finite(1));
        assert_eq!(om[1], Omega::Unbounded);
    }

    #[test]
    fn cross_type_eq() {
        let m: IdxMarking = [1, 2, 3].into();
        let om = IdxOmegaMarking::from(&m);
        assert_eq!(m, om);
        assert_eq!(om, m);
    }

    #[test]
    fn cross_type_lt() {
        let m: IdxMarking = [1, 2, 3].into();
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded, Omega::Finite(3)].into();
        assert!(m < om);
        assert!(om > m);
    }

    #[test]
    fn incomparable_markings() {
        let a: IdxMarking = [2, 0, 1].into();
        let b: IdxMarking = [0, 2, 1].into();
        assert!(a.partial_cmp(&b).is_none());
        assert_ne!(a, b);
    }

    #[test]
    fn covering_relation_equal() {
        let a: IdxMarking = [1, 2, 3].into();
        let b: IdxMarking = [1, 2, 3].into();
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
    }

    #[test]
    fn omega_incomparable() {
        let a: IdxOmegaMarking = [Omega::Unbounded, Omega::Finite(0)].into();
        let b: IdxOmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(a.partial_cmp(&b).is_none());
    }

    #[test]
    fn cross_type_incomparable() {
        let u32m: IdxMarking = [5, 0].into();
        let om: IdxOmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(u32m.partial_cmp(&om).is_none());
    }

    #[test]
    fn cross_type_covering() {
        let u32m: IdxMarking = [1, 2].into();
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert!(u32m < om);
        assert!(om > u32m);
    }

    #[test]
    fn omega_try_from_all_finite() {
        let om: IdxOmegaMarking = [Omega::Finite(10), Omega::Finite(20)].into();
        let result: Result<IdxMarking<u32>, _> = om.try_into();
        assert_eq!(result.unwrap(), IdxMarking::from([10, 20]));
    }

    #[test]
    fn omega_try_from_has_unbounded() {
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        let result: Result<IdxMarking<u32>, _> = om.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn from_iterator() {
        let m: IdxMarking = (0..5).collect();
        assert_eq!(m.place_count(), 5);
        assert_eq!(m[3], 3);
    }

    #[test]
    fn into_iterator() {
        let m: IdxMarking = [10, 20, 30].into();
        let v: Vec<u32> = m.into_iter().collect();
        assert_eq!(v, vec![10, 20, 30]);
    }
}
