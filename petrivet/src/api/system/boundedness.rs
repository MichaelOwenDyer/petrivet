use crate::boundedness::{Boundedness, K};
use crate::core::analysis::semi_decision;
use crate::net::{Net, Place};
use crate::prelude::PetriNet;
use crate::state_space::ExplorationOrder;

/// Result of boundedness analysis.
///
/// `place_bound` returns the bound for any place in the net by its key.
/// When proved via the structural LP, bounds are derived upper estimates
/// (potentially loose). When proved via the coverability graph, bounds are exact.
#[derive(Debug, Clone)]
pub struct BoundednessAnalysis {
    /// All places in the net paired with their bounds. The order is not guaranteed.
    pub bounds: Box<[(Place, Boundedness)]>,
    /// How the result was obtained.
    pub method: BoundednessAnalysisMethod,
}

impl BoundednessAnalysis {
    /// Returns the bound of the system as a whole: the maximum over all places.
    #[must_use]
    pub fn system_bound(&self) -> Boundedness {
        self.bounds.iter()
            .map(|(_, b)| *b)
            .max()
            .expect("at least one place")
    }

    /// Returns the bound for a specific place identified by its [`Place`].
    ///
    /// Returns `None` if the place does not belong to the analysed net.
    #[must_use]
    pub fn place_bound(&self, place: Place) -> Boundedness {
        self.bounds.iter()
            .find(|(p, _)| *p == place)
            .map_or(Boundedness::Bounded(Some(0)), |(_, b)| *b)
    }
}

/// Evidence for a boundedness result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BoundednessAnalysisMethod {
    /// Structural LP found a positive vector y with yᵀN ≤ 0.
    /// Bounds are derived as M\[p\] ≤ ⌊(y·M₀) / y\[p\]⌋: valid but
    /// potentially loose.
    PositivePlaceSubvariant(Box<[f64]>),
    /// Full coverability graph explored. Bounds are exact.
    CoverabilityGraph,
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns true if the entire system is bounded.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.net.as_ref().is_structurally_bounded()
            || self.analyze_boundedness().system_bound().is_bounded()
    }

    /// Returns true if the entire system is `k`-bounded for the given `k`.
    #[must_use]
    pub fn is_k_bounded(&self, k: K) -> bool {
        self.is_structurally_k_bounded(k)
            || self.analyze_boundedness().system_bound().is_k_bounded(k)
    }

    /// Returns true if the entire system is safe (1-bounded).
    pub fn is_safe(&self) -> bool {
        self.is_k_bounded(1)
    }

    /// Returns true if the net is *structurally* k-bounded for the given `k`.
    /// This is simultaneously a structural check and one which depends on the
    /// initial marking; we investigate whether the structure of the net prevents
    /// unbounded places, and if so, whether the initial marking allows for a bound
    /// of `k` to be derived on all places.
    pub fn is_structurally_k_bounded(&self, k: K) -> bool {
        use crate::core::analysis::semi_decision::find_positive_place_subvariant;
        let Some(weights) = find_positive_place_subvariant(&self.dense_net) else {
            return false;
        };
        let weighted_sum: f64 = weights.iter()
            .zip(self.reset.iter())
            .map(|(&w, &m)| w * f64::from(m))
            .sum();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        weights.iter().all(|&w| (weighted_sum / w).floor() as usize <= k)
    }

    #[must_use]
    pub fn is_place_structurally_k_bounded(&self, place: &Place, k: K) -> bool {
        todo!()
    }

    /// Returns true if some reachable marking puts more than one token in any place.
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .find(|m| m.iter().any(|&t| t > 1))
            .is_some()
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
                    let bound = (weighted_sum / weight).floor() as usize;
                    (place, Boundedness::Bounded(Some(bound)))
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
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, p0, _t0, _p1, _t1) = crate::api::system::tests::two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_bounded());
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
}