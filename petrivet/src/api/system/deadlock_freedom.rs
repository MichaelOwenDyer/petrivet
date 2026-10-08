use crate::core::net::IdxNode;
use crate::net::Net;
use crate::net::class::NetClass;
use crate::state_space::{ExplorationOrder, ReachabilityGraphExplorer};
use crate::system::PetriNet;
use crate::system::marking::Marking;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNodeReferences;

/// A **deadlock** is a [`Marking`] which enables no [`Transition`](crate::net::Transition).
///
/// **Deadlock-freedom** is a property of a system (N, M₀)
/// which holds when no reachable marking is a deadlock.
///
/// Deadlock-freedom is a desirable property in many systems,
/// as it guarantees that the system can always make progress
/// and will never get stuck in a state where no actions are possible.
pub type Deadlock = Marking<u32>;

/// An incremental iterator over all reachable deadlock markings in a system.
pub enum Deadlocks<'a> {
    /// The system is certified deadlock-free: no reachable marking is a deadlock.
    DeadlockFree,
    /// The initial marking iff it is itself a deadlock - yielded first, exactly once.
    InitialDeadlock(Option<Deadlock>),
    /// The system is not certified deadlock-free: explore the reachable markings for deadlocks.
    Explorer(ReachabilityGraphExplorer<'a>),
}

impl Iterator for Deadlocks<'_> {
    type Item = Deadlock;

    fn next(&mut self) -> Option<Deadlock> {
        match self {
            Deadlocks::DeadlockFree => None,
            Deadlocks::InitialDeadlock(deadlock) => deadlock.take(),
            Deadlocks::Explorer(reachability_explorer) => reachability_explorer
                .core
                .find(|m| reachability_explorer.core.state_space.net.is_deadlock(m))
                .map(|m| reachability_explorer.mapping.marking(m.clone())),
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure for deadlock-freedom
    /// for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_deadlock_free(&self) -> Option<bool> {
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() > 0),
            _ if self.has_source_transition() => Some(true),
            NetClass::StateMachine => Some(
                self.marking.sum() > 0 && self.deadlock_freedom_via_state_machine_marked_sccs()
            ),
            NetClass::MarkedGraph => Some(!self.has_unmarked_circuit()),
            _ => None,
        }
    }

    /// Returns an iterator over all reachable deadlock markings in the system.
    #[must_use]
    pub fn deadlocks(&self) -> Deadlocks<'_> {
        if self.is_efficiently_deadlock_free() == Some(true)
            || self.commoner_hack_criterion().is_ok() {
            Deadlocks::DeadlockFree
        } else if self.dense_net.is_deadlock(&self.marking) {
            Deadlocks::InitialDeadlock(Some(self.mapping.marking(self.marking.clone())))
        } else {
            Deadlocks::Explorer(self.explore_reachability_graph(ExplorationOrder::BreadthFirst))
        }
    }

    /// Returns true if the system is deadlock-free.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.deadlocks().next().is_none()
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// For a state machine, we can determine deadlock-freedom via DFS through the
    /// condensation graph of strongly connected components (SCCs).
    /// The system is deadlock-free iff some marked SCC is upstream of only nontrivial terminal SCCs.
    /// TODO: Proof sketch
    fn deadlock_freedom_via_state_machine_marked_sccs(&self) -> bool {
        /// Perform a depth-first search through the condensation graph from a marked source SCC,
        /// keeping track of whether all reachable terminal SCCs are nontrivial.
        /// If a trivial terminal SCC is found, the search can stop early and return false.
        fn dfs_from_markable_scc_all_terminals_are_nontrivial(
            condensation_graph: &Graph<Vec<IdxNode>, ()>,
            scc: NodeIndex,
            all_terminals_nontrivial: &mut bool,
            visited: &mut [bool],
        ) {
            if visited[scc.index()] {
                return;
            }
            visited[scc.index()] = true;
            let mut neighbors = condensation_graph.neighbors(scc).peekable();
            if neighbors.peek().is_some() {
                for neighbor in neighbors {
                    dfs_from_markable_scc_all_terminals_are_nontrivial(
                        condensation_graph,
                        neighbor,
                        all_terminals_nontrivial,
                        visited,
                    );
                    if !*all_terminals_nontrivial {
                        // if we have already found a terminal SCC that is trivial,
                        // we can stop the search early.
                        return;
                    }
                }
            } else {
                // we have reached a terminal SCC:
                // check if it is trivial (contains only one node).
                *all_terminals_nontrivial &= condensation_graph[scc].len() > 1;
            }
        }

        let condensation_graph = petgraph::algo::condensation(self.graph.clone(), true);
        let mut all_terminals_nontrivial;
        let mut visited = vec![false; condensation_graph.node_count()];
        // perform the depth-first search from each marked SCC
        for (marked_scc, _) in condensation_graph.node_references().filter(|(_, scc)| {
            scc.iter().any(|node| matches!(node, &IdxNode::Place(p_idx) if self.marking[p_idx] > 0))
        }) {
            all_terminals_nontrivial = true;
            visited.fill(false);
            dfs_from_markable_scc_all_terminals_are_nontrivial(
                &condensation_graph,
                marked_scc,
                &mut all_terminals_nontrivial,
                &mut visited,
            );
            if all_terminals_nontrivial {
                // if we found a marked SCC from which all reachable terminal SCCs are nontrivial,
                // then the system is deadlock-free.
                return true;
            }
        }
        // if we have not found any marked SCC from which all reachable terminal SCCs are nontrivial,
        // then there must exist a firing sequence which puts all tokens into trivial terminal SCCs,
        // which is a deadlock.
        false
    }
}

#[cfg(test)]
mod tests {
    /// Soundness regression. When the INITIAL marking is itself a total deadlock,
    /// the system is **not** deadlock-free. The state-space explorer's `search`
    /// evaluates the deadlock predicate only on newly-discovered *successor*
    /// markings, so the (reachable) seed marking was never tested: `deadlocks()`
    /// returned nothing and `is_deadlock_free()` fabricated a `true`.
    /// `deadlocks()` now tests the seed first, exactly once.
    #[test]
    fn initial_marking_deadlock_is_detected() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        let dead = net.with_initial_marking([]);
        assert!(!dead.is_deadlock_free(), "an m₀ deadlock must be detected");
        assert!(
            dead.deadlocks().next().is_some(),
            "deadlocks() must yield the initial deadlock marking"
        );
        // Control: the SAME cycle marked is live, hence deadlock-free; m₀ is not a
        // deadlock and the fix does not over-report.
        let live = net.with_initial_marking([(p0, 1)]);
        assert!(
            live.is_deadlock_free(),
            "a live marked cycle is deadlock-free"
        );
        assert!(
            live.deadlocks().next().is_none(),
            "a live marked cycle has no deadlock"
        );
    }
}
