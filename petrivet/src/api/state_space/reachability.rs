//! Reachability graph exploration and analysis for Petri nets.
//!
//! This module provides tools for exploring all possible markings reachable
//! from a given initial marking in a Petri net, and analyzing the resulting
//! reachability graph for properties like liveness and deadlock-freedom.

use ahash::HashMap;
use crate::boundedness::{Boundedness, K};
use crate::net::Place;
use crate::state_space::{StateGraph, StateGraphExplorer};
use crate::system::liveness::{LivenessAnalysis, LivenessMethod};

/// An incremental exploration handle which lazily enumerates every single marking
/// in the state space of the [`PetriNet`].
///
/// Note that an [`unbounded`](crate::literature#Boundedness)
///
/// Works for any net (bounded or unbounded). For unbounded nets, the frontier
/// never empties - the caller must impose their own termination condition.
///
/// Once exploration is complete (`is_fully_explored()` returns `true`), convert
/// to a [`ReachabilityGraph`] for exact analysis.
///
/// # Examples
///
/// ```
/// use petrivet::{NetBuilder, PetriNet};
/// use petrivet::state_space::ExplorationOrder;
/// use petrivet::state_space::reachability::{ReachabilityExplorer, ReachabilityGraph};
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0] = b.add_transitions();
/// b.add_arcs((p0, t0));
/// b.add_arc((t0, p0));
/// b.add_arc((t0, p1));
/// let net = b.build().unwrap();
/// let sys = PetriNet::new(net, [1, 0]);
///
/// // Explore an unbounded net incrementally, stopping after 50 states
/// let mut explorer = sys.explore_reachability(ExplorationOrder::BreadthFirst);
/// explorer.iter().take(50).for_each(|s| println!("{:#?}", s.marking));
/// assert!(explorer.state_count() >= 50);
/// assert!(!explorer.is_fully_explored()); // unbounded → never finishes
/// ```
pub type ReachabilityExplorer<'a> = StateGraphExplorer<'a, u32>;

impl std::fmt::Debug for ReachabilityExplorer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachabilityExplorer")
            .field("markings", &self.marking_count())
            .field("transitions", &self.transition_count())
            .field("frontier", &self.core.frontier_count())
            .finish()
    }
}

/// A fully explored, finite reachability graph.
///
/// **Warning**: This can quickly become a **HUGE** structure for even moderately complex nets.
///
/// This type is a proof that exploration terminated (the net is bounded under
/// this initial marking). Exact analysis methods - liveness levels, deadlock
/// detection - are available here.
///
/// The reachability graph is infinite for unbounded systems. For unknown systems,
/// prefer building a [`CoverabilityExplorer`] first (always terminates, decides
/// coverability and boundedness), then attempt to promote to a `ReachabilityGraph`,
/// which succeeds if and only if the net is bounded.
pub type ReachabilityGraph<'a> = StateGraph<'a, u32>;

impl ReachabilityGraph<'_> {
    /// Returns all transitions in the Petri net with their associated liveness levels.
    #[must_use]
    pub fn transition_liveness(&self) -> LivenessAnalysis {
        let levels = self.mapping
            .transitions()
            .zip(self.state_space.liveness_levels())
            .collect();
        LivenessAnalysis {
            levels,
            method: LivenessMethod::ReachabilityGraph,
        }
    }

    /// Returns the maximum value of each place across all markings in the coverability graph.
    /// Places that were marked as Omega are unbounded.
    #[must_use]
    pub fn place_bounds(&self) -> HashMap<Place, Boundedness> {
        let mut markings = self.state_space.markings();
        let initial_marking = markings.next().expect("always at least one marking in the graph");
        let bounds = markings.fold(
            initial_marking.iter().copied().collect::<Vec<_>>(),
            |mut bounds_so_far, next_marking| {
                bounds_so_far.iter_mut()
                    .zip(next_marking.iter())
                    .for_each(|(bound, &next)| {
                        *bound = core::cmp::max(*bound, next);
                    });
                bounds_so_far
            },
        ).into_iter().map(|max| Boundedness::Bounded(max as K));
        self.mapping.places().zip(bounds).collect()
    }
}