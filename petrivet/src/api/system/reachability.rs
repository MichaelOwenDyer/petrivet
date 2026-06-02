use crate::core::analysis::semi_decision;
use crate::marking::Marking;
use crate::model::{ReachabilityProof, ReachabilityResult, UnreachabilityProof};
use crate::net::Net;
use crate::prelude::PetriNet;

// reachability
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