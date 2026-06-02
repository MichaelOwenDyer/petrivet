use crate::core::analysis::semi_decision;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;

/// Result of reachability analysis.
///
/// Three possible outcomes:
/// - `Reachable`: the target is definitely reachable, with a witness.
/// - `Unreachable`: the target is definitely unreachable, with a proof.
/// - `Inconclusive`: current algorithms could not decide (e.g., unbounded
///   general net where LP/ILP filters pass but full exploration is infinite).
#[derive(Debug, Clone)]
pub enum ReachabilityResult {
    /// The target marking is reachable from M₀.
    Reachable(ReachabilityProof),
    /// The target marking is definitely not reachable from M₀.
    Unreachable(UnreachabilityProof),
    /// Current algorithms could not decide.
    Inconclusive,
}

impl ReachabilityResult {
    /// Whether the target is definitely reachable.
    #[must_use]
    pub const fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable(_))
    }

    /// Whether the target is definitely unreachable.
    #[must_use]
    pub const fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable(_))
    }

    /// Whether the analysis was inconclusive.
    #[must_use]
    pub const fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive)
    }
}

/// A transition firing sequence.
pub type FiringSequence = Box<[Transition]>;

#[derive(Debug, Clone)]
pub enum ReachabilityProof {
    StronglyConnectedStateMachine {
        marking_sum: u32,
    },
    StateMachineMarkingEquationRationalSolution(Box<[(Transition, f64)]>),
    MarkedGraphMarkingEquationIntegerSolution(Box<[(Transition, u32)]>),
    FiringSequence(Box<[Transition]>),
}

impl ReachabilityProof {
    /// If this is a `FiringSequence` proof, returns the sequence.
    /// Returns `None` for structural proofs.
    #[must_use]
    pub fn firing_sequence(&self) -> Option<&[Transition]> {
        match self {
            Self::FiringSequence(seq) => Some(seq),
            _ => None,
        }
    }
}

/// Proof that a marking is unreachable.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UnreachabilityProof {
    /// The net is an S-net and the target marking has a different
    /// token sum than the initial marking.
    StateMachineTokenConservation,
    /// The LP marking equation (rational relaxation) is infeasible.
    /// Some S-invariant is violated.
    MarkingEquationNoRationalSolution,
    /// The ILP marking equation (integer) is infeasible.
    /// Stronger than LP: no integer firing count vector exists.
    MarkingEquationNoIntegerSolution,
    /// Full state space explored; target not found.
    ExhaustiveSearch,
}

impl From<ReachabilityProof> for ReachabilityResult {
    fn from(value: ReachabilityProof) -> Self {
        ReachabilityResult::Reachable(value)
    }
}

impl From<UnreachabilityProof> for ReachabilityResult {
    fn from(value: UnreachabilityProof) -> Self {
        ReachabilityResult::Unreachable(value)
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether `target` is reachable from the initial marking.
    ///
    /// Delegates to [`analyze_reachability`](Self::analyze_reachability).
    /// Returns `false` for inconclusive results.
    #[must_use]
    pub fn is_reachable(&self, target: Marking<u32>) -> bool {
        self.analyze_reachability(target).is_reachable()
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
                    ReachabilityProof::StronglyConnectedStateMachine {
                        marking_sum: initial_marking_sum,
                    }.into()
                } else {
                    UnreachabilityProof::StateMachineTokenConservation.into()
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
                    ReachabilityProof::StateMachineMarkingEquationRationalSolution(solution).into()
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
                    ReachabilityProof::MarkedGraphMarkingEquationIntegerSolution(solution).into()
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

        match self.try_build_reachability_graph() {
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
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::class::NetClass;

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, p0, _t0, p1, _t1) = crate::api::system::tests::two_place_cycle();
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
}