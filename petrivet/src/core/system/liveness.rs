use crate::core::net::{DenseNet, IdxNode, TransitionIdx};
use crate::core::system::marking::IdxMarking;
use crate::system::liveness::{LivenessProvenance, LivenessLevel, LivenessQuery, LivenessScope};
use fixedbitset::FixedBitSet;
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNodeReferences;

impl LivenessScope {
    fn liveness(&self, by_transition: &[Liveness]) -> Liveness {
        match self {
            LivenessScope::Transition(t_idx) => by_transition[*t_idx],
            LivenessScope::PetriNet => Liveness::aggregate(by_transition),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liveness {
    /// We are sure the true liveness level is at least this value.
    least: LivenessLevel,
    /// Evidence for the least bound.
    least_provenance: LivenessProvenance,
    /// We are sure the true liveness level is at most this value.
    most: LivenessLevel,
    /// Evidence for the most bound.
    most_provenance: LivenessProvenance,
}

impl Liveness {
    fn new() -> Liveness {
        Liveness {
            least: LivenessLevel::L0,
            least_provenance: LivenessProvenance::None,
            most: LivenessLevel::L4,
            most_provenance: LivenessProvenance::None,
        }
    }
    fn aggregate(by_transition: &[Liveness]) -> Self {
        let least = by_transition.iter().map(|l| l.least).min().unwrap_or(LivenessLevel::L0);
        let least_provenance = LivenessProvenance::None;
        let most = by_transition.iter().map(|l| l.most).min().unwrap_or(LivenessLevel::L4);
        let most_provenance = LivenessProvenance::None;
        Self { least, least_provenance, most, most_provenance }
    }
}

#[derive(Debug, Clone)]
pub struct LivenessCache {
    /// The liveness level of each transition, if known.
    by_transition: Box<[Liveness]>,
    /// Whether we should refine liveness by unmarked siphon.
    pub(crate) should_refine_by_unmarked_siphon: bool,
}

impl LivenessCache {
    pub fn new(transition_count: u32) -> Self {
        let by_transition = vec![Liveness::new(); transition_count as usize].into_boxed_slice();
        let should_refine_by_unmarked_siphon = true;
        Self { by_transition, should_refine_by_unmarked_siphon }
    }

    /// Updates the liveness level for all transitions to the given level.
    pub fn update_all(&mut self, level: LivenessLevel) {
        for liveness in &mut self.by_transition[..] {
            liveness.least = level;
            liveness.most = level;
        }
    }

    /// Updates the liveness level for a transition to the given level.
    pub fn update_exact(&mut self, t_idx: TransitionIdx, level: LivenessLevel) {
        let liveness = &mut self.by_transition[t_idx];
        debug_assert!(liveness.least <= level, "cannot decrease the least known liveness level");
        debug_assert!(liveness.most >= level, "cannot increase the most known liveness level");
        liveness.least = level;
        liveness.most = level;
    }

    /// Updates the least known liveness level for a transition to the given level.
    pub fn update_lower_bound(&mut self, t_idx: TransitionIdx, level: LivenessLevel) {
        let liveness = &mut self.by_transition[t_idx];
        debug_assert!(liveness.least <= level, "cannot decrease the least known liveness level");
        liveness.least = level;
    }

    /// Returns true if we can answer the liveness question for the given scope and query
    /// with the current liveness cache.
    pub fn is_answered(&self, scope: LivenessScope, query: LivenessQuery) -> bool {
        match query {
            LivenessQuery::Exact => self.read_exact(scope).is_some(),
            LivenessQuery::AtLeast(level) => self.read_at_least(scope, level).is_some(),
        }
    }

    /// Returns Some(liveness) if the liveness level for the given scope is known exactly,
    /// and None if it is not yet known.
    pub fn read_exact(&self, scope: LivenessScope) -> Option<LivenessLevel> {
        let liveness = scope.liveness(&self.by_transition);
        if liveness.least == liveness.most {
            Some(liveness.least)
        } else {
            None
        }
    }

    /// Returns Some(true) if the Petri net is guaranteed to be at least the given liveness level,
    /// Some(false) if it is guaranteed to be less than the given level,
    /// and None if the answer has not yet been computed.
    pub fn read_at_least(&self, scope: LivenessScope, level: LivenessLevel) -> Option<bool> {
        let liveness = scope.liveness(&self.by_transition);
        if liveness.least >= level {
            Some(true)
        } else if liveness.most < level {
            Some(false)
        } else {
            None
        }
    }
}

impl DenseNet {
    /// For a non-strongly-connected state machine, we can determine individual transaction
    /// liveness levels by analyzing the marked strongly connected components of the net
    /// and which other components they can reach.
    ///
    /// Runs in linear time: one pass over the condensation graph in topological order finds
    /// the components reachable from a marked one, and one pass in reverse topological order
    /// finds, for every component, which terminal components are reachable from it.
    pub fn liveness_via_state_machine_marked_sccs(
        &self,
        marking: &IdxMarking<u32>,
    ) -> Vec<LivenessLevel> {
        /// It is important for us to know whether a given token is guaranteed to arrive in exactly
        /// one terminal nontrivial SCC, because that makes the destination SCC L4-live.
        /// But if a token might end up in one or another, then we can only conclude L3-liveness
        /// for all destination SCCs. This is not a final conclusion until we have analyzed
        /// all marked SCCs, because it only takes one guaranteed token arrival to upgrade an SCC from L3 to L4.
        #[derive(Clone, Copy)]
        enum ReachableSinks {
            One(NodeIndex),
            Multiple,
        }

        impl ReachableSinks {
            /// The terminal SCCs reachable from a node are the union of those reachable
            /// from its successors, so they are unique iff all successors agree on the same one.
            fn join(self, other: Self) -> Self {
                match (self, other) {
                    (Self::One(a), Self::One(b)) if a == b => Self::One(a),
                    _ => Self::Multiple,
                }
            }
        }

        let condensation_graph = petgraph::algo::condensation(self.graph.clone(), true);
        let topological_order = petgraph::algo::toposort(&condensation_graph, None)
            .expect("the condensation graph is acyclic");
        let is_marked = {
            let mut is_marked = FixedBitSet::with_capacity(condensation_graph.node_count());
            for (scc, nodes) in condensation_graph.node_references() {
                if nodes.iter().any(|node| matches!(node, &IdxNode::Place(p_idx) if marking[p_idx] > 0)) {
                    is_marked.insert(scc.index());
                }
            }
            is_marked
        };

        // an SCC is reachable if it is marked or downstream of a marked SCC.
        // topological order guarantees that all predecessors are final when we visit an SCC.
        let mut reachable = is_marked.clone();
        for &scc in &topological_order {
            if reachable[scc.index()] {
                for neighbor in condensation_graph.neighbors(scc) {
                    reachable.insert(neighbor.index());
                }
            }
        }

        // the terminal SCCs reachable from each SCC. reverse topological order guarantees
        // that all successors are final when we visit an SCC.
        let mut reachable_sinks: Vec<Option<ReachableSinks>> = vec![None; condensation_graph.node_count()];
        for &scc in topological_order.iter().rev() {
            reachable_sinks[scc.index()] = condensation_graph
                .neighbors(scc)
                .map(|neighbor| reachable_sinks[neighbor.index()].expect("successors are visited first"))
                .reduce(ReachableSinks::join)
                // no successors: this is a terminal SCC
                .or(Some(ReachableSinks::One(scc)));
        }

        // any token from a marked SCC whose only reachable terminal SCC is nontrivial
        // is guaranteed to end up there, making all transitions in it L4-live
        let guaranteed_sinks: FixedBitSet = {
            let mut guaranteed_sinks = FixedBitSet::with_capacity(condensation_graph.node_count());
            for scc in condensation_graph.node_indices() {
                if is_marked[scc.index()]
                    && let Some(ReachableSinks::One(sink)) = reachable_sinks[scc.index()] {
                    guaranteed_sinks.insert(sink.index());
                }
            }
            guaranteed_sinks
        };

        let mut liveness = vec![LivenessLevel::L0; self.transition_count()];

        // a reachable trivial SCC can only be L1, a reachable nontrivial SCC is at least L3
        // and unreachable SCCs are L0.
        for (scc, nodes) in condensation_graph.node_references() {
            let level = match (reachable[scc.index()], nodes.len()) {
                (false, _) => LivenessLevel::L0,
                (true, 1) => LivenessLevel::L1,
                (true, _) if guaranteed_sinks.contains(scc.index()) => LivenessLevel::L4,
                (true, _) => LivenessLevel::L3,
            };
            for node in nodes {
                if let &IdxNode::Transition(t_idx) = node {
                    liveness[t_idx] = level;
                }
            }
        }

        liveness
    }

    /// For a marked graph, we can determine liveness levels by analyzing which transitions
    /// are on unmarked circuits (L0), and which transitions can be reached from them via pathways
    /// which may contain some marked places (L1) or only unmarked places (L0).
    /// All other transitions are L4.
    pub fn liveness_via_marked_graph_unmarked_circuits(
        &self,
        marking: &IdxMarking<u32>,
    ) -> Vec<LivenessLevel> {
        /// Perform a depth-first search through the net from a given transition,
        /// recording L0-liveness if there has been no marked place on the path so far,
        /// and L1-liveness if there has been and the transition is not already marked L0.
        fn dfs_finite_pathways(
            net: &DenseNet,
            marking: &IdxMarking<u32>,
            t_idx: TransitionIdx,
            marked: bool,
            liveness: &mut [LivenessLevel],
        ) {
            match liveness[t_idx] {
                LivenessLevel::L0 => return,
                LivenessLevel::L1 if marked => return,
                _ => {
                    liveness[t_idx] = if marked {
                        LivenessLevel::L1
                    } else {
                        LivenessLevel::L0
                    };
                }
            }
            for &p_idx in &net.postset_t[t_idx] {
                let marked = marked || marking[p_idx] > 0;
                let Some(&next_t_idx) = net.postset_p[p_idx].first() else {
                    continue;
                };
                dfs_finite_pathways(net, marking, next_t_idx, marked, liveness);
            }
        }

        // all transitions in an unmarked circuit are L0,
        // and all transitions downstream from them are at most L1. the rest are L4.
        let mut liveness = vec![LivenessLevel::L4; self.transition_count() as usize];
        for t_idx in self.transitions_on_unmarked_circuits(marking) {
            dfs_finite_pathways(self, marking, t_idx, false, &mut liveness);
        }
        liveness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_with(levels: &[Option<LivenessLevel>]) -> LivenessCache {
        let mut cache = LivenessCache::new(levels.len() as u32);
        for (t_idx, level) in levels.iter().enumerate() {
            if let Some(level) = level {
                cache.update_exact(t_idx, *level);
            }
        }
        cache
    }

    /// The level of a net is the level of its least live transition: the lower bound is the
    /// minimum of the transitions' lower bounds and the upper bound is the minimum of their upper bounds.
    #[test]
    fn net_level_is_the_minimum_over_its_transitions() {
        use LivenessLevel::{L0, L1, L4};
        let net = LivenessScope::PetriNet;

        let all_live = cache_with(&[Some(L4), Some(L4)]);
        assert_eq!(all_live.read_exact(net), Some(L4));
        assert_eq!(all_live.read_at_least(net, L4), Some(true));

        let one_dead = cache_with(&[Some(L4), Some(L4), Some(L0)]);
        assert_eq!(one_dead.read_exact(net), Some(L0));
        assert_eq!(one_dead.read_at_least(net, L4), Some(false));
        assert_eq!(one_dead.read_at_least(net, L1), Some(false));

        // a known dead transition decides the net even while another transition is still unknown ...
        let dead_and_unknown = cache_with(&[None, Some(L0)]);
        assert_eq!(dead_and_unknown.read_at_least(net, L1), Some(false));
        assert_eq!(dead_and_unknown.read_at_least(net, L4), Some(false));
        // ... whereas a live one cannot
        let live_and_unknown = cache_with(&[None, Some(L4)]);
        assert_eq!(live_and_unknown.read_at_least(net, L4), None);
        assert_eq!(live_and_unknown.read_at_least(net, L1), None);
    }
}
