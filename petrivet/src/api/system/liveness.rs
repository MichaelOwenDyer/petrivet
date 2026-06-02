use crate::liveness::LivenessLevel;
use crate::net::{Net, Place, Transition};
use crate::prelude::PetriNet;
use crate::system::chc::CommonerHackCriterion;

/// Result of liveness analysis.
/// When proved via Commoner's theorem (free-choice nets), all transitions are L4.
/// When proved via SCC analysis on the reachability graph, levels are
/// individually computed.
#[derive(Debug, Clone)]
pub struct LivenessAnalysis {
    /// All transitions in the net paired with their liveness levels.
    pub levels: Box<[(Transition, LivenessLevel)]>,
    /// How the result was obtained.
    pub method: LivenessMethod,
}

impl LivenessAnalysis {
    /// Returns the liveness level of the Petri net as a whole,
    /// defined as the minimum liveness level among all transitions.
    #[must_use]
    pub fn global_level(&self) -> LivenessLevel {
        self.levels
            .iter()
            .map(|(_, level)| *level)
            .min()
            .expect("at least one liveness level")
    }

    /// Returns the liveness level of the provided `transition`.
    ///
    /// If the transition does not
    #[must_use]
    pub fn level(&self, transition: Transition) -> LivenessLevel {
        self.levels.iter()
            .find(|(t, _)| *t == transition)
            .map_or(LivenessLevel::L0, |(_, level)| *level)
    }

    /// Returns true if all transitions in the Petri net are live.
    ///
    /// This is a convenience method which
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.levels.iter().all(|(_, level)| level.is_l4_live())
    }

    /// Whether every transition appears on some edge of the reachability graph,
    /// i.e. no transition in the Petri net is dead.
    #[must_use]
    pub fn is_quasi_live(&self) -> bool {
        self.levels.iter().all(|(_, level)| level.is_l1_live())
    }
}

/// Evidence for liveness analysis of an S-net.
///
/// In an S-net, each transition has exactly one input and one output place.
/// The "place graph" (places as nodes, transitions as directed edges) determines
/// liveness levels via its SCC decomposition:
///
/// - **Sink SCC, marked**: transitions on internal cycles are **L4** (tokens
///   can never leave; can always be routed to fire any internal transition).
/// - **Non-sink SCC, marked**: internal transitions are **L3** (tokens *can*
///   stay cycling forever, but *can also* escape via outgoing transitions,
///   so not L4). See Primer §5.6 Case 2: CHC fails for non-final SCCs.
/// - **Inter-SCC transitions**: at most **L1** (each token passes through at
///   most once; total tokens conserved in S-nets).
/// - **Unreachable**: **L0**.
///
/// References:
/// - [Murata 1989, Theorem 4](crate::literature#theorem-4--liveness-of-s-nets-state-machines) (SC liveness)
/// - [Murata 1989, Theorem 5](crate::literature#theorem-5--safety-of-s-nets-state-machines) (safety via token count)
/// - [Primer, Corollary 5.30](crate::literature#corollary-530--liveness-of-s-systems)
/// - [Primer, Proposition 5.39](crate::literature#proposition-539--boundedness-criterion-for-live-s-systems) (per-place bounds)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SNetLivenessEvidence {
    /// The SCCs of the place graph, in topological order (sources first).
    pub components: Box<[SNetComponent]>,
}

/// A strongly connected component in the place graph of an S-net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SNetComponent {
    /// Places in this SCC.
    pub places: Box<[Place]>,
    /// Transitions internal to this SCC (both endpoints in the same SCC).
    pub transitions: Box<[Transition]>,
    /// Total token count on places in this SCC under M₀.
    pub token_sum: u32,
    /// Whether this SCC has no outgoing transitions to other SCCs.
    pub is_sink: bool,
}

/// Evidence for liveness analysis of a T-net (marked graph).
///
/// In a T-net, each place has exactly one input and one output transition.
/// A fundamental invariant: **the token count on every directed circuit is
/// constant under all firings** (each transition on the circuit removes one
/// token from its input place and adds one to its output place; external
/// transitions cannot touch circuit places).
///
/// Consequence: every transition in a T-net is either **L0** or **L4** — no
/// intermediate liveness levels are possible.
///
/// A transition t is L4 iff every directed circuit containing t is marked
/// AND all predecessor transitions (in the SCC DAG of the transition graph)
/// are L4.
///
/// References:
/// - [Murata 1989, Theorem 7](crate::literature#theorem-7--liveness-of-t-nets-marked-graphs) (SC liveness)
/// - [Murata 1989, Theorem 8](crate::literature#theorem-8--place-bounds-in-t-nets-marked-graphs) (exact place bounds via circuit token counts)
/// - [Murata 1989, Theorem 9](crate::literature#theorem-9--safety-of-t-nets-marked-graphs) (safety iff every circuit carries 1 token)
/// - [Murata 1989, Theorem 26](crate::literature#theorem-26--circuit-token-invariance-in-t-nets) (circuit token invariance)
/// - [Primer, Theorem 5.31](crate::literature#theorem-531--liveness-and-realisability-in-t-systems)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TNetLivenessEvidence {
    /// The SCCs of the transition graph, in topological order (sources first).
    /// Each SCC is live (all transitions L4) iff all internal circuits are
    /// marked AND all predecessor SCCs are live.
    pub components: Box<[TNetComponent]>,
}

/// A strongly connected component in the transition graph of a T-net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TNetComponent {
    /// Transitions in this SCC.
    pub transitions: Box<[Transition]>,
    /// Places internal to this SCC (both endpoint transitions in the same SCC).
    pub places: Box<[Place]>,
    /// Whether all directed circuits within this SCC carry at least one token
    /// under M₀. (Vacuously true for acyclic/singleton SCCs.)
    pub all_circuits_marked: bool,
    /// Whether all predecessor SCCs in the DAG are live.
    /// Combined with `all_circuits_marked`, determines if transitions here are L4.
    pub predecessors_live: bool,
}

/// Evidence for a liveness result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LivenessMethod {
    /// S-net SCC analysis.
    ///
    /// Per-transition levels derived from the SCC decomposition of the place
    /// graph and the token distribution across components.
    ///
    /// References: [Murata 1989 Theorem 4](crate::literature#theorem-4--liveness-of-s-nets-state-machines), [Primer Corollary 5.30](crate::literature#corollary-530--liveness-of-s-systems).
    SNet(SNetLivenessEvidence),
    /// T-net circuit analysis.
    ///
    /// Per-transition levels derived from the SCC decomposition of the
    /// transition graph. Every transition is either L0 or L4 due to the
    /// circuit token invariance property.
    ///
    /// References: [Murata 1989 Theorems 7 & 26](crate::literature#theorem-7--liveness-of-t-nets-marked-graphs), [Primer Theorem 5.31](crate::literature#theorem-531--liveness-and-realisability-in-t-systems).
    TNet(TNetLivenessEvidence),
    /// (Full) Liveness was determined via the Commoner/Hack criterion, which is necessary and sufficient for
    /// liveness in free-choice nets: every siphon contains a trap marked at the initial marking.
    ///
    /// Reference: [Primer Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc), [Murata 1989 Theorem 12](crate::literature#theorem-12--commonerhack-criterion).
    FreeChoice(CommonerHackCriterion),
    /// Strongly-connected component analysis on the full reachability graph (bounded net).
    ReachabilityGraph,
    /// Current algorithms could not decide (unbounded general net).
    Inconclusive,
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether the system is live (L4): every transition can fire from
    /// every reachable marking (possibly after further firings).
    ///
    /// Delegates to [`analyze_liveness`](Self::analyze_liveness).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.analyze_liveness().global_level().is_live()
    }

    /// Analyzes liveness and returns per-transition levels with evidence.
    ///
    /// Strategy (ascending cost):
    /// 1. **S-nets**: SCC decomposition of the place graph. Polynomial.
    ///    Sink SCCs → L4, non-sink SCCs → L3, inter-SCC → L1.
    /// 2. **T-nets**: SCC decomposition of the transition graph. Polynomial.
    ///    Every transition is L0 or L4 (circuit token invariance).
    /// 3. **Free-choice nets**: Commoner's theorem (structural).
    ///    If the criterion holds, all transitions are L4.
    /// 4. **General**: CG → RG → SCC analysis (exponential worst-case).
    #[must_use]
    pub fn analyze_liveness(&self) -> LivenessAnalysis {
        // TODO: Optimize for state machines and marked graphs
        //  by analyzing SCCs of the appropriate graph

        if self.class().is_free_choice()
            && let Ok(chc) = self.commoner_hack_criterion() {
            return LivenessAnalysis {
                levels: self.transitions().zip(std::iter::repeat(LivenessLevel::L4)).collect(),
                method: LivenessMethod::FreeChoice(chc),
            };
        }

        match self.try_build_reachability_graph() {
            Ok(rg) => rg.transition_liveness(),
            Err(_cg) => {
                // TODO: liveness for unbounded nets
                LivenessAnalysis {
                    levels: self.transitions().zip(std::iter::repeat(LivenessLevel::L0)).collect(),
                    method: LivenessMethod::Inconclusive,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::class::NetClass;
    use crate::liveness::LivenessLevel;
    use crate::prelude::PetriNet;
    use crate::system::liveness::LivenessMethod;

    #[test]
    fn cycle_is_live() {
        let (net, p0, _t0, _p1, _t1) = crate::api::system::tests::two_place_cycle();
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

        b.add_arcs((idle1, t_req1, wait1));
        b.add_arcs((wait1, t_enter1, crit1));
        b.add_arcs((crit1, t_exit1, idle1));
        b.add_arcs((idle2, t_req2, wait2));
        b.add_arcs((wait2, t_enter2, crit2));
        b.add_arcs((crit2, t_exit2, idle2));
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
        let liveness = sys.analyze_liveness();
        assert!(liveness.level(t0).is_dead());
        assert!(liveness.level(t1).is_dead());
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, p0, t0, _p1, t1) = crate::api::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        let liveness = sys.analyze_liveness();
        assert_eq!(liveness.level(t0), LivenessLevel::L1);
        assert_eq!(liveness.level(t1), LivenessLevel::L1);
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
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L4);
        assert_eq!(analysis.level(t1), LivenessLevel::L4);
        assert!(matches!(analysis.method, LivenessMethod::SNet(_)));
    }

    /// SC S-net (circuit): unmarked → all L0.
    #[test]
    fn s_net_sc_unmarked_all_l0() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 0), (p1, 0)]);
        let analysis = sys.analyze_liveness();
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
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0), (p2, 0), (p3, 0)]);
        let analysis = sys.analyze_liveness();

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
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        // Disconnected cycle linked only via p1:
        // p1 → t1 → p2 → t2 → p3 → t3 → p1
        let [t1, t2, t3] = b.add_transitions();
        b.add_arc((p1, t1)); b.add_arc((t1, p2));
        b.add_arc((p2, t2)); b.add_arc((t2, p3));
        b.add_arc((p3, t3)); b.add_arc((t3, p1));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::StateMachine);

        let sys = PetriNet::new(net, [(p0, 0), (p1, 0), (p2, 0), (p3, 0)]);
        let analysis = sys.analyze_liveness();
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
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        b.add_arc((t0, p2)); b.add_arc((p2, t1)); // second path
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, [(p0, 1), (p1, 1), (p2, 1)]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L4);
        assert_eq!(analysis.level(t1), LivenessLevel::L4);
        assert!(matches!(analysis.method, LivenessMethod::TNet(_)));
    }

    /// SC T-net: unmarked circuit → transitions on it are L0.
    #[test]
    fn t_net_unmarked_circuit_l0() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, [(p0, 0), (p1, 0)]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
    }

    /// Non-SC T-net with source transition: source always L4, downstream L4
    /// if all circuits are marked.
    #[test]
    fn t_net_source_transition_l4() {
        // t0 (source, no input places) → p0 → t1 → p1 → t0 forms a cycle
        // But t0 also has p1 as input, making it a cycle.
        // Let's make a true source: t_src → p_src → t0, where t0 → p0 → t1 → p1 → t0
        let mut b = NetBuilder::new();
        let [p_src, p0, p1] = b.add_places();
        let [t_src, t0, t1] = b.add_transitions();
        // Source: t_src → p_src → t0 (t_src has no input places)
        b.add_arc((t_src, p_src)); b.add_arc((p_src, t0));
        // Cycle: t0 → p0 → t1 → p1 → t0
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        // Cycle {p0, p1} has 1 token → marked
        let sys = PetriNet::new(net, [(p_src, 0), (p0, 1), (p1, 0)]);
        let analysis = sys.analyze_liveness();
        // t_src is always enabled (no inputs) → L4
        assert_eq!(analysis.level(t_src), LivenessLevel::L4);
        // t0 depends on p_src (from L4 t_src) and p1 (from marked cycle) → L4
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
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        // Link: t1 → p_link → t2
        b.add_arc((t1, p_link)); b.add_arc((p_link, t2));
        // SCC_B cycle: t2 → p2 → t3 → p3 → t2 (marked, but predecessor dead)
        b.add_arc((t2, p2)); b.add_arc((p2, t3));
        b.add_arc((t3, p3)); b.add_arc((p3, t2));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        // SCC_A unmarked, SCC_B marked but predecessor dead
        let sys = PetriNet::new(net, [(p0, 0), (p1, 0), (p_link, 0), (p2, 1), (p3, 0)]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.level(t0), LivenessLevel::L0);
        assert_eq!(analysis.level(t1), LivenessLevel::L0);
        assert_eq!(analysis.level(t2), LivenessLevel::L0);
        assert_eq!(analysis.level(t3), LivenessLevel::L0);
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
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.global_level(), LivenessLevel::L4);
        assert!(matches!(analysis.method, LivenessMethod::FreeChoice(_)));
    }
}