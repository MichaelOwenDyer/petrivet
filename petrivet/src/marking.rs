//! Markings: the state of a Petri net.
//!
//! A marking assigns a token count to each place. The default token type is
//! `u32`. For coverability analysis, [`Omega`] extends token counts with an
//! unbounded symbol ω, and [`OmegaMarking`] is a type alias for `Marking<Omega>`.
//!
//! ```
//! use petrivet::marking::Marking;
//! let m: Marking = [1, 0, 3].into();
//! ```

use crate::net::{Place, PlaceMap};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::{Index, IndexMut};
use std::{fmt, iter};

/// A marking: one value of type `T` per place, indexed by [`Place`].
///
/// The default token type is `u32`. For coverability analysis, use
/// [`OmegaMarking`] (alias for `Marking<Omega>`).
///
/// Create from arrays or vectors:
/// ```
/// use petrivet::marking::Marking;
/// let m: Marking = [1, 0, 3].into();
/// let m = Marking::from([1, 0, 3]);
/// let m: Marking = vec![1, 0, 3].into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Marking<T = u32>(PlaceMap<T>);

/// An ω-marking: a marking where token counts can be finite or "infinity" (ω).
/// Used to construct the Karp-Miller coverability tree, where ω represents unbounded growth of tokens.
pub type OmegaMarking = Marking<Omega>;

impl<T> Marking<T> {
    /// Number of places in this marking.
    #[must_use]
    #[expect(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Iterator over token counts in place-index order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.values()
    }

    /// Mutable iterator over token counts in place-index order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.values_mut()
    }
}

impl Marking<u32> {
    /// Whether all places have zero tokens.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.values().all(|&t| t == 0)
    }

    /// Total number of tokens across all places.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.0.values().map(|&t| u64::from(t)).sum()
    }

    /// Places that have at least one token.
    pub fn support(&self) -> impl Iterator<Item = Place> + '_ {
        self.0.iter().filter_map(|(p, &t)| if t > 0 { Some(p) } else { None })
    }
}

impl<T: Default + Clone> Marking<T> {
    /// Creates a marking with the default value for each place.
    /// For `u32` this is 0; for `Omega` this is `Omega::Finite(0)`.
    #[must_use]
    pub fn zeros(n_places: usize) -> Self {
        Self(PlaceMap::new(n_places))
    }
}

impl<T: Ord + Copy> Marking<T> {
    /// Element-wise maximum: `self[i] = max(self[i], other[i])`.
    pub fn ceil_assign(&mut self, other: &Self) {
        debug_assert_eq!(self.len(), other.len());
        for (a, &b) in self.0.values_mut().zip(other.0.values()) {
            *a = (*a).max(b);
        }
    }
}

impl<T: PartialEq> PartialEq<Marking<T>> for &Marking<T> {
    fn eq(&self, other: &Marking<T>) -> bool {
        *self == other
    }
}

impl<T: PartialEq> PartialEq<&Marking<T>> for Marking<T> {
    fn eq(&self, other: &&Marking<T>) -> bool {
        self == *other
    }
}

impl<T> Index<Place> for Marking<T> {
    type Output = T;
    fn index(&self, p: Place) -> &T {
        &self.0[p]
    }
}

impl<T> IndexMut<Place> for Marking<T> {
    fn index_mut(&mut self, p: Place) -> &mut T {
        &mut self.0[p]
    }
}

impl<T> From<PlaceMap<T>> for Marking<T> {
    fn from(m: PlaceMap<T>) -> Self {
        Self(m)
    }
}

impl<T> From<Vec<T>> for Marking<T> {
    fn from(v: Vec<T>) -> Self {
        Self(PlaceMap::from(v))
    }
}

impl<T, const N: usize> From<[T; N]> for Marking<T> {
    fn from(a: [T; N]) -> Self {
        Self(PlaceMap::from(a))
    }
}

impl<T> FromIterator<T> for Marking<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(PlaceMap::from_iter(iter))
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: fmt::Display> fmt::Display for Marking<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, val) in self.0.values().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{val}")?;
        }
        write!(f, ")")
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
impl<T: Ord> PartialOrd for Marking<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        debug_assert_eq!(self.len(), other.len());
        iter::zip(self.0.values(), other.0.values())
            .map(|(a, b)| a.cmp(b))
            .try_fold(Ordering::Equal, merge_ordering)
    }
}

/// A token count that is either finite or ω (unbounded).
///
/// "Omega" as the name of this enum is a slight misnomer,
/// since ω represents unboundedness but this enum
/// represents either boundedness or unboundedness.
/// However, it is the most concise name that is immediately
/// recognizable to Petri net researchers that the au
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

impl fmt::Display for Omega {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Omega::Finite(n) => write!(f, "{n}"),
            Omega::Unbounded => write!(f, "ω"),
        }
    }
}

impl Marking<Omega> {
    /// Returns `true` if all components are finite (no ω).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.0.values().all(|o| o.is_finite())
    }
}

impl From<&Marking<u32>> for Marking<Omega> {
    fn from(marking: &Marking<u32>) -> Self {
        Marking(marking.iter().map(|&n| Omega::Finite(n)).collect())
    }
}

impl From<Marking<u32>> for Marking<Omega> {
    fn from(marking: Marking<u32>) -> Self {
        Marking(marking.into_iter().map(Omega::Finite).collect())
    }
}

impl TryFrom<Marking<Omega>> for Marking<u32> {
    type Error = ();
    fn try_from(omega_marking: Marking<Omega>) -> Result<Self, ()> {
        omega_marking.into_iter()
            .map(|o| o.finite().ok_or(()))
            .collect::<Result<Vec<_>, _>>()
            .map(Marking::from)
    }
}

impl PartialEq<Marking<Omega>> for Marking<u32> {
    fn eq(&self, other: &Marking<Omega>) -> bool {
        self.len() == other.len()
            && iter::zip(self.0.values(), other.0.values()).all(|(&n, &o)| o == Omega::Finite(n))
    }
}

impl PartialEq<Marking<u32>> for Marking<Omega> {
    fn eq(&self, other: &Marking<u32>) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<Marking<Omega>> for Marking<u32> {
    fn partial_cmp(&self, other: &Marking<Omega>) -> Option<Ordering> {
        debug_assert_eq!(self.len(), other.len());
        iter::zip(self.0.values(), other.0.values())
            .map(|(&n, o)| Omega::Finite(n).cmp(o))
            .try_fold(Ordering::Equal, merge_ordering)
    }
}

impl PartialOrd<Marking<u32>> for Marking<Omega> {
    fn partial_cmp(&self, other: &Marking<u32>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_array() {
        let m: Marking = [1, 0, 3].into();
        assert_eq!(m[Place { idx: 0 }], 1);
        assert_eq!(m[Place { idx: 1 }], 0);
        assert_eq!(m[Place { idx: 2 }], 3);
    }

    #[test]
    fn partial_order() {
        let m0: Marking = [1, 3, 0].into();
        let m1: Marking = [2, 3, 0].into();
        let m2: Marking = [1, 4, 0].into();
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
        let om: OmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert_eq!(om[Place { idx: 0 }], Omega::Finite(1));
        assert_eq!(om[Place { idx: 1 }], Omega::Unbounded);
    }

    #[test]
    fn cross_type_eq() {
        let m: Marking = [1, 2, 3].into();
        let om = OmegaMarking::from(&m);
        assert_eq!(m, om);
        assert_eq!(om, m);
    }

    #[test]
    fn cross_type_lt() {
        let m: Marking = [1, 2, 3].into();
        let om: OmegaMarking = [Omega::Finite(1), Omega::Unbounded, Omega::Finite(3)].into();
        assert!(m < om);
        assert!(om > m);
    }

    #[test]
    fn ceil_assign() {
        let mut m: Marking = [1, 3].into();
        let other: Marking = [2, 1].into();
        m.ceil_assign(&other);
        assert_eq!(m, Marking::from([2, 3]));
    }

    #[test]
    fn display() {
        let m: Marking = [1, 0, 3].into();
        assert_eq!(m.to_string(), "(1, 0, 3)");

        let om: OmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert_eq!(om.to_string(), "(1, ω)");
    }

    #[test]
    fn zero_length_marking() {
        let m: Marking = Marking::from(Vec::<u32>::new());
        assert_eq!(m.len(), 0);
        assert!(m.is_zero());
        assert_eq!(m.total_tokens(), 0);

        let m2: Marking = Marking::from(Vec::<u32>::new());
        assert_eq!(m, m2);
        assert_eq!(m.partial_cmp(&m2), Some(Ordering::Equal));
    }

    #[test]
    fn incomparable_markings() {
        let a: Marking = [2, 0, 1].into();
        let b: Marking = [0, 2, 1].into();
        assert!(a.partial_cmp(&b).is_none());
        assert_ne!(a, b);
    }

    #[test]
    fn covering_relation_equal() {
        let a: Marking = [1, 2, 3].into();
        let b: Marking = [1, 2, 3].into();
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
    }

    #[test]
    fn omega_incomparable() {
        let a: OmegaMarking = [Omega::Unbounded, Omega::Finite(0)].into();
        let b: OmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(a.partial_cmp(&b).is_none());
    }

    #[test]
    fn cross_type_incomparable() {
        let u32m: Marking = [5, 0].into();
        let om: OmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(u32m.partial_cmp(&om).is_none());
    }

    #[test]
    fn cross_type_covering() {
        let u32m: Marking = [1, 2].into();
        let om: OmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert!(u32m < om);
        assert!(om > u32m);
    }

    #[test]
    fn support_sparse() {
        let m: Marking = [0, 0, 5, 0, 3, 0].into();
        let support: Vec<Place> = m.support().collect();
        assert_eq!(support, vec![Place { idx: 2 }, Place { idx: 4 }]);
    }

    #[test]
    fn omega_try_from_all_finite() {
        let om: OmegaMarking = [Omega::Finite(10), Omega::Finite(20)].into();
        let result: Result<Marking<u32>, _> = om.try_into();
        assert_eq!(result.unwrap(), Marking::from([10, 20]));
    }

    #[test]
    fn omega_try_from_has_unbounded() {
        let om: OmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        let result: Result<Marking<u32>, _> = om.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn from_iterator() {
        let m: Marking = (0..5).collect();
        assert_eq!(m.len(), 5);
        assert_eq!(m[Place { idx: 3 }], 3);
    }

    #[test]
    fn into_iterator() {
        let m: Marking = [10, 20, 30].into();
        let v: Vec<u32> = m.into_iter().collect();
        assert_eq!(v, vec![10, 20, 30]);
    }

    #[test]
    fn from_place_map() {
        let pm: PlaceMap<u32> = vec![1u32, 2, 3].into();
        let m = Marking::from(pm);
        assert_eq!(m[Place { idx: 0 }], 1);
        assert_eq!(m[Place { idx: 2 }], 3);
    }
}
