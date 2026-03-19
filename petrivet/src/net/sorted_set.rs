/// An owned, sorted, set backed by a vec.
///
/// Constructed once from a `Vec<T>`: the constructor sorts and deduplicates
/// in place, then freezes the result into a `Box<[T]>`. The invariant
/// (strictly ascending order, no duplicates) is established at construction
/// and cannot be violated afterward since there is no mutable access.
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
pub struct SortedSet<T>(Vec<T>);

impl<T> Default for SortedSet<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Ord> SortedSet<T> {
    /// Creates a new empty `SortedSet`.
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// Binary search insert. O(log n) for search, O(n) for insert.
    pub(crate) fn add(&mut self, item: T) -> bool {
        match self.0.binary_search(&item) {
            Ok(_) => false, // already present
            Err(pos) => {
                // insert at the correct position
                self.0.insert(pos, item);
                true
            },
        }
    }

    /// Binary search membership test. O(log n).
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.0.binary_search(item).is_ok()
    }

    /// Binary search removal. O(log n) for search, O(n) for removal.
    /// Returns `true` if the item was present and removed.
    pub(crate) fn remove(&mut self, item: &T) -> bool {
        match self.0.binary_search(item) {
            Ok(pos) => {
                self.0.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

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

impl<T> std::ops::Deref for SortedSet<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T: PartialEq, const N: usize> PartialEq<[T; N]> for SortedSet<T> {
    fn eq(&self, other: &[T; N]) -> bool {
        *self.0 == *other
    }
}

impl<'a, T> IntoIterator for &'a SortedSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
