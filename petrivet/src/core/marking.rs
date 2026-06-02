use crate::core::net::PlaceIdx;
use crate::core::state_space::TokenOps;
use std::cmp::Ordering;
use std::ops::{Index, IndexMut};
use std::{iter, vec};

/// A marking: one value of type `T` per place, indexed by [`PlaceIdx`].
///
/// The default token type is `u32`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdxMarking<T>(pub(crate) Box<[T]>);

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
            .filter(|&(_, tokens)| tokens != &T::ZERO)
            .map(|(idx, _)| idx)
    }

    /// Creates a marking with `n_places` places, all initialized to zero tokens.
    #[must_use]
    pub fn zeros(n_places: u32) -> Self {
        Self(vec![T::ZERO; n_places as usize].into_boxed_slice())
    }

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

/// Merges two orderings in the context of element-wise comparison of markings.
/// If either is `Equal`, returns the other. If both are `Less` or both are `Greater`, returns that.
/// Otherwise, returns `None` (incomparable).
pub(crate) const fn merge_ordering(acc: Ordering, next: Ordering) -> Option<Ordering> {
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