use crate::core::net::TransitionIdx;
use crate::core::state_space::{DenseStateGraph, DenseStateGraphExplorer, ExploreNext, TokenOps};
use crate::liveness::LivenessLevel;
use petgraph::graph::NodeIndex;

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

        let transition_count = self.net.transition_count();
        let graph = &self.graph;
        let sccs = petgraph::algo::tarjan_scc(graph);

        let mut node_to_scc = vec![0usize; graph.node_count()];
        for (scc_id, scc) in sccs.iter().enumerate() {
            for &node in scc {
                node_to_scc[node.index()] = scc_id;
            }
        }

        let mut has_external_edge = vec![false; sccs.len()];
        let mut scc_is_nontrivial = vec![false; sccs.len()];
        let mut scc_has_t = vec![vec![false; transition_count]; sccs.len()];
        let mut t_fires_anywhere = vec![false; transition_count];

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
                // Never seen in the reachability graph, dead
                LivenessLevel::L0
            } else if (0..sccs.len()).all(|scc_idx| {
                // t appears in all terminal SCCs
                // (equivalently: every SCC either contains t or is non-terminal).
                // This implies that t is eventually fireable from any reachable
                // marking (L4-liveness).
                scc_has_t[scc_idx][t_idx] || has_external_edge[scc_idx]
            }) {
                LivenessLevel::L4
            } else if (0..sccs.len()).any(|scc_idx| {
                // t appears in some non-trivial SCC, meaning there exists
                // an infinite firing sequence which contains t infinitely
                // many times (L3-liveness).
                scc_has_t[scc_idx][t_idx] && scc_is_nontrivial[scc_idx]
            }) {
                LivenessLevel::L3
            } else {
                // t is fireable somewhere, but not in any non-trivial SCCs (loops),
                // so there is no infinite firing sequence containing t infinitely
                // many times. in fact, no firing sequence can fire t from the same
                // marking more than once, since that would create a loop with t in it.
                // we would therefore need an infinite supply of new markings to continue
                // firing t an arbitrary number of times.
                //
                // Therefore, L2-liveness cannot exist without unboundedness.
                // since we have a finite state space, we cannot have unboundedness,
                // so we cannot have L2-liveness.
                //
                // therefore, we can conclude that the transition is L1-live.
                // todo: how to detect L2 without trying to construct an infinite state space?
                LivenessLevel::L1
            }
        })
    }
}