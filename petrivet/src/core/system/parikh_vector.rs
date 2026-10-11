use crate::core::net::TransitionIdx;
use crate::core::net::idx_set::TransitionIdxSet;
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdxParikhVector<T>(pub Vec<T>);

impl<T> IdxParikhVector<T> {
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T: Default + PartialEq> IdxParikhVector<T> {
    pub fn support(&self) -> TransitionIdxSet {
        let mut support = TransitionIdxSet::none_of(self.0.len());
        for (t_idx, value) in self.0.iter().enumerate() {
            if *value != Default::default() {
                support.set(t_idx as TransitionIdx, true);
            }
        }
        support
    }
}

impl<T> AsRef<[T]> for IdxParikhVector<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}

impl<T> Index<TransitionIdx> for IdxParikhVector<T> {
    type Output = T;
    fn index(&self, t: TransitionIdx) -> &T {
        &self.0[t]
    }
}

impl<T> IndexMut<TransitionIdx> for IdxParikhVector<T> {
    fn index_mut(&mut self, t: TransitionIdx) -> &mut T {
        &mut self.0[t]
    }
}

impl<T, const N: usize> From<[T; N]> for IdxParikhVector<T> {
    fn from(a: [T; N]) -> Self {
        Self(Vec::from(a))
    }
}

impl<T> FromIterator<T> for IdxParikhVector<T> {
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}