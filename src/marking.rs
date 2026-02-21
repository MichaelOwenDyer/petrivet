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

use crate::net::Place;
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Index, IndexMut};

// ===========================================================================
// Marking<T>
// ===========================================================================

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
/// let m: Marking = vec![1u32, 0, 3].into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Marking<T = u32>(Box<[T]>);

/// Alias for coverability markings that may contain ω.
pub type OmegaMarking = Marking<Omega>;

// --- Methods for all Marking<T> ---

impl<T> Marking<T> {
    /// Number of places in this marking.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this marking has zero places.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterator over values.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Mutable iterator over values.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

impl<T: Default + Clone> Marking<T> {
    /// Creates a marking with the default value for each place.
    /// For `u32` this is 0; for `Omega` this is `Omega::Finite(0)`.
    #[must_use]
    pub fn zeros(n_places: usize) -> Self {
        Self(vec![T::default(); n_places].into_boxed_slice())
    }
}

impl<T: Ord + Copy> Marking<T> {
    /// Element-wise maximum: `self[i] = max(self[i], other[i])`.
    pub fn ceil_assign(&mut self, other: &Self) {
        debug_assert_eq!(self.len(), other.len());
        for (a, &b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = (*a).max(b);
        }
    }
}

// --- Cross-reference PartialEq (lets assert_eq! compare &Marking with Marking) ---

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

// --- Index by Place ---

impl<T> Index<Place> for Marking<T> {
    type Output = T;
    fn index(&self, p: Place) -> &T {
        &self.0[p.0]
    }
}

impl<T> IndexMut<Place> for Marking<T> {
    fn index_mut(&mut self, p: Place) -> &mut T {
        &mut self.0[p.0]
    }
}

// --- Conversions (generic) ---

impl<T> From<Vec<T>> for Marking<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v.into_boxed_slice())
    }
}

impl<T, const N: usize> From<[T; N]> for Marking<T> {
    fn from(a: [T; N]) -> Self {
        Self(Box::new(a))
    }
}

impl<T> FromIterator<T> for Marking<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

// --- Display ---

impl<T: fmt::Display> fmt::Display for Marking<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, val) in self.0.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{val}")?;
        }
        write!(f, ")")
    }
}

// --- Partial ordering (covering relation) ---
// M1 >= M2 iff M1(p) >= M2(p) for all places p.
// Two markings may be incomparable.

/// Folds element-wise orderings into a single partial ordering.
/// Returns `None` if the elements are incomparable (some greater, some lesser).
fn partial_cmp_fold(mut iter: impl Iterator<Item = Ordering>) -> Option<Ordering> {
    iter.try_fold(Ordering::Equal, |acc, next| match (acc, next) {
        (Ordering::Equal, o) | (o, Ordering::Equal) => Some(o),
        (Ordering::Less, Ordering::Less) => Some(Ordering::Less),
        (Ordering::Greater, Ordering::Greater) => Some(Ordering::Greater),
        _ => None,
    })
}

impl<T: Ord> PartialOrd for Marking<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        debug_assert_eq!(self.len(), other.len());
        partial_cmp_fold(self.0.iter().zip(other.0.iter()).map(|(a, b)| a.cmp(b)))
    }
}

// ===========================================================================
// Marking<u32> — concrete token markings
// ===========================================================================

impl Marking<u32> {
    /// Whether all places have zero tokens.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&t| t == 0)
    }

    /// Total number of tokens across all places.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.0.iter().map(|&t| u64::from(t)).sum()
    }

    /// Places that have at least one token.
    pub fn support(&self) -> impl Iterator<Item = Place> + '_ {
        self.0.iter().enumerate().filter_map(|(i, &t)| {
            if t > 0 { Some(Place(i)) } else { None }
        })
    }

    /// Applies a signed incidence vector to this marking.
    ///
    /// Returns `None` if any place would go below zero (transition not enabled).
    #[must_use]
    pub fn apply_delta(&self, delta: &[i64]) -> Option<Marking<u32>> {
        debug_assert_eq!(self.len(), delta.len());
        let mut result = Vec::with_capacity(self.len());
        for (&tokens, &d) in self.0.iter().zip(delta.iter()) {
            let new_val = i64::from(tokens) + d;
            result.push(u32::try_from(new_val).ok()?);
        }
        Some(Marking(result.into_boxed_slice()))
    }
}

// ===========================================================================
// Omega — extended token count for coverability
// ===========================================================================

/// A token count that is either finite or ω (unbounded).
///
/// Used in coverability graph construction where places can grow without bound.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Omega {
    /// A concrete finite token count.
    Finite(u32),
    /// An unbounded token count (ω). Greater than any finite value.
    Unbounded,
}

impl Omega {
    /// Returns the finite value, or `None` if unbounded.
    #[must_use]
    pub fn finite(self) -> Option<u32> {
        match self {
            Omega::Finite(n) => Some(n),
            Omega::Unbounded => None,
        }
    }

    /// Returns `true` if this is a finite value.
    #[must_use]
    pub fn is_finite(self) -> bool {
        matches!(self, Omega::Finite(_))
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

// ===========================================================================
// Marking<Omega> (OmegaMarking) — specialized methods
// ===========================================================================

impl Marking<Omega> {
    /// Creates an all-ω marking.
    #[must_use]
    pub fn unbounded(n_places: usize) -> Self {
        Self(vec![Omega::Unbounded; n_places].into_boxed_slice())
    }

    /// Returns `true` if all components are finite (no ω).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|o| o.is_finite())
    }

    /// Applies a signed incidence delta. ω values absorb any finite change.
    /// Returns `None` if any finite place would go below zero.
    #[must_use]
    pub fn apply_delta(&self, delta: &[i64]) -> Option<OmegaMarking> {
        debug_assert_eq!(self.len(), delta.len());
        let mut result = Vec::with_capacity(self.len());
        for (&omega, &d) in self.0.iter().zip(delta.iter()) {
            match omega {
                Omega::Unbounded => result.push(Omega::Unbounded),
                Omega::Finite(n) => {
                    let new_val = i64::from(n) + d;
                    let casted = u32::try_from(new_val).ok()?;
                    result.push(Omega::Finite(casted));
                }
            }
        }
        Some(Marking(result.into_boxed_slice()))
    }
}

// ===========================================================================
// Cross-type conversions and comparisons
// ===========================================================================

impl From<&Marking<u32>> for Marking<Omega> {
    fn from(m: &Marking<u32>) -> Self {
        Marking(m.iter().map(|&n| Omega::Finite(n)).collect())
    }
}

impl From<Marking<u32>> for Marking<Omega> {
    fn from(m: Marking<u32>) -> Self {
        Marking(m.into_iter().map(Omega::Finite).collect())
    }
}

impl TryFrom<Marking<Omega>> for Marking<u32> {
    type Error = ();
    fn try_from(om: Marking<Omega>) -> Result<Self, ()> {
        om.into_iter()
            .map(|o| o.finite().ok_or(()))
            .collect::<Result<Vec<_>, _>>()
            .map(Marking::from)
    }
}

impl PartialEq<Marking<Omega>> for Marking<u32> {
    fn eq(&self, other: &Marking<Omega>) -> bool {
        self.len() == other.len()
            && self.0.iter().zip(other.0.iter())
                .all(|(&n, &o)| o == Omega::Finite(n))
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
        partial_cmp_fold(
            self.0.iter().zip(other.0.iter())
                .map(|(&n, &o)| Omega::Finite(n).cmp(&o))
        )
    }
}

impl PartialOrd<Marking<u32>> for Marking<Omega> {
    fn partial_cmp(&self, other: &Marking<u32>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_array() {
        let m: Marking = [1, 0, 3].into();
        assert_eq!(m[Place(0)], 1);
        assert_eq!(m[Place(1)], 0);
        assert_eq!(m[Place(2)], 3);
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
    fn apply_delta_success() {
        let m: Marking = [2, 1, 0].into();
        let result = m.apply_delta(&[-1, 1, 1]).unwrap();
        assert_eq!(result, Marking::from([1, 2, 1]));
    }

    #[test]
    fn apply_delta_underflow() {
        let m: Marking = [0, 1].into();
        assert!(m.apply_delta(&[-1, 0]).is_none());
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
        assert_eq!(om[Place(0)], Omega::Finite(1));
        assert_eq!(om[Place(1)], Omega::Unbounded);
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
    fn omega_apply_delta() {
        let om: OmegaMarking = [Omega::Finite(2), Omega::Unbounded].into();
        let result = om.apply_delta(&[-1, 5]).unwrap();
        assert_eq!(result[Place(0)], Omega::Finite(1));
        assert_eq!(result[Place(1)], Omega::Unbounded);
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
}
