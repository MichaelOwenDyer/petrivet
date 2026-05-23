use crate::core::marking;
use crate::core::marking::IdxMarking;
use crate::core::net::TransitionIdx;
use crate::core::state_space::{DenseStateGraph, DenseStateGraphExplorer, TokenOps};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::iter;

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

impl TokenOps for Omega {
    fn zero() -> Self { Omega::Finite(0) }
    fn one() -> Self { Omega::Finite(1) }
    fn at_least_one(&self) -> bool {
        match self {
            Omega::Finite(n) => *n >= 1,
            Omega::Unbounded => true,
        }
    }
    fn increment(&mut self) {
        if let Omega::Finite(n) = self { *n += 1; }
    }
    fn decrement(&mut self) {
        if let Omega::Finite(n) = self { *n -= 1; }
    }
}

/// An ω-marking: a marking where token counts can be finite or "infinity" (ω).
/// Used to construct the Karp-Miller coverability graph, where ω represents unbounded growth of tokens.
pub type IdxOmegaMarking = IdxMarking<Omega>;

impl IdxMarking<Omega> {
    /// Returns `true` if all components are finite (no ω).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.iter().all(|o| o.is_finite())
    }
}

impl From<&IdxMarking<u32>> for IdxMarking<Omega> {
    fn from(marking: &IdxMarking<u32>) -> Self {
        IdxMarking(marking.iter().map(|&n| Omega::Finite(n)).collect())
    }
}

impl From<IdxMarking<u32>> for IdxMarking<Omega> {
    fn from(marking: IdxMarking<u32>) -> Self {
        IdxMarking(marking.into_iter().map(Omega::Finite).collect())
    }
}

impl TryFrom<IdxMarking<Omega>> for IdxMarking<u32> {
    type Error = ();
    fn try_from(omega_marking: IdxMarking<Omega>) -> Result<Self, ()> {
        omega_marking.into_iter()
            .map(|o| o.finite().ok_or(()))
            .collect()
    }
}

impl PartialEq<IdxMarking<Omega>> for IdxMarking<u32> {
    fn eq(&self, other: &IdxMarking<Omega>) -> bool {
        self.place_count() == other.place_count() && iter::zip(self.0.iter(), other.0.iter())
            .all(|(&t, &o)| o == Omega::Finite(t))
    }
}

impl PartialEq<IdxMarking<u32>> for IdxMarking<Omega> {
    fn eq(&self, other: &IdxMarking<u32>) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<IdxMarking<Omega>> for IdxMarking<u32> {
    fn partial_cmp(&self, other: &IdxMarking<Omega>) -> Option<Ordering> {
        debug_assert_eq!(self.place_count(), other.place_count());
        iter::zip(self.0.iter(), other.0.iter())
            .map(|(&n, o)| Omega::Finite(n).cmp(o))
            .try_fold(Ordering::Equal, marking::merge_ordering)
    }
}

impl PartialOrd<IdxMarking<u32>> for IdxMarking<Omega> {
    fn partial_cmp(&self, other: &IdxMarking<u32>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}

impl DenseStateGraphExplorer<'_, Omega> {
    pub fn explore_next(&mut self) -> Option<(TransitionIdx, NodeIndex, bool)> {
        /// Karp–Miller acceleration: if any ancestor of `src` (including `src`
        /// itself) carries a marking strictly smaller than `new_marking`,
        /// promote each strictly-greater component of `new_marking` to ω.
        fn omega_accelerate(
            state_space: &DenseStateGraph<'_, Omega>,
            new_marking: &mut IdxOmegaMarking, src: NodeIndex
        ) {
            let mut stack = vec![src];
            let mut visited: HashSet<NodeIndex> = HashSet::new();
            while let Some(predecessor_node) = stack.pop() {
                if !visited.insert(predecessor_node) {
                    continue;
                }
                let ancestor_marking = state_space.marking_at(predecessor_node);
                if ancestor_marking < new_marking {
                    for (component, prev) in new_marking.iter_mut().zip(ancestor_marking.iter()) {
                        if *component > *prev {
                            *component = Omega::Unbounded;
                        }
                    }
                }
                for incoming_edge in state_space.graph.edges_directed(
                    predecessor_node,
                    petgraph::Direction::Incoming
                ) {
                    stack.push(incoming_edge.source());
                }
            }
        }

        loop {
            let (src_node_idx, transition_idx) = self.pop_frontier()?;
            if !self.is_enabled(src_node_idx, transition_idx) {
                continue;
            }
            let mut marking = self.fire(src_node_idx, transition_idx);
            omega_accelerate(&self.state_space, &mut marking, src_node_idx);
            let (is_new, node_idx) = self.register(src_node_idx, transition_idx, marking.clone());
            return Some((transition_idx, node_idx, is_new));
        }
    }

    /// Drive exploration until either:
    ///
    /// - `predicate` returns `true` for some reachable marking — in which
    ///   case the marking is returned immediately, or
    /// - the frontier is exhausted (the entire reachability graph has been
    ///   explored without the predicate ever firing) — in which case
    ///   `None` is returned.
    pub fn find(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking<Omega>) -> bool,
    ) -> Option<&IdxOmegaMarking> {
        for &node in self.state_space.seen.values() {
            if predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        while let Some((_t_idx, node, is_new)) = self.explore_next() {
            if is_new && predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::core::marking::IdxMarking;
    use crate::core::state_space::coverability::{IdxOmegaMarking, Omega};
    use std::cmp::Ordering;

    #[test]
    fn omega_marking_from_array() {
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert_eq!(om[0], Omega::Finite(1));
        assert_eq!(om[1], Omega::Unbounded);
    }

    #[test]
    fn cross_type_eq() {
        let m: IdxMarking<u32> = [1, 2, 3].into();
        let om = IdxOmegaMarking::from(&m);
        assert_eq!(m, om);
        assert_eq!(om, m);
    }

    #[test]
    fn cross_type_lt() {
        let m: IdxMarking<u32> = [1, 2, 3].into();
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded, Omega::Finite(3)].into();
        assert!(m < om);
        assert!(om > m);
    }

    #[test]
    fn incomparable_markings() {
        let a: IdxMarking<u32> = [2, 0, 1].into();
        let b: IdxMarking<u32> = [0, 2, 1].into();
        assert!(a.partial_cmp(&b).is_none());
        assert_ne!(a, b);
    }

    #[test]
    fn covering_relation_equal() {
        let a: IdxMarking<u32> = [1, 2, 3].into();
        let b: IdxMarking<u32> = [1, 2, 3].into();
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
    }

    #[test]
    fn omega_incomparable() {
        let a: IdxOmegaMarking = [Omega::Unbounded, Omega::Finite(0)].into();
        let b: IdxOmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(a.partial_cmp(&b).is_none());
    }

    #[test]
    fn cross_type_incomparable() {
        let u32m: IdxMarking<u32> = [5, 0].into();
        let om: IdxOmegaMarking = [Omega::Finite(0), Omega::Unbounded].into();
        assert!(u32m.partial_cmp(&om).is_none());
    }

    #[test]
    fn cross_type_covering() {
        let u32m: IdxMarking<u32> = [1, 2].into();
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        assert!(u32m < om);
        assert!(om > u32m);
    }

    #[test]
    fn omega_try_from_all_finite() {
        let om: IdxOmegaMarking = [Omega::Finite(10), Omega::Finite(20)].into();
        let result: Result<IdxMarking<u32>, _> = om.try_into();
        assert_eq!(result.unwrap(), IdxMarking::from([10, 20]));
    }

    #[test]
    fn omega_try_from_has_unbounded() {
        let om: IdxOmegaMarking = [Omega::Finite(1), Omega::Unbounded].into();
        let result: Result<IdxMarking<u32>, _> = om.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn omega_ordering() {
        assert!(Omega::Finite(100) < Omega::Unbounded);
        assert!(Omega::Unbounded > Omega::Finite(u32::MAX));
        assert_eq!(Omega::Finite(5), Omega::Finite(5));
    }
}