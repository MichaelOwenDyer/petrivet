//! Markings: the state of a Petri net.
//!
//! A marking assigns a token count to each place. The default token type is
//! `u32`. For coverability analysis, [`Omega`] extends token counts with an
//! unbounded symbol ω, and [`OmegaMarking`] is a type alias for `Marking<Omega>`.

use crate::api::net::Place;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::Sum;
use std::ops::Index;
use std::{iter, vec};

/// The public-facing marking type. Contains a slice of (Place, Token) pairs.
/// A place does not appear in the list iff it has `T::default()` tokens.
/// Places are sorted ascending by place ID.
#[derive(Debug, Clone)]
pub struct Marking<T>(pub(crate) Box<[(Place, T)]>);
pub type OmegaMarking = Marking<Omega>;

impl<T> AsRef<[(Place, T)]> for Marking<T> {
    fn as_ref(&self) -> &[(Place, T)] {
        &self.0
    }
}

impl<T: Default + Eq + Hash> FromIterator<(Place, T)> for Marking<T> {
    fn from_iter<I: IntoIterator<Item = (Place, T)>>(iter: I) -> Self {
        let mut x = iter
            .into_iter()
            .filter(|(_, t)| *t != T::default())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Box<_>>();
        x.sort_unstable_by_key(|&(Place(id), _)| id);
        Marking(x)
    }
}

impl<T: Default + Eq + Hash, const N: usize> From<[(Place, T); N]> for Marking<T> {
    fn from(array: [(Place, T); N]) -> Self {
        Marking::from_iter(array)
    }
}

impl<T> IntoIterator for Marking<T> {
    type Item = (Place, T);
    type IntoIter = vec::IntoIter<(Place, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Marking<u32>> for OmegaMarking {
    fn from(value: Marking<u32>) -> Self {
        value.into_iter().map(|(p, t)| (p, Omega::Finite(t))).collect()
    }
}

impl<T: Default + Copy> Marking<T> {
    /// Gets the number of tokens this marking assigns to the provided place.
    /// Returns `T::default()` if the place is not present in the marking.
    #[must_use]
    pub fn get(&self, place: Place) -> T {
        self.0.iter().find(|(p, _)| *p == place).map(|(_, t)| t).copied().unwrap_or_default()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&mut Place, &mut T)> {
        self.0.iter_mut().map(|(p, t)| (p, t))
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Place, &T)> {
        self.0.iter().map(|(p, t)| (p, t))
    }
}

impl<T: Clone + Sum> Marking<T> {
    #[must_use]
    pub fn total_tokens(&self) -> T {
        self.0.iter().map(|(_, t)| t.clone()).sum()
    }
    pub fn support(&self) -> impl Iterator<Item = Place> {
        self.0.iter().map(|(p, _)| *p)
    }
}

impl<T: Default + Copy> Index<Place> for Marking<T> {
    type Output = T;
    fn index(&self, place: Place) -> &Self::Output {
        self.0.iter().find(|(p, _)| *p == place).map(|(_, t)| t).unwrap()
    }
}

impl<T: PartialEq> PartialEq for Marking<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && iter::zip(self.0.iter(), other.0.iter())
            .all(|((p1, t1), (p2, t2))| p1 == p2 && t1 == t2)
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
    pub const fn is_finite(self) -> bool {
        matches!(self, Omega::Finite(_))
    }

    /// Returns `true` if this value is unbounded (omega).
    #[must_use]
    pub const fn is_unbounded(self) -> bool {
        matches!(self, Omega::Unbounded)
    }

    /// Returns true if this is a finite value less than or equal to `b`.
    #[must_use]
    pub const fn is_b_bounded(self, b: u32) -> bool {
        matches!(self, Omega::Finite(bound) if bound <= b)
    }

    /// Returns the finite value, or `None` if unbounded.
    #[must_use]
    pub const fn finite(self) -> Option<u32> {
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
