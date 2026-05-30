use petgraph::graph::NodeIndex;
use crate::core::net::TransitionIdx;
use crate::core::state_space::{DenseStateGraph, DenseStateGraphExplorer, ExploreNext, TokenOps};
use crate::liveness::LivenessLevel;

impl TokenOps for u32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    fn at_least_one(&self) -> bool { *self >= 1 }
    fn increment(&mut self) { *self += 1; }
    fn decrement(&mut self) { *self -= 1; }
}

/// The core reachability graph exploration algorithm,
/// implemented for the case of markings with `u32` tokens.
impl ExploreNext<u32> for DenseStateGraphExplorer<'_, u32> {
    /// Advance exploration by one step.
    ///
    /// Returns `None` when the frontier is exhausted (fully explored).
    ///
    /// The second tuple element is the graph [`NodeIndex`] of the marking
    /// reached by firing the transition (new or existing).
    fn explore_next(&mut self) -> Option<(TransitionIdx, NodeIndex, bool)> {
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
}

impl DenseStateGraph<'_, u32> {
    /// Computes liveness levels for all transitions in a single pass.
    ///
    /// SCC-based decision procedure for bounded nets ([Murata 1989 §V-C](crate::literature#v-c--liveness-via-reachability-graph-sccs)):
    /// - [`L0`](LivenessLevel::L0): `t` does not label any edge.
    /// - [`L1`](LivenessLevel::L1): `t` labels at least one edge.
    /// - [`L2`](LivenessLevel::L2): equivalent to L3 because this is a finite state space.
    /// - [`L3`](LivenessLevel::L3): `t` labels an edge within some non-trivial SCC.
    /// - [`L4`](LivenessLevel::L4): `t` labels an edge in **every** terminal SCC.
    pub fn liveness_levels(&self) -> impl Iterator<Item = LivenessLevel> {
        use petgraph::visit::EdgeRef;

        let transition_count = self.net.transition_count() as usize;
        let graph = &self.graph;
        let sccs = petgraph::algo::tarjan_scc(graph);

        let mut node_to_scc = vec![0usize; graph.node_count()];
        for (scc_id, scc) in sccs.iter().enumerate() {
            for &node in scc {
                node_to_scc[node.index()] = scc_id;
            }
        }

        let scc_count = sccs.len();
        let mut has_external_edge = vec![false; scc_count].into_boxed_slice();
        let mut scc_is_nontrivial = vec![false; scc_count].into_boxed_slice();
        let mut scc_has_t = vec![vec![false; transition_count].into_boxed_slice(); scc_count].into_boxed_slice();
        let mut t_fires_anywhere = vec![false; transition_count].into_boxed_slice();

        for edge in graph.edge_references() {
            let t = *edge.weight();
            let src_scc = node_to_scc[edge.source().index()];
            let dst_scc = node_to_scc[edge.target().index()];

            t_fires_anywhere[t] = true;

            if src_scc == dst_scc {
                scc_is_nontrivial[src_scc] = true;
                scc_has_t[src_scc][t] = true;
            } else {
                has_external_edge[src_scc] = true;
            }
        }

        (0..transition_count).map(move |t_idx| {
            if !t_fires_anywhere[t_idx] {
                LivenessLevel::L0
            } else if (0..scc_count).all(|scc_idx| has_external_edge[scc_idx] || scc_has_t[scc_idx][t_idx]) {
                LivenessLevel::L4
            } else if (0..scc_count).any(|scc_idx| scc_is_nontrivial[scc_idx] && scc_has_t[scc_idx][t_idx]) {
                LivenessLevel::L3
            } else {
                LivenessLevel::L1
            }
        })
    }
}