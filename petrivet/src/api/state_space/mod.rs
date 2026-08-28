mod coverability;
mod reachability;

pub use coverability::*;
pub use reachability::*;

use crate::core::mapping::DenseMapping;
use crate::core::marking::IdxMarking;
use crate::core::state_space::{DenseStateGraph, DenseStateGraphExplorer, ExploreNext, TokenOps};
use crate::net::{Net, Place, Transition};
use crate::system::{PetriNet, marking::Marking};
use std::iter::Sum;

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

impl<'a, T: TokenOps> StateGraphExplorer<'a, T> {
    /// Create a new [`StateGraphExplorer`] for a Petri net with the provided exploration order.
    #[must_use]
    pub fn new<N: AsRef<Net>>(sys: &'a PetriNet<N>, order: ExplorationOrder) -> Self
    where
        IdxMarking<T>: From<IdxMarking<u32>>,
    {
        let initial_marking = IdxMarking::from(sys.marking.clone());
        Self {
            core: DenseStateGraphExplorer::new(&sys.dense_net, initial_marking, order),
            mapping: &sys.mapping,
        }
    }

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

    /// Advances exploration until a marking covering `target` is found,
    /// and returns the marking and a firing sequence from the initial marking to it.
    pub fn find_cover(&mut self, target: Marking<T>) -> Option<Marking<T>> {
        let target_idx_marking = self.mapping.idx_marking(target);
        self.core
            .find(|idx_marking| *idx_marking >= target_idx_marking)
            .map(|idx_marking| self.mapping.marking(idx_marking.clone()))
    }

    /// Drive exploration until the entire reachable state space is discovered,
    /// returning the resulting state graph.
    ///
    /// **Note**: if the graph is large, this may take a long time and consume a lot of memory.
    /// If the state space is infinite, this will not terminate and will eventually exhaust all available memory.
    /// Use with caution, and consider incremental exploration via `explore_next` or `explore_iter` instead.
    #[must_use]
    pub fn build_graph(mut self) -> StateGraph<'a, T> {
        while self.core.explore_next().is_some() {}
        StateGraph {
            state_space: self.core.state_space,
            mapping: self.mapping,
        }
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

/// Controls frontier traversal order.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ExplorationOrder {
    /// Breadth-first: `path_to` returns shortest firing sequences.
    #[default]
    BreadthFirst,
    /// Depth-first: may use less memory on wide state spaces.
    DepthFirst,
}