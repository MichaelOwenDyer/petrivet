//! Dense per-node maps for Petri net places and transitions.
//!
//! [`IndexMap<K, T>`] is a single implementation keyed by any [`NodeKey`] type.
//! The usual aliases are [`PlaceMap<T>`] (`NodeMap<Place, T>`) and
//! [`TransitionMap<T>`] (`NodeMap<Transition, T>`).
//!
//! These are `pub(crate)` — external users interact with [`PlaceKey`] /
//! [`TransitionKey`] through the `Net` API.

use super::{Place, Transition};
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

mod private {
    pub trait Sealed {}
}

/// Types that can index a [`IndexMap`]: currently only [`Place`] and [`Transition`].
///
/// This trait is sealed; it cannot be implemented outside this crate.
pub(crate) trait NodeKey: Copy + Eq + Hash + fmt::Debug + private::Sealed {
    /// Dense index used as the backing array offset (0 .. node_count-1).
    fn dense_index(self) -> usize;
    /// Reconstruct a handle from a dense index (must match the net's ordering).
    fn from_dense_index(i: usize) -> Self;
}

impl private::Sealed for Place {}
impl private::Sealed for Transition {}

impl NodeKey for Place {
    fn dense_index(self) -> usize {
        self.idx as usize
    }
    fn from_dense_index(i: usize) -> Self {
        Place::from_index(u32::try_from(i).expect("dense index fits in u32"))
    }
}

impl NodeKey for Transition {
    fn dense_index(self) -> usize {
        self.idx as usize
    }
    fn from_dense_index(i: usize) -> Self {
        Transition::from_index(u32::try_from(i).expect("dense index fits in u32"))
    }
}

/// Dense map: one `V` per node of type `K` (place or transition).
///
/// Backed by `Box<[T]>` for O(1) access. `K` is only a type-level label;
/// the runtime representation is the same for all key types.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct IndexMap<K, V>(
    pub(crate) Box<[V]>,
    PhantomData<fn() -> K>,
);

/// One value per place — alias for [`IndexMap<Place, T>`].
pub(crate) type PlaceMap<T> = IndexMap<Place, T>;

/// One value per transition — alias for [`IndexMap<Transition, T>`].
pub(crate) type TransitionMap<T> = IndexMap<Transition, T>;

impl<K: NodeKey, T: Default + Clone> IndexMap<K, T> {
    /// Creates a map covering `n` nodes, each initialised to `T::default()`.
    #[must_use]
    pub(crate) fn new(n: usize) -> Self {
        Self(vec![T::default(); n].into_boxed_slice(), PhantomData)
    }
}

impl<K: NodeKey, T> IndexMap<K, T> {
    /// Number of nodes this map covers.
    #[must_use]
    #[expect(clippy::len_without_is_empty)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns a reference to the value for `key`, or `None` if out of range.
    #[must_use]
    pub(crate) fn get(&self, key: K) -> Option<&T> {
        self.0.get(key.dense_index())
    }

    /// Returns a mutable reference to the value for `key`, or `None` if out of range.
    pub(crate) fn get_mut(&mut self, key: K) -> Option<&mut T> {
        self.0.get_mut(key.dense_index())
    }

    /// Iterator over `(K, &T)` pairs in ascending dense-index order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (K, &T)> + '_ {
        self.0
            .iter()
            .enumerate()
            .map(|(i, v)| (K::from_dense_index(i), v))
    }

    /// Mutable iterator over `(K, &mut T)` pairs in ascending dense-index order.
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut T)> + '_ {
        self.0
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (K::from_dense_index(i), v))
    }

    /// Values in ascending dense-index order.
    pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Mutable values in ascending dense-index order.
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

impl<K: NodeKey, T: fmt::Debug> fmt::Debug for IndexMap<K, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(
                self.0
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (K::from_dense_index(i), v)),
            )
            .finish()
    }
}

impl<K, T> Default for IndexMap<K, T> {
    fn default() -> Self {
        Self(Box::new([]), PhantomData)
    }
}

impl<K: NodeKey, T> Index<K> for IndexMap<K, T> {
    type Output = T;
    fn index(&self, key: K) -> &T {
        &self.0[key.dense_index()]
    }
}

impl<K: NodeKey, T> IndexMut<K> for IndexMap<K, T> {
    fn index_mut(&mut self, key: K) -> &mut T {
        &mut self.0[key.dense_index()]
    }
}

impl<K, T> From<Vec<T>> for IndexMap<K, T> {
    fn from(v: Vec<T>) -> Self {
        Self(v.into_boxed_slice(), PhantomData)
    }
}

impl<K, T> From<Box<[T]>> for IndexMap<K, T> {
    fn from(b: Box<[T]>) -> Self {
        Self(b, PhantomData)
    }
}

impl<K, T, const N: usize> From<[T; N]> for IndexMap<K, T> {
    fn from(a: [T; N]) -> Self {
        Self(Box::new(a), PhantomData)
    }
}

impl<K, T> FromIterator<T> for IndexMap<K, T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect(), PhantomData)
    }
}

impl<K, T> IntoIterator for IndexMap<K, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
