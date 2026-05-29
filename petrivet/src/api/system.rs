//! A Petri net system: net + marking, with simulation and behavioral analysis.
//!
//! `System<N>` pairs a net structure with a mutable marking, providing methods
//! to simulate (check enablement, fire transitions) and analyze behavior
//! (boundedness, liveness, deadness).
//!
//! # Quick start
//!
//! ```
//! use petrivet::builder::NetBuilder;
//! use petrivet::net::system::PetriNet;
//!
//! // Build a simple producer-consumer net
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arcs((idle, start, busy, finish, idle));
//! let net = b.build().expect("valid net");
//!
//! let mut sys = PetriNet::new(&net, [(idle, 1)].into());
//!
//! // Simulation
//! assert!(sys.is_enabled(start));
//! sys.fire_unchecked(start);
//! assert_eq!(sys.current_marking(), [(busy, 1)].into());
//!
//! // Behavioral analysis
//! let sys = PetriNet::new(&net, [(idle, 1)].into());
//! assert!(sys.is_bounded());
//! assert!(sys.is_live());
//! ```
//!
//! # Firing patterns
//!
//! Three patterns for firing transitions:
//!
//! ```
//! # use petrivet::builder::NetBuilder;
//! # use petrivet::net::system::PetriNet;
//! # let mut b = NetBuilder::new();
//! # let [p0, p1] = b.add_places();
//! # let [t0, t1] = b.add_transitions();
//! # b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! # b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! # let net = b.build().unwrap();
//! # let mut sys = PetriNet::new(net, [1, 0]);
//! // 1. I know which transition - just try it
//! sys.try_fire(t0).unwrap();
//!
//! // 2. I need to choose from the enabled set - zero redundant checks
//! sys.choose_and_fire(|enabled| enabled.first());
//!
//! // 3. Fire anything, I don't care which
//! sys.fire_any();
//! ```

use crate::api::model::{BoundednessAnalysis, BoundednessAnalysisMethod, CoverabilityProof, CoverabilityResult, DeadlockAnalysis, DeadlockAnalysisMethod, LivenessMethod, NonCoverabilityProof, ReachabilityProof, ReachabilityResult, UnreachabilityProof};
use crate::core::analysis::semi_decision;
use crate::core::analysis::siphon_trap;
use crate::core::liveness::LivenessLevel;
use crate::core::mapping::DenseMapping;
use crate::core::marking::IdxMarking;
use crate::core::state_space::coverability::IdxOmegaMarking;
use crate::core::state_space::ExplorationOrder;
use crate::model::{CommonerHackCriterionResult, LivenessAnalysis, SiphonTrapPair};
use crate::prelude::{Marking, Net, Place, Transition};
use crate::state_space::{CoverabilityExplorer, CoverabilityGraph, Omega, OmegaMarking};
use crate::state_space::{ReachabilityExplorer, ReachabilityGraph};
use std::fmt;
use std::ops::Deref;

/// A Petri net system `(N, M₀)` consists of a [`Net`] `N` and an initial [`Marking`] `M₀`.
///
/// ```no_run
/// let pn = PetriNet::new(&net, [(p0, 1), (p2, 5)]);
/// ```
///
/// You may simulate the behavior of the system, mutating its marking,
/// by firing [`Transitions`](Transition).
///
/// ```no_run
/// pn.try_fire(t0).ok_or(|| "not enabled!")?;
/// ```
///
/// If you want to fire any enabled transition without caring which one,
/// use [`fire_any()`](Self::fire_any).
///
/// ```no_run
/// pn.fire_any().ok_or("deadlock!")?;
/// ```
///
/// To reset the system back to the marking it was initialized with,
/// use [`reset()`](Self::reset).
#[derive(Debug, Clone)]
pub struct PetriNet<N = Net> {
    /// The [`Net`] structure, which is immutable and can be shared across
    /// [`PetriNet`]s depending on the choice of `N` (e.g. `Arc<Net>`).
    pub net: N,
    pub(crate) reset: IdxMarking<u32>,
    pub(crate) marking: IdxMarking<u32>,
}

impl<N: AsRef<Net>> Deref for PetriNet<N> {
    type Target = Net;

    fn deref(&self) -> &Self::Target {
        self.net.as_ref()
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Creates a new Petri net from a net and initial marking.
    #[must_use]
    pub fn new(net: N, initial_marking: impl Into<Marking<u32>>) -> Self {
        let initial_marking = initial_marking.into();
        let initial_marking = net.as_ref().mapping.idx_marking(initial_marking);
        let current_marking = initial_marking.clone();
        Self { net, reset: initial_marking, marking: current_marking }
    }

    /// Returns the current marking of the system.
    #[must_use]
    pub fn current_marking(&self) -> Marking<u32> {
        let current_marking = self.marking.clone();
        self.mapping.marking(current_marking)
    }

    /// Returns the initial marking of the system.
    pub fn initial_marking(&self) -> Marking<u32> {
        let initial_marking = self.reset.clone();
        self.mapping.marking(initial_marking)
    }

    /// Resets the current marking to the initial marking.
    /// Returns the marking before the reset.
    pub fn reset(&mut self) -> Marking<u32> {
        let previous = std::mem::replace(
            &mut self.marking,
            self.reset.clone()
        );
        self.mapping.marking(previous)
    }

    /// Returns the token count at a place identified by its [`Place`].
    /// Returns 0 for places which do not exist in the net.
    #[must_use]
    pub fn current_tokens(&self, p: Place) -> u32 {
        self.mapping
            .place_idx(p)
            .map_or(0, |p_idx| self.marking[p_idx])
    }

    /// Consumes the system and returns (`net`, `initial_marking`, `current_marking`).
    #[must_use]
    pub fn into_parts(self) -> (N, Marking<u32>, Marking<u32>) {
        let PetriNet { net, reset: initial_marking, marking: current_marking } = self;
        let initial_marking = net.as_ref().mapping.marking(initial_marking);
        let current_marking = net.as_ref().mapping.marking(current_marking);
        (net, initial_marking, current_marking)
    }

    /// Returns a [`ReachabilityExplorer`] for this system
    /// initialized with the given [`ExplorationOrder`].
    pub fn explore_reachability(&self, order: ExplorationOrder) -> ReachabilityExplorer<'_> {
        ReachabilityExplorer::new(self, order)
    }

    /// Returns a
    pub fn explore_coverability(&self, order: ExplorationOrder) -> CoverabilityExplorer<'_> {
        CoverabilityExplorer::new(self, order)
    }

    /// Returns the complete coverability graph for this system.
    ///
    /// Warning! This may be a HUGE structure!
    pub fn build_coverability_graph(&self) -> CoverabilityGraph<'_> {
        CoverabilityGraph::new(self)
    }

    /// Returns the complete reachability graph for this system.
    ///
    /// WARNING! For unbounded nets, this will not terminate!
    pub fn build_reachability_graph(&self) -> ReachabilityGraph<'_> {
        ReachabilityGraph::build(self)
    }

    /// Attempt to construct a [`ReachabilityGraph`] of this [`PetriNet`],
    /// returning either itself if the system is bounded or a partially-explored
    /// [`CoverabilityExplorer`] if it is unbounded.
    ///
    /// Not knowing whether we will encounter unboundedness, this method first
    /// constructs a Karp-Miller coverability tree which introduces ω as soon
    /// as unbounded growth is detected. This comes at the cost of an additional
    /// check per explored marking (omega acceleration). If we finish exploring
    /// the coverability tree without ever introducing ω, we have in fact explored
    /// the full reachability graph and can return it directly. Otherwise, we return
    /// the [`CoverabilityExplorer`] in its current state, which contains the explored
    /// portion of the coverability graph up to the first ω, and can be further explored.
    ///
    /// This is the right entry point when you want the speed of exploring
    /// the reachability graph directly but cannot rule out unboundedness
    /// upfront. For unbounded nets you avoid the cost of completing the
    /// full coverability graph; for bounded nets the cost is identical to
    /// `build_reachability_graph` (no ω is ever introduced, no extra work).
    ///
    /// # Errors
    /// Returns `Err(partial_explorer)` as soon as any explored marking
    /// contains ω. The frontier is preserved, so callers may resume.
    #[allow(clippy::result_large_err)]
    pub fn try_build_reachability_graph(&self) -> Result<ReachabilityGraph<'_>, CoverabilityExplorer<'_>> {
        CoverabilityExplorer::new(self, ExplorationOrder::BreadthFirst).build_reachability_or_coverability()
    }

    /// Returns true if some reachable marking puts more than one token in any place.
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .search(|m| m.iter().any(|&t| t > 1))
            .is_some()
    }

    /// Returns true if state space enumeration encounters any deadlock marking.
    pub fn has_reachable_deadlock_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .search(|m| self.dense_net.is_deadlock(m))
            .is_some()
    }

    /// True iff structural analysis alone proves the net 1-safe under the
    /// initial marking. Uses [`find_positive_place_subvariant`] to derive
    /// per-place upper bounds in polynomial time; if every place is bounded
    /// by 1, the answer is TRUE without any state-space exploration.
    /// Returns `false` when the bound is loose or the LP is infeasible —
    /// it is a one-sided check, not a decision procedure.
    ///
    /// [`find_positive_place_subvariant`]: semi_decision::find_positive_place_subvariant
    pub fn is_structurally_one_safe(&self) -> bool {
        use crate::core::analysis::semi_decision::find_positive_place_subvariant;
        let Some(weights) = find_positive_place_subvariant(&self.dense_net) else {
            return false;
        };
        let weighted_sum: f64 = weights.iter()
            .zip(self.reset.iter())
            .map(|(&w, &m)| w * f64::from(m))
            .sum();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        weights.iter().all(|&w| (weighted_sum / w).floor() as u32 <= 1)
    }

    /// Whether a transition is enabled under the current marking.
    ///
    /// A transition t is enabled if every input place p in its preset has
    /// at least one token.
    #[must_use]
    pub fn is_enabled(&self, t: Transition) -> bool {
        self.mapping
            .transition_idx(t)
            .is_some_and(|t_idx| self.dense_net.is_enabled_in(t_idx, &self.marking))
    }

    /// Returns the set of currently enabled transitions.
    ///
    /// This is a read-only query. To fire one of these, use [`try_fire`](Self::try_fire)
    /// or [`choose_and_fire`](Self::choose_and_fire).
    pub fn enabled_transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.dense_net
            .transition_indices()
            .filter(|&t_idx| self.dense_net.is_enabled_in(t_idx, &self.marking))
            .map(|idx| self.mapping.transition(idx))
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub fn is_deadlocked(&self) -> bool {
        self.enabled_transitions().next().is_none()
    }

    /// Check-and-fire a specific transition `t`.
    ///
    /// Returns `Ok(t)` if the transition was enabled and has been fired.
    ///
    /// # Errors
    /// Returns `Err(NotEnabled(t))` if it was not enabled (i.e. if any input place had zero tokens),
    /// or if the transition does not exist in the net.
    pub fn try_fire(&mut self, t: Transition) -> Result<Transition, NotEnabled> {
        self.mapping
            .transition_idx(t)
            .ok_or(NotEnabled(t))
            .and_then(|t_idx| {
                if self.dense_net.is_enabled_in(t_idx, &self.marking) {
                    self.fire_unchecked(t);
                    Ok(t)
                } else {
                    Err(NotEnabled(t))
                }
            })
    }

    /// Fire any single enabled transition.
    ///
    /// Returns the transition that was fired, or `None` if no transition is
    /// enabled (deadlock).
    pub fn fire_any(&mut self) -> Option<Transition> {
        let t = self.transitions().find(|&t| self.is_enabled(t))?;
        self.fire_unchecked(t);
        Some(t)
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled. Underflow will
    /// panic in debug mode and wrap in release mode.
    ///
    /// # Panics
    ///
    /// Panics if the transition is not enabled (i.e. if any input place has zero tokens)
    /// or if any token count would overflow `u32::MAX`.
    pub fn fire_unchecked(&mut self, t: Transition) {
        if let Some(t_idx) = self.mapping.transition_idx(t) {
            for &p_idx in &self.net.as_ref().dense_net.preset_t[t_idx] {
                self.marking[p_idx].checked_sub(1).expect("fire_unchecked: token underflow");
            }
            for &p_idx in &self.net.as_ref().dense_net.postset_t[t_idx] {
                self.marking[p_idx].checked_add(1).expect("fire_unchecked: token overflow");
            }
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Checks the Commoner/Hack criterion, which is fulfilled when all siphons in the system
    /// contain a trap marked at the initial marking.
    /// This is a necessary and sufficient condition for liveness in free-choice nets,
    /// and a sufficient condition for deadlock-freedom in general nets.
    pub fn commoner_hack_criterion(&self) -> CommonerHackCriterionResult {
        fn to_api(mapping: &DenseMapping, pair: siphon_trap::SiphonTrapPair) -> SiphonTrapPair {
            SiphonTrapPair {
                siphon: pair.siphon.into_iter().map(|p_idx| mapping.place(p_idx)).collect(),
                trap: pair.trap.into_iter().map(|p_idx| mapping.place(p_idx)).collect(),
            }
        }

        siphon_trap::commoner_hack_criterion(&self.dense_net, &self.marking)
            .map(|siphon_trap_pairs| {
                siphon_trap_pairs.into_iter().map(|pair| to_api(&self.mapping, pair)).collect()
            })
            .map_err(|counterexample| {
                to_api(&self.mapping, counterexample)
            })
    }

    /// Analyzes boundedness and returns per-place bounds with evidence.
    ///
    /// Strategy (ascending cost):
    /// 1. Structural boundedness LP: if feasible, derives upper bounds from
    ///    the weight vector and the initial marking. Fast but bounds may be loose.
    /// 2. Coverability graph: always terminates. Gives exact per-place bounds.
    #[must_use]
    pub fn analyze_boundedness(&self) -> BoundednessAnalysis {
        // todo: also consider checking for semi-positive subvariants for subsections of the net.
        //  but how to decide which places to check?
        if let Some(place_weights) = semi_decision::find_positive_place_subvariant(&self.dense_net) {
            // Esparza lecture notes proposition 4.3.8
            let weighted_sum: f64 = place_weights.iter()
                .zip(self.reset.iter())
                .map(|(&weight, &tokens)| weight * f64::from(tokens))
                .sum();
            let bounds = self.places()
                .zip(place_weights.iter())
                .map(|(place, &weight)| {
                    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    let bound = (weighted_sum / weight).floor() as u32;
                    (place, Omega::Finite(bound))
                })
                .collect();

            return BoundednessAnalysis {
                bounds,
                method: BoundednessAnalysisMethod::PositivePlaceSubvariant(place_weights),
            };
        }

        BoundednessAnalysis {
            bounds: self.build_coverability_graph().place_bounds(),
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
        // TODO: Optimize for state machines and marked graphs
        //  by analyzing SCCs of the appropriate graph

        if self.class().is_free_choice()
            && let chc = self.commoner_hack_criterion()
            && chc.is_ok() {
            return LivenessAnalysis {
                levels: self.transitions().zip(std::iter::repeat(LivenessLevel::L4)).collect(),
                method: LivenessMethod::FreeChoice(chc),
            };
        }

        match self.build_coverability_graph().into_reachability_graph() {
            Ok(rg) => {
                let levels = rg.liveness_levels();
                LivenessAnalysis {
                    levels: self.transitions().zip(levels).collect(),
                    method: LivenessMethod::ReachabilityGraph,
                }
            }
            Err(_cg) => {
                // TODO: liveness for unbounded nets
                LivenessAnalysis {
                    levels: self.transitions().zip(std::iter::repeat(LivenessLevel::L0)).collect(),
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
        if let chc = self.commoner_hack_criterion()
            && chc.is_ok() {
            return DeadlockAnalysis {
                deadlocks: Box::new([]),
                evidence: DeadlockAnalysisMethod::CommonerTheorem(chc),
            };
        }

        match self.build_coverability_graph().into_reachability_graph() {
            Ok(rg) => {
                let deadlocks = rg.deadlocks().collect();
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
    pub fn analyze_reachability(&self, target: Marking<u32>) -> ReachabilityResult {
        let idx_target = self.mapping.idx_marking(target.clone());

        if self.marking == idx_target {
            return ReachabilityProof::FiringSequence(Box::new([])).into();
        }

        if self.class().is_state_machine() {
            if self.is_strongly_connected() {
                let initial_marking_sum = self.marking.iter().sum::<u32>();
                let target_marking_sum = idx_target.iter().sum::<u32>();
                return if initial_marking_sum == target_marking_sum {
                    ReachabilityProof::StronglyConnectedSNetTokenConservation {
                        marking_sum: initial_marking_sum,
                    }.into()
                } else {
                    UnreachabilityProof::SNetTokenConservationViolation.into()
                };
            }
            return semi_decision::find_marking_equation_rational_solution(
                &self.dense_net,
                &self.marking,
                &idx_target
            ).map_or_else(
                || UnreachabilityProof::MarkingEquationNoRationalSolution.into(),
                |solution| {
                    let solution = self.transitions().zip(solution).collect();
                    ReachabilityProof::SNetMarkingEquationRationalSolution(solution).into()
                }
            )
        }

        if self.class().is_marked_graph() {
            return semi_decision::find_marking_equation_integer_solution(
                &self.dense_net,
                &self.marking,
                &idx_target
            ).map_or_else(
                || UnreachabilityProof::MarkingEquationNoIntegerSolution.into(),
                |solution| {
                    let solution = self.transitions().zip(solution).collect();
                    ReachabilityProof::TNetMarkingEquationIntegerSolution(solution).into()
                }
            )
        }

        if semi_decision::find_marking_equation_rational_solution(
            &self.dense_net,
            &self.marking,
            &idx_target,
        ).is_none() {
            return UnreachabilityProof::MarkingEquationNoRationalSolution.into();
        }

        // todo: only test ILP if the rational solution is already an integer solution
        if semi_decision::find_marking_equation_integer_solution(
            &self.dense_net,
            &self.marking,
            &idx_target,
        ).is_none() {
            return UnreachabilityProof::MarkingEquationNoIntegerSolution.into();
        }

        match self.build_coverability_graph().into_reachability_graph() {
            Ok(rg) => {
                // todo: pass IdxMarking
                rg.find_path_from_initial(target).map_or_else(
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
    pub fn analyze_coverability(&self, target: Marking<u32>) -> CoverabilityResult {
        let target_idx_marking = self.mapping.idx_marking(target.clone());

        if self.marking >= target_idx_marking {
            return CoverabilityProof {
                firing_sequence: Box::new([]),
                covering_marking: self.mapping.marking(IdxOmegaMarking::from(self.marking.clone())),
            }.into();
        }

        if semi_decision::find_covering_equation_rational_solution(
            &self.dense_net,
            &self.marking,
            &target_idx_marking
        ).is_none() {
            return NonCoverabilityProof::MarkingEquationNoRationalSolution.into();
        }

        // todo: only test ILP if the rational solution is not already an integer solution
        if semi_decision::find_covering_equation_integer_solution(
            &self.dense_net,
            &self.marking,
            &target_idx_marking
        ).is_none() {
            return NonCoverabilityProof::MarkingEquationNoIntegerSolution.into();
        }

        // todo: backwards coverability
        let mut explorer = self.explore_coverability(ExplorationOrder::BreadthFirst);
        explorer
            .find_cover(OmegaMarking::from(target))
            .map_or_else(
                || NonCoverabilityProof::ExhaustiveSearch.into(),
                |cover| {
                    let firing_sequence = explorer.find_path_from_initial(cover.clone()).unwrap();
                    CoverabilityProof {
                        firing_sequence,
                        covering_marking: cover,
                    }.into()
                }
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
        self.analyze_liveness().global_level().is_live()
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
    pub fn is_reachable(&self, target: Marking<u32>) -> bool {
        self.analyze_reachability(target).is_reachable()
    }

    /// Whether `target` is coverable from the initial marking.
    ///
    /// Delegates to [`analyze_coverability`](Self::analyze_coverability).
    pub fn is_coverable(&self, target: Marking<u32>) -> bool {
        self.analyze_coverability(target).is_coverable()
    }
}

/// Error returned when attempting to fire a transition that is not enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotEnabled(Transition);

impl fmt::Display for NotEnabled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotEnabled(_) => write!(f, "transition is not enabled"),
        }
    }
}

impl std::error::Error for NotEnabled {}

#[cfg(feature = "pnml")]
mod pnml {
    use crate::pnml::convert::PnmlConversionError;
    use crate::pnml::PnmlDocument;
    use crate::prelude::{Net, PetriNet};
    use std::error::Error;
    use std::fmt;
    use std::fmt::{Display, Formatter};

    #[derive(Debug, Clone)]
    pub enum FromPnmlError {
        Syntax(quick_xml::DeError),
        Empty,
        Conversion(PnmlConversionError),
    }

    impl Display for FromPnmlError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                FromPnmlError::Syntax(e) => write!(f, "PNML syntax error: {e}"),
                FromPnmlError::Empty => write!(f, "PNML document contains no nets"),
                FromPnmlError::Conversion(e) => write!(f, "PNML conversion error: {e}"),
            }
        }
    }

    impl Error for FromPnmlError {}

    impl PetriNet<Net> {
        /// Parses the first Petri Net (including initial marking) out of a PNML document.
        /// Accepts the PNML content as a string slice.
        ///
        /// # Errors
        ///
        /// Returns an error if the XML failed to parse, if there were no nets in the file,
        /// or if the first petri net in the file is not a PT net, as specified by `net_type`.
        pub fn from_pnml(pnml: &str) -> Result<Self, FromPnmlError> {
            PnmlDocument::from_xml(pnml).map_err(FromPnmlError::Syntax)
                .and_then(|doc| doc.nets.into_iter().next().ok_or(FromPnmlError::Empty))
                .and_then(|net| net.to_pt_system().map_err(FromPnmlError::Conversion))
        }
    }

    impl Net {
        /// Parses the first Net (not including initial marking) out of a PNML document.
        /// Accepts the PNML content as a string slice.
        ///
        /// # Errors
        ///
        /// Returns an error if the XML failed to parse, if there were no nets in the file,
        /// or if the first net in the file is not a PT net, as specified by `net_type`.
        ///
        /// TODO: Allow parsing just the net structure out of any type of PNML
        pub fn from_pnml(pnml: &str) -> Result<Self, FromPnmlError> {
            PetriNet::from_pnml(pnml).map(|system| system.into_parts().0)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::model::{CoverabilityProof, CoverabilityResult, LivenessMethod, NonCoverabilityProof};
    use crate::core::liveness::LivenessLevel;
    use crate::prelude::{Marking, Net, NetBuilder, NetClass, PetriNet, Place, Transition};
    use crate::state_space::Omega;

    /// Builds a simple two-place cycle: p0 -> t0 -> p1 -> t1 -> p0
    fn two_place_cycle() -> (Net, Place, Transition, Place, Transition) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid net");
        (net, p0, t0, p1, t1)
    }

    #[test]
    fn basic_firing() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.current_marking(), [(p0, 1)].into());
        assert!(sys.is_enabled(t0));
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.current_marking(), [(p1, 1)].into());
    }

    #[test]
    fn try_fire_not_enabled() {
        let (net, p0, _t0, _p1, t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.try_fire(t1).is_err());
    }

    #[test]
    fn fire_any_deadlock() {
        let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([]);
        assert!(sys.is_deadlocked());
        assert!(sys.fire_any().is_none());
    }

    #[test]
    fn enabled_transitions_query() {
        let (net, p0, t0, p1, t1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        let enabled = sys.enabled_transitions().collect::<Box<_>>();
        assert!(enabled.contains(&t0));
        assert!(enabled.contains(&t1));
    }

    #[test]
    fn into_parts() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        sys.try_fire(t0).unwrap();
        let (_, _, current) = sys.into_parts();
        assert_eq!(current, Marking::from([(p1, 1)]));
    }

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_bounded());
    }

    #[test]
    fn cycle_is_live() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_live());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
        let sys = net.with_initial_marking([]);
        assert!(!sys.is_live());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, _p0, t0, _p1, t1) = two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = net.with_initial_marking([]);
        let liveness = sys.analyze_liveness();
        assert!(liveness.level(t0).is_dead());
        assert!(liveness.level(t1).is_dead());
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, p0, t0, _p1, t1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        let liveness = sys.analyze_liveness();
        assert_eq!(liveness.level(t0), LivenessLevel::L1);
        assert_eq!(liveness.level(t1), LivenessLevel::L1);
    }

    #[test]
    fn unbounded_not_structurally_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        assert!(!net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(!sys.is_bounded());
    }

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        assert_eq!(net.class(), NetClass::StateMachine);
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_reachable([(p1, 1)].into()));
        assert!(sys.is_reachable([(p0, 1)].into()));
        assert!(!sys.is_reachable([(p0, 2)].into()));
        assert!(!sys.is_reachable([].into()));
    }

    #[test]
    fn t_net_reachability_dispatches() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p1, t0));
        b.add_arcs((t0, p2, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p1));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        assert!(sys.is_reachable([(p2, 1)].into()));
        assert!(sys.is_reachable([(p0, 1), (p1, 1)].into()));
        assert!(!sys.is_reachable([(p1, 1)].into()));
    }

    #[test]
    fn general_net_reachability_fallback() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arcs((p0, t0, p1));
        b.add_arcs((p0, t1, p2));
        b.add_arcs((p1, t2, p0));
        b.add_arcs((p2, t2, p0));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::General);
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_reachable([(p0, 1)].into()));
        assert!(sys.is_reachable([(p1, 1)].into()));
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

    #[test]
    fn coverability_initial_marking_covers() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        let res = sys.analyze_coverability([(p0, 1)].into());
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable(CoverabilityProof { firing_sequence, covering_marking }) => {
                assert_eq!(covering_marking, [(p0, 1.into())].into());
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
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1)]);

        let res = sys.analyze_coverability([(p0, 1), (p1, 1)].into());
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
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        let res = sys.analyze_coverability([(p0, 1), (p1, 10)].into());
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable(CoverabilityProof { covering_marking, .. }) => {
                // p0 stays 1; p1 becomes ω in the coverability graph.
                assert_eq!(covering_marking.get(p0), Omega::Finite(1));
                assert!(covering_marking.get(p1) >= Omega::Finite(100_000));
            }
            _ => panic!("expected coverability-graph proof"),
        }
    }
}
