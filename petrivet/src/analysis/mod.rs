//! Structural and behavioral analysis of Petri nets.
//!
//! This module provides low-level analysis primitives:
//!
//! - [`structural`]: S-invariants, T-invariants, siphons, traps, and
//!   Commoner's theorem. These depend only on net topology and can prove
//!   properties like conservativeness and liveness without state space
//!   exploration.
//!
//! - [`semi_decision`]: LP/ILP formulations of the marking equation for
//!   reachability filtering and structural boundedness. These run in
//!   polynomial time and serve as fast necessary-condition checks.
//!
//! - [`math`]: Integer linear algebra (Bareiss null space) used internally
//!   by invariant computation.
//!
//! Most users should start with the high-level behavioral queries on
//! [`System`] (e.g. `is_bounded`, `is_live`, `liveness_levels`),
//! which internally dispatch to the best available algorithm based on
//! net class. Use this module directly when you need access to
//! invariant vectors, siphon/trap sets, or marking equation
//! results for custom analysis.

use crate::analysis::model::{BoundednessAnalysis, BoundednessAnalysisMethod, CoverabilityProof, CoverabilityResult, Deadlock, DeadlockAnalysis, DeadlockAnalysisMethod, LivenessAnalysis, LivenessLevel, LivenessMethod, NonCoverabilityProof, ReachabilityProof, ReachabilityResult, UnreachabilityProof};
use crate::net::Place;
use crate::{ExplorationOrder, Marking, Net, Omega, OmegaMarking, PlaceMap, System, TransitionMap};

pub mod structural;
pub mod semi_decision;
pub mod math;
pub mod model;

impl<N: AsRef<Net>> System<N> {
    /// Analyzes boundedness and returns per-place bounds with evidence.
    ///
    /// Strategy (ascending cost):
    /// 1. Structural boundedness LP: if feasible, derives upper bounds from
    ///    the weight vector and the initial marking. Fast but bounds may be loose.
    /// 2. Coverability graph: always terminates. Gives exact per-place bounds.
    #[must_use]
    pub fn analyze_boundedness(&self) -> BoundednessAnalysis {
        let net = self.net.as_ref();

        if let Some(place_weights) = semi_decision::find_positive_place_subvariant(net) {
            let weighted_sum: f64 = net.places()
                .map(|p| place_weights[p] * f64::from(self.marking[p]))
                .sum();
            let place_bounds: PlaceMap<Omega> = net.places()
                .map(|p| {
                    let bound = (weighted_sum / place_weights[p]).floor() as u32;
                    Omega::Finite(bound)
                })
                .collect();

            return BoundednessAnalysis {
                place_bounds,
                method: BoundednessAnalysisMethod::PositivePlaceSubvariant(place_weights),
            };
        }

        let cg = self.build_coverability_graph();
        let place_bounds = cg.place_bounds();

        // todo: also return cg?
        BoundednessAnalysis {
            place_bounds,
            method: BoundednessAnalysisMethod::CoverabilityGraph,
        }
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
        let net = self.net.as_ref();

        if net.is_s_net() {
            return structural::analyze_liveness_s_net(net, &self.marking);
        }

        if net.is_t_net() {
            return structural::analyze_liveness_t_net(net, &self.marking);
        }

        if net.is_free_choice_net()
            && let chc = structural::commoner_hack_criterion(net, &self.marking)
            && chc.is_satisfied() {
            return LivenessAnalysis {
                levels: TransitionMap::from(vec![LivenessLevel::L4; net.transition_count() as usize]),
                method: LivenessMethod::FreeChoice(chc),
            };
        }

        match self.build_coverability_graph().into_reachability_graph() {
            Ok(rg) => {
                let levels = rg.liveness_levels();
                LivenessAnalysis {
                    levels,
                    method: LivenessMethod::ReachabilityGraphSCC,
                }
            }
            Err(_cg) => {
                // TODO: liveness for unbounded nets
                LivenessAnalysis {
                    levels: std::iter::repeat_n(LivenessLevel::L0, net.transition_count() as usize).collect(),
                    method: LivenessMethod::Inconclusive,
                }
            }
        }
    }

    /// Analyzes deadlock-freedom and returns deadlock witnesses with evidence.
    ///
    /// Strategy:
    /// 1. Siphon/trap check (Commoner criterion): if every siphon contains
    ///    a marked trap, the system is deadlock-free (no exploration needed).
    /// 2. If the structural check is inconclusive, escalates to state space
    ///    exploration (CG → RG) and reports all reachable deadlocks with
    ///    firing sequences.
    #[must_use]
    pub fn analyze_deadlock_freedom(&self) -> DeadlockAnalysis {
        let net = self.net.as_ref();

        let chc = structural::commoner_hack_criterion(net, &self.marking);
        if chc.is_satisfied() {
            return DeadlockAnalysis {
                deadlocks: Box::new([]),
                evidence: DeadlockAnalysisMethod::CommonerTheorem(chc),
            };
        }

        match self.build_coverability_graph().into_reachability_graph() {
            Ok(rg) => {
                let deadlock_markings = rg.deadlocks();
                let deadlocks: Box<[Deadlock]> = deadlock_markings
                    .into_iter()
                    .cloned()
                    .map(|marking| {
                        let firing_sequence = rg.path_to(&marking).unwrap_or_default();
                        Deadlock {
                            firing_sequence,
                            marking,
                        }
                    })
                    .collect();
                DeadlockAnalysis {
                    deadlocks,
                    evidence: DeadlockAnalysisMethod::Exploration,
                }
            }
            Err(_cg) => {
                // TODO: deadlock-freedom for unbounded nets is currently inconclusive rather than attempting infinite exploration.
                DeadlockAnalysis {
                    deadlocks: Box::new([]),
                    evidence: DeadlockAnalysisMethod::Inconclusive,
                }
            }
        }
    }

    /// Analyzes reachability of a target marking with structured evidence.
    ///
    /// Returns [`ReachabilityResult::Reachable`] with a firing sequence,
    /// [`ReachabilityResult::Unreachable`] with a proof, or
    /// [`ReachabilityResult::Inconclusive`] if current algorithms cannot decide.
    ///
    /// Strategy (ascending cost):
    /// 1. **S-nets**: token conservation (exact, polynomial).
    /// 2. **T-nets**: ILP marking equation (exact).
    /// 3. **General**: LP filter → ILP filter → state space exploration.
    ///
    /// For unbounded general nets where LP/ILP filters pass, returns
    /// `Inconclusive` rather than attempting infinite exploration.
    #[must_use]
    pub fn analyze_reachability(&self, target: &Marking) -> ReachabilityResult {
        let net = self.net.as_ref();

        if self.marking == *target {
            return ReachabilityProof::FiringSequence(Box::new([])).into();
        }

        if net.is_s_net() {
            if net.is_strongly_connected() {
                let initial_marking_sum = self.marking.iter().sum::<u32>();
                let target_marking_sum = target.iter().sum::<u32>();
                return if initial_marking_sum == target_marking_sum {
                    ReachabilityProof::StronglyConnectedSNetTokenConservation {
                        marking_sum: initial_marking_sum,
                    }.into()
                } else {
                    UnreachabilityProof::SNetTokenConservationViolation {
                        initial_marking_sum,
                        target_marking_sum,
                    }.into()
                };
            }
            return semi_decision::find_marking_equation_rational_solution(net, &self.marking, target)
                .map_or_else(
                    || UnreachabilityProof::MarkingEquationNoRationalSolution.into(),
                    |s| ReachabilityProof::SNetMarkingEquationRationalSolution(s).into()
                )
        }

        if net.is_t_net() {
            return semi_decision::find_marking_equation_integer_solution(net, &self.marking, target)
                .map_or_else(
                    || UnreachabilityProof::MarkingEquationNoIntegerSolution.into(),
                    |s| ReachabilityProof::TNetMarkingEquationIntegerSolution(s).into()
                )
        }

        if semi_decision::find_marking_equation_rational_solution(
            net, &self.marking, target,
        ).is_none() {
            return UnreachabilityProof::MarkingEquationNoRationalSolution.into();
        }

        // todo: only test ILP if the rational solution is already an integer solution
        if semi_decision::find_marking_equation_integer_solution(
            net, &self.marking, target,
        ).is_none() {
            return UnreachabilityProof::MarkingEquationNoIntegerSolution.into();
        }

        // todo: short circuit evaluation once we find it
        match self.build_coverability_graph().into_reachability_graph() {
            Ok(rg) => {
                rg.path_to(target).map_or_else(
                    || UnreachabilityProof::ExhaustiveSearch.into(),
                    |path| ReachabilityProof::FiringSequence(path).into()
                )
            }
            Err(_cg) => {
                ReachabilityResult::Inconclusive
            }
        }
    }

    /// Analyzes coverability of a target marking with structured evidence.
    ///
    /// A marking `target` is **coverable** if there exists a reachable marking `M`
    /// such that `M(p) >= target(p)` for every place `p`.
    ///
    /// Strategy (ascending cost):
    /// 1. Trivial: if `M₀ >= target`, return immediately.
    /// 2. LP covering equation (necessary): if infeasible, `target` is uncoverable.
    /// 3. ILP covering equation (stronger necessary): if infeasible, uncoverable.
    /// 4. Coverability graph (Karp–Miller): always terminates; exact.
    ///
    /// References:
    /// - [Murata 1989, §V-A](crate::literature#v-a--the-coverability-tree) (coverability tree properties)
    /// - [Primer, Proposition 3.23](crate::literature#proposition-323--finiteness-of-the-coverability-trees-and-graphs) (termination)
    /// - [Primer, Proposition 3.27](crate::literature#proposition-327--all-that-can-be-checked-on-a-coverability-graph) (coverability via Cov(N))
    /// - [Primer, Proposition 4.3](crate::literature#proposition-43--state-equation) (necessary condition underpinning LP/ILP filters)
    /// - [Esparza Lecture Notes, Theorem 3.2.5](crate::literature#theorem-325--coverability-graph-terminates) (termination, supplementary)
    /// - [Esparza Lecture Notes, Theorem 3.2.8](crate::literature#theorem-328--coverability-characterization) (correctness, supplementary)
    #[must_use]
    pub fn analyze_coverability(&self, target: &Marking) -> CoverabilityResult {
        let net = self.net.as_ref();

        if self.marking >= *target {
            return CoverabilityProof {
                firing_sequence: Box::new([]),
                covering_marking: OmegaMarking::from(self.marking.clone()),
            }.into();
        }

        if semi_decision::find_covering_equation_rational_solution(
            net,
            &self.marking,
            target
        ).is_none() {
            return NonCoverabilityProof::MarkingEquationNoRationalSolution.into();
        }

        // todo: only test ILP if the rational solution is already an integer solution
        if semi_decision::find_covering_equation_integer_solution(net, &self.marking, target).is_none() {
            return NonCoverabilityProof::MarkingEquationNoIntegerSolution.into();
        }

        self.explore_coverability(ExplorationOrder::BreadthFirst)
            .find_cover(&OmegaMarking::from(target))
            .map_or_else(
                || NonCoverabilityProof::ExhaustiveSearch.into(),
                CoverabilityResult::Coverable
            )
    }

    /// Whether the system is bounded (all places have finite token counts
    /// across all reachable markings).
    ///
    /// Delegates to [`analyze_boundedness`](Self::analyze_boundedness).
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.analyze_boundedness().system_bound().is_finite()
    }

    /// Whether the system is live (L4): every transition can fire from
    /// every reachable marking (possibly after further firings).
    ///
    /// Delegates to [`analyze_liveness`](Self::analyze_liveness).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.analyze_liveness().net_level().is_live()
    }

    /// Whether the system is deadlock-free: no reachable marking has zero
    /// enabled transitions.
    ///
    /// This is a convenience method which delegates to
    /// [`analyze_deadlock_freedom`](Self::analyze_deadlock_freedom)
    /// and throws away the witnesses and evidence.
    /// For detailed analysis, call the latter method directly
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.analyze_deadlock_freedom().is_deadlock_free()
    }

    /// Whether `target` is reachable from the initial marking.
    ///
    /// Delegates to [`analyze_reachability`](Self::analyze_reachability).
    /// Returns `false` for inconclusive results.
    #[must_use]
    pub fn is_reachable(&self, target: &Marking) -> bool {
        self.analyze_reachability(target).is_reachable()
    }

    /// Whether `target` is coverable from the initial marking.
    ///
    /// Delegates to [`analyze_coverability`](Self::analyze_coverability).
    pub fn is_coverable(&self, target: &Marking) -> bool {
        self.analyze_coverability(target).is_coverable()
    }
}

/// Checks whether there exists a directed cycle of zero-token internal places
/// within a single SCC of a T-net's transition graph.
///
/// If such a cycle exists, it is an unmarked circuit, meaning not all circuits
/// in the SCC are marked.
fn has_zero_token_cycle(
    net: &Net,
    marking: &Marking,
    internal_places: &[Place],
    trans_to_scc: &[usize],
    scc_idx: usize,
) -> bool {
    use std::collections::HashSet;

    let zero_places: HashSet<Place> = internal_places.iter()
        .copied()
        .filter(|&p| marking[p] == 0)
        .collect();

    if zero_places.is_empty() {
        return false;
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum DfsState { Unvisited, InStack, Done }
    let mut state = vec![DfsState::Unvisited; net.place_count() as usize];

    for &start in &zero_places {
        if state[start.usize_index()] != DfsState::Unvisited {
            continue;
        }
        // Iterative DFS with explicit stack.
        let mut stack: Vec<(Place, usize)> = vec![(start, 0)];
        state[start.usize_index()] = DfsState::InStack;

        while let Some((place, child_idx)) = stack.last_mut() {
            let t_out = net.dense_output_transitions(*place)[0];
            let successors: Vec<Place> = net.dense_output_places(t_out).iter()
                .copied()
                .filter(|&p2| {
                    zero_places.contains(&p2)
                        && trans_to_scc[net.dense_output_transitions(p2)[0].usize_index()] == scc_idx
                })
                .collect();

            if *child_idx < successors.len() {
                let next = successors[*child_idx];
                *child_idx += 1;
                match state[next.usize_index()] {
                    DfsState::InStack => return true, // found a cycle
                    DfsState::Unvisited => {
                        state[next.usize_index()] = DfsState::InStack;
                        stack.push((next, 0));
                    }
                    DfsState::Done => {}
                }
            } else {
                state[place.usize_index()] = DfsState::Done;
                stack.pop();
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::builder::NetBuilder;
    use crate::Omega;

    /// SC S-net (circuit): marked → all L4.
    #[test]
    fn s_net_sc_marked_all_l4() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        let net = b.build().unwrap();
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        let sys = System::new(net, [1u32, 0]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L4);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L4);
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
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        let sys = System::new(net, [0u32, 0]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L0);
    }

    /// Non-SC S-net: sink SCC marked → L4; non-sink SCC marked → L3;
    /// inter-SCC transition → L1.
    ///
    ///   p0 ←→ p1 (SCC_A, non-sink, 1 token)
    ///       ↓ (t2, inter-SCC)
    ///   p2 ←→ p3 (SCC_B, sink, 0 tokens initially)
    #[test]
    fn s_net_non_sc_mixed_levels() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        // SCC_A cycle: p0 → t0 → p1 → t1 → p0
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        // Inter-SCC: p0 → t2 → p2
        let t2 = b.add_transition();
        b.add_arc((p0, t2)); b.add_arc((t2, p2));
        // SCC_B cycle: p2 → t3 → p3 → t4 → p2
        let [t3, t4] = b.add_transitions();
        b.add_arc((p2, t3)); b.add_arc((t3, p3));
        b.add_arc((p3, t4)); b.add_arc((t4, p2));

        let net = b.build().unwrap();
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        let t2 = net.dense_transition(t2);
        let t3 = net.dense_transition(t3);
        let t4 = net.dense_transition(t4);
        assert!(net.is_s_net());
        let sys = System::new(net, [1u32, 0, 0, 0]);
        let analysis = sys.analyze_liveness();

        // SCC_A is non-sink and marked → internal transitions L3
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L3);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L3);
        // Inter-SCC transition → L1
        assert_eq!(analysis.transition_level(t2), LivenessLevel::L1);
        // SCC_B is sink and reachable (receives tokens from SCC_A) → L4
        assert_eq!(analysis.transition_level(t3), LivenessLevel::L4);
        assert_eq!(analysis.transition_level(t4), LivenessLevel::L4);
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
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        let t2 = net.dense_transition(t2);
        let t3 = net.dense_transition(t3);
        assert!(net.is_s_net());

        // No tokens anywhere → everything L0
        let sys = System::new(net, [0u32, 0, 0, 0]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t2), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t3), LivenessLevel::L0);
    }

    /// SC T-net: all circuits marked → all L4.
    #[test]
    fn t_net_sc_all_circuits_marked_l4() {
        // Simple marked graph cycle: t0 → p0 → t1 → p1 → t0
        // Each place has |•p| = 1, |p•| = 1.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        b.add_arc((t0, p2)); b.add_arc((p2, t1)); // second path
        let net = b.build().unwrap();
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        assert!(net.is_t_net());

        // Mark all circuits: p0=1, p1=1, p2=0 → circuit {p0,p2}→ sum=1, {p1}→ sum=1
        // Actually let's mark all places for safety
        let sys = System::new(net, [1u32, 1, 1]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L4);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L4);
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
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        assert!(net.is_t_net());

        let sys = System::new(net, [0u32, 0]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L0);
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
        let t_src = net.dense_transition(t_src);
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        assert!(net.is_t_net());

        // Cycle {p0, p1} has 1 token → marked
        let sys = System::new(net, [0u32, 1, 0]);
        let analysis = sys.analyze_liveness();
        // t_src is always enabled (no inputs) → L4
        assert_eq!(analysis.transition_level(t_src), LivenessLevel::L4);
        // t0 depends on p_src (from L4 t_src) and p1 (from marked cycle) → L4
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L4);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L4);
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
        let t0 = net.dense_transition(t0);
        let t1 = net.dense_transition(t1);
        let t2 = net.dense_transition(t2);
        let t3 = net.dense_transition(t3);
        assert!(net.is_t_net());

        // SCC_A unmarked, SCC_B marked but predecessor dead
        let sys = System::new(net, [0u32, 0, 0, 1, 0]);
        let analysis = sys.analyze_liveness();
        assert_eq!(analysis.transition_level(t0), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t1), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t2), LivenessLevel::L0);
        assert_eq!(analysis.transition_level(t3), LivenessLevel::L0);
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
        assert_eq!(net.class(), crate::class::NetClass::FreeChoice);
        assert!(!net.is_s_net());
        assert!(!net.is_t_net());

        // Live with 1 token on s1 and 1 on s2
        let sys = System::new(net, [1u32, 1, 0, 0, 0, 0, 0, 0]);
        let analysis = sys.analyze_liveness();
        assert!(analysis.net_level().is_live());
        assert!(matches!(analysis.method, LivenessMethod::FreeChoice(_)));
    }

    #[test]
    fn coverability_initial_marking_covers() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        let net = b.build().unwrap();
        let sys = System::new(net, [1u32, 0]);

        let res = sys.analyze_coverability(&Marking::from([1u32, 0]));
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable(CoverabilityProof { firing_sequence, covering_marking }) => {
                assert_eq!(covering_marking, Marking::from([1u32, 0]));
                assert_eq!(firing_sequence.len(), 0);
            }
            _ => panic!("expected InitialMarking proof"),
        }
    }

    #[test]
    fn coverability_uncoverable_detected_by_lp() {
        // Two-place cycle with one token: cannot cover (1,1).
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        let net = b.build().unwrap();
        let sys = System::new(net, [1u32, 0]);

        let res = sys.analyze_coverability(&Marking::from([1u32, 1]));
        assert!(res.is_uncoverable());
        assert!(matches!(
            res,
            CoverabilityResult::Uncoverable(NonCoverabilityProof::MarkingEquationNoRationalSolution)
        ));
    }

    #[test]
    fn coverability_unbounded_omega_witness() {
        // Unbounded producer: t0 consumes p0 and produces p0 and p1.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().unwrap();
        let p0 = net.dense_place(p0);
        let p1 = net.dense_place(p1);
        let sys = System::new(net, [1u32, 0]);

        let res = sys.analyze_coverability(&Marking::from([1u32, 10]));
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable(CoverabilityProof { covering_marking, .. }) => {
                // p0 stays 1; p1 becomes ω in the coverability graph.
                assert_eq!(covering_marking[p0], Omega::Finite(1));
                assert!(covering_marking[p1] >= Omega::Finite(10));
            }
            _ => panic!("expected coverability-graph proof"),
        }
    }
}
