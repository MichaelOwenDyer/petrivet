pub mod coverability;
pub mod reachability;

use crate::core::marking::IdxMarking;
use crate::core::net::{DenseNet, TransitionIdx};
use ahash::{HashMap, HashMapExt};
use petgraph::graph::NodeIndex;
use std::collections::VecDeque;
use std::hash::Hash;
use std::iter::Sum;
use std::ops::ControlFlow;

/// Controls frontier traversal order.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ExplorationOrder {
    /// Breadth-first: `path_to` returns shortest firing sequences.
    #[default]
    BreadthFirst,
    /// Depth-first: may use less memory on wide state spaces.
    DepthFirst,
}

/// Operations on a token count needed for state space exploration.
pub trait TokenOps: Clone + Copy + Eq + Ord + Hash + Sum {
    const ZERO: Self;
    const ONE: Self;
    fn at_least_one(&self) -> bool;
    fn increment(&mut self);
    fn decrement(&mut self);
}

/// The core trait for state space exploration,
/// implemented by both reachability and coverability graph explorers.
pub trait ExploreNext<T: TokenOps> {
    /// Explore the next item in the frontier: fire a transition from a marking
    /// and record the new edge (and possibly new marking) in the state graph.
    ///
    /// Return the transition which was fired, the graph index referencing the
    /// resulting marking, and whether the marking was newly discovered (vs. already seen).
    ///
    /// Return `None` if the frontier is empty and exploration is complete.
    fn explore_next(&mut self) -> Option<(TransitionIdx, NodeIndex, bool)>;
}

/// The shared exploration engine for both reachability and coverability graphs.
///
/// Manages the petgraph, seen-set, and frontier. Both `CoverabilityGraph` and
/// `ReachabilityGraph` own one of these and drive it via the helper methods.
///
/// Borrows the [`Net`] for its lifetime - the graph cannot outlive the net
/// it explores.
#[derive(Debug, Clone)]
pub struct DenseStateGraphExplorer<'a, T: TokenOps> {
    /// The state space being explored.
    /// Can be extracted once exploration is complete.
    pub state_space: DenseStateGraph<'a, T>,
    /// The exploration order: breadth-first or depth-first.
    /// Corresponds to queue vs stack behavior of the frontier.
    pub order: ExplorationOrder,
    /// The worklist of source markings and *potentially* enabled transitions
    /// which we have not yet investigated.
    frontier: VecDeque<(NodeIndex, TransitionIdx)>,
    /// Transitions with empty presets - always enabled, and should
    /// always be explored from every new marking regardless of the
    /// marked places.
    source_transitions: Box<[TransitionIdx]>,
}

impl<'a, T: TokenOps> DenseStateGraphExplorer<'a, T> {
    /// Create a new explorer from a net reference and initial marking.
    ///
    /// Seeds the frontier with source transitions (empty preset, always
    /// enabled) plus transitions whose presets overlap with the support
    /// of the initial marking.
    pub fn new(net: &'a DenseNet, initial_marking: IdxMarking<T>, order: ExplorationOrder) -> Self {
        let mut graph = petgraph::Graph::new();
        let initial_idx = graph.add_node(initial_marking.clone());

        let source_transitions: Box<[TransitionIdx]> = net
            .transition_indices()
            .filter(|&t| net.preset_t[t].is_empty())
            .collect();

        let frontier: VecDeque<_> = Self::potentially_enabled_transitions(
            &initial_marking,
            &net.postset_p,
            &source_transitions,
        )
        .into_iter()
        .map(|t_idx| (initial_idx, t_idx))
        .collect();

        let mut seen = HashMap::new();
        seen.insert(initial_marking, initial_idx);

        let state_space = DenseStateGraph {
            net,
            initial_idx,
            graph,
            seen,
        };

        Self {
            state_space,
            order,
            frontier,
            source_transitions,
        }
    }

    /// Compute the transitions which could possibly be enabled at a marking, based on
    /// the support of the marking and the postset of places in the net,
    /// plus source transitions which are always enabled.
    ///
    /// The transitions are not guaranteed to be enabled, since the preset of a transition may
    /// include places not in the support of the marking, but this is a sound over-approximation
    /// which allows us to avoid iterating over all transitions.
    fn potentially_enabled_transitions(
        marking: &IdxMarking<T>,
        postset_p: &[Box<[TransitionIdx]>],
        source_transitions: &[TransitionIdx],
    ) -> Vec<TransitionIdx> {
        let mut transitions = marking
            .support()
            .flat_map(|p| postset_p[p].iter().copied())
            .collect::<Vec<_>>();
        transitions.sort_unstable();
        transitions.dedup();
        transitions.extend(source_transitions);
        transitions
    }

    /// The number of items in the frontier.
    pub fn frontier_count(&self) -> usize {
        self.frontier.len()
    }

    /// Returns a firing sequence from the initial marking to `target`,
    /// among states discovered so far in the exploration, if one exists.
    pub fn path_from_initial_to(&self, target: &IdxMarking<T>) -> Option<Vec<TransitionIdx>> {
        self.state_space.path_from_initial_to(target)
    }

    /// Pop the next `(NodeIndex, Transition)` from the frontier.
    pub fn pop_frontier(&mut self) -> Option<(NodeIndex, TransitionIdx)> {
        match self.order {
            ExplorationOrder::BreadthFirst => self.frontier.pop_front(),
            ExplorationOrder::DepthFirst => self.frontier.pop_back(),
        }
    }

    /// Whether a transition is enabled at the marking stored in `node`.
    pub fn is_enabled(&self, marking_idx: NodeIndex, t_idx: TransitionIdx) -> bool {
        let marking = &self.state_space.graph[marking_idx];
        self.state_space.net.preset_t[t_idx]
            .iter()
            .all(|&p| marking[p].at_least_one())
    }

    /// Compute the marking that results from firing `t` at `node`.
    ///
    /// Caller must ensure the transition is enabled.
    pub fn fire(&self, node: NodeIndex, t: TransitionIdx) -> IdxMarking<T> {
        let mut result = self.state_space.graph[node].clone();
        for &p in &self.state_space.net.preset_t[t] {
            result[p].decrement();
        }
        for &p in &self.state_space.net.postset_t[t] {
            result[p].increment();
        }
        result
    }

    /// Register a marking in the graph.
    ///
    /// If already seen, adds an edge and returns `(false, existing_index)`.
    /// If new, adds the node, seeds the frontier with all potentially enabled
    /// transitions, adds the edge, and returns `(true, new_index)`.
    pub fn register(
        &mut self,
        from: NodeIndex,
        over: TransitionIdx,
        marking: IdxMarking<T>,
    ) -> (bool, NodeIndex) {
        // if we have seen this marking before, just add an edge from the source to the existing node
        if let Some(&idx) = self.state_space.seen.get(&marking) {
            self.state_space.graph.add_edge(from, idx, over);
            return (false, idx);
        }

        // add the marking as a new node, and an edge from the source to it
        let idx = self.state_space.graph.add_node(marking.clone());
        self.state_space.graph.add_edge(from, idx, over);

        // seed frontier with all potentially enabled transitions from this marking
        for t_idx in Self::potentially_enabled_transitions(
            &marking,
            &self.state_space.net.postset_p,
            &self.source_transitions,
        ) {
            self.frontier.push_back((idx, t_idx));
        }

        // record the marking as seen at this index
        self.state_space.seen.insert(marking, idx);

        (true, idx)
    }

    /// Check all currently discovered markings for one satisfying `predicate`,
    /// and return it if found.
    #[allow(dead_code)]
    pub fn any_matched(
        &self,
        mut predicate: impl FnMut(&IdxMarking<T>) -> bool,
    ) -> Option<&IdxMarking<T>> {
        self.state_space
            .seen
            .values()
            .map(|&node| self.state_space.marking_at(node))
            .find(|marking| predicate(marking))
    }
}

impl<'a, T: TokenOps> DenseStateGraphExplorer<'a, T>
where
    DenseStateGraphExplorer<'a, T>: ExploreNext<T>,
{
    /// Check all currently discovered markings for one satisfying `predicate`,
    /// and return it if found.
    ///
    /// Otherwise, drive exploration until either a marking satisfying `predicate` is found
    /// (and returned), or the frontier is exhausted (in which case `None` is returned).
    /// See [`search`](DenseStateGraphExplorer::search) for the latter behavior.
    ///
    /// **Does not terminate** if there are infinite states and the predicate never returns true.
    pub fn find(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking<T>) -> bool,
    ) -> Option<&IdxMarking<T>> {
        // todo: possible to call .any_matched() here instead?
        for &node in self.state_space.seen.values() {
            if predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        self.search(predicate)
    }

    /// Drive exploration until either:
    ///
    /// - `predicate` returns `true` for some reachable marking — in which
    ///   case the marking is returned immediately, or
    /// - the frontier is exhausted (the entire reachability graph has been
    ///   explored without the predicate ever firing) — in which case
    ///   `None` is returned.
    ///
    /// **Does not terminate** if there are infinite states and the predicate never returns true.
    pub fn search(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking<T>) -> bool,
    ) -> Option<&IdxMarking<T>> {
        while let Some((_t_idx, node_idx, is_new)) = self.explore_next() {
            if is_new && predicate(self.state_space.marking_at(node_idx)) {
                return Some(self.state_space.marking_at(node_idx));
            }
        }
        None
    }

    pub fn visit<F, B>(&mut self, mut visitor: F) -> Option<B>
    where
        F: FnMut((TransitionIdx, NodeIndex, bool)) -> ControlFlow<B>,
    {
        loop {
            if let ControlFlow::Break(b) = visitor(self.explore_next()?) {
                return Some(b);
            }
        }
    }
}

/// A fully explored state space graph of a Petri net.
#[derive(Debug, Clone)]
pub struct DenseStateGraph<'a, T: TokenOps> {
    /// Reference to the net.
    pub net: &'a DenseNet,
    /// Reference to the graph's initial node, for pathfinding.
    pub initial_idx: NodeIndex,
    /// The underlying graph structure. Nodes are markings, edges are transitions.
    pub graph: petgraph::Graph<IdxMarking<T>, TransitionIdx>,
    /// A hash table of seen markings to their node indices in the graph,
    /// for O(1) lookup.
    pub seen: HashMap<IdxMarking<T>, NodeIndex>,
}

impl<T: TokenOps> DenseStateGraph<'_, T> {
    /// Number of distinct markings discovered so far.
    pub fn marking_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph so far.
    pub fn transition_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns an iterator over all markings in the graph.
    pub fn markings(&self) -> impl Iterator<Item = &IdxMarking<T>> + '_ {
        self.graph.node_weights()
    }

    /// Get the marking stored at node index `idx`.
    pub fn marking_at(&self, idx: NodeIndex) -> &IdxMarking<T> {
        &self.graph[idx]
    }

    /// Returns a reference to the initial marking, the starting point of exploration.
    pub fn initial_marking(&self) -> &IdxMarking<T> {
        self.marking_at(self.initial_idx)
    }

    /// Returns `true` if the given marking has been discovered so far in the exploration.
    pub fn contains_marking(&self, marking: &IdxMarking<T>) -> bool {
        self.seen.contains_key(marking)
    }

    /// Returns an iterator over markings which enable no transitions (deadlocks).
    pub fn deadlocks(&self) -> impl Iterator<Item = &IdxMarking<T>> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .next()
                    .is_none()
            })
            .map(|idx| self.marking_at(idx))
    }

    /// Returns a firing sequence from the initial marking to `target`,
    /// if one exists.
    pub fn path_from_initial_to(&self, target: &IdxMarking<T>) -> Option<Vec<TransitionIdx>> {
        let target = *self.seen.get(target)?;

        if target == self.initial_idx {
            return Some(Vec::new());
        }

        let (_len, node_path) = petgraph::algo::astar(
            &self.graph,
            self.initial_idx,
            |n| n == target,
            |_| 1u32,
            |_| 0u32,
        )?;

        let firing_sequence = node_path
            .array_windows()
            .map(|&[m1_idx, m2_idx]| {
                self.graph
                    .find_edge(m1_idx, m2_idx)
                    .expect("edge must exist")
            })
            .map(|edge_idx| self.graph[edge_idx])
            .collect();
        Some(firing_sequence)
    }
}
