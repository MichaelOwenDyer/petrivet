use crate::core::net::TransitionIdx;
use crate::core::system::liveness::LivenessCache;
use crate::net::class::NetClass;
use crate::net::{Net, Transition};
use crate::system::PetriNet;
use ahash::HashMap;
use bon::bon;

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
/// - [Murata 1989, Definition 5.1]
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

/// The procedure that produced a [`LivenessAnalysis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LivenessProvenance {
    /// No evidence.
    None,
    /// A circuit is live iff it carries a token.
    Circuit,
    /// A strongly connected state machine is live iff it carries a token.
    StronglyConnectedStateMachine,
    /// Analysis of the marked strongly connected components of a state machine.
    StateMachine,
    /// Analysis of the unmarked circuits of a marked graph.
    MarkedGraph,
    /// The Commoner/Hack criterion: every proper siphon contains a marked trap.
    CommonerHack,
    /// Liveness levels were computed from the reachability graph.
    ReachabilityGraph,
}

/// Result of liveness analysis.
/// When proved via Commoner's theorem (free-choice nets), all transitions are L4.
/// When proved via SCC analysis on the reachability graph, levels are
/// individually computed.
#[derive(Debug, Clone)]
pub struct LivenessAnalysis {
    levels: HashMap<Transition, LivenessLevel>,
    method: LivenessProvenance,
}

impl LivenessAnalysis {
    pub(crate) fn new(levels: HashMap<Transition, LivenessLevel>, method: LivenessProvenance) -> Self {
        Self { levels, method }
    }

    /// The procedure that produced this analysis.
    #[must_use]
    pub const fn method(&self) -> LivenessProvenance {
        self.method
    }

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

#[derive(Debug, Clone, Copy)]
pub enum LivenessEffort {
    /// Use only polynomial-time procedures.
    Efficient,
    /// Use procedures of any complexity.
    Any
}

#[derive(Debug, Clone, Copy)]
pub enum LivenessScope {
    Transition(TransitionIdx),
    PetriNet,
}

#[derive(Debug, Clone, Copy)]
pub enum LivenessQuery {
    AtLeast(LivenessLevel),
    Exact,
}

#[bon]
impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns true if there exists a proper siphon in the Petri net which is currently unmarked.
    pub fn contains_unmarked_proper_siphon(&self) -> bool {
        self.dense_net.maximal_unmarked_proper_siphon(&self.marking).is_some()
    }

    #[builder]
    pub fn is_live(
        &self,
        transition: Option<Transition>,
        level: Option<LivenessLevel>,
        effort: Option<LivenessEffort>,
        timeout: Option<std::time::Duration>,
    ) -> Option<bool> {
        let level = level.unwrap_or(LivenessLevel::L4);
        let query = LivenessQuery::AtLeast(level);
        let scope = match transition {
            None => LivenessScope::PetriNet,
            Some(t) if let Some(t_idx) = self.mapping.transition_idx(t) => LivenessScope::Transition(t_idx),
            Some(_) => return None,
        };
        let effort = effort.unwrap_or(LivenessEffort::Any);
        let deadline = timeout.and_then(|t| std::time::Instant::now().checked_add(t));
        self.with_liveness_cache(|cache| {
            self.refine_liveness(cache, query, scope, effort, deadline);
            cache.read_at_least(scope, level)
        })
    }

    #[builder]
    pub fn liveness(
        &self,
        of: Option<Transition>,
        effort: Option<LivenessEffort>,
        timeout: Option<std::time::Duration>,
    ) -> Option<LivenessLevel> {
        let query = LivenessQuery::Exact;
        let scope = match of {
            None => LivenessScope::PetriNet,
            Some(t) if let Some(t_idx) = self.mapping.transition_idx(t) => LivenessScope::Transition(t_idx),
            Some(_) => return None,
        };
        let effort = effort.unwrap_or(LivenessEffort::Any);
        let deadline = timeout.and_then(|t| std::time::Instant::now().checked_add(t));
        self.with_liveness_cache(|cache| {
            self.refine_liveness(cache, query, scope, effort, deadline);
            cache.read_exact(scope)
        })
    }

    fn refine_liveness(
        &self,
        cache: &mut LivenessCache,
        query: LivenessQuery,
        scope: LivenessScope,
        effort: LivenessEffort,
        deadline: Option<std::time::Instant>,
    ) {
        if cache.is_answered(scope, query) {
            return;
        }
        let is_deadline_expired = || deadline.is_some_and(|d| std::time::Instant::now() >= d);
        if cache.should_refine_by_unmarked_siphon {
            self.refine_liveness_by_unmarked_siphon(cache);
            if is_deadline_expired() || cache.is_answered(scope, query) {
                return;
            }
        }
        self.refine_liveness_by_efficient_subclass(cache);
        if is_deadline_expired() || cache.is_answered(scope, query) {
            return;
        }
        if matches!(effort, LivenessEffort::Efficient) {
            return;
        }
        self.refine_liveness_by_all_procedures(cache);
    }

    /// Sets transitions to L0 whose preset overlaps with the largest unmarked proper siphon.
    fn refine_liveness_by_unmarked_siphon(
        &self,
        cache: &mut LivenessCache,
    ) {
        if let Some(siphon) = self.dense_net.maximal_unmarked_proper_siphon(&self.marking) {
            let transitions = siphon.iter().flat_map(|p_idx| self.dense_net.postset_p[p_idx].iter());
            for &t_idx in transitions {
                cache.update_exact(t_idx, LivenessLevel::L0)
            }
        }
        cache.should_refine_by_unmarked_siphon = false;
    }

    /// Refines liveness by subclass where there is an efficient procedure.
    fn refine_liveness_by_efficient_subclass(
        &self,
        cache: &mut LivenessCache,
    ) {
        match self.class() {
            _ if self.class().is_state_machine()
                && self.is_strongly_connected() => {
                let level = if self.marking.sum() == 0 {
                    LivenessLevel::L0
                } else {
                    LivenessLevel::L4
                };
                cache.update_all(level);
            },
            NetClass::StateMachine => {
                let liveness = self.dense_net.liveness_via_state_machine_marked_sccs(&self.marking);
                for (t_idx, level) in liveness.into_iter().enumerate() {
                    cache.update_exact(t_idx, level);
                }
            },
            NetClass::MarkedGraph => {
                let liveness = self.dense_net.liveness_via_marked_graph_unmarked_circuits(&self.marking);
                for (t_idx, level) in liveness.into_iter().enumerate() {
                    cache.update_exact(t_idx, level);
                }
            },
            _ => {}
        }
    }

    fn refine_liveness_by_all_procedures(
        &self,
        cache: &mut LivenessCache,
    ) {
        match self.class() {
            NetClass::FreeChoice | NetClass::AsymmetricChoice if self.commoner_hack_criterion() == Some(Ok(())) => {
                for t_idx in self.dense_net.transition_indices() {
                    cache.update_exact(t_idx, LivenessLevel::L4);
                }
            },
            _ if let Ok(rg) = self.try_build_reachability_graph() => {
                for (transition, liveness) in rg.transition_liveness().levels {
                    if let Some(t_idx) = self.mapping.transition_idx(transition) {
                        cache.update_exact(t_idx, liveness);
                    }
                }
            },
            _ => {}
        }
    }

    /// If an efficient (polynomial-time) procedure for liveness
    /// is known for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_live(&self) -> Option<bool> {
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() > 0),
            NetClass::StateMachine => Some(self.is_strongly_connected() && self.marking.sum() > 0),
            _ if self.contains_unmarked_proper_siphon() => Some(false),
            NetClass::MarkedGraph => Some(!self.has_unmarked_circuit()),
            _ => None,
        }
    }

    pub fn efficient_liveness(&self) -> Option<LivenessAnalysis> {
        let all = |level| self.transitions().map(|t| (t, level)).collect::<HashMap<_, _>>();
        let (levels, method) = match self.class() {
            NetClass::Circuit if self.marking.sum() == 0 => {
                (all(LivenessLevel::L0), LivenessProvenance::Circuit)
            },
            NetClass::StateMachine if self.marking.sum() == 0 => {
                (all(LivenessLevel::L0), LivenessProvenance::StateMachine)
            },
            NetClass::Circuit => {
                (all(LivenessLevel::L4), LivenessProvenance::Circuit)
            },
            NetClass::StateMachine if self.is_strongly_connected() => {
                (all(LivenessLevel::L4), LivenessProvenance::StronglyConnectedStateMachine)
            }
            NetClass::StateMachine => {
                (self.liveness_via_state_machine_marked_sccs(), LivenessProvenance::StateMachine)
            },
            NetClass::MarkedGraph => {
                (self.liveness_via_marked_graph_unmarked_circuits(), LivenessProvenance::MarkedGraph)
            }
            _ => return None,
        };
        Some(LivenessAnalysis::new(levels, method))
    }
}

// specialized procedures for state machines and marked graphs
impl<N: AsRef<Net>> PetriNet<N> {
    /// For a non-strongly-connected state machine, we can determine individual transaction
    /// liveness levels by analyzing the marked strongly connected components of the net
    /// and which other components they can reach.
    ///
    /// Runs in linear time: one pass over the condensation graph in topological order finds
    /// the components reachable from a marked one, and one pass in reverse topological order
    /// finds, for every component, which terminal components are reachable from it.
    fn liveness_via_state_machine_marked_sccs(&self) -> HashMap<Transition, LivenessLevel> {
        self.transitions()
            .zip(self.dense_net.liveness_via_state_machine_marked_sccs(&self.marking))
            .collect()
    }

    /// Returns true if there exists a directed circuit in the net in which every place has zero tokens.
    pub fn has_unmarked_circuit(&self) -> bool {
        self.dense_net.transitions_on_unmarked_circuits(&self.marking).next().is_some()
    }

    /// For a marked graph, we can determine liveness levels by analyzing which transitions
    /// are on unmarked circuits (L0), and which transitions can be reached from them via pathways
    /// which may contain some marked places (L1) or only unmarked places (L0).
    /// All other transitions are L4.
    fn liveness_via_marked_graph_unmarked_circuits(&self) -> HashMap<Transition, LivenessLevel> {
        self.transitions()
            .zip(self.dense_net.liveness_via_marked_graph_unmarked_circuits(&self.marking))
            .collect()
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
        assert!(sys.is_live().call().unwrap());
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
        assert!(sys.is_bounded().call().unwrap());
        assert!(sys.is_live().call().unwrap());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let (net, _p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([]);
        assert!(!sys.is_live().call().unwrap());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, _p0, t0, _p1, t1) = crate::system::tests::two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = net.with_initial_marking([]);
        assert!(sys.liveness().of(t0).call().unwrap().is_dead());
        assert!(sys.liveness().of(t1).call().unwrap().is_dead());
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, p0, t0, _p1, t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L4);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L4);
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
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L4);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L4);
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
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L0);
    }

    #[test]
    fn one_of_each_liveness() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [l0, l1, l3, l4] = b.add_transitions();
        b.add_arcs((p0, l0, p1, l3, p1, l1, p2, l4, p2));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p1, 1)]);
        assert_eq!(sys.liveness().of(l0).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(l1).call().unwrap(), LivenessLevel::L1);
        assert_eq!(sys.liveness().of(l3).call().unwrap(), LivenessLevel::L3);
        assert_eq!(sys.liveness().of(l4).call().unwrap(), LivenessLevel::L4);
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

        // SCC_A is non-sink and marked → internal transitions L3
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L3);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L3);
        // Inter-SCC transition → L1
        assert_eq!(sys.liveness().of(switch).call().unwrap(), LivenessLevel::L1);
        // SCC_B is sink and reachable (receives tokens from SCC_A) → L4
        assert_eq!(sys.liveness().of(t3).call().unwrap(), LivenessLevel::L4);
        assert_eq!(sys.liveness().of(t4).call().unwrap(), LivenessLevel::L4);
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
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t2).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t3).call().unwrap(), LivenessLevel::L0);
    }

    /// Builds a marked cycle `a` that can move its token into either of two sink cycles `b` and `c`.
    fn cycle_with_two_sink_cycles() -> (
        crate::net::Net,
        [crate::net::Place; 3],
        [crate::net::Transition; 4],
        [crate::net::Transition; 4],
    ) {
        let mut b = NetBuilder::new();
        let [a0, a1] = b.add_places();
        let [ta0, ta1] = b.add_transitions();
        b.add_arcs((a0, ta0, a1, ta1, a0));
        let [b0, b1] = b.add_places();
        let [tb0, tb1] = b.add_transitions();
        b.add_arcs((b0, tb0, b1, tb1, b0));
        let [c0, c1] = b.add_places();
        let [tc0, tc1] = b.add_transitions();
        b.add_arcs((c0, tc0, c1, tc1, c0));
        let [to_b, to_c] = b.add_transitions();
        b.add_arcs((a0, to_b, b0));
        b.add_arcs((a0, to_c, c0));
        (
            b.build().unwrap(),
            [a0, b0, c0],
            [ta0, ta1, to_b, to_c],
            [tb0, tb1, tc0, tc1],
        )
    }

    /// Non-SC S-net: a token that may end up in either of two sink SCCs is not
    /// guaranteed to arrive in any of them → everything on a cycle is L3, nothing is L4.
    #[test]
    fn s_net_two_reachable_sinks_l3() {
        let (net, [a0, ..], [ta0, ta1, to_b, to_c], [tb0, tb1, tc0, tc1]) = cycle_with_two_sink_cycles();
        assert_eq!(net.class(), NetClass::StateMachine);
        let sys = PetriNet::new(net, [(a0, 1)]);

        for t in [ta0, ta1, tb0, tb1, tc0, tc1] {
            assert_eq!(sys.liveness().of(t).call().unwrap(), LivenessLevel::L3);
        }
        for t in [to_b, to_c] {
            assert_eq!(sys.liveness().of(t).call().unwrap(), LivenessLevel::L1);
        }
    }

    /// Non-SC S-net: one marked source that is guaranteed to arrive in a sink SCC is enough
    /// to make it L4, even if another marked source reaches several sinks.
    #[test]
    fn s_net_second_marked_source_upgrades_sink() {
        let (net, [a0, b0, _], [ta0, ta1, to_b, to_c], [tb0, tb1, tc0, tc1]) = cycle_with_two_sink_cycles();
        let sys = PetriNet::new(net, [(a0, 1), (b0, 1)]);

        // the token that starts inside the sink cycle `b` can never leave it
        for t in [tb0, tb1] {
            assert_eq!(sys.liveness().of(t).call().unwrap(), LivenessLevel::L4);
        }
        // the token on `a` may still end up in either sink
        for t in [ta0, ta1, tc0, tc1] {
            assert_eq!(sys.liveness().of(t).call().unwrap(), LivenessLevel::L3);
        }
        for t in [to_b, to_c] {
            assert_eq!(sys.liveness().of(t).call().unwrap(), LivenessLevel::L1);
        }
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
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L4);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L4);
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
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L0);
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
        assert_eq!(sys.liveness().of(t_src).call().unwrap(), LivenessLevel::L4);
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L4);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L4);
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
        assert_eq!(sys.liveness().of(t0).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t1).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t2).call().unwrap(), LivenessLevel::L0);
        assert_eq!(sys.liveness().of(t3).call().unwrap(), LivenessLevel::L1);
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
        assert_eq!(sys.liveness().call().unwrap(), LivenessLevel::L4);
    }
}
