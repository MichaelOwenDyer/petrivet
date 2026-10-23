mod coverability;
mod reachability;

pub use coverability::*;
pub use reachability::*;

use crate::core::mapping::DenseMapping;
use crate::core::state_space::{DenseStateGraph, DenseStateGraphExplorer, ExploreNext, TokenOps};
use crate::net::{Net, Place, Transition};
use crate::system::{PetriNet, marking::Marking};
use std::iter::Sum;
use std::time::{Duration, Instant};

/// Exploration stopped because its deadline passed before it could finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineExceeded;

impl std::fmt::Display for DeadlineExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the deadline passed before exploration finished")
    }
}

impl std::error::Error for DeadlineExceeded {}

/// The point in time `timeout` from now, or `None` (no deadline) if that is out of range.
fn deadline_after(timeout: Duration) -> Option<Instant> {
    Instant::now().checked_add(timeout)
}

/// Controls frontier traversal order.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ExplorationOrder {
    /// Breadth-first: `path_to` returns shortest firing sequences.
    #[default]
    BreadthFirst,
    /// Depth-first: may use less memory on wide state spaces.
    DepthFirst,
}

/// An in-progress exploration of the state graph of a Petri net.
///
/// Edges are [`Transitions`](Transition), and nodes are [`Markings`](Marking) of token type `T`.
#[derive(Clone)]
pub struct StateGraphExplorer<'a, T: TokenOps> {
    pub(super) core: DenseStateGraphExplorer<'a, T>,
    pub(super) mapping: &'a DenseMapping,
}

/// A single step in state space exploration.
#[derive(Debug, Clone)]
pub struct ExplorationStep<T: TokenOps> {
    /// The transition that was fired.
    pub transition: Transition,
    /// The resulting marking.
    pub marking: Marking<T>,
    /// Whether this marking was newly discovered (vs. already seen).
    pub is_new: bool,
}

impl<T: TokenOps> StateGraphExplorer<'_, T> {
    /// Current exploration order.
    #[must_use]
    pub const fn exploration_order(&self) -> ExplorationOrder {
        self.core.order
    }

    /// Change the exploration order for subsequent steps.
    pub const fn set_exploration_order(&mut self, order: ExplorationOrder) {
        self.core.order = order;
    }

    /// Whether exploration has completed.
    #[must_use]
    pub fn is_fully_explored(&self) -> bool {
        self.core.frontier_count() == 0
    }

    /// Number of distinct markings discovered so far.
    #[must_use]
    pub fn marking_count(&self) -> usize {
        self.core.state_space.marking_count()
    }

    /// Number of edges (transition firings) in the graph so far.
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.core.state_space.transition_count()
    }

    /// Returns the initial marking of the state graph.
    #[must_use]
    pub fn initial_marking(&self) -> Marking<T> {
        let marking = self.core.state_space.initial_marking().clone();
        self.mapping.marking(marking)
    }

    /// Returns `true` if the given marking has been discovered so far in the exploration.
    #[must_use]
    pub fn contains_marking(&self, marking: Marking<T>) -> bool {
        let idx_marking = self.mapping.idx_marking(marking);
        self.core.state_space.contains_marking(&idx_marking)
    }

    /// All markings discovered so far which enable no transitions.
    pub fn deadlocks(&self) -> impl Iterator<Item=Marking<T>> {
        self.core
            .state_space
            .deadlocks()
            .cloned()
            .map(|marking| self.mapping.marking(marking))
    }

    /// Returns a firing sequence from the initial marking to `target`,
    /// among states discovered so far in the exploration, if one exists.
    #[must_use]
    pub fn find_path_from_initial(&self, target: Marking<T>) -> Option<Vec<Transition>> {
        let target = self.mapping.idx_marking(target);
        self.core.path_from_initial_to(&target).map(|path| {
            path.into_iter()
                .map(|t_idx| self.mapping.transition(t_idx))
                .collect()
        })
    }
}

impl<'a, T: TokenOps> StateGraphExplorer<'a, T>
where
    DenseStateGraphExplorer<'a, T>: ExploreNext<T>,
{
    /// Explore the next item in the frontier: fire a transition from a marking
    /// and record the new edge (and possibly new marking) in the state graph.
    ///
    /// Return the transition which was fired, the graph index referencing the
    /// resulting marking, and whether the marking was newly discovered (vs. already seen).
    ///
    /// Return `None` if the frontier is empty and exploration is complete.
    pub fn explore_next(&mut self) -> Option<ExplorationStep<T>> {
        self.core
            .explore_next()
            .map(|(transition_idx, node_idx, is_new)| {
                let idx_marking = self.core.state_space.marking_at(node_idx);
                ExplorationStep {
                    transition: self.mapping.transition(transition_idx),
                    marking: self.mapping.marking(idx_marking.clone()),
                    is_new,
                }
            })
    }

    /// Returns an iterator that drives exploration step by step.
    ///
    /// See [`explore_next`](StateGraphExplorer::explore_next).
    pub fn explore_iter(&mut self) -> impl Iterator<Item=ExplorationStep<T>> + '_ {
        std::iter::from_fn(move || self.explore_next())
    }

    /// Advances exploration until a marking satisfying `predicate` is found, and returns it.
    ///
    /// The markings discovered so far are checked first, then exploration continues until a marking
    /// satisfies the predicate or the state space is exhausted (`None`). The predicate is called once
    /// for every marking, when it is discovered, and never for the same marking twice.
    ///
    /// **Does not terminate** if the state space is infinite and no marking satisfies the predicate.
    /// Use [`find_until`](StateGraphExplorer::find_until) or [`find_timeout`](StateGraphExplorer::find_timeout)
    /// to bound the time.
    pub fn find(&mut self, predicate: impl FnMut(&Marking<T>) -> bool) -> Option<Marking<T>> {
        self.find_until(predicate, None).unwrap_or(None)
    }

    /// Like [`find`](StateGraphExplorer::find), but gives up after `timeout` has elapsed.
    ///
    /// # Errors
    /// Returns [`DeadlineExceeded`] if the timeout elapsed before a marking satisfying the predicate was found
    /// or the state space was exhausted. Exploration keeps its progress, so it can be resumed.
    pub fn find_timeout(
        &mut self,
        predicate: impl FnMut(&Marking<T>) -> bool,
        timeout: Duration,
    ) -> Result<Option<Marking<T>>, DeadlineExceeded> {
        self.find_until(predicate, deadline_after(timeout))
    }

    /// Like [`find`](StateGraphExplorer::find), but gives up once `deadline` has passed.
    ///
    /// The clock is read periodically, not after every marking, so exploration may run slightly
    /// past the deadline. `Ok(None)` means the whole state space was explored without finding a
    /// marking that satisfies the predicate, which is different from running out of time.
    ///
    /// # Errors
    /// Returns [`DeadlineExceeded`] if the deadline passed before a marking satisfying the predicate was found
    /// or the state space was exhausted. Exploration keeps its progress, so it can be resumed.
    pub fn find_until(
        &mut self,
        mut predicate: impl FnMut(&Marking<T>) -> bool,
        deadline: Option<Instant>,
    ) -> Result<Option<Marking<T>>, DeadlineExceeded> {
        let mapping = self.mapping;
        let found = self
            .core
            .search(|idx_marking| predicate(&mapping.marking(idx_marking.clone())), deadline)
            .map(|idx_marking| mapping.marking(idx_marking.clone()));
        // `search` also returns `None` when it runs out of time, which proves nothing
        if found.is_none() && !self.is_fully_explored() {
            return Err(DeadlineExceeded);
        }
        Ok(found)
    }

    /// Advances exploration until a marking covering `target` is found,
    /// and returns the marking.
    pub fn find_cover(&mut self, target: Marking<T>) -> Option<Marking<T>> {
        let target_idx_marking = self.mapping.idx_marking(target);
        self.core
            .search(|idx_marking| *idx_marking >= target_idx_marking, None)
            .map(|idx_marking| self.mapping.marking(idx_marking.clone()))
    }

    /// Like [`find_cover`](StateGraphExplorer::find_cover), but gives up after `timeout` has elapsed.
    ///
    /// # Errors
    /// Returns [`DeadlineExceeded`] if the timeout elapsed before a covering marking was found
    /// or the state space was exhausted. Exploration keeps its progress, so it can be resumed.
    pub fn find_cover_timeout(
        &mut self,
        target: Marking<T>,
        timeout: Duration,
    ) -> Result<Option<Marking<T>>, DeadlineExceeded> {
        self.find_cover_until(target, deadline_after(timeout))
    }

    /// Like [`find_cover`](StateGraphExplorer::find_cover), but gives up once `deadline` has passed.
    ///
    /// The clock is read periodically, not after every marking, so exploration may run slightly
    /// past the deadline. `Ok(None)` means the whole state space was explored without finding a
    /// covering marking, which is different from running out of time.
    ///
    /// # Errors
    /// Returns [`DeadlineExceeded`] if the deadline passed before a covering marking was found
    /// or the state space was exhausted. Exploration keeps its progress, so it can be resumed.
    pub fn find_cover_until(
        &mut self,
        target: Marking<T>,
        deadline: Option<Instant>,
    ) -> Result<Option<Marking<T>>, DeadlineExceeded> {
        let target_idx_marking = self.mapping.idx_marking(target);
        let found = self
            .core
            .search(|idx_marking| *idx_marking >= target_idx_marking, deadline)
            .map(|idx_marking| self.mapping.marking(idx_marking.clone()));
        // `search` also returns `None` when it runs out of time, which proves nothing
        if found.is_none() && !self.is_fully_explored() {
            return Err(DeadlineExceeded);
        }
        Ok(found)
    }

    /// Drive exploration until the entire reachable state space is discovered,
    /// returning the resulting state graph.
    ///
    /// **Note**: if the graph is large, this may take a long time and consume a lot of memory.
    /// If the state space is infinite, this will not terminate and will eventually exhaust all available memory.
    /// Use with caution, and consider incremental exploration via `explore_next` or `explore_iter` instead,
    /// or bound the time with [`build_graph_timeout`](StateGraphExplorer::build_graph_timeout).
    #[must_use]
    pub fn build_graph(mut self) -> StateGraph<'a, T> {
        while self.core.explore_next().is_some() {}
        StateGraph {
            state_space: self.core.state_space,
            mapping: self.mapping,
        }
    }

    /// Like [`build_graph`](StateGraphExplorer::build_graph), but gives up after `timeout` has elapsed.
    ///
    /// # Errors
    /// Returns the partially explored explorer if the timeout elapsed first.
    /// Its frontier is preserved, so exploration can be resumed if desired.
    #[allow(clippy::result_large_err)]
    pub fn build_graph_timeout(self, timeout: Duration) -> Result<StateGraph<'a, T>, Self> {
        self.build_graph_until(deadline_after(timeout))
    }

    /// Like [`build_graph`](StateGraphExplorer::build_graph), but gives up once `deadline` has passed.
    ///
    /// The clock is read periodically, not after every marking, so exploration may run slightly
    /// past the deadline.
    ///
    /// # Errors
    /// Returns the partially explored explorer if the deadline passed first.
    /// Its frontier is preserved, so exploration can be resumed if desired.
    #[allow(clippy::result_large_err)]
    pub fn build_graph_until(mut self, deadline: Option<Instant>) -> Result<StateGraph<'a, T>, Self> {
        let mut time_is_up = crate::core::state_space::deadline_checker(deadline);
        while self.core.explore_next().is_some() {
            if time_is_up() && !self.is_fully_explored() {
                return Err(self);
            }
        }
        Ok(StateGraph {
            state_space: self.core.state_space,
            mapping: self.mapping,
        })
    }
}

/// A fully explored state graph of a Petri net.
///
/// Edges are [`Transitions`](Transition), and nodes are [`Markings`](Marking) of token type `T`.
#[derive(Debug, Clone)]
pub struct StateGraph<'a, T: TokenOps> {
    pub(crate) state_space: DenseStateGraph<'a, T>,
    mapping: &'a DenseMapping,
}

impl<T: TokenOps> StateGraph<'_, T> {
    /// Number of distinct markings in the coverability graph.
    #[must_use]
    pub fn marking_count(&self) -> usize {
        self.state_space.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.state_space.graph.edge_count()
    }

    /// Iterator over all distinct markings in the graph.
    pub fn markings(&self) -> impl Iterator<Item=Marking<T>> {
        self.state_space
            .markings()
            .cloned()
            .map(|marking| self.mapping.marking(marking))
    }

    /// Whether the given marking has been discovered.
    ///
    /// **Note**: this checks for exact presence, not coverability.
    /// For coverability queries, use `cover()`.
    #[must_use]
    pub fn contains_marking(&self, marking: Marking<T>) -> bool {
        self.state_space
            .seen
            .contains_key(&self.mapping.idx_marking(marking))
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> Marking<T> {
        let marking = self
            .state_space
            .marking_at(self.state_space.initial_idx)
            .clone();
        self.mapping.marking(marking)
    }

    /// Upper bound on the token count for a given place across all
    /// discovered markings. Returns `Omega::Unbounded` if the place is
    /// unbounded.
    #[must_use]
    pub fn place_bound(&self, p: Place) -> T {
        self.mapping.place_idx(p).map_or(T::ZERO, |p_idx| {
            self.state_space
                .markings()
                .map(|marking| marking[p_idx])
                .max()
                .unwrap_or(T::ZERO)
        })
    }

    /// Tries to find a marking which covers the provided marking.
    #[must_use]
    pub fn cover(&self, target: Marking<T>) -> Option<Marking<T>> {
        let target = self.mapping.idx_marking(target);
        self.state_space
            .markings()
            .find(|&marking| marking >= &target)
            .cloned()
            .map(|marking| self.mapping.marking(marking))
    }

    /// All discovered markings that have no enabled transitions.
    pub fn deadlocks(&self) -> impl Iterator<Item=Marking<T>> {
        self.state_space
            .deadlocks()
            .cloned()
            .map(|marking| self.mapping.marking(marking))
    }

    /// Whether the graph contains no deadlocks.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.state_space.deadlocks().next().is_none()
    }

    /// Whether some reachable marking has no enabled transitions.
    ///
    /// This is the answer to `EF deadlock`, used by the MCC
    /// `ReachabilityDeadlock` examination: TRUE iff a deadlock is reachable.
    /// Equivalent to `!is_deadlock_free()`, exposed under this name to
    /// match the formula it answers.
    #[must_use]
    pub fn has_reachable_deadlock(&self) -> bool {
        !self.is_deadlock_free()
    }

    /// The largest single-place bound across the entire net.
    #[must_use]
    pub fn max_token_in_any_place(&self) -> T {
        self.state_space
            .markings()
            .map(|m| m.iter().max().expect("marking must have length > 0"))
            .max()
            .copied()
            .expect("initial marking is always present")
    }

    /// The largest total token count across all reachable markings.
    #[must_use]
    pub fn max_token_per_marking(&self) -> T
    where
        T: Sum,
    {
        self.state_space
            .markings()
            .map(|m| m.iter().copied().sum::<T>())
            .max()
            .expect("initial marking is always present")
    }

    /// Whether every reachable marking puts at most one token in every place.
    #[must_use]
    pub fn is_one_safe(&self) -> bool {
        self.max_token_in_any_place() <= T::ONE
    }

    /// Whether some place holds the same token count in every reachable marking.
    #[must_use]
    pub fn has_stable_place(&self) -> bool {
        let mut markings = self.state_space.markings();
        let first = markings.next().expect("initial marking is always present");
        // Per-place flag: is the count still equal to its first-observed value?
        let mut still_stable = vec![true; self.mapping.place_count() as usize];
        for marking in markings {
            for (p_idx, token_count) in marking.iter().copied().enumerate() {
                if still_stable[p_idx] && token_count != first[p_idx] {
                    still_stable[p_idx] = false;
                }
            }
        }
        still_stable.into_iter().any(|b| b)
    }

    /// Returns a firing sequence from the initial marking to `target`, if one exists.
    ///
    /// If the graph was explored with a breadth-first order,
    /// this is guaranteed to be a shortest path.
    #[must_use]
    pub fn firing_sequence_from_initial_to(&self, target: Marking<T>) -> Option<Vec<Transition>> {
        let target = self.mapping.idx_marking(target);
        self.state_space.path_from_initial_to(&target).map(|path| {
            path.into_iter()
                .map(|t_idx| self.mapping.transition(t_idx))
                .collect()
        })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns the complete coverability graph for this system.
    ///
    /// Warning! This may be a HUGE structure!
    pub fn build_coverability_graph(&self) -> CoverabilityGraph<'_> {
        self.explore_coverability_graph(ExplorationOrder::BreadthFirst).build_graph()
    }

    /// Like [`build_coverability_graph`](PetriNet::build_coverability_graph),
    /// but gives up after `timeout` has elapsed.
    ///
    /// # Errors
    /// Returns the partially explored coverability explorer if the timeout elapsed first.
    #[allow(clippy::result_large_err)]
    pub fn build_coverability_graph_timeout(
        &self,
        timeout: Duration,
    ) -> Result<CoverabilityGraph<'_>, CoverabilityExplorer<'_>> {
        self.explore_coverability_graph(ExplorationOrder::BreadthFirst).build_graph_timeout(timeout)
    }

    /// Attempt to construct a [`ReachabilityGraph`] of this [`PetriNet`],
    /// returning either itself if the system is bounded or a partially-explored
    /// [`CoverabilityExplorer`] if it is unbounded.
    ///
    /// Not knowing whether we will encounter unboundedness, this method first
    /// constructs a Karp-Miller coverability tree which introduces ω as soon
    /// as unbounded growth is detected. This comes at the cost of an additional
    /// check per explored marking (omega acceleration). If we finish exploring
    /// the coverability tree without ever introducing ω, we have in fact explored
    /// the full reachability graph and can return it directly. Otherwise, we return
    /// the [`CoverabilityExplorer`] in its current state, which contains the explored
    /// portion of the coverability graph up to the first ω, and can be further explored.
    ///
    /// This is the right entry point when you want the speed of exploring
    /// the reachability graph directly but cannot rule out unboundedness
    /// upfront. For unbounded nets you avoid the cost of completing the
    /// full coverability graph; for bounded nets the cost is identical to
    /// `build_reachability_graph` (no ω is ever introduced, no extra work).
    ///
    /// # Errors
    /// Returns `Err(partial_explorer)` as soon as any explored marking
    /// contains ω. The frontier is preserved, so callers may resume.
    #[allow(clippy::result_large_err)]
    pub fn try_build_reachability_graph(&self) -> Result<ReachabilityGraph<'_>, CoverabilityExplorer<'_>> {
        self.explore_coverability_graph(ExplorationOrder::BreadthFirst).try_build_reachability_graph()
    }

    /// Like [`try_build_reachability_graph`](PetriNet::try_build_reachability_graph),
    /// but gives up after `timeout` has elapsed.
    ///
    /// # Errors
    /// Returns the partially explored coverability explorer, with the reason it stopped:
    /// the system is unbounded, or the timeout elapsed.
    #[allow(clippy::result_large_err)]
    pub fn try_build_reachability_graph_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ReachabilityGraph<'_>, ReachabilityGraphStopped<'_>> {
        self.explore_coverability_graph(ExplorationOrder::BreadthFirst)
            .try_build_reachability_graph_timeout(timeout)
    }

    /// Returns the complete reachability graph for this system.
    ///
    /// WARNING! For unbounded nets, this will not terminate!
    /// Use [`build_reachability_graph_timeout`](PetriNet::build_reachability_graph_timeout)
    /// when boundedness is not known.
    pub fn build_reachability_graph(&self) -> ReachabilityGraph<'_> {
        self.explore_reachability_graph(ExplorationOrder::BreadthFirst).build_graph()
    }

    /// Like [`build_reachability_graph`](PetriNet::build_reachability_graph),
    /// but gives up after `timeout` has elapsed, which also ends the exploration of an unbounded net.
    ///
    /// # Errors
    /// Returns the partially explored reachability explorer if the timeout elapsed first.
    #[allow(clippy::result_large_err)]
    pub fn build_reachability_graph_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ReachabilityGraph<'_>, ReachabilityGraphExplorer<'_>> {
        self.explore_reachability_graph(ExplorationOrder::BreadthFirst).build_graph_timeout(timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::builder::NetBuilder;

    /// Forty tokens circulating through a triangle of places: 861 markings and 2583 edges,
    /// more steps than are taken between two readings of the clock.
    fn a_few_thousand_steps() -> (PetriNet, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));
        b.add_arc((p2, t2));
        b.add_arc((t2, p0));
        (PetriNet::new(b.build().unwrap(), [(p0, 40)]), p0)
    }

    #[test]
    fn build_graph_gives_up_at_the_deadline_and_can_be_resumed() {
        let (sys, _) = a_few_thousand_steps();
        let expired = Some(Instant::now());

        let partial = sys
            .explore_reachability_graph(ExplorationOrder::BreadthFirst)
            .build_graph_until(expired)
            .expect_err("the deadline has passed");
        assert!(!partial.is_fully_explored());
        assert!(partial.marking_count() < 861);

        // the frontier is preserved: resuming without a deadline finishes the job
        let graph = partial.build_graph_until(None).expect("no deadline");
        assert_eq!(graph.marking_count(), 861);

        assert!(sys.build_reachability_graph_timeout(Duration::from_secs(60)).is_ok());
        assert!(sys.build_coverability_graph_timeout(Duration::from_secs(60)).is_ok());
        assert!(sys.try_build_reachability_graph_timeout(Duration::from_secs(60)).is_ok());
    }

    #[test]
    fn find_stops_at_the_first_marking_that_satisfies_the_predicate() {
        let (sys, place) = a_few_thousand_steps();
        let mut explorer = sys.explore_reachability_graph(ExplorationOrder::BreadthFirst);
        // the initial marking is checked first, without exploring anything
        let found = explorer.find(|m| m.get(place) == 40).expect("the initial marking holds forty tokens");
        assert_eq!(found.get(place), 40);
        assert_eq!(explorer.marking_count(), 1);
        // a marking one step away is found long before the 861 markings are exhausted
        let found = explorer.find(|m| m.get(place) == 39).expect("one token can leave the place");
        assert_eq!(found.get(place), 39);
        assert!(!explorer.is_fully_explored());
    }

    #[test]
    fn find_calls_the_predicate_once_per_marking() {
        let (sys, place) = a_few_thousand_steps();
        let mut explorer = sys.explore_reachability_graph(ExplorationOrder::BreadthFirst);
        let mut seen = std::collections::HashSet::new();
        let mut calls = 0;
        assert_eq!(
            explorer.find_until(
                |m| {
                    calls += 1;
                    assert!(seen.insert(m.clone()), "predicate called twice for one marking");
                    m.get(place) == 1000
                },
                None
            ),
            Ok(None)
        );
        assert_eq!(calls, 861);
        // the explorer is exhausted: asking again re-checks the discovered markings, again once each
        assert!(explorer.find(|m| m.get(place) == 1000).is_none());
    }

    #[test]
    fn find_until_tells_exhaustion_from_running_out_of_time() {
        let (sys, place) = a_few_thousand_steps();
        let never = |m: &Marking<u32>| m.get(place) > 40;
        let mut explorer = sys.explore_reachability_graph(ExplorationOrder::BreadthFirst);
        assert_eq!(explorer.find_until(never, Some(Instant::now())), Err(DeadlineExceeded));
        assert!(!explorer.is_fully_explored());
        // resumed without a deadline the exhausted state space is a definite "no"
        assert_eq!(explorer.find_until(never, None), Ok(None));
        assert_eq!(explorer.find_timeout(never, Duration::from_secs(60)), Ok(None));
    }

    #[test]
    fn find_cover_tells_a_missing_cover_from_running_out_of_time() {
        let (sys, place) = a_few_thousand_steps();
        // there are only forty tokens, so forty-one on one place is not coverable
        let uncoverable: Marking<u32> = [(place, 41)].into_iter().collect();

        let mut explorer = sys.explore_reachability_graph(ExplorationOrder::BreadthFirst);
        assert_eq!(
            explorer.find_cover_until(uncoverable.clone(), Some(Instant::now())),
            Err(DeadlineExceeded)
        );
        // resumed without a deadline, the exhausted state space is a definite "no"
        assert_eq!(explorer.find_cover_until(uncoverable, None), Ok(None));
    }
}
