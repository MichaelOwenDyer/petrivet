use crate::class::NetClass;
use crate::core::net::{IdxNode, TransitionIdx, path::IdxCircuit};
use crate::liveness::LivenessLevel;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;
use ahash::HashMap;
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNodeReferences;
use petgraph::Graph;

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
    ///
    /// If the transition does not
    #[must_use]
    pub fn level(&self, transition: Transition) -> LivenessLevel {
        self.levels
            .get(&transition)
            .copied()
            .unwrap_or(LivenessLevel::L0)
    }

    /// Returns true if all transitions in the Petri net are live.
    ///
    /// This is a convenience method which
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
        match self.class() {
            // `wide_sum() > 0`, not `sum() > 0`: a `u32` wrap that lands on exactly
            // 0 (a marked total that is a multiple of 2^32) would mint a false
            // `Some(false)` — a live net reported dead, a fabricated negative.
            NetClass::Circuit => Some(self.marking.wide_sum() > 0),
            NetClass::StateMachine => {
                Some(self.is_strongly_connected() && self.marking.wide_sum() > 0)
            }
            NetClass::MarkedGraph => Some(!self.has_unmarked_circuit()),
            NetClass::FreeChoice => Some(self.commoner_hack_criterion().is_ok()),
            _ => None,
        }
    }

    /// Returns true if the Petri net is [`live`](LivenessLevel::L4).
    ///
    /// When liveness cannot be decided (an unbounded net, where the reachability
    /// graph does not terminate), this returns `false` — a *convenience*
    /// collapse of the honest three-valued answer. Callers who must distinguish
    /// "provably non-live" from "could not decide" should use
    /// [`try_liveness`](Self::try_liveness), which abstains with `None` rather
    /// than fabricating a verdict. (A5: the type-distinct inconclusive case is
    /// completed by the M1 `Verdict` type; this method preserves the boolean
    /// contract while no longer resting on a fabricated all-L0 analysis.)
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.is_efficiently_live().unwrap_or_else(|| {
            self.try_liveness().is_some_and(|analysis| analysis.is_live())
        })
    }

    pub fn efficient_liveness(&self) -> Option<LivenessAnalysis> {
        let levels = match self.class() {
            // `wide_sum() == 0`, not `sum() == 0`: a `u32` wrap to exactly 0 on a
            // marked net would falsely declare every transition dead (all-L0).
            NetClass::Circuit | NetClass::StateMachine if self.marking.wide_sum() == 0 => {
                Some(self.transitions().map(|t| (t, LivenessLevel::L0)).collect())
            }
            NetClass::Circuit => {
                Some(self.transitions().map(|t| (t, LivenessLevel::L4)).collect())
            },
            NetClass::StateMachine => {
                if self.is_strongly_connected() {
                    Some(self.transitions().map(|t| (t, LivenessLevel::L4)).collect())
                } else {
                    Some(self.liveness_via_state_machine_marked_sccs())
                }
            },
            NetClass::MarkedGraph => {
                Some(self.liveness_via_marked_graph_unmarked_circuits())
            },
            NetClass::FreeChoice => {
                self.commoner_hack_criterion().ok().map(|_| {
                    self.transitions().map(|t| (t, LivenessLevel::L4)).collect()
                })
            },
            _ => None,
        };
        levels.map(|levels| LivenessAnalysis { levels })
    }

    /// Analyses liveness, dispatching to the cheapest known procedure first,
    /// abstaining (`None`) when liveness cannot be decided.
    ///
    /// `None` means "could not decide" — distinct from a per-transition `L0`
    /// ("provably dead"). This is the honest three-valued answer the A5 defect
    /// removed: the unbounded-net fallback no longer fabricates an all-L0
    /// analysis (which made "unknown" indistinguishable from "dead"). An
    /// unbounded net, where the reachability graph does not terminate, returns
    /// `None`. The full type-distinct verdict (`Verdict<LivenessAnalysis, …>`)
    /// is the M1 `model` contract (A1); this method supplies the abstaining
    /// boundary M0 requires. Rationale: foundational-design §F3 (the L0 repair),
    /// BACKLOG A5.
    #[must_use]
    pub fn try_liveness(&self) -> Option<LivenessAnalysis> {
        self.efficient_liveness()
            .map_or_else(|| self.liveness_via_reachability_graph(), Some)
    }

    /// Analyses liveness, dispatching to the cheapest known procedure first.
    ///
    /// # Panics
    ///
    /// Panics if liveness cannot be decided (an unbounded net). Prefer
    /// [`try_liveness`](Self::try_liveness), which abstains honestly with `None`
    /// instead of fabricating a verdict. This method is retained for callers
    /// that have already established decidability (e.g. a bounded net).
    #[must_use]
    pub fn liveness(&self) -> LivenessAnalysis {
        self.try_liveness()
            .expect("liveness() called on a net whose liveness cannot be decided; use try_liveness()")
    }

    /// SCC analysis on the full reachability graph. Only called as a fallback
    /// from [`try_liveness`](Self::try_liveness) for net classes without an
    /// efficient procedure.
    ///
    /// Returns `None` when the reachability graph cannot be built (the net is
    /// unbounded): liveness is then undecided by this engine. A5 — this arm
    /// previously fabricated an all-`L0` analysis, conflating "unknown" with
    /// "provably dead"; it now abstains instead.
    #[must_use]
    fn liveness_via_reachability_graph(&self) -> Option<LivenessAnalysis> {
        self.try_build_reachability_graph()
            .ok()
            .map(|rg| rg.transition_liveness())
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
            if neighbors.peek().is_none() {
                *reachable_sinks = match *reachable_sinks {
                    ReachableSinks::NoneYet => ReachableSinks::One(scc),
                    ReachableSinks::One(sink) if sink == scc => ReachableSinks::One(sink),
                    _ => ReachableSinks::Multiple,
                };
            } else {
                neighbors.for_each(|neighbor| {
                    dfs_from_markable_scc(condensation_graph, neighbor, liveness, reachable_sinks, visited);
                });
            }
        }

        let condensation_graph = petgraph::algo::condensation(self.graph.clone(), true);
        let mut liveness = vec![LivenessLevel::L0; condensation_graph.node_count()];
        let mut visited = vec![false; condensation_graph.node_count()];
        // perform the depth-first search from each marked SCC
        for (marked_scc, _) in condensation_graph
            .node_references()
            .filter(|(_, scc)| {
                scc.iter().any(|node| {
                    matches!(node, &IdxNode::Place(p_idx) if self.marking[p_idx] > 0)
                })
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
                && condensation_graph[sink].len() > 1 {
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
                    liveness[t_idx] = if marked { LivenessLevel::L1 } else { LivenessLevel::L0 };
                },
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
        let mut dead_transition_indices = unmarked_circuits.iter()
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
    use crate::builder::NetBuilder;
    use crate::class::NetClass;
    use crate::liveness::LivenessLevel;
    use crate::prelude::PetriNet;

    #[test]
    fn cycle_is_live() {
        let (net, p0, _t0, _p1, _t1) = crate::api::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_live());
    }

    /// Verdict-path overflow (the fabricated-negative direction): a Circuit is
    /// live iff it is marked (`total tokens > 0`). Testing that with a `u32` sum
    /// *wraps* in release, and a marked total that is a multiple of `2^32` wraps
    /// to exactly 0 — falsely reporting the live net as dead (`Some(false)`). The
    /// widened `u64` sum closes it.
    ///
    /// Construction: total tokens `= 2^32 = 4_294_967_296` (`4e9 + 294_967_296`),
    /// which `u32` sums to 0. The net is plainly marked, hence live.
    #[test]
    fn marked_total_at_two_to_the_32_is_still_live() {
        let (net, q0, _u0, q1, _u1) = crate::api::system::tests::two_place_cycle();
        assert_eq!(net.class(), NetClass::Circuit);
        // total = 2^32: u32 sum wraps to 0; the net is plainly marked, hence live.
        let sys = net.with_initial_marking([(q0, 4_000_000_000_u32), (q1, 294_967_296_u32)]);
        assert_eq!(
            sys.is_efficiently_live(),
            Some(true),
            "a marked circuit whose total is a multiple of 2^32 must not be \
             reported dead by a u32 sum wrapping to 0"
        );
        assert!(sys.is_live());
    }

    /// A5 regression: a net whose liveness cannot be decided (an unbounded net,
    /// where the reachability graph does not terminate) must ABSTAIN, not
    /// fabricate an all-`L0` ("provably dead") analysis. Before A5,
    /// `liveness_via_reachability_graph` assigned every transition `L0` when the
    /// reachability graph could not be built, making "unknown" indistinguishable
    /// from "dead" (and, for a transition that fires infinitely often, an
    /// outright wrong negative). The honest answer is `try_liveness() == None`.
    ///
    /// Net: a *general* (non-free-choice) unbounded net so that liveness has no
    /// efficient structural shortcut and must fall to the reachability-graph
    /// fallback, which cannot terminate. `p0` feeds both `t0` and `t1` with
    /// differing presets (so it is not free-choice → General); `t0` returns a
    /// token to `p0` and also pumps `p1` without bound. Built with explicit
    /// `add_arc`.
    #[test]
    fn a5_unbounded_net_liveness_abstains_not_l0() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        // t0: {p0} -> {p0, p1}  (pumps p1 → unbounded)
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        // t1: {p0, p2} -> {p2}  (shares p0 with t0 but with a different preset
        // → not free-choice → General class, so no efficient liveness path)
        b.add_arc((p0, t1));
        b.add_arc((p2, t1));
        b.add_arc((t1, p2));
        let net = b.build().expect("valid net");
        // Asymmetric-choice (p0 feeds t0 and t1 with differing presets): no
        // efficient liveness procedure, so dispatch falls to the RG fallback.
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        let sys = PetriNet::new(net, [(p0, 1), (p2, 1)]);

        // No efficient liveness procedure for this class.
        assert!(sys.is_efficiently_live().is_none(), "no efficient liveness path");
        // This net is genuinely unbounded: the reachability graph does not
        // terminate, so liveness is undecided by this engine.
        assert!(sys.try_build_reachability_graph().is_err(), "net must be unbounded");

        // The honest answer is abstention, NOT a fabricated all-L0 analysis.
        assert!(
            sys.try_liveness().is_none(),
            "A5: undecidable liveness must abstain (None), never fabricate L0"
        );
    }

    /// A2 sentinel (the soundness north star). A live marked graph — strongly
    /// connected with every circuit marked — must never be reported non-live.
    /// The marked-graph efficient-liveness arm (`is_efficiently_live`,
    /// `Some(!self.has_unmarked_circuit())`) is a real computation, not the old
    /// `Some(false)` stub; this sentinel guards against any regression to a
    /// fabricated non-live verdict on the efficient path. Built with explicit
    /// `add_arc` calls (not the `add_arcs` tuple chain).
    #[test]
    fn a2_live_marked_graph_not_reported_non_live() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        // Two parallel paths between t0 and t1 forming a marked graph with two
        // circuits: t0 -> p0 -> t1 -> p1 -> t0 and t0 -> p2 -> t1.
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        b.add_arc((t0, p2)); b.add_arc((p2, t1));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::MarkedGraph);
        // Mark both circuits so the net is live.
        let sys = PetriNet::new(net, [(p1, 1), (p2, 1)]);
        // The efficient path must report live (Some(true)), never a fabricated
        // Some(false).
        assert_eq!(
            sys.is_efficiently_live(),
            Some(true),
            "A2: a live marked graph must not be reported non-live on the efficient path"
        );
        assert!(sys.is_live(), "the marked graph with every circuit marked is live");
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
        let (net, _p0, _t0, _p1, _t1) = crate::api::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([]);
        assert!(!sys.is_live());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, _p0, t0, _p1, t1) = crate::api::system::tests::two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = net.with_initial_marking([]);
        let liveness = sys.liveness();
        assert!(liveness.level(t0).is_dead());
        assert!(liveness.level(t1).is_dead());
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, p0, t0, _p1, t1) = crate::api::system::tests::two_place_cycle();
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
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
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
    fn s_net_non_sc_mixed_levels() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let [t2, t3, t4] = b.add_transitions();
        b.add_arcs((p0, t2, p2, t3, p3, t4, p2));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::StateMachine);
        let sys = PetriNet::new(net, [(p0, 1)]);
        let analysis = sys.liveness();

        // SCC_A is non-sink and marked → internal transitions L3
        assert_eq!(analysis.level(t0), LivenessLevel::L3);
        assert_eq!(analysis.level(t1), LivenessLevel::L3);
        // Inter-SCC transition → L1
        assert_eq!(analysis.level(t2), LivenessLevel::L1);
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
    /// when all circuits are marked.
    ///
    /// The circuit `t0 -> p0 -> t1 -> p1 -> t0` is marked (a token on `p0`), so by
    /// the marked-graph liveness theorem (a marked graph is live iff every directed
    /// circuit holds a token) every transition on it is live; the source `t_src`
    /// (empty preset) is always enabled, hence L4. The original fixture left the
    /// circuit *unmarked* yet asserted L4 for `t0`/`t1`, which contradicts both the
    /// theorem and the test's own "if all circuits are marked" precondition; it had
    /// been masked by the scrambled-`Net::graph` bug fixed in `NetBuilder::build`
    /// (see `builder::tests::petgraph_mirror_agrees_with_dense_adjacency`) which
    /// corrupted `circuits()` and so the marked-graph liveness pass. M0 repair:
    /// foundational-design §F3.
    #[test]
    fn t_net_source_transition_l4() {
        let mut b = NetBuilder::new();
        let [p_src, p0, p1] = b.add_places();
        let [t_src, t0, t1] = b.add_transitions();
        b.add_arcs((t_src, p_src, t0));
        b.add_arcs((t0, p0, t1, p1, t0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        // A token on the circuit (p0) marks the single circuit, so all of t0/t1 are
        // live; t_src is a source and always L4.
        let sys = PetriNet::new(net, [(p0, 1)]);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t_src), LivenessLevel::L4);
        assert_eq!(analysis.level(t0), LivenessLevel::L4);
        assert_eq!(analysis.level(t1), LivenessLevel::L4);
    }

    /// Companion to `t_net_source_transition_l4`: the *unmarked*-circuit case the
    /// original (buggy) fixture conflated with the marked case. With the circuit
    /// `t0 -> p0 -> t1 -> p1 -> t0` carrying no token, neither circuit transition
    /// can ever fire (the source `t_src` only feeds `p_src`, off the circuit), so
    /// `t0`/`t1` are dead (L0) while the source `t_src` stays L4. This pins the
    /// honest marked-graph semantics that the graph fix restored.
    #[test]
    fn t_net_source_transition_unmarked_circuit_dead() {
        let mut b = NetBuilder::new();
        let [p_src, p0, p1] = b.add_places();
        let [t_src, t0, t1] = b.add_transitions();
        b.add_arcs((t_src, p_src, t0));
        b.add_arcs((t0, p0, t1, p1, t0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, []);
        let analysis = sys.liveness();
        assert_eq!(analysis.level(t_src), LivenessLevel::L4);
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
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
        b.add_arc((s1, t1)); b.add_arc((s2, t1));
        b.add_arc((s1, t2)); b.add_arc((s2, t2));
        // Fork from t1 and t2
        b.add_arc((t1, s3)); b.add_arc((t1, s4));
        b.add_arc((t2, s5)); b.add_arc((t2, s6));
        // Independent paths
        b.add_arc((s3, t3)); b.add_arc((t3, s7));
        b.add_arc((s4, t4)); b.add_arc((t4, s8));
        b.add_arc((s5, t5)); b.add_arc((t5, s7));
        b.add_arc((s6, t6)); b.add_arc((t6, s8));
        // Join: •t7 = {s7, s8}
        b.add_arc((s7, t7)); b.add_arc((s8, t7));
        b.add_arc((t7, s1)); b.add_arc((t7, s2));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::FreeChoice);

        let sys = PetriNet::new(net, [
            (s1, 1),
            (s2, 1),
            (s3, 0),
            (s4, 0),
            (s5, 0),
            (s6, 0),
            (s7, 0),
            (s8, 0)
        ]);
        let analysis = sys.liveness();
        assert_eq!(analysis.global_level(), LivenessLevel::L4);
    }
}