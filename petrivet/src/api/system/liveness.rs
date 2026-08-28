use crate::core::net::{IdxNode, TransitionIdx, path::IdxCircuit};
use crate::net::class::NetClass;
use crate::net::{Net, Transition};
use crate::system::PetriNet;
use ahash::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNodeReferences;

/// The liveness level of a [`Transition`] describes how often it
/// can be fired in a [`Petri net`](crate::PetriNet) `(N, M₀)`.
///
/// Liveness level of a transition from a given initial marking `M₀`,
/// following Murata 1989 §V-C.
///
/// The levels form a strict hierarchy: L4 ⊂ L3 ⊂ L2 ⊂ L1, and L0 means
/// the transition is dead (not even L1).
///
/// The liveness level of a Petri net is that of its *least* live transition.
///
/// References:
/// - [Murata 1989, Definition 5.1](crate::literature#definition-51--liveness-levels-l0l4)
/// - Petri Net Primer, §5.4 (liveness)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LivenessLevel {
    /// A [Transition] `t` is **L0-live** (or *dead*) if there exists no firing sequence
    /// from `M₀` which enables `t`.
    ///
    /// In other words, `t` can never fire.
    ///
    /// L0-live transitions are always *strictly* L0-live, in the sense that
    /// no transition can simultaneously be L0-live and L1-live or higher.
    ///
    /// [Transition]: crate::net::Transition
    L0,
    /// A transition `t` is **L1-live** if it is *not* [`L0-live`](LivenessLevel::L0).
    ///
    /// In other words, there exists at least one firing sequence from `M₀` which enables `t`.
    ///
    /// Transitions which are [`L4`](LivenessLevel::L4), [`L3`](LivenessLevel::L3),
    /// or [`L2-live`](LivenessLevel::L2) are also L1-live.
    ///
    /// A transition which is L1-live but not L2-live is called *strictly* L1-live.
    L1,
    /// A transition `t` is **L2-live** if for any positive finite integer `k`,
    /// there exists a firing sequence from `M₀` which fires `t` `k` times.
    ///
    /// In other words, we can find a finite firing sequence which fires `t`
    /// any arbitrary number of times.
    ///
    /// Note that this does *not* imply the existence of an *infinite* firing sequence
    /// containing `t` infinitely many times, which is the definition of [`L3-liveness`](LivenessLevel::L3).
    ///
    /// For **bounded** Petri nets, any L2-live transition is also L3-live.
    /// This is because in a finite marking space the only way to fire a transition
    /// any arbitrary number of times is with a cycle, which immediately also enables
    /// the infinite firing sequence which spins around in that cycle forever.
    ///
    /// Transitions which are [`L4`](LivenessLevel::L4) or [`L3-live`](LivenessLevel::L3)
    /// are also L2-live.
    ///
    /// A transition which is L2-live but not L3-live is called *strictly* L2-live.
    /// Such transitions can only exist in the presence of
    /// [unboundedness](crate::boundedness::Boundedness::Unbounded).
    L2,
    /// A transition `t` is **L3-live** if there exists an *infinite* firing sequence
    /// from `M₀` which fires `t` infinitely many times.
    ///
    /// Transitions which are [`L4`](LivenessLevel::L4)-live are also L3-live.
    ///
    /// A transition which is L3-live but not L4-live is called *strictly* L3-live.
    L3,
    /// A transition `t` is **L4-live** (or just *live*) if it is [`L1`](LivenessLevel::L1)-live
    /// from *every* marking reachable from `M₀`.
    ///
    /// In other words, no matter which transitions we fire from `M₀`, and no matter which
    /// reachable marking `M` we end up in, there exists a firing sequence from `M` which enables `t`.
    /// It is impossible for `t` to become [`dead`](LivenessLevel::L0).
    L4,
}

impl LivenessLevel {
    /// Returns true if the transition is L0-live (dead).
    #[must_use]
    pub const fn is_l0_live(&self) -> bool {
        matches!(self, Self::L0)
    }

    /// Returns true if the transition is L0-live (dead).
    ///
    /// This is a synonym for `is_l0_live`.
    #[must_use]
    pub const fn is_dead(&self) -> bool {
        self.is_l0_live()
    }

    /// Returns true if the transition is [`L1-live`](LivenessLevel::L1) or higher.
    #[must_use]
    pub const fn is_l1_live(&self) -> bool {
        matches!(self, Self::L1 | Self::L2 | Self::L3 | Self::L4)
    }

    /// Returns true if the transition is *strictly* L1-live, i.e. L1-live but not L2-live.
    #[must_use]
    pub const fn is_strictly_l1_live(&self) -> bool {
        matches!(self, Self::L1)
    }

    /// Returns true if the transition is [`L2-live`](LivenessLevel::L2) or higher.
    #[must_use]
    pub const fn is_l2_live(&self) -> bool {
        matches!(self, Self::L2 | Self::L3 | Self::L4)
    }

    /// Returns true if the transition is *strictly* L2-live, i.e. L2-live but not L3-live.
    #[must_use]
    pub const fn is_strictly_l2_live(&self) -> bool {
        matches!(self, Self::L2)
    }

    /// Returns true if the transition is [`L3-live`](LivenessLevel::L3) or higher.
    #[must_use]
    pub const fn is_l3_live(&self) -> bool {
        matches!(self, Self::L3 | Self::L4)
    }

    /// Returns true if the transition is *strictly* L3-live, i.e. L3-live but not L4-live.
    #[must_use]
    pub const fn is_strictly_l3_live(&self) -> bool {
        matches!(self, Self::L3)
    }

    /// Returns true if the transition is [`L4-live`](LivenessLevel::L4) (live).
    #[must_use]
    pub const fn is_l4_live(&self) -> bool {
        matches!(self, Self::L4)
    }

    /// Returns true if the transition is [`L4-live`](LivenessLevel::L4) (live).
    ///
    /// This is a synonym for `is_l4_live`.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.is_l4_live()
    }
}

impl std::fmt::Display for LivenessLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L0 => write!(f, "L0-live"),
            Self::L1 => write!(f, "L1-live"),
            Self::L2 => write!(f, "L2-live"),
            Self::L3 => write!(f, "L3-live"),
            Self::L4 => write!(f, "L4-live"),
        }
    }
}

/// Result of liveness analysis.
/// When proved via Commoner's theorem (free-choice nets), all transitions are L4.
/// When proved via SCC analysis on the reachability graph, levels are
/// individually computed.
#[derive(Debug, Clone)]
pub struct LivenessAnalysis {
    /// All transitions in the net paired with their liveness levels.
    pub(crate) levels: HashMap<Transition, LivenessLevel>,
}

impl LivenessAnalysis {
    /// Returns the liveness level of the Petri net as a whole,
    /// defined as the minimum liveness level among all transitions.
    #[must_use]
    pub fn global_level(&self) -> LivenessLevel {
        self.levels
            .values()
            .min()
            .copied()
            .unwrap_or(LivenessLevel::L0)
    }

    /// Returns the liveness level of the provided `transition`.
    #[must_use]
    pub fn level(&self, transition: Transition) -> LivenessLevel {
        self.levels
            .get(&transition)
            .copied()
            .unwrap_or(LivenessLevel::L0)
    }

    /// Returns true if all transitions in the Petri net are live.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.levels.values().all(LivenessLevel::is_l4_live)
    }

    /// Whether every transition appears on some edge of the reachability graph,
    /// i.e. no transition in the Petri net is dead.
    #[must_use]
    pub fn is_quasi_live(&self) -> bool {
        self.levels.values().all(LivenessLevel::is_l1_live)
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If an efficient (polynomial-time) procedure for liveness
    /// is known for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_live(&self) -> Option<bool> {
        // todo: cheap condition necessary for liveness: no unmarked proper siphon in M0
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() > 0),
            NetClass::StateMachine => Some(self.is_strongly_connected() && self.marking.sum() > 0),
            NetClass::MarkedGraph => Some(!self.has_unmarked_circuit()),
            NetClass::FreeChoice => Some(self.commoner_hack_criterion().is_ok()),
            NetClass::AsymmetricChoice => self.commoner_hack_criterion().ok().map(|_| true),
            NetClass::General => None,
        }
    }

    /// Returns true if the Petri net is [`live`](LivenessLevel::L4).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.is_efficiently_live()
            .unwrap_or_else(|| self.liveness().is_live())
    }

    pub fn efficient_liveness(&self) -> Option<LivenessAnalysis> {
        let levels = match self.class() {
            NetClass::Circuit | NetClass::StateMachine if self.marking.sum() == 0 => {
                Some(self.transitions().map(|t| (t, LivenessLevel::L0)).collect())
            }
            NetClass::Circuit => Some(self.transitions().map(|t| (t, LivenessLevel::L4)).collect()),
            NetClass::StateMachine if self.is_strongly_connected() => {
                Some(self.transitions().map(|t| (t, LivenessLevel::L4)).collect())
            }
            NetClass::StateMachine => Some(self.liveness_via_state_machine_marked_sccs()),
            NetClass::MarkedGraph => Some(self.liveness_via_marked_graph_unmarked_circuits()),
            NetClass::FreeChoice => self
                .commoner_hack_criterion()
                .ok()
                .map(|_| self.transitions().map(|t| (t, LivenessLevel::L4)).collect()),
            _ => None,
        };
        levels.map(|levels| LivenessAnalysis { levels })
    }

    /// Analyses liveness, dispatching to the cheapest known procedure first.
    #[must_use]
    pub fn liveness(&self) -> LivenessAnalysis {
        self.efficient_liveness()
            .unwrap_or_else(|| self.liveness_via_reachability_graph())
    }

    /// SCC analysis on the full reachability graph. Only called as a fallback
    /// from [`liveness`](Self::liveness) for net classes without an efficient procedure.
    #[must_use]
    fn liveness_via_reachability_graph(&self) -> LivenessAnalysis {
        self.try_build_reachability_graph().map_or_else(
            |_| {
                // todo: liveness of unbounded systems
                LivenessAnalysis {
                    levels: self.transitions().map(|t| (t, LivenessLevel::L0)).collect(),
                }
            },
            |rg| rg.transition_liveness(),
        )
    }
}

// specialized procedures for state machines and marked graphs
impl<N: AsRef<Net>> PetriNet<N> {
    /// For a non-strongly-connected state machine, we can determine individual transaction
    /// liveness levels by analyzing the marked strongly connected components of the net
    /// and which other components they can reach.
    fn liveness_via_state_machine_marked_sccs(&self) -> HashMap<Transition, LivenessLevel> {
        /// It is important for us to know whether a given token is guaranteed to arrive in some
        /// terminal nontrivial strongly connected component, because that makes the SCC L4-live.
        /// But if a token might end up in one or another, then we can only conclude L3-liveness
        /// for all SCCs that might receive it. This is not a final conclusion until we have analyzed
        /// all marked SCCs, because it only takes one guaranteed token arrival to upgrade an SCC from L3 to L4.
        enum ReachableSinks {
            NoneYet,
            One(NodeIndex),
            Multiple,
        }

        /// Perform a depth-first search through the condensation graph from a marked source SCC,
        /// recording L1-liveness for trivial SCCs we pass through and L3 for nontrivial ones,
        /// and keeping track of whether we reach a single terminal nontrivial SCC (L4) or multiple (L3).
        ///
        /// This function assumes that the condensation graph is acyclic, otherwise we would need
        /// to keep track of visited nodes to avoid infinite recursion.
        fn dfs_from_markable_scc(
            condensation_graph: &Graph<Vec<IdxNode>, ()>,
            scc: NodeIndex,
            liveness: &mut [LivenessLevel],
            reachable_sinks: &mut ReachableSinks,
            visited: &mut [bool],
        ) {
            if visited[scc.index()] {
                return;
            }
            visited[scc.index()] = true;
            let scc_nodes = &condensation_graph[scc];
            if scc_nodes.len() == 1 {
                // a trivial SCC can only be L1
                liveness[scc.index()] = LivenessLevel::L1;
            } else {
                // a reachable nontrivial SCC could be L3 or L4,
                // depending on whether this is the only reachable terminal SCC or not.
                // mark provisionally as L3, we might overwrite this with L4 later.
                if liveness[scc.index()] < LivenessLevel::L3 {
                    liveness[scc.index()] = LivenessLevel::L3;
                }
            }
            let mut neighbors = condensation_graph.neighbors(scc).peekable();
            if neighbors.peek().is_some() {
                for neighbor in neighbors {
                    dfs_from_markable_scc(
                        condensation_graph,
                        neighbor,
                        liveness,
                        reachable_sinks,
                        visited,
                    );
                }
            } else {
                // we have reached a terminal SCC.
                // keep track of whether this is the only one possible to reach (L4)
                // or if there are multiple possibilities (L3)
                *reachable_sinks = match *reachable_sinks {
                    ReachableSinks::NoneYet => ReachableSinks::One(scc),
                    ReachableSinks::One(sink) if sink == scc => ReachableSinks::One(sink),
                    _ => ReachableSinks::Multiple,
                };
            }
        }

        let condensation_graph = petgraph::algo::condensation(self.graph.clone(), true);
        let mut liveness = vec![LivenessLevel::L0; condensation_graph.node_count()];
        let mut visited = vec![false; condensation_graph.node_count()];
        // perform the depth-first search from each marked SCC
        for (marked_scc, _) in condensation_graph.node_references().filter(|(_, scc)| {
            scc.iter()
                .any(|node| matches!(node, &IdxNode::Place(p_idx) if self.marking[p_idx] > 0))
        }) {
            let mut reachable_sinks = ReachableSinks::NoneYet;
            visited.fill(false);
            dfs_from_markable_scc(
                &condensation_graph,
                marked_scc,
                &mut liveness,
                &mut reachable_sinks,
                &mut visited,
            );
            if let ReachableSinks::One(sink) = reachable_sinks
                && condensation_graph[sink].len() > 1
            {
                // any token from this source SCC is guaranteed to end up in
                // this non-trivial terminal SCC, making it L4-live
                liveness[sink.index()] = LivenessLevel::L4;
            }
        }

        // for each SCC, return each of its transitions
        // with the liveness level we determined for that SCC.
        condensation_graph
            .node_references()
            .flat_map(|(scc, nodes)| {
                let scc_liveness = &liveness[scc.index()];
                nodes.iter().filter_map(|node| {
                    if let &IdxNode::Transition(t_idx) = node {
                        let transition = self.mapping.transition(t_idx);
                        Some((transition, *scc_liveness))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// For a marked graph, we can determine liveness levels by analyzing which transitions
    /// are on unmarked circuits (L0), and which transitions can be reached from them via pathways
    /// which may contain some marked places (L1) or only unmarked places (L0).
    /// All other transitions are L4.
    fn liveness_via_marked_graph_unmarked_circuits(&self) -> HashMap<Transition, LivenessLevel> {
        /// Perform a depth-first search through the net from a given transition,
        /// recording L0-liveness if there has been no marked place on the path so far,
        /// and L1-liveness if there has been and the transition is not already marked L0.
        fn dfs_finite_pathways<N: AsRef<Net>>(
            pn: &PetriNet<N>,
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
            for &p_idx in &pn.dense_net.postset_t[t_idx] {
                let marked = marked || pn.marking[p_idx] > 0;
                let Some(&next_t_idx) = pn.dense_net.postset_p[p_idx].first() else {
                    continue;
                };
                dfs_finite_pathways(pn, next_t_idx, marked, liveness);
            }
        }

        // all transitions in an unmarked circuit are L0,
        // and all transitions downstream from them are at most L1. the rest are L4.
        let unmarked_circuits: Vec<IdxCircuit> = self.unmarked_circuits().collect();
        let mut dead_transition_indices = unmarked_circuits
            .iter()
            .flat_map(|circuit| circuit.transition_indices())
            .collect::<Vec<_>>();
        dead_transition_indices.sort_unstable();
        dead_transition_indices.dedup();
        let mut liveness = vec![LivenessLevel::L4; self.transition_count() as usize];
        for t_idx in dead_transition_indices {
            dfs_finite_pathways(self, t_idx, false, &mut liveness);
        }
        self.transitions().zip(liveness).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::net::builder::NetBuilder;
    use crate::net::class::NetClass;
    use crate::system::PetriNet;
    use crate::system::liveness::LivenessLevel;

    #[test]
    fn cycle_is_live() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_live());
    }

    #[test]
    fn mutex_is_live_and_bounded() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arcs((idle1, t_req1, wait1, t_enter1, crit1, t_exit1, idle1));
        b.add_arcs((idle2, t_req2, wait2, t_enter2, crit2, t_exit2, idle2));
        b.add_arcs((mutex, t_enter1, mutex));
        b.add_arcs((mutex, t_enter2, mutex));

        let net = b.build().expect("valid net");
        let sys = PetriNet::new(net, [(idle1, 1), (idle2, 1), (mutex, 1)]);
        assert!(sys.is_bounded());
        assert!(sys.is_live());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let (net, _p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([]);
        assert!(!sys.is_live());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, _p0, t0, _p1, t1) = crate::system::tests::two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = net.with_initial_marking([]);
        let liveness = sys.liveness();
        assert!(liveness.level(t0).is_dead());
        assert!(liveness.level(t1).is_dead());
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, p0, t0, _p1, t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        let liveness = sys.liveness();
        assert_eq!(liveness.level(t0), LivenessLevel::L4);
        assert_eq!(liveness.level(t1), LivenessLevel::L4);
    }

    /// SC S-net (circuit): marked → all L4.
    #[test]
    fn s_net_sc_marked_all_l4() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L4);
        assert_eq!(analysis.level(t1), LivenessLevel::L4);
    }

    /// SC S-net (circuit): unmarked → all L0.
    #[test]
    fn s_net_sc_unmarked_all_l0() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, []);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
    }

    #[test]
    fn one_of_each_liveness() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [l0, l1, l3, l4] = b.add_transitions();
        b.add_arcs((p0, l0, p1, l3, p1, l1, p2, l4, p2));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p1, 1)]);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(l0), LivenessLevel::L0);
        assert_eq!(analysis.level(l1), LivenessLevel::L1);
        assert_eq!(analysis.level(l3), LivenessLevel::L3);
        assert_eq!(analysis.level(l4), LivenessLevel::L4);
    }

    #[test]
    fn s_net_non_sc_mixed_levels() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let [p2, p3] = b.add_places();
        let [t3, t4] = b.add_transitions();
        b.add_arcs((p2, t3, p3, t4, p2));

        let switch = b.add_transition();
        b.add_arcs((p0, switch, p2));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::StateMachine);
        let sys = PetriNet::new(net, [(p0, 1)]);
        let analysis = sys.liveness();

        // SCC_A is non-sink and marked → internal transitions L3
        assert_eq!(analysis.level(t0), LivenessLevel::L3);
        assert_eq!(analysis.level(t1), LivenessLevel::L3);
        // Inter-SCC transition → L1
        assert_eq!(analysis.level(switch), LivenessLevel::L1);
        // SCC_B is sink and reachable (receives tokens from SCC_A) → L4
        assert_eq!(analysis.level(t3), LivenessLevel::L4);
        assert_eq!(analysis.level(t4), LivenessLevel::L4);
    }

    /// Non-SC S-net: unreachable sink SCC → L0.
    #[test]
    fn s_net_unreachable_sink_l0() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        // Chain: p0 → t0 → p1
        let t0 = b.add_transition();
        b.add_arcs((p0, t0, p1));
        // Disconnected cycle linked only via p1:
        // p1 → t1 → p2 → t2 → p3 → t3 → p1
        let [t1, t2, t3] = b.add_transitions();
        b.add_arcs((p1, t1, p2, t2, p3, t3, p1));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::StateMachine);

        let sys = PetriNet::new(net, []);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
        assert_eq!(analysis.level(t2), LivenessLevel::L0);
        assert_eq!(analysis.level(t3), LivenessLevel::L0);
    }

    /// SC T-net: all circuits marked → all L4.
    #[test]
    fn t_net_sc_all_circuits_marked_l4() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((t0, p0, t1, p1, t0));
        b.add_arcs((t0, p2, t1)); // second path
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, [(p0, 1), (p2, 1)]);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L4);
        assert_eq!(analysis.level(t1), LivenessLevel::L4);
    }

    /// SC T-net: unmarked circuit → transitions on it are L0.
    #[test]
    fn t_net_unmarked_circuit_l0() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((t0, p0, t1, p1, t0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::Circuit);

        let sys = PetriNet::new(net, []);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
    }

    /// Non-SC T-net with source transition: source always L4, downstream L4
    /// if all circuits are marked.
    #[test]
    fn t_net_source_transition_l4() {
        let mut b = NetBuilder::new();
        let [p_src, p0, p1] = b.add_places();
        let [t_src, t0, t1] = b.add_transitions();
        b.add_arcs((t_src, p_src, t0));
        b.add_arcs((t0, p0, t1, p1, t0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, [(p0, 1)]);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t_src), LivenessLevel::L4);
        assert_eq!(analysis.level(t0), LivenessLevel::L4);
        assert_eq!(analysis.level(t1), LivenessLevel::L4);
    }

    /// Non-SC T-net: predecessor SCC dead → downstream dead.
    #[test]
    fn t_net_dead_predecessor_propagates() {
        let mut b = NetBuilder::new();
        let [p0, p1, p_link, p2, p3] = b.add_places();
        let [t0, t1, t2, t3] = b.add_transitions();
        // SCC_A cycle: t0 → p0 → t1 → p1 → t0 (unmarked → dead)
        b.add_arcs((t0, p0, t1, p1, t0));
        // Link: t1 → p_link → t2
        b.add_arcs((t1, p_link, t2));
        // SCC_B cycle: t2 → p2 → t3 → p3 → t2 (marked, but predecessor dead)
        b.add_arcs((t2, p2, t3, p3, t2));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        // SCC_A unmarked, SCC_B marked but predecessor dead
        let sys = PetriNet::new(net, [(p2, 1)]);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
        assert_eq!(analysis.level(t2), LivenessLevel::L0);
        assert_eq!(analysis.level(t3), LivenessLevel::L1);
    }

    /// Free-choice net liveness dispatch (via CHC).
    ///
    /// Uses the net from Esparza's Lecture Notes, Figure 5.3:
    /// 8 places, 7 transitions. •t1 = •t2 = {s1, s2} (free choice).
    /// t7 synchronizes on {s7, s8}. Not S-net, not T-net.
    #[test]
    fn free_choice_chc_dispatch() {
        let mut b = NetBuilder::new();
        let [s1, s2, s3, s4, s5, s6, s7, s8] = b.add_places();
        let [t1, t2, t3, t4, t5, t6, t7] = b.add_transitions();
        // Choice: •t1 = •t2 = {s1, s2}
        b.add_arc((s1, t1));
        b.add_arc((s2, t1));
        b.add_arc((s1, t2));
        b.add_arc((s2, t2));
        // Fork from t1 and t2
        b.add_arc((t1, s3));
        b.add_arc((t1, s4));
        b.add_arc((t2, s5));
        b.add_arc((t2, s6));
        // Independent paths
        b.add_arc((s3, t3));
        b.add_arc((t3, s7));
        b.add_arc((s4, t4));
        b.add_arc((t4, s8));
        b.add_arc((s5, t5));
        b.add_arc((t5, s7));
        b.add_arc((s6, t6));
        b.add_arc((t6, s8));
        // Join: •t7 = {s7, s8}
        b.add_arc((s7, t7));
        b.add_arc((s8, t7));
        b.add_arc((t7, s1));
        b.add_arc((t7, s2));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::FreeChoice);

        let sys = PetriNet::new(
            net,
            [
                (s1, 1),
                (s2, 1),
                (s3, 0),
                (s4, 0),
                (s5, 0),
                (s6, 0),
                (s7, 0),
                (s8, 0),
            ],
        );
        let analysis = sys.liveness();
        assert_eq!(analysis.global_level(), LivenessLevel::L4);
    }
}
