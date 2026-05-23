use crate::core::state_space::TokenOps;
use crate::core::PlaceIdx;
use crate::api::marking::Omega;
use std::cmp::Ordering;
use std::ops::{Index, IndexMut};
use std::{iter, vec};

/// A marking: one value of type `T` per place, indexed by [`PlaceIdx`].
///
/// The default token type is `u32`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdxMarking<T>(Box<[T]>);

/// An ω-marking: a marking where token counts can be finite or "infinity" (ω).
/// Used to construct the Karp-Miller coverability graph, where ω represents unbounded growth of tokens.
pub type IdxOmegaMarking = IdxMarking<Omega>;

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

impl<T: TokenOps> IdxMarking<T> {
    pub fn support(&self) -> impl Iterator<Item = PlaceIdx> {
        self.iter()
            .enumerate()
            .filter(|&(_, tokens)| tokens != &T::zero())
            .map(|(idx, _)| idx)
    }
}

impl<T: Ord + Clone> IdxMarking<T> {
    /// Returns the componentwise maximum of `self` and `other`.
    pub fn componentwise_max(mut acc: Self, other: &Self) -> Self {
        for (bound, tokens) in acc.0.iter_mut().zip(other.0.iter()) {
            if *bound < *tokens {
                *bound = tokens.clone();
            }
        }
        acc
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
    pub fn zeros(n_places: u32) -> Self {
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

impl<T: Ord> IdxMarking<T> {
    /// Returns true if this marking covers the other.
    ///
    /// In other words, `self` has at least as many tokens as `other` in every place,
    /// and strictly more in at least one place.
    pub fn covers(&self, other: &Self) -> bool {
        self >= other
    }
}

/// Covering relation on markings:
/// M1 >= M2 iff M1(p) >= M2(p) for all places p.
/// Two markings may be incomparable if some places are greater and others are lesser.
impl<T: Ord> PartialOrd for IdxMarking<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        debug_assert_eq!(self.place_count(), other.place_count());
        iter::zip(&self.0, &other.0)
            .map(|(a, b)| a.cmp(b))
            .try_fold(Ordering::Equal, merge_ordering)
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

/// Merges two orderings in the context of element-wise comparison of markings.
/// If either is `Equal`, returns the other. If both are `Less` or both are `Greater`, returns that.
/// Otherwise, returns `None` (incomparable).
const fn merge_ordering(acc: Ordering, next: Ordering) -> Option<Ordering> {
    match (acc, next) {
        (Ordering::Equal, o) | (o, Ordering::Equal) => Some(o),
        (Ordering::Less, Ordering::Less) => Some(Ordering::Less),
        (Ordering::Greater, Ordering::Greater) => Some(Ordering::Greater),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_array() {
        let m: IdxMarking<u32> = [1, 0, 3].into();
        assert_eq!(m[0], 1);
        assert_eq!(m[1], 0);
        assert_eq!(m[2], 3);
    }

    #[test]
    fn partial_order() {
        let m0: IdxMarking<u32> = [1, 3, 0].into();
        let m1: IdxMarking<u32> = [2, 3, 0].into();
        let m2: IdxMarking<u32> = [1, 4, 0].into();
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
        let m: IdxMarking<u32> = [1, 2, 3].into();
        let om = IdxOmegaMarking::from(&m);
        assert_eq!(m, om);
        assert_eq!(om, m);
    }

    #[test]
    fn cross_type_lt() {
        let m: IdxMarking<u32> = [1, 2, 3].into();
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded, Omega::Finite(3)].into();
        assert!(m < om);
        assert!(om > m);
    }

    #[test]
    fn incomparable_markings() {
        let a: IdxMarking<u32> = [2, 0, 1].into();
        let b: IdxMarking<u32> = [0, 2, 1].into();
        assert!(a.partial_cmp(&b).is_none());
        assert_ne!(a, b);
    }

    #[test]
    fn covering_relation_equal() {
        let a: IdxMarking<u32> = [1, 2, 3].into();
        let b: IdxMarking<u32> = [1, 2, 3].into();
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
        let u32m: IdxMarking<u32> = [5, 0].into();
        let om: IdxOmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(u32m.partial_cmp(&om).is_none());
    }

    #[test]
    fn cross_type_covering() {
        let u32m: IdxMarking<u32> = [1, 2].into();
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
        let m: IdxMarking<u32> = (0..5).collect();
        assert_eq!(m.place_count(), 5);
        assert_eq!(m[3], 3);
    }

    #[test]
    fn into_iterator() {
        let m: IdxMarking<u32> = [10, 20, 30].into();
        let v: Vec<u32> = m.into_iter().collect();
        assert_eq!(v, vec![10, 20, 30]);
    }
}