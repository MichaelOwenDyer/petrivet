//! Dense per-node maps for Petri net places and transitions.
//!
//! [`NodeMap<K, T>`] is a single implementation keyed by any [`NodeKey`] type.
//! The usual aliases are [`PlaceMap<T>`] (`NodeMap<Place, T>`) and
//! [`TransitionMap<T>`] (`NodeMap<Transition, T>`). Prefer these over a
//! `Vec<T>` indexed by `.index()` — the `Index` impls accept [`Place`] /
//! [`Transition`] directly.
//!
//! # Example
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::{PlaceMap, TransitionMap};
//!
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arc((idle, start)); b.add_arc((start, busy));
//! b.add_arc((busy, finish)); b.add_arc((finish, idle));
//! let net = b.build().unwrap();
//!
//! let mut labels: PlaceMap<&str> = net.places().map(|_| "").collect();
//! labels[idle] = "Idle";
//! labels[busy] = "Busy";
//! assert_eq!(labels[idle], "Idle");
//!
//! for (p, label) in labels.iter() {
//!     println!("{p}: {label}");
//! }
//! ```

use super::{Place, Transition};
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

mod private {
    pub trait Sealed {}
}

/// Types that can index a [`NodeMap`]: currently only [`Place`] and [`Transition`].
///
/// This trait is sealed; it cannot be implemented outside this crate.
pub trait NodeKey: Copy + Eq + Hash + fmt::Debug + private::Sealed {
    /// Dense index used as the backing array offset (0 .. node_count-1).
    fn dense_index(self) -> usize;
    /// Reconstruct a handle from a dense index (must match the net's ordering).
    fn from_dense_index(i: usize) -> Self;
}

impl private::Sealed for Place {}
impl private::Sealed for Transition {}

impl NodeKey for Place {
    fn dense_index(self) -> usize {
        self.idx
    }
    fn from_dense_index(i: usize) -> Self {
        Place::from_index(i)
    }
}

impl NodeKey for Transition {
    fn dense_index(self) -> usize {
        self.idx
    }
    fn from_dense_index(i: usize) -> Self {
        Transition::from_index(i)
    }
}

/// Dense map: one `T` per node of type `K` (place or transition).
///
/// Backed by `Box<[T]>` for O(1) access. `K` is only a type-level label;
/// the runtime representation is the same for all key types.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodeMap<K, T>(
    pub(crate) Box<[T]>,
    PhantomData<fn() -> K>,
);

/// One value per place — alias for [`NodeMap<Place, T>`].
pub type PlaceMap<T> = NodeMap<Place, T>;

/// One value per transition — alias for [`NodeMap<Transition, T>`].
pub type TransitionMap<T> = NodeMap<Transition, T>;

impl<K: NodeKey, T: Default + Clone> NodeMap<K, T> {
    /// Creates a map covering `n` nodes, each initialised to `T::default()`.
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self(vec![T::default(); n].into_boxed_slice(), PhantomData)
    }
}

impl<K: NodeKey, T> NodeMap<K, T> {
    /// Number of nodes this map covers.
    #[must_use]
    #[expect(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns a reference to the value for `key`, or `None` if out of range.
    #[must_use]
    pub fn get(&self, key: K) -> Option<&T> {
        self.0.get(key.dense_index())
    }

    /// Returns a mutable reference to the value for `key`, or `None` if out of range.
    pub fn get_mut(&mut self, key: K) -> Option<&mut T> {
        self.0.get_mut(key.dense_index())
    }

    /// Iterator over `(K, &T)` pairs in ascending dense-index order.
    pub fn iter(&self) -> impl Iterator<Item = (K, &T)> {
        self.0
            .iter()
            .enumerate()
            .map(|(i, v)| (K::from_dense_index(i), v))
    }

    /// Mutable iterator over `(K, &mut T)` pairs in ascending dense-index order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut T)> {
        self.0
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (K::from_dense_index(i), v))
    }

    /// Values in ascending dense-index order.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Mutable values in ascending dense-index order.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

impl<K: NodeKey, T: fmt::Debug> fmt::Debug for NodeMap<K, T> {
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

impl<K, T> Default for NodeMap<K, T> {
    fn default() -> Self {
        Self(Box::new([]), PhantomData)
    }
}

impl<K: NodeKey, T> Index<K> for NodeMap<K, T> {
    type Output = T;
    fn index(&self, key: K) -> &T {
        &self.0[key.dense_index()]
    }
}

impl<K: NodeKey, T> IndexMut<K> for NodeMap<K, T> {
    fn index_mut(&mut self, key: K) -> &mut T {
        &mut self.0[key.dense_index()]
    }
}

impl<K, T> From<Vec<T>> for NodeMap<K, T> {
    fn from(v: Vec<T>) -> Self {
        Self(v.into_boxed_slice(), PhantomData)
    }
}

impl<K, T> From<Box<[T]>> for NodeMap<K, T> {
    fn from(b: Box<[T]>) -> Self {
        Self(b, PhantomData)
    }
}

impl<K, T, const N: usize> From<[T; N]> for NodeMap<K, T> {
    fn from(a: [T; N]) -> Self {
        Self(Box::new(a), PhantomData)
    }
}

impl<K, T> FromIterator<T> for NodeMap<K, T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect(), PhantomData)
    }
}

impl<K, T> IntoIterator for NodeMap<K, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
