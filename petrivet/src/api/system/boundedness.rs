use crate::core::net::idx_set::PlaceIdxSet;
use crate::core::net::structural_boundedness::find_place_subinvariant;
use crate::core::system::boundedness::{BoundsCache, Effort, Goal, Scope};
use crate::net::class::NetClass;
use crate::net::{Net, Place};
use crate::state_space::ExplorationOrder;
use crate::system::PetriNet;

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
    Bounded(K),

    /// *Unboundedness* describes the situation where there is
    /// no upper limit on the number of tokens that can appear
    /// on a [Place](Place) in a reachable marking
    /// of a [Petri net](PetriNet).
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

/// Evidence for a boundedness result.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum BoundednessAnalysisMethod {
    /// This bound was derived from the initial marking.
    InitialMarking,
    /// In a strongly connected state machine, the sum of tokens is conserved,
    /// and every token can reach every place independently.
    StronglyConnectedStateMachine,
    /// In a state machine, the sum of tokens is conserved,
    /// but some tokens may not be able to reach every place independently.
    StateMachine,
    /// In a marked graph, the sum of tokens in a circuit is conserved.
    MarkedGraph,
    /// Upper bound derived from the structure of the net using a linear programming formulation.
    StructuralLP,
    /// Full coverability graph explored.
    CoverabilityGraph,
}

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

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns true if the Petri net is bounded, and false if it is unbounded.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        let scope = Scope::PetriNet;
        self.with_bounds(|cache| {
            self.refine(cache, scope, Goal::Bounded, Effort::Any);
            cache.read_bounded(scope)
        }).expect("Effort::Any always decides")
    }

    /// Returns true if the given place is bounded, and false if it is unbounded.
    #[must_use]
    pub fn is_place_bounded(&self, place: Place) -> bool {
        self.mapping.place_idx(place).is_none_or(|p_idx| {
            self.with_bounds(|cache| {
                let scope = Scope::Place(p_idx);
                self.refine(cache, scope, Goal::Bounded, Effort::Any);
                cache.read_bounded(scope)
            }).expect("Effort::Any always decides")
        })
    }

    /// Returns Some(true) if the Petri net is bounded,
    /// Some(false) if it is not bounded,
    /// and None if the answer cannot be efficiently computed.
    #[must_use]
    pub fn is_efficiently_bounded(&self) -> Option<bool> {
        self.with_bounds(|cache| {
            let scope = Scope::PetriNet;
            self.refine(cache, scope, Goal::Bounded, Effort::Efficient);
            cache.read_bounded(scope)
        })
    }

    /// Returns Some(true) if the given place is bounded,
    /// Some(false) if it is not bounded,
    /// and None if the answer cannot be efficiently computed.
    #[must_use]
    pub fn is_place_efficiently_bounded(&self, place: Place) -> Option<bool> {
        self.mapping.place_idx(place).map_or(Some(true), |p_idx| {
            self.with_bounds(|cache| {
                let scope = Scope::Place(p_idx);
                self.refine(cache, scope, Goal::Bounded, Effort::Efficient);
                cache.read_bounded(scope)
            })
        })
    }

    /// Returns true if the Petri net is k-bounded for the given k,
    /// and false if it is unbounded or bounded with a bound greater than k.
    #[must_use]
    pub fn is_k_bounded(&self, k: K) -> bool {
        self.with_bounds(|cache| {
            let scope = Scope::PetriNet;
            self.refine(cache, scope, Goal::KBounded(k), Effort::Any);
            cache.read_k_bounded(scope, k)
        }).expect("Effort::Any always decides")
    }

    /// Returns true if the given place is k-bounded for the given k,
    /// and false if it is unbounded or bounded with a bound greater than k.
    #[must_use]
    pub fn is_place_k_bounded(&self, place: Place, k: K) -> bool {
        self.mapping.place_idx(place).is_none_or(|p_idx| {
            self.with_bounds(|cache| {
                let scope = Scope::Place(p_idx);
                self.refine(cache, scope, Goal::KBounded(k), Effort::Any);
                cache.read_k_bounded(scope, k)
            }).expect("Effort::Any always decides")
        })
    }

    /// Returns Some(true) if the Petri net is k-bounded for the given k,
    /// Some(false) if it is unbounded or bounded with a bound greater than k,
    /// and None if the answer cannot be efficiently computed.
    #[must_use]
    pub fn is_efficiently_k_bounded(&self, k: K) -> Option<bool> {
        self.with_bounds(|cache| {
            let scope = Scope::PetriNet;
            self.refine(cache, scope, Goal::KBounded(k), Effort::Efficient);
            cache.read_k_bounded(scope, k)
        })
    }

    /// Returns Some(true) if the given place is k-bounded for the given k,
    /// Some(false) if it is unbounded or bounded with a bound greater than k,
    /// and None if the answer cannot be efficiently computed.
    #[must_use]
    pub fn is_place_efficiently_k_bounded(&self, place: Place, k: K) -> Option<bool> {
        self.mapping.place_idx(place).map_or(Some(true), |p_idx| {
            self.with_bounds(|cache| {
                let scope = Scope::Place(p_idx);
                self.refine(cache, scope, Goal::KBounded(k), Effort::Efficient);
                cache.read_k_bounded(scope, k)
            })
        })
    }

    /// Returns the exact boundedness of the Petri net.
    pub fn boundedness(&self) -> Boundedness {
        self.with_bounds(|cache| {
            let scope = Scope::PetriNet;
            self.refine(cache, scope, Goal::Exact, Effort::Any);
            cache.read_exact(scope)
        }).expect("Effort::Any always decides")
    }

    /// Returns the exact boundedness of the given place.
    pub fn place_boundedness(&self, place: Place) -> Boundedness {
        self.mapping.place_idx(place).map_or(Boundedness::Bounded(0), |p_idx| {
            self.with_bounds(|cache| {
                let scope = Scope::Place(p_idx);
                self.refine(cache, scope, Goal::Exact, Effort::Any);
                cache.read_exact(scope)
            }).expect("Effort::Any always decides")
        })
    }

    /// Returns the exact boundedness of the Petri net, if it can be efficiently computed.
    #[must_use]
    pub fn efficient_boundedness(&self) -> Option<Boundedness> {
        self.with_bounds(|cache| {
            let scope = Scope::PetriNet;
            self.refine(cache, scope, Goal::Exact, Effort::Efficient);
            cache.read_exact(scope)
        })
    }

    /// Returns the exact boundedness of the given place, if it can be efficiently computed.
    pub fn efficient_place_boundedness(&self, place: Place) -> Option<Boundedness> {
        self.mapping.place_idx(place).map_or(Some(Boundedness::Bounded(0)), |p_idx| {
            self.with_bounds(|cache| {
                let scope = Scope::Place(p_idx);
                self.refine(cache, scope, Goal::Exact, Effort::Efficient);
                cache.read_exact(scope)
            })
        })
    }

    /// Returns true if some reachable marking puts more than one token in any place.
    #[deprecated(note = "Use `is_k_bounded(1)` instead.")]
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability_graph(ExplorationOrder::BreadthFirst)
            .core
            .search(|m| m.iter().any(|&t| t > 1))
            .is_some()
    }

    /// Adds new information to the bounds cache for the given scope and goal,
    /// using the specified effort level. If the goal has already been answered
    /// for the given scope, this method does nothing.
    fn refine(&self, cache: &mut BoundsCache, scope: Scope, goal: Goal, effort: Effort) {
        if cache.is_answered(scope, goal) {
            return;
        }
        self.refine_efficient_subclasses(scope, cache);
        if cache.is_answered(scope, goal) {
            return;
        }
        self.refine_structural_lp(scope, cache);
        if cache.is_answered(scope, goal) {
            return;
        }
        if matches!(effort, Effort::Efficient) {
            return;
        }
        self.refine_via_coverability_graph(cache);
    }

    /// Refines the bounds for subclasses of Petri nets that have efficient
    /// (polynomial-time) procedures for boundedness.
    fn refine_efficient_subclasses(&self, scope: Scope, cache: &mut BoundsCache) {
        let places = scope
            .places(self.dense_net.place_count())
            .intersection(&cache.should_refine_by_subclass);
        if places.is_empty() {
            return;
        }
        match self.class() {
            NetClass::Circuit | NetClass::StateMachine if self.is_strongly_connected() => {
                let marking_sum = Boundedness::Bounded(self.marking.sum() as K);
                for bounds in &mut cache.by_place.iter_mut() {
                    bounds.update_exact(marking_sum, BoundednessAnalysisMethod::StronglyConnectedStateMachine);
                }
                cache.should_refine_by_subclass.set_all(false);
            }
            NetClass::StateMachine => {
                for p_idx in places {
                    let mut place = PlaceIdxSet::none_of(self.dense_net.place_count());
                    place.add(p_idx);
                    let marking_sum = self.dense_net
                        .pre_star(place)
                        .iter()
                        .map(|p_idx| self.marking[p_idx])
                        .sum::<u32>() as K;
                    let bound = Boundedness::Bounded(marking_sum);
                    cache.by_place[p_idx].update_exact(bound, BoundednessAnalysisMethod::StateMachine);
                    cache.should_refine_by_subclass.remove(p_idx);
                }
            }
            NetClass::MarkedGraph if self.is_live().unwrap() => {
                for p_idx in places {
                    let minimum_tokens = self.dense_net.minimum_tokens_in_circuit_containing(p_idx, &self.marking);
                    let bound = minimum_tokens.map_or(
                        Boundedness::Unbounded,
                        |token_sum| Boundedness::Bounded(token_sum as K)
                    );
                    cache.by_place[p_idx].update_exact(
                        bound,
                        BoundednessAnalysisMethod::MarkedGraph
                    );
                    cache.should_refine_by_subclass.remove(p_idx);
                }
            }
            NetClass::MarkedGraph => {
                for p_idx in places {
                    let minimum_tokens = self.dense_net.minimum_tokens_in_circuit_containing(p_idx, &self.marking);
                    let bound = minimum_tokens.map_or(
                        Boundedness::Unbounded,
                        |token_sum| Boundedness::Bounded(token_sum as K)
                    );
                    cache.by_place[p_idx].update_upper(
                        bound,
                        BoundednessAnalysisMethod::MarkedGraph
                    );
                    cache.should_refine_by_subclass.remove(p_idx);
                }
            }
            _ => {}
        }
    }

    /// Refines the bounds for the given scope by searching for a place subinvariant
    /// covering the given places.
    fn refine_structural_lp(&self, scope: Scope, cache: &mut BoundsCache) {
        let places = scope
            .places(self.dense_net.place_count())
            .intersection(&cache.should_refine_by_structural_lp);
        if places.is_empty() {
            return;
        }
        let Some(place_subinvariant) = find_place_subinvariant(
            &self.dense_net,
            |&p| places.contains(p)
        ) else {
            cache.should_refine_by_structural_lp.remove_all(&places);
            return;
        };
        let weighted_sum_m0 = self
            .dense_net
            .place_indices()
            .zip(place_subinvariant.iter())
            .map(|(p_idx, weight)| *weight * u64::from(self.marking[p_idx]))
            .sum::<u64>();
        self.dense_net
            .place_indices()
            .zip(place_subinvariant)
            .for_each(|(p_idx, weight)| {
                if let Some(upper_bound) = weighted_sum_m0.checked_div(weight) {
                    cache.by_place[p_idx].update_upper(
                        Boundedness::Bounded(upper_bound as K),
                        BoundednessAnalysisMethod::StructuralLP,
                    );
                    cache.should_refine_by_structural_lp.remove(p_idx);
                }
            });
    }

    /// Refines the bounds for the entire Petri net using the coverability graph.
    fn refine_via_coverability_graph(&self, bounds: &mut BoundsCache) {
        let place_bounds = self.build_coverability_graph().place_bounds();
        // todo: avoid mapping place to place_idx twice
        for (place, bound) in place_bounds {
            if let Some(p_idx) = self.mapping.place_idx(place) {
                bounds.by_place[p_idx].update_exact(
                    bound,
                    BoundednessAnalysisMethod::CoverabilityGraph
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::system::boundedness::Boundedness;

    #[test]
    fn boundedness_ord() {
        assert!(Boundedness::Bounded(0) < Boundedness::Bounded(1));
        assert!(Boundedness::Bounded(usize::MAX) < Boundedness::Unbounded);
    }
}
