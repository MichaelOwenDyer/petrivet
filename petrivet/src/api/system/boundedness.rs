use crate::core::net::idx_set::PlaceIdxSet;
use crate::core::net::structural_boundedness::find_place_subinvariant;
use crate::core::system::boundedness::{BoundednessQuery, BoundednessScope, BoundsCache};
use crate::core::system::marking::IdxMarking;
use crate::net::class::NetClass;
use crate::net::{Net, Place};
use crate::state_space::ExplorationOrder;
use crate::system::PetriNet;
use bon::bon;

/// A type alias for the upper bound `k` in k-boundedness.
pub type K = u32;

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

/// Evidence for a boundedness result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// k-bounded reachability graph explored.
    ReachabilityGraph,
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

#[derive(Debug, Clone, Copy)]
pub enum BoundednessEffort {
    /// Only use efficient (polynomial-time) procedures.
    Efficient,
    /// Use any available procedure.
    Any,
}

#[bon]
impl<N: AsRef<Net>> PetriNet<N> {
    /// Attempts to decide (k-)boundedness.
    /// If a place is specified, the result is for that place; otherwise, it is for the entire Petri net.
    /// If `k` is specified, the result is for k-boundedness; otherwise, it is for general boundedness.
    /// If `effort` is specified, it controls the algorithmic complexity of the procedures used to
    /// refine the bounds; otherwise, any effort level is allowed.
    /// If `timeout` is specified, the method will give up after the specified duration and return
    /// `None` if no conclusion could be reached.
    #[must_use]
    #[builder]
    pub fn is_bounded(
        &self,
        place: Option<Place>,
        k: Option<K>,
        effort: Option<BoundednessEffort>,
        timeout: Option<std::time::Duration>
    ) -> Option<bool> {
        let scope = match place {
            None => BoundednessScope::PetriNet,
            Some(p) if let Some(p_idx) = self.mapping.place_idx(p) => BoundednessScope::Place(p_idx),
            Some(_) => return None,
        };
        let goal = k.map_or(BoundednessQuery::Bounded, BoundednessQuery::KBounded);
        let effort = effort.unwrap_or(BoundednessEffort::Any);
        let deadline = timeout.and_then(|t| std::time::Instant::now().checked_add(t));
        self.with_boundedness_cache(|cache| {
            self.refine_bounds(cache, goal, scope, effort, deadline);
            k.map_or_else(
                || cache.read_bounded(scope),
                |k| cache.read_k_bounded(scope, k)
            )
        })
    }

    /// Attempts to compute the exact k-bound.
    /// If a place is specified, the result is for that place; otherwise, it is for the entire Petri net.
    /// If `effort` is specified, it controls the algorithmic complexity of the procedures used to refine the bounds; otherwise, any effort level is allowed.
    /// If `timeout` is specified, the method will give up after the specified duration and return `None` if no conclusion could be reached.
    #[must_use]
    #[builder]
    pub fn boundedness(
        &self,
        place: Option<Place>,
        effort: Option<BoundednessEffort>,
        timeout: Option<std::time::Duration>,
    ) -> Option<Boundedness> {
        let scope = match place {
            None => BoundednessScope::PetriNet,
            Some(p) if let Some(p_idx) = self.mapping.place_idx(p) => BoundednessScope::Place(p_idx),
            Some(_) => return None,
        };
        let effort = effort.unwrap_or(BoundednessEffort::Any);
        let deadline = timeout.and_then(|t| std::time::Instant::now().checked_add(t));
        self.with_boundedness_cache(|cache| {
            self.refine_bounds(cache, BoundednessQuery::Exact, scope, effort, deadline);
            cache.read_exact(scope)
        })
    }

    /// Returns true if some reachable marking puts more than one token in any place.
    #[deprecated(note = "Use `is_k_bounded(1)` instead.")]
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability_graph(ExplorationOrder::BreadthFirst)
            .core
            .search(|m| m.iter().any(|&t| t > 1), None)
            .is_some()
    }

    /// Adds new information to the bounds cache for the given scope and goal,
    /// using the specified effort level. If the goal has already been answered
    /// for the given scope, this method does nothing.
    fn refine_bounds(
        &self,
        cache: &mut BoundsCache,
        query: BoundednessQuery,
        scope: BoundednessScope,
        effort: BoundednessEffort,
        deadline: Option<std::time::Instant>,
    ) {
        if cache.is_answered(scope, query) {
            return;
        }
        self.refine_efficient_subclasses(scope, cache);
        let is_deadline_expired = || deadline.is_some_and(|d| std::time::Instant::now() >= d);
        if is_deadline_expired() || cache.is_answered(scope, query) {
            return;
        }
        self.refine_structural_lp(scope, cache);
        if is_deadline_expired() || cache.is_answered(scope, query) {
            return;
        }
        if matches!(effort, BoundednessEffort::Efficient) {
            return;
        }
        match query {
            BoundednessQuery::KBounded(k) => {
                let mut rg = self.explore_coverability_graph(ExplorationOrder::BreadthFirst);
                rg.core.find(|m| m.iter().any(|&t| t > k), deadline);
                let max = rg.core.state_space.markings().fold(IdxMarking::zeros(self.place_count()), IdxMarking::componentwise_max);
                cache.update_lower_bound(&max, BoundednessAnalysisMethod::CoverabilityGraph);
                if rg.is_fully_explored() {
                    cache.make_all_exact();
                }
            },
            BoundednessQuery::Bounded => {
                let explorer = self.explore_coverability_graph(ExplorationOrder::BreadthFirst);
                match explorer.try_build_reachability_graph_until(deadline) {
                    Ok(rg) => {
                        let max = rg.state_space.markings().fold(IdxMarking::zeros(self.place_count()), IdxMarking::componentwise_max);
                        cache.update_exact(&max.into(), BoundednessAnalysisMethod::ReachabilityGraph);
                    }
                    Err(stopped) => {
                        // Unbounded, or out of time: either way every explored marking is reachable
                        // (or covered by a reachable one), so it is a valid lower bound.
                        for marking in stopped.explorer.core.state_space.markings() {
                            cache.update_lower_bound(marking, BoundednessAnalysisMethod::CoverabilityGraph);
                        }
                    }
                }
            },
            BoundednessQuery::Exact => {
                let mut cg = self.explore_coverability_graph(ExplorationOrder::BreadthFirst);
                cg.core.find(|_| false, deadline);
                let max = cg.core.state_space.markings().fold(IdxMarking::zeros(self.place_count()), IdxMarking::componentwise_max);
                if cg.is_fully_explored() {
                    cache.update_exact(&max, BoundednessAnalysisMethod::CoverabilityGraph);
                } else {
                    // out of time: the maximum seen so far is only a lower bound
                    cache.update_lower_bound(&max, BoundednessAnalysisMethod::CoverabilityGraph);
                }
            }
        }
    }

    /// Refines the bounds for subclasses of Petri nets that have efficient
    /// (polynomial-time) procedures for boundedness.
    fn refine_efficient_subclasses(&self, scope: BoundednessScope, cache: &mut BoundsCache) {
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
                        .sum::<K>();
                    let bound = Boundedness::Bounded(marking_sum);
                    cache.by_place[p_idx].update_exact(bound, BoundednessAnalysisMethod::StateMachine);
                    cache.should_refine_by_subclass.remove(p_idx);
                }
            }
            NetClass::MarkedGraph if self.is_live().call() == Some(true) => {
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
    fn refine_structural_lp(&self, scope: BoundednessScope, cache: &mut BoundsCache) {
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
}

#[cfg(test)]
mod tests {
    use crate::system::boundedness::{Boundedness, K};

    #[test]
    fn boundedness_ord() {
        assert!(Boundedness::Bounded(0) < Boundedness::Bounded(1));
        assert!(Boundedness::Bounded(K::MAX) < Boundedness::Unbounded);
    }

    /// Regression: an unbounded marked graph with an unmarked circuit made the structural
    /// boundedness procedure unwrap an unanswered liveness query and panic.
    /// The circuit t0, p1, t1, p0 runs forever and every pass adds a token to `p4`; the
    /// circuit t2, p3, t3, p2 is unmarked, so the marked graph is not live.
    #[test]
    fn unbounded_marked_graph_that_is_not_live_is_answered() {
        use crate::net::builder::NetBuilder;
        use crate::net::class::NetClass;
        use crate::system::PetriNet;

        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3, p4] = b.add_places();
        let [t0, t1, t2, t3] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p4));
        b.add_arc((p4, t2));
        b.add_arc((p2, t2));
        b.add_arc((t2, p3));
        b.add_arc((p3, t3));
        b.add_arc((t3, p2));
        let sys = PetriNet::new(b.build().expect("valid net"), [(p0, 1)]);

        assert_eq!(sys.class(), NetClass::MarkedGraph);
        assert_eq!(sys.is_bounded().call(), Some(false));
        assert_eq!(sys.is_bounded().k(1).call(), Some(false));
        assert_eq!(sys.boundedness().call(), Some(Boundedness::Unbounded));
    }
}
