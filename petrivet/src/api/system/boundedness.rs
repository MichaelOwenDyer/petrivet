use crate::boundedness::{Boundedness, K};
use crate::class::NetClass;
use crate::core::net::IdxNode;
use crate::net::{Net, Place};
use crate::prelude::PetriNet;
use crate::state_space::ExplorationOrder;
use ahash::HashMap;

/// Result of boundedness analysis.
///
/// `place_bound` returns the bound for any place in the net by its key.
/// When proved via the structural LP, bounds are derived upper estimates
/// (potentially loose). When proved via the coverability graph, bounds are exact.
#[derive(Debug, Clone)]
pub struct BoundednessAnalysis {
    /// All places in the net paired with their bounds. The order is not guaranteed.
    pub bounds: HashMap<Place, Boundedness>,
    /// How the result was obtained.
    pub method: BoundednessAnalysisMethod,
}

impl BoundednessAnalysis {
    /// Returns the bound of the system as a whole: the maximum over all places.
    #[must_use]
    pub fn global_bound(&self) -> Boundedness {
        self.bounds
            .values()
            .copied()
            .max()
            .unwrap_or(Boundedness::Bounded(0))
    }

    /// Returns the bound for a specific place identified by its [`Place`].
    ///
    /// Returns `None` if the place does not belong to the analysed net.
    #[must_use]
    pub fn place_bound(&self, place: Place) -> Boundedness {
        self.bounds
            .get(&place)
            .map_or(Boundedness::Bounded(0), |&bound| bound)
    }
}

/// Evidence for a boundedness result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BoundednessAnalysisMethod {
    /// Structural LP found a positive vector y with yᵀN ≤ 0.
    /// Bounds are derived as M\[p\] ≤ ⌊(y·M₀) / y\[p\]⌋: valid but
    /// potentially loose.
    PositivePlaceSubinvariant(Box<[f64]>),
    /// Full coverability graph explored. Bounds are exact.
    CoverabilityGraph,
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If an efficient (polynomial-time) procedure for boundedness
    /// is known for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_bounded(&self) -> Option<bool> {
        match self.class() {
            // conservative - token count never changes
            NetClass::Circuit | NetClass::StateMachine => Some(true),
            // A marked graph is bounded iff it is strongly connected
            NetClass::MarkedGraph => Some(self.is_strongly_connected()),
            // A live free-choice system is bounded iff every place belongs to an s-component
            NetClass::FreeChoice if self.is_live() => Some(self.is_covered_by_s_components()),
            _ => None,
        }
    }

    pub fn is_place_efficiently_bounded(&self, place: Place) -> Option<bool> {
        match self.class() {
            // conservative - token count never changes
            NetClass::Circuit | NetClass::StateMachine => Some(true),
            // A place in a marked graph is bounded iff it belongs to some circuit
            NetClass::MarkedGraph => {
                // todo: optimize by only constructing circuits which contain the place
                self.mapping.place_idx(place).map_or(Some(true), |p_idx| {
                    Some(
                        self.circuits()
                            .any(|circuit| circuit.contains(&IdxNode::Place(p_idx))),
                    )
                })
            }
            _ => None,
        }
    }

    /// Returns the boundedness of the entire system, if it can be efficiently computed.
    #[must_use]
    pub fn efficient_boundedness(&self) -> Option<Boundedness> {
        match self.class() {
            NetClass::Circuit => Some(Boundedness::Bounded(self.marking.sum() as K)),
            NetClass::StateMachine => {
                if self.is_strongly_connected() {
                    Some(Boundedness::Bounded(self.marking.sum() as K))
                } else {
                    None // todo
                }
            }
            NetClass::MarkedGraph => {
                if self.is_strongly_connected() {
                    None // todo
                } else {
                    Some(Boundedness::Unbounded)
                }
            }
            _ => None,
        }
    }

    pub fn efficient_place_boundedness(&self, place: Place) -> Option<Boundedness> {
        match self.class() {
            NetClass::Circuit => Some(Boundedness::Bounded(self.marking.sum() as K)),
            NetClass::StateMachine => {
                if self.is_strongly_connected() {
                    Some(Boundedness::Bounded(self.marking.sum() as K))
                } else {
                    None
                }
            }
            NetClass::MarkedGraph => {
                self.mapping
                    .place_idx(place)
                    .map_or(Some(Boundedness::Bounded(0)), |p_idx| {
                        Some(
                            self.circuits()
                                .filter(|circuit| circuit.contains(&IdxNode::Place(p_idx)))
                                .map(|circuit| {
                                    circuit
                                        .place_indices()
                                        .map(|p_idx| self.marking[p_idx])
                                        .sum::<u32>() as K
                                })
                                .min()
                                .map_or(Boundedness::Unbounded, Boundedness::Bounded),
                        )
                    })
            }
            _ => None,
        }
    }

    /// Returns true if the entire system is bounded.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.is_efficiently_bounded().unwrap_or_else(|| {
            self.net.as_ref().is_structurally_bounded()
                || self
                    .boundedness_via_coverability_graph()
                    .global_bound()
                    .is_bounded()
        })
    }

    /// Returns true iff the given place is bounded.
    pub fn is_place_bounded(&self, place: Place) -> bool {
        self.is_place_efficiently_bounded(place).unwrap_or_else(|| {
            self.net.as_ref().is_place_structurally_bounded(place)
                || self
                    .boundedness_via_coverability_graph()
                    .place_bound(place)
                    .is_bounded()
        })
    }

    /// Returns the boundedness of the entire system.
    pub fn boundedness(&self) -> Boundedness {
        self.efficient_boundedness()
            .unwrap_or_else(|| self.boundedness_via_coverability_graph().global_bound())
    }

    pub fn place_boundedness(&self, place: Place) -> Boundedness {
        self.efficient_place_boundedness(place)
            .unwrap_or_else(|| self.boundedness_via_coverability_graph().place_bound(place))
    }

    /// Returns true if some reachable marking puts more than one token in any place.
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .find(|m| m.iter().any(|&t| t > 1))
            .is_some()
    }

    /// Analyzes boundedness and returns per-place bounds with evidence.
    #[must_use]
    fn boundedness_via_coverability_graph(&self) -> BoundednessAnalysis {
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
