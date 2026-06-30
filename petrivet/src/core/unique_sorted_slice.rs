use std::hash::Hash;

/// A deduplicated, sorted Vec.
///
/// `Deref<Target = [T]>` provides transparent access to all slice methods.
/// Set-relational operations (`is_subset_of`, `intersects`, etc.) exploit
/// the sorted order for O(n + m) merge scans.
///
/// Used to represent presets and postsets of places and transitions:
///
/// ```text
/// net.postset_p(p1).is_subset_of(net.postset_p(p0))   // p1• ⊆ p0•
/// net.preset_t(t).contains(&p)                         // p ∈ •t
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniqueSortedSlice<T>(pub(crate) Vec<T>);

impl<T> Default for UniqueSortedSlice<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: PartialEq + Ord> From<Vec<T>> for UniqueSortedSlice<T> {
    fn from(mut v: Vec<T>) -> Self {
        v.sort_unstable();
        v.dedup(); // removes all duplicates since the vector is sorted
        Self(v)
    }
}

impl<T> std::ops::Deref for UniqueSortedSlice<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T> IntoIterator for UniqueSortedSlice<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a UniqueSortedSlice<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Ord> UniqueSortedSlice<T> {
    /// `self ⊆ other`. O(n + m) merge scan.
    #[must_use]
    pub fn is_subset_of(&self, other: &Self) -> bool {
        let (a, b) = (&*self.0, &*other.0);
        if a.len() > b.len() {
            return false;
        }
        let mut j = 0;
        for elem in a {
            while j < b.len() && b[j] < *elem {
                j += 1;
            }
            if j >= b.len() || b[j] != *elem {
                return false;
            }
            j += 1;
        }
        true
    }

    /// `self ∩ other ≠ ∅`. O(n + m) merge scan.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        let (a, b) = (&*self.0, &*other.0);
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            match a[i].cmp(&b[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }

    /// `self ∩ other = ∅`. O(n + m) merge scan.
    #[must_use]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        !self.intersects(other)
    }
}

#[cfg(test)]
mod tests {
    use super::UniqueSortedSlice;

    #[test]
    fn test_uniqueness_and_sortedness() {
        let s: UniqueSortedSlice<_> = vec![3, 1, 2, 2].into();
        assert_eq!(&*s, &[1, 2, 3]);
    }
}
