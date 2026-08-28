use crate::core::net::IdxNode;
use crate::net::class::NetClass;
use crate::net::{Net, Place};
use crate::state_space::ExplorationOrder;
use crate::system::PetriNet;
use ahash::HashMap;

/// Boundedness describes the maximum number of tokens that can
/// appear on a place in any reachable marking of a Petri net.
///
/// It is a fundamental property of Petri nets that has significant
/// implications for their behavior and the complexity of analyzing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Boundedness {
    /// A [`Place`] is bounded in a given [`PetriNet`] if there exists
    /// an upper limit on the number of tokens it will ever hold in any
    /// reachable marking in the Petri net's state space.
    ///
    /// A place is *k-bounded* if it is bounded and the upper limit on
    /// the number of tokens it can hold is less than or equal to `k`.
    ///
    /// A [`PetriNet`] is (k-)bounded if all of its places are (k-)bounded.
    ///
    /// Note that `k` might be a loose upper bound: `k` might not
    /// be the smallest integer for which the place is k-bounded,
    /// depending on the method used to determine it.
    Bounded(K),

    /// *Unboundedness* describes the situation where there is
    /// no upper limit on the number of tokens that can appear
    /// on a [Place](crate::net::Place) in a reachable marking
    /// of a [Petri net](crate::system::PetriNet).
    ///
    /// It occurs if and only if a transition firing sequence
    /// turns some marking `M` into a different marking
    /// `M'` where `M'` has at least as many tokens in all
    /// places as `M` and strictly more in at least one.
    ///
    /// Those places with strictly more tokens are unbounded:
    /// we can fire the same sequence of transitions from `M'`
    /// to reach a marking `M''`, which will have even more
    /// tokens on those places, and the firing sequence will
    /// again be enabled in `M''`. The firing sequence will
    /// always be enabled due to the property of Petri nets
    /// known as *monotonicity*. We can therefore repeat the
    /// process indefinitely, generating markings with
    /// arbitrarily many tokens on any unbounded place.
    ///
    /// A Petri net is unbounded if it has one or more unbounded
    /// places. Unbounded Petri nets have an infinite state space,
    /// and are generally more difficult to analyze than bounded ones.
    Unbounded,
}

/// A type alias for the upper bound `k` in k-boundedness.
pub type K = usize;

impl Boundedness {
    /// Returns `true` if the place or Petri net is bounded.
    #[must_use]
    pub const fn is_bounded(&self) -> bool {
        matches!(self, &Boundedness::Bounded(_))
    }

    /// Returns `true` if the place or Petri net is known to be k-bounded
    /// for the given `k`. If we know that it is not k-bounded, or we
    /// cannot guarantee it to be k-bounded (due to loose bounds),
    /// this returns `false`.
    #[must_use]
    pub const fn is_k_bounded(&self, k: K) -> bool {
        matches!(self, &Boundedness::Bounded(bound) if bound <= k)
    }

    /// Returns `true` if the place or Petri net is safe (1-bounded).
    #[must_use]
    pub const fn is_safe(&self) -> bool {
        self.is_k_bounded(1)
    }

    /// Returns `true` if the place or Petri net is unbounded.
    #[must_use]
    pub const fn is_unbounded(&self) -> bool {
        matches!(self, Boundedness::Unbounded)
    }
}

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
            // NetClass::FreeChoice if self.is_live() => Some(self.is_covered_by_s_components()),
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
    use crate::net::builder::NetBuilder;
    use crate::system::boundedness::Boundedness;

    #[test]
    fn boundedness_ord() {
        assert!(Boundedness::Bounded(0) < Boundedness::Bounded(1));
        assert!(Boundedness::Bounded(usize::MAX) < Boundedness::Unbounded);
    }

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
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
