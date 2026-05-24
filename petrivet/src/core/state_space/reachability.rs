use crate::core::marking::IdxMarking;
use crate::core::net::TransitionIdx;
use crate::core::state_space::{DenseStateGraphExplorer, TokenOps};
use petgraph::graph::NodeIndex;

impl TokenOps for u32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    fn at_least_one(&self) -> bool { *self >= 1 }
    fn increment(&mut self) { *self += 1; }
    fn decrement(&mut self) { *self -= 1; }
}

impl DenseStateGraphExplorer<'_, u32> {
    /// Advance exploration by one step.
    ///
    /// Returns `None` when the frontier is exhausted (fully explored).
    ///
    /// The second tuple element is the graph [`NodeIndex`] of the marking
    /// reached by firing the transition (new or existing).
    pub fn explore_next(&mut self) -> Option<(TransitionIdx, NodeIndex, bool)> {
        loop {
            let (src_idx, t_idx) = self.pop_frontier()?;
            if !self.is_enabled(src_idx, t_idx) {
                continue;
            }
            let new_marking = self.fire(src_idx, t_idx);
            let (is_new, node_idx) = self.register(src_idx, t_idx, new_marking);
            return Some((t_idx, node_idx, is_new));
        }
    }

    /// Drive exploration until either:
    ///
    /// - `predicate` returns `true` for some reachable marking — in which
    ///   case the marking is returned immediately, or
    /// - the frontier is exhausted (the entire reachability graph has been
    ///   explored without the predicate ever firing) — in which case
    ///   `None` is returned.
    ///
    /// **Does not terminate on unbounded nets.** Callers must rule that
    /// out before calling — typically via
    /// [`Net::is_structurally_bounded`](crate::Net::is_structurally_bounded)
    /// or by going through the coverability path first.
    pub fn search(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking<u32>) -> bool,
    ) -> Option<&IdxMarking<u32>> {
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