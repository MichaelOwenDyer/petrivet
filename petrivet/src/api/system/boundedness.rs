use crate::core::analysis::semi_decision;
use crate::model::{BoundednessAnalysis, BoundednessAnalysisMethod};
use crate::net::Net;
use crate::prelude::PetriNet;
use crate::state_space::{ExplorationOrder, Omega};

impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether the system is bounded (all places have finite token counts
    /// across all reachable markings).
    ///
    /// Delegates to [`analyze_boundedness`](Self::analyze_boundedness).
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.analyze_boundedness().system_bound().is_finite()
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

    /// Returns true if some reachable marking puts more than one token in any place.
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .find(|m| m.iter().any(|&t| t > 1))
            .is_some()
    }
}