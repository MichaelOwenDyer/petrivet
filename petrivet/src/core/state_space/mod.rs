pub mod reachability;
pub mod coverability;

use crate::core::marking::IdxMarking;
use crate::core::net::{DenseNet, TransitionIdx};
use ahash::{HashMap, HashMapExt, HashSet};
use petgraph::graph::NodeIndex;
use std::collections::VecDeque;
use std::hash::Hash;
use std::iter::Sum;

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
    /// The worklist of potentially enabled transitions which we have not
    /// yet investigated firing from their source markings.
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
    pub fn new(
        net: &'a DenseNet,
        initial_marking: IdxMarking<T>,
        order: ExplorationOrder
    ) -> Self {
        let mut graph = petgraph::Graph::new();
        let initial_idx = graph.add_node(initial_marking.clone());

        let source_transitions: Box<[TransitionIdx]> = net
            .transition_indices()
            .filter(|&t| net.preset_t[t].is_empty())
            .collect();

        let frontier: VecDeque<_> = initial_marking
            .support()
            .flat_map(|p| net.postset_p[p].iter().copied())
            .chain(source_transitions.iter().copied())
            .collect::<HashSet<TransitionIdx>>()
            .into_iter()
            .map(|t| (initial_idx, t))
            .collect();

        let mut seen = HashMap::new();
        seen.insert(initial_marking, initial_idx);

        let state_space = DenseStateGraph { net, initial_idx, graph, seen };

        Self { state_space, order, frontier, source_transitions }
    }

    /// The number of items in the frontier, for debugging or instrumentation.
    pub fn frontier_count(&self) -> usize {
        self.frontier.len()
    }

    /// Whether the frontier is empty (exploration complete).
    pub fn is_fully_explored(&self) -> bool {
        self.frontier.is_empty()
    }

    /// Pop the next `(NodeIndex, Transition)` from the frontier.
    pub fn pop_frontier(&mut self) -> Option<(NodeIndex, TransitionIdx)> {
        match self.order {
            ExplorationOrder::BreadthFirst => self.frontier.pop_front(),
            ExplorationOrder::DepthFirst => self.frontier.pop_back(),
        }
    }

    /// Whether a transition is enabled at the marking stored in `node`.
    pub fn is_enabled(&self, node: NodeIndex, t: TransitionIdx) -> bool {
        let marking = &self.state_space.graph[node];
        self.state_space.net.preset_t[t].iter().all(|&p| marking[p].at_least_one())
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
        if let Some(&idx) = self.state_space.seen.get(&marking) {
            self.state_space.graph.add_edge(from, idx, over);
            return (false, idx);
        }

        let idx = self.state_space.graph.add_node(marking.clone());
        self.state_space.graph.add_edge(from, idx, over);

        // seed frontier with all transitions that could possibly be enabled at this marking
        marking
            .support()
            .flat_map(|p_idx| self.state_space.net.postset_p[p_idx].iter().copied())
            .chain(self.source_transitions.iter().copied())
            .collect::<HashSet<TransitionIdx>>() // dedup
            .into_iter()
            .for_each(|t_idx| self.frontier.push_back((idx, t_idx)));

        self.state_space.seen.insert(marking, idx);

        (true, idx)
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
    /// Returns an iterator over all markings in the graph.
    pub fn markings(&self) -> impl Iterator<Item = &IdxMarking<T>> + '_ {
        self.graph.node_weights()
    }

    /// Get the marking stored at node index `idx`.
    pub fn marking_at(&self, idx: NodeIndex) -> &IdxMarking<T> {
        &self.graph[idx]
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

    /// Find a path from initial to target using A*.
    pub fn path_from_initial_to(&self, target: NodeIndex) -> Option<Box<[TransitionIdx]>> {
        if target == self.initial_idx {
            return Some(Box::new([]));
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
                self.graph.find_edge(m1_idx, m2_idx).expect("edge must exist")
            })
            .map(|edge_idx| self.graph[edge_idx])
            .collect();
        Some(firing_sequence)
    }
}