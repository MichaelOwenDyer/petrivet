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

use crate::analysis::model::{BoundednessAnalysis, BoundednessAnalysisMethod, CoverabilityProof, CoverabilityResult, Deadlock, DeadlockAnalysis, DeadlockAnalysisMethod, LivenessAnalysis, LivenessLevel, LivenessMethod, ReachabilityProof, ReachabilityResult, SNetComponent, SNetLivenessEvidence, TNetComponent, TNetLivenessEvidence, NonCoverabilityProof, UnreachabilityProof};
use crate::net::{Place, Transition};
use crate::{CoverabilityGraph, ExplorationOrder, Marking, Net, Omega, OmegaMarking, System};

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
                .map(|p| place_weights[p.index()] * f64::from(self.initial_marking[p]))
                .sum();
            let place_bounds: Box<[Omega]> = net.places()
                .map(|p| {
                    let bound = (weighted_sum / place_weights[p.index()]).floor() as u32;
                    Omega::Finite(bound)
                })
                .collect();

            return BoundednessAnalysis {
                place_bounds,
                method: BoundednessAnalysisMethod::PositivePlaceSubvariant(place_weights),
            };
        }

        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        let place_bounds: Box<[Omega]> = net.places()
            .map(|p| cg.place_bound(p))
            .collect();

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
            return analyze_liveness_s_net(net, &self.initial_marking);
        }

        if net.is_t_net() {
            return analyze_liveness_t_net(net, &self.initial_marking);
        }

        if net.is_free_choice_net()
            && let chc = structural::commoner_hack_criterion(net, &self.initial_marking)
            && chc.is_satisfied() {
            return LivenessAnalysis {
                levels: vec![LivenessLevel::L4; net.n_transitions()].into_boxed_slice(),
                method: LivenessMethod::FreeChoice(chc),
            };
        }

        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        if let Ok(rg) = cg.into_reachability_graph() {
            let levels = rg.liveness_levels();
            LivenessAnalysis {
                levels,
                method: LivenessMethod::ReachabilityGraphSCC,
            }
        } else {
            LivenessAnalysis {
                levels: vec![LivenessLevel::L0; net.n_transitions()].into_boxed_slice(),
                method: LivenessMethod::Inconclusive,
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

        let chc = structural::commoner_hack_criterion(net, &self.initial_marking);
        if chc.is_satisfied() {
            return DeadlockAnalysis {
                deadlocks: Box::new([]),
                evidence: DeadlockAnalysisMethod::CommonerTheorem(chc),
            };
        }

        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        match cg.into_reachability_graph() {
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

        if self.initial_marking == *target {
            return ReachabilityProof::FiringSequence(Box::new([])).into();
        }

        if net.is_s_net() {
            if net.is_strongly_connected() {
                let initial_marking_sum = self.initial_marking.iter().sum::<u32>();
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
            return semi_decision::find_marking_equation_rational_solution(net, &self.initial_marking, target)
                .map_or_else(
                    || UnreachabilityProof::MarkingEquationNoRationalSolution.into(),
                    |s| ReachabilityProof::SNetMarkingEquationRationalSolution(s).into()
                )
        }

        if net.is_t_net() {
            return semi_decision::find_marking_equation_integer_solution(net, &self.initial_marking, target)
                .map_or_else(
                    || UnreachabilityProof::MarkingEquationNoIntegerSolution.into(),
                    |s| ReachabilityProof::TNetMarkingEquationIntegerSolution(s).into()
                )
        }

        if semi_decision::find_marking_equation_rational_solution(
            net, &self.initial_marking, target,
        ).is_none() {
            return UnreachabilityProof::MarkingEquationNoRationalSolution.into();
        }

        if semi_decision::find_marking_equation_integer_solution(
            net, &self.initial_marking, target,
        ).is_none() {
            return UnreachabilityProof::MarkingEquationNoIntegerSolution.into();
        }

        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        if let Ok(rg) = cg.into_reachability_graph() {
            rg.path_to(target).map_or_else(
                || UnreachabilityProof::ExhaustiveSearch.into(),
                |path| ReachabilityProof::FiringSequence(path).into()
            )
        } else {
            ReachabilityResult::Inconclusive
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

        if self.initial_marking >= *target {
            return CoverabilityProof {
                firing_sequence: Box::new([]),
                covering_marking: OmegaMarking::from(self.initial_marking.clone()),
            }.into();
        }

        if semi_decision::find_covering_equation_rational_solution(net, &self.initial_marking, target).is_none() {
            return NonCoverabilityProof::MarkingEquationNoRationalSolution.into();
        }

        if semi_decision::find_covering_equation_integer_solution(net, &self.initial_marking, target).is_none() {
            return NonCoverabilityProof::MarkingEquationNoIntegerSolution.into();
        }

        CoverabilityGraph::new(self, ExplorationOrder::BreadthFirst)
            .cover(&OmegaMarking::from(target))
            .map_or_else(
                || NonCoverabilityProof::ExhaustiveSearch.into(),
                |proof| CoverabilityResult::Coverable(proof)
            )
    }

    /// Whether the system is bounded (all places have finite token counts
    /// across all reachable markings).
    ///
    /// Delegates to [`analyze_boundedness`](Self::analyze_boundedness).
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.analyze_boundedness().of_system().is_finite()
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

/// Liveness analysis for S-nets via SCC decomposition of the place graph.
///
/// In an S-net each transition has exactly one input place and one output place,
/// so transitions are directed edges in the "place graph." The SCC decomposition
/// of this graph determines per-transition liveness:
///
/// - Sink SCC, marked (token sum > 0): internal transitions are **L4**.
/// - Non-sink SCC, marked: internal transitions are **L3** (tokens *can* stay
///   cycling forever but *can also* escape, preventing L4).
/// - Inter-SCC transitions (connecting different SCCs): **L1** if the source
///   SCC is reachable by tokens, **L0** otherwise.
/// - Transitions whose source SCC has no tokens and cannot receive any: **L0**.
///
/// References: [Murata 1989 Theorem 4](crate::literature#theorem-4--liveness-of-s-nets-state-machines),
/// [Primer Corollary 5.30](crate::literature#corollary-530--liveness-of-s-systems).
fn analyze_liveness_s_net(net: &Net, marking: &Marking) -> LivenessAnalysis {
    use petgraph::graph::NodeIndex;

    let n_p = net.n_places();
    let n_t = net.n_transitions();

    // Build the place graph: places are nodes, transitions are directed edges
    // from their single input place to their single output place.
    let mut place_graph = petgraph::Graph::<Place, Transition>::with_capacity(n_p, n_t);
    let p_nodes: Vec<NodeIndex> = net.places()
        .map(|p| place_graph.add_node(p))
        .collect();

    for t in net.transitions() {
        let src = net.preset_t(t)[0];
        let dst = net.postset_t(t)[0];
        place_graph.add_edge(p_nodes[src.index()], p_nodes[dst.index()], t);
    }

    // Compute SCCs (Kosaraju returns them in reverse topological order).
    let sccs = petgraph::algo::kosaraju_scc(&place_graph);

    // Map each place to its SCC index (we'll reverse the order to get
    // topological order with sources first).
    let n_sccs = sccs.len();
    let mut place_to_scc = vec![0usize; n_p];
    for (rev_idx, scc) in sccs.iter().enumerate() {
        let scc_idx = n_sccs - 1 - rev_idx;
        for &node_idx in scc {
            let place = place_graph[node_idx];
            place_to_scc[place.index()] = scc_idx;
        }
    }

    // Build component info in topological order (sources first).
    let mut components: Vec<SNetComponent> = Vec::with_capacity(n_sccs);
    let mut scc_has_outgoing = vec![false; n_sccs];

    // Classify each transition as internal (same SCC) or inter-SCC,
    // and track which SCCs have outgoing transitions.
    let mut transition_scc: Vec<Option<usize>> = vec![None; n_t];
    for t in net.transitions() {
        let src = net.preset_t(t)[0];
        let dst = net.postset_t(t)[0];
        let src_scc = place_to_scc[src.index()];
        let dst_scc = place_to_scc[dst.index()];
        if src_scc == dst_scc {
            transition_scc[t.index()] = Some(src_scc);
        } else {
            scc_has_outgoing[src_scc] = true;
        }
    }

    // Build components in topological order.
    for (rev_idx, scc) in sccs.iter().enumerate() {
        let scc_idx = n_sccs - 1 - rev_idx;
        let places: Box<[Place]> = scc.iter()
            .map(|&ni| place_graph[ni])
            .collect();
        let token_sum: u32 = places.iter()
            .map(|&p| marking[p])
            .sum();
        let transitions: Box<[Transition]> = net.transitions()
            .filter(|t| transition_scc[t.index()] == Some(scc_idx))
            .collect();
        let is_sink = !scc_has_outgoing[scc_idx];

        components.push(SNetComponent {
            places,
            transitions,
            token_sum,
            is_sink,
        });
    }

    // Sort components into topological order (sources first).
    // kosaraju_scc returns reverse topological, so we built them reversed above,
    // but pushed in reverse order. Let's just reverse the vec.
    components.reverse();

    // Determine which SCCs can receive tokens (transitively from marked SCCs).
    let mut scc_reachable = vec![false; n_sccs];
    for scc_idx in 0..n_sccs {
        if components[scc_idx].token_sum > 0 {
            scc_reachable[scc_idx] = true;
        }
    }
    // Propagate reachability along inter-SCC transitions (topological order).
    for t in net.transitions() {
        let src = net.preset_t(t)[0];
        let dst = net.postset_t(t)[0];
        let src_scc = place_to_scc[src.index()];
        let dst_scc = place_to_scc[dst.index()];
        if src_scc != dst_scc && scc_reachable[src_scc] {
            scc_reachable[dst_scc] = true;
        }
    }

    // Assign liveness levels.
    let mut levels = vec![LivenessLevel::L0; n_t];
    for t in net.transitions() {
        let src = net.preset_t(t)[0];
        let dst = net.postset_t(t)[0];
        let src_scc = place_to_scc[src.index()];
        let dst_scc = place_to_scc[dst.index()];

        if src_scc == dst_scc {
            // Internal transition
            let comp = &components[src_scc];
            if comp.token_sum > 0 || scc_reachable[src_scc] {
                if comp.is_sink {
                    levels[t.index()] = LivenessLevel::L4;
                } else {
                    levels[t.index()] = LivenessLevel::L3;
                }
            }
        } else {
            // Inter-SCC transition: L1 if the source SCC has/receives tokens
            if scc_reachable[src_scc] {
                levels[t.index()] = LivenessLevel::L1;
            }
        }
    }

    LivenessAnalysis {
        levels: levels.into_boxed_slice(),
        method: LivenessMethod::SNet(SNetLivenessEvidence {
            components: components.into_boxed_slice(),
        }),
    }
}

/// Liveness analysis for T-nets via SCC decomposition of the transition graph.
///
/// In a T-net each place has exactly one input and one output transition,
/// so places are directed edges in the "transition graph." The circuit token
/// invariance property ([Murata Theorem 26](crate::literature#theorem-26--circuit-token-invariance-in-t-nets)) guarantees that every transition
/// is either **L0** or **L4** — no intermediate levels are possible.
///
/// Algorithm:
/// 1. Build the transition graph (transitions as nodes, places as edges).
/// 2. Compute SCCs.
/// 3. For each SCC, check if all internal circuits are marked.
///    Within a strongly connected T-net sub-graph, this is equivalent to
///    checking that every cycle of places has at least one token (Theorem 7).
///    A practical check: the SCC sub-T-net is live iff it has no token-free
///    cycle, which we verify by checking for empty-marked places that form
///    a cycle (DFS for a zero-token cycle).
/// 4. Process SCCs in topological order: an SCC is L4 iff all internal
///    circuits are marked AND all predecessor SCCs are L4.
///
/// References: [Murata 1989 Theorems 7 & 26](crate::literature#theorem-7--liveness-of-t-nets-marked-graphs), [Primer Theorem 5.31](crate::literature#theorem-531--liveness-and-realisability-in-t-systems).
fn analyze_liveness_t_net(net: &Net, marking: &Marking) -> LivenessAnalysis {
    use petgraph::graph::NodeIndex;

    let n_p = net.n_places();
    let n_t = net.n_transitions();

    // Build the transition graph: transitions are nodes, places are directed
    // edges from their single input transition to their single output transition.
    let mut trans_graph = petgraph::Graph::<Transition, Place>::with_capacity(n_t, n_p);
    let t_nodes: Vec<NodeIndex> = net.transitions()
        .map(|t| trans_graph.add_node(t))
        .collect();

    for p in net.places() {
        let src = net.preset_p(p)[0];
        let dst = net.postset_p(p)[0];
        trans_graph.add_edge(t_nodes[src.index()], t_nodes[dst.index()], p);
    }

    // Compute SCCs (Kosaraju returns reverse topological order).
    let sccs = petgraph::algo::kosaraju_scc(&trans_graph);
    let n_sccs = sccs.len();

    // Map each transition to its SCC index (topological order, sources first).
    let mut trans_to_scc = vec![0usize; n_t];
    for (rev_idx, scc) in sccs.iter().enumerate() {
        let scc_idx = n_sccs - 1 - rev_idx;
        for &node_idx in scc {
            let transition = trans_graph[node_idx];
            trans_to_scc[transition.index()] = scc_idx;
        }
    }

    // For each SCC, find internal places and check if all circuits are marked.
    // A place is internal if both its input and output transitions are in the same SCC.
    // Within a strongly connected sub-T-net, all circuits are marked iff
    // there is no cycle consisting entirely of zero-token places.
    // Since the circuit token sum is invariant, an unmarked circuit means some
    // subset of internal places has total tokens = 0 AND forms a cycle.
    // Efficient check: if any internal place has 0 tokens, check if there's a
    // zero-token cycle through it using DFS on zero-token internal places.
    let mut components: Vec<TNetComponent> = Vec::with_capacity(n_sccs);

    for (rev_idx, scc) in sccs.iter().enumerate() {
        let scc_idx = n_sccs - 1 - rev_idx;
        let transitions: Box<[Transition]> = scc.iter()
            .map(|&ni| trans_graph[ni])
            .collect();

        let places: Box<[Place]> = net.places()
            .filter(|&p| {
                let src = net.preset_p(p)[0];
                let dst = net.postset_p(p)[0];
                trans_to_scc[src.index()] == scc_idx && trans_to_scc[dst.index()] == scc_idx
            })
            .collect();

        // For singleton or acyclic SCCs (no internal places forming cycles),
        // all_circuits_marked is vacuously true.
        let all_circuits_marked = if places.is_empty() {
            true
        } else {
            !has_zero_token_cycle(net, marking, &places, &trans_to_scc, scc_idx)
        };

        components.push(TNetComponent {
            transitions,
            places,
            all_circuits_marked,
            predecessors_live: false, // filled in below
        });
    }

    // Reverse to get topological order (sources first).
    components.reverse();

    // Propagate predecessor liveness in topological order.
    // Also track which SCCs have all predecessors live.
    let mut scc_live = vec![false; n_sccs];
    for scc_idx in 0..n_sccs {
        // Check all predecessor SCCs via inter-SCC places.
        let all_preds_live = components[scc_idx].transitions.iter().all(|&t| {
            net.preset_t(t).iter().all(|&p| {
                let src_t = net.preset_p(p)[0];
                let src_scc = trans_to_scc[src_t.index()];
                src_scc == scc_idx || scc_live[src_scc]
            })
        });

        components[scc_idx].predecessors_live = all_preds_live;
        scc_live[scc_idx] = components[scc_idx].all_circuits_marked && all_preds_live;
    }

    // Assign liveness levels: L4 if SCC is live, L0 otherwise.
    let mut levels = vec![LivenessLevel::L0; n_t];
    for t in net.transitions() {
        let scc_idx = trans_to_scc[t.index()];
        if scc_live[scc_idx] {
            levels[t.index()] = LivenessLevel::L4;
        }
    }

    LivenessAnalysis {
        levels: levels.into_boxed_slice(),
        method: LivenessMethod::TNet(TNetLivenessEvidence {
            components: components.into_boxed_slice(),
        }),
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

    // DFS on the sub-graph of zero-token internal places.
    // An edge p1 → p2 exists if p1's output transition t has an output place p2
    // that is also a zero-token internal place (i.e., t → p2, where t = p1•[0]
    // in a T-net... but t can have multiple output places).
    // Actually, in the transition graph, places are edges. We need to follow:
    // p1 → (output transition of p1) → (other output places of that transition
    // that are also zero-token internal places in the same SCC).
    //
    // More precisely: p1.postset = {t_out}. t_out's output places include p1's
    // "successor" places via the transition. Among those outputs, the ones that
    // are internal zero-token places and whose output transition is also in
    // this SCC form the successor set.

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum DfsState { Unvisited, InStack, Done }
    let mut state = vec![DfsState::Unvisited; net.n_places()];

    for &start in &zero_places {
        if state[start.index()] != DfsState::Unvisited {
            continue;
        }
        // Iterative DFS with explicit stack.
        let mut stack: Vec<(Place, usize)> = vec![(start, 0)];
        state[start.index()] = DfsState::InStack;

        while let Some((place, child_idx)) = stack.last_mut() {
            let t_out = net.postset_p(*place)[0];
            let successors: Vec<Place> = net.postset_t(t_out).iter()
                .copied()
                .filter(|&p2| {
                    zero_places.contains(&p2)
                        && trans_to_scc[net.postset_p(p2)[0].index()] == scc_idx
                })
                .collect();

            if *child_idx < successors.len() {
                let next = successors[*child_idx];
                *child_idx += 1;
                match state[next.index()] {
                    DfsState::InStack => return true, // found a cycle
                    DfsState::Unvisited => {
                        state[next.index()] = DfsState::InStack;
                        stack.push((next, 0));
                    }
                    DfsState::Done => {}
                }
            } else {
                state[place.index()] = DfsState::Done;
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
