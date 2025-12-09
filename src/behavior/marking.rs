use crate::structure::{Net, Place};
use derive_more::{Add, Sub, AddAssign, SubAssign};
use num_traits::Zero;
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Index, IndexMut, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Add, Sub, AddAssign, SubAssign)]
pub struct Tokens(pub i32);

impl<T: Into<i32>> From<T> for Tokens {
    fn from(value: T) -> Self {
        Tokens(value.into())
    }
}

impl Zero for Tokens {
    fn zero() -> Self {
        Tokens(0)
    }

    fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A marking represented by a vector of length |S|, indexed by place ID.
#[derive(Debug, Default, Clone, Eq, Hash)]
pub struct Marking<T>(Box<[T]>);

impl<T> From<Vec<T>> for Marking<T> {
    fn from(value: Vec<T>) -> Self {
        Marking(value.into_boxed_slice())
    }
}

impl<T> From<Box<[T]>> for Marking<T> {
    fn from(value: Box<[T]>) -> Self {
        Marking(value)
    }
}

impl<T, const N: usize> From<[T; N]> for Marking<T> {
    fn from(value: [T; N]) -> Self {
        Marking(Box::new(value))
    }
}

impl<T> FromIterator<T> for Marking<T> {
    fn from_iter<I: IntoIterator<Item: Into<T>>>(iter: I) -> Self {
        Marking(iter.into_iter().map(Into::into).collect())
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

macro_rules! impl_marking_trait_for_tuple {
    ($($name:tt),+) => {
        impl<T, $($name),+> From<($($name),+)> for Marking<T>
        where
            $($name: Into<T>,)+
        {
            #[allow(non_snake_case)]
            fn from(($($name),+): ($($name),+)) -> Self {
                Marking(Box::new([$($name.into()),+]))
            }
        }
    };
}

// impl_marking_trait_for_tuple!(T1);
// macro currently generates (T1) when it should generate (T1,)
// so we implement the 1-tuple case manually
impl<T, T1> From<(T1,)> for Marking<T>
where
    T1: Into<T>,
{
    #[allow(non_snake_case)]
    fn from((T1,): (T1,)) -> Self {
        Marking(Box::new([T1.into()]))
    }
}

impl_marking_trait_for_tuple!(T1, T2);
impl_marking_trait_for_tuple!(T1, T2, T3);
impl_marking_trait_for_tuple!(T1, T2, T3, T4);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5, T6);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_marking_trait_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);

impl<T> Index<Place> for Marking<T> {
    type Output = T;

    fn index(&self, place: Place) -> &Self::Output {
        self.0.get(place.index).expect("place index out of bounds")
    }
}

impl<T> IndexMut<Place> for Marking<T> {
    fn index_mut(&mut self, place: Place) -> &mut Self::Output {
        self.0.get_mut(place.index).expect("place index out of bounds")
    }
}

impl<T> Marking<T> {
    /// Creates a new empty marking with the given capacity (number of places).
    #[must_use]
    pub fn zeroes(n_places: impl Into<usize>) -> Self where T: Zero + Clone {
        Self(vec![T::zero(); n_places.into()].into_boxed_slice())
    }

    /// Returns the number of places in the marking.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }

    /// Returns true if the marking is zero (all places have zero tokens).
    #[must_use]
    pub fn is_zero(&self) -> bool where T: Zero + PartialEq {
        self.0.iter().all(|t| *t == T::zero())
    }
    /// In-place ceiling operation: self[i] = max(self[i], other[i])
    pub fn ceil(&mut self, other: &Marking<T>) where T: Clone + PartialOrd {
        debug_assert!(self.len() == other.len(), "mismatched length");
        for (a, b) in Iterator::zip(self.iter_mut(), other.iter()) {
            if *a < *b {
                *a = b.clone();
            }
        }
    }
    /// In-place floor operation: self[i] = min(self[i], other[i])
    pub fn floor(&mut self, other: &Marking<T>) where T: Clone + PartialOrd {
        debug_assert!(self.len() == other.len(), "mismatched length");
        for (a, b) in Iterator::zip(self.iter_mut(), other.iter()) {
            if *a > *b {
                *a = b.clone();
            }
        }
    }
    pub fn support(&self) -> impl Iterator<Item = Place>
    where
        T: Zero + PartialOrd,
    {
        self.iter()
            .enumerate()
            .filter_map(|(index, t)| {
                if t > &T::zero() {
                    Some(Place { index })
                } else {
                    None
                }
            })
    }
    /// Tries to add two markings together. Returns Err(()) if addition overflows for any place.
    pub fn try_add(mut self, other: &NormalMarking) -> Result<Self, ()>
    where
        T: Zero + AddAssign<Tokens> + PartialOrd,
    {
        debug_assert!(self.len() == other.len(), "mismatched length");
        for (a, b) in Iterator::zip(self.iter_mut(), other.iter()) {
            *a += *b;
            if (*a < T::zero()) {
                return Err(());
            }
        }
        Ok(self)
    }
}

impl OmegaMarking {
    pub fn omegas(n_places: impl Into<usize>) -> Self {
        Self(vec![Omega::Omega; n_places.into()].into_boxed_slice())
    }
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.iter().all(|t| matches!(t, Omega::Finite(_)))
    }
}

pub struct Unbounded;

impl Add for Omega<Tokens> {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        match (self, other) {
            (Omega::Finite(a), Omega::Finite(b)) => Omega::Finite(a + b),
            _ => Omega::Omega,
        }
    }
}

impl AddAssign for Omega<Tokens> {
    fn add_assign(&mut self, other: Self) {
        *self = match (&self, other) {
            (Omega::Finite(a), Omega::Finite(b)) => Omega::Finite(*a + b),
            _ => Omega::Omega,
        }
    }
}

impl AddAssign<Tokens> for Omega<Tokens> {
    fn add_assign(&mut self, other: Tokens) {
        *self = match &self {
            Omega::Finite(a) => Omega::Finite(*a + other),
            Omega::Omega => Omega::Omega,
        }
    }
}

impl Zero for Omega<Tokens> {
    fn zero() -> Self {
        Omega::Finite(Tokens::default())
    }

    fn is_zero(&self) -> bool {
        matches!(self, Omega::Finite(t) if *t == Tokens::default())
    }
}

impl TryFrom<Omega<Tokens>> for Tokens {
    type Error = Unbounded;

    fn try_from(value: Omega<Tokens>) -> Result<Self, Self::Error> {
        match value {
            Omega::Finite(t) => Ok(t),
            Omega::Omega => Err(Unbounded),
        }
    }
}

impl TryFrom<OmegaMarking> for NormalMarking {
    type Error = Unbounded;

    fn try_from(value: OmegaMarking) -> Result<Self, Self::Error> {
        value.into_iter().map(TryInto::try_into).collect()
    }
}

pub fn ceil<T: Clone + PartialOrd>(a: &Marking<T>, b: &Marking<T>) -> Marking<T> {
    debug_assert!(a.len() == b.len(), "mismatched length");
    Iterator::zip(a.iter(), b.iter())
        .map(|(x, y)| if x > y { x.clone() } else { y.clone() })
        .collect()
}

pub fn floor<T: Clone + PartialOrd>(a: &Marking<T>, b: &Marking<T>) -> Marking<T> {
    debug_assert!(a.len() == b.len(), "mismatched length");
    Iterator::zip(a.iter(), b.iter())
        .map(|(x, y)| if x < y { x.clone() } else { y.clone() })
        .collect()
}

impl<T: fmt::Display> fmt::Display for Marking<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, token) in self.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{token}")?;
        }
        write!(f, ")")
    }
}

impl<T, R> PartialEq<Marking<R>> for Marking<T>
where
    T: PartialEq<R>,
{
    fn eq(&self, other: &Marking<R>) -> bool {
        Iterator::zip(self.iter(), other.iter()).all(|(a, b)| a == b)
    }
}

/// Implements the covering relation for markings.
/// A marking M1 covers M2 if for all places p, M1(p) >= M2(p).
/// M1 > M2 if M1 covers M2 and there exists at least one place p where M1(p) > M2(p).
/// M1 == M2 if for all places p, M1(p) == M2(p).
/// M1 <= M2 if M2 covers M1.
/// M1 < M2 if M2 covers M1 and there exists at least one place p where M2(p) > M1(p).
/// It is possible that neither M1 <= M2 nor M1 >= M2 (incomparable), therefore this
/// is only a partial ordering.
/// Markings do not have to be of the same type to be compared, as long as their token types
/// can be compared.
///
/// ```
/// use petrivet::behavior::NormalMarking;
/// let m0: NormalMarking = (1, 3, 0).into();
/// let m1: NormalMarking = (2, 3, 0).into();
/// let m2: NormalMarking = (1, 4, 0).into();
/// assert!(m1 > m0);
/// assert!(m2 > m0);
/// assert!(m0 < m1);
/// assert!(m0 < m2);
/// assert!(m1.partial_cmp(&m2).is_none()); // m1 and m2 are incomparable
/// ```
impl<T, R> PartialOrd<Marking<R>> for Marking<T>
where
    T: PartialOrd<R>,
{
    fn partial_cmp(&self, other: &Marking<R>) -> Option<Ordering> {
        Iterator::zip(self.iter(), other.iter())
            .map(|(x, y)| x.partial_cmp(y))
            .try_fold(Ordering::Equal, |acc, next| match (acc, next) {
                (Ordering::Equal, Some(o)) | (o, Some(Ordering::Equal)) => Some(o),
                (Ordering::Less, Some(Ordering::Less)) => Some(Ordering::Less),
                (Ordering::Greater, Some(Ordering::Greater)) => Some(Ordering::Greater),
                _ => None,
            })
    }
}

pub type NormalMarking = Marking<Tokens>;

impl Add<OmegaMarking> for NormalMarking {
    type Output = OmegaMarking;

    fn add(self, rhs: OmegaMarking) -> Self::Output {
        debug_assert!(self.len() == rhs.len(), "mismatched length");
        Iterator::zip(self.into_iter(), rhs)
            .map(|(a, b)| a + b)
            .collect()
    }
}

impl Add<NormalMarking> for OmegaMarking {
    type Output = OmegaMarking;

    fn add(self, rhs: NormalMarking) -> Self::Output {
        rhs + self
    }
}

impl Add<Omega<Tokens>> for Tokens {
    type Output = Omega<Tokens>;

    fn add(self, rhs: Omega<Tokens>) -> Self::Output {
        match rhs {
            Omega::Finite(t) => Omega::Finite(self + t),
            Omega::Omega => Omega::Omega,
        }
    }
}

impl Add<Tokens> for Omega<Tokens> {
    type Output = Omega<Tokens>;

    fn add(self, rhs: Tokens) -> Self::Output {
        rhs + self
    }
}

/// Omega represents either a specific number of tokens (Finite)
/// or an unbounded number of tokens (Omega).
/// This is used in the coverability graph to represent places that can grow without bound.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Omega<T> {
    Finite(T),
    Omega, // Omega is greater than any finite value
}

impl<T: fmt::Display> fmt::Display for Omega<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Omega::Finite(t) => write!(f, "{t}"),
            Omega::Omega => write!(f, "ω"),
        }
    }
}

/// Allow easy conversion from a Token count to an Omega.
impl<T> From<T> for Omega<Tokens> where T: Into<Tokens> {
    fn from(t: T) -> Self {
        Omega::Finite(t.into())
    }
}

/// Default Omega is Finite(0).
impl<T: Default> Default for Omega<T> {
    fn default() -> Self {
        Omega::Finite(T::default())
    }
}

/// Makes it possible to compare an omega marking with a finite token count
/// Omega is always unequal to any finite number
/// This implementation is specifically for comparing Omega<Tokens> with Tokens
impl PartialEq<Tokens> for Omega<Tokens> {
    fn eq(&self, other: &Tokens) -> bool {
        match self {
            Omega::Finite(t) => t == other,
            Omega::Omega => false,
        }
    }
}

/// Makes it possible to compare a finite token count with an omega marking
/// Omega is always unequal to any finite number
/// This implementation is specifically for comparing Tokens with Omega<Tokens>
impl PartialEq<Omega<Tokens>> for Tokens {
    fn eq(&self, other: &Omega<Tokens>) -> bool {
        other == self
    }
}

/// Makes it possible to compare an omega marking with a finite token count
/// Omega is always larger than any finite number
/// This implementation is specifically for comparing Omega<Tokens> with Tokens
impl PartialOrd<Tokens> for Omega<Tokens> {
    fn partial_cmp(&self, other: &Tokens) -> Option<Ordering> {
        match self {
            Omega::Omega => Some(Ordering::Greater),
            Omega::Finite(t) => t.partial_cmp(other),
        }
    }
}

/// Makes it possible to compare a finite token count with an omega marking
/// Omega is always larger than any finite number
/// This implementation is specifically for comparing Tokens with Omega<Tokens>
impl PartialOrd<Omega<Tokens>> for Tokens {
    fn partial_cmp(&self, other: &Omega<Tokens>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}

pub type OmegaMarking = Marking<Omega<Tokens>>;

impl OmegaMarking {
    #[must_use]
    pub fn is_normal(&self) -> bool {
        self.iter().all(|t| matches!(t, Omega::Finite(_)))
    }
}

impl From<NormalMarking> for OmegaMarking {
    fn from(marking: NormalMarking) -> Self {
        Marking(marking.into_iter().map(Omega::Finite).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn size() {
        const { assert!(size_of::<Omega<Tokens>>() == 8); }
        const { assert!(size_of::<Tokens>() == 4); }
    }

    #[test]
    fn test_normal_marking_eq_omega_marking_all_finite() {
        let normal: NormalMarking = (1, 2, 3).into();
        let omega: OmegaMarking = (1, 2, 3).into();
        assert_eq!(normal, omega);
        assert_eq!(omega, normal); // symmetric
    }

    #[test]
    fn test_normal_marking_ne_omega_marking_with_omega() {
        let normal: NormalMarking = (1, 2, 3).into();
        let omega: OmegaMarking = (1, Omega::Omega, 3).into();
        assert_ne!(normal, omega);
        assert_ne!(omega, normal); // symmetric
    }

    #[test]
    fn test_normal_marking_lt_omega_marking_with_omega() {
        let normal: NormalMarking = (1, 2, 3).into();
        let omega: OmegaMarking = (1, Omega::Omega, 3).into();
        assert!(normal < omega);
        assert!(omega > normal); // symmetric
    }

    #[test]
    fn test_omega_never_equal_to_finite() {
        let token = Tokens(5);
        let omega_finite = Omega::Finite(Tokens(5));
        let omega_inf = Omega::Omega;

        // Finite token vs Finite omega - equal
        assert_eq!(token, omega_finite);
        assert_eq!(omega_finite, token);

        // Finite token vs Omega - never equal
        assert_ne!(token, omega_inf);
        assert_ne!(omega_inf, token);
    }

    #[test]
    fn test_omega_always_greater_than_finite() {
        let token = Tokens(i32::MAX); // Even a very large number
        let omega = Omega::Omega;

        assert!(token < omega);
        assert!(omega > token);
        assert_eq!(token.partial_cmp(&omega), Some(Ordering::Less));
        assert_eq!(omega.partial_cmp(&token), Some(Ordering::Greater));
    }

    #[test]
    fn test_marking_incomparable() {
        let m1: NormalMarking = (2, 3, 0).into();
        let m2: NormalMarking = (1, 4, 0).into();
        assert!(m1.partial_cmp(&m2).is_none()); // m1 and m2 are incomparable
        assert!(m2.partial_cmp(&m1).is_none()); // m2 and m1 are incomparable
    }
}