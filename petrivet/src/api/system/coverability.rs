use crate::core::analysis::semi_decision;
use crate::core::state_space::coverability::IdxOmegaMarking;
use crate::marking::Marking;
use crate::model::{CoverabilityProof, CoverabilityResult, NonCoverabilityProof};
use crate::net::Net;
use crate::prelude::PetriNet;
use crate::state_space::{ExplorationOrder, OmegaMarking};

// coverability
impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether `target` is coverable from the initial marking.
    ///
    /// Delegates to [`analyze_coverability`](Self::analyze_coverability).
    pub fn is_coverable(&self, target: Marking<u32>) -> bool {
        self.analyze_coverability(target).is_coverable()
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
}