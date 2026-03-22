//! Shared exploration core for reachability and coverability graph construction.
//!
//! This module is crate-private. Users interact with
//! [`CoverabilityGraph`](crate::CoverabilityExplorer) and
//! [`ReachabilityGraph`](crate::ReachabilityGraph) instead.

use crate::marking::{Marking, Omega};
use crate::net::{Net, Transition};
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

/// Operations on a token count needed for state space exploration.
///
/// Implemented for `u32` (reachability) and `Omega` (coverability).
pub(super) trait TokenOps: Clone + Eq + Hash + Default {
    fn at_least_one(&self) -> bool;
    fn increment(&mut self);
    fn decrement(&mut self);
}

impl TokenOps for u32 {
    fn at_least_one(&self) -> bool { *self >= 1 }
    fn increment(&mut self) { *self += 1; }
    fn decrement(&mut self) { *self -= 1; }
}

impl TokenOps for Omega {
    fn at_least_one(&self) -> bool {
        match self {
            Omega::Finite(n) => *n >= 1,
            Omega::Unbounded => true,
        }
    }
    fn increment(&mut self) {
        if let Omega::Finite(n) = self { *n += 1; }
    }
    fn decrement(&mut self) {
        if let Omega::Finite(n) = self { *n -= 1; }
    }
}

/// Controls frontier traversal order.
// TODO: Find better home for this?
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ExplorationOrder {
    /// Breadth-first: `path_to` returns shortest firing sequences.
    #[default]
    BreadthFirst,
    /// Depth-first: may use less memory on wide state spaces.
    DepthFirst,
}

/// The shared exploration engine for both reachability and coverability graphs.
///
/// Manages the petgraph, seen-set, and frontier. Both `CoverabilityGraph` and
/// `ReachabilityGraph` own one of these and drive it via the helper methods.
///
/// Borrows the [`Net`] for its lifetime - the graph cannot outlive the net
/// it explores.
#[derive(Debug, Clone)]
pub(super) struct StateSpaceExplorer<'a, T: TokenOps> {
    /// The state space being explored.
    /// Can be extracted once exploration is complete.
    pub(super) state_space: StateGraph<'a, T>,
    /// The exploration order: breadth-first or depth-first.
    /// Corresponds to queue vs stack behavior of the frontier.
    pub(super) order: ExplorationOrder,
    /// The worklist of potentially enabled transitions which we have not
    /// yet investigated firing from their source markings.
    frontier: VecDeque<(NodeIndex, Transition)>,
    /// Transitions with empty presets - always enabled, and should
    /// always be explored from every new marking regardless of the
    /// marked places.
    source_transitions: Box<[Transition]>,
}

impl<'a, T: TokenOps> StateSpaceExplorer<'a, T> {
    /// Create a new explorer from a net reference and initial marking.
    ///
    /// Seeds the frontier with source transitions (empty preset, always
    /// enabled) plus transitions whose presets overlap with the support
    /// of the initial marking.
    pub fn new(net: &'a Net, initial_marking: Marking<T>, order: ExplorationOrder) -> Self {
        let mut graph = Graph::new();
        let initial_idx = graph.add_node(initial_marking.clone());

        let source_transitions: Box<[Transition]> = net
            .transitions()
            .filter(|&t| net.dense_input_places(t).is_empty())
            .collect();

        let frontier: VecDeque<_> = net.places()
            .filter(|&p| initial_marking[p].at_least_one())
            .flat_map(|p| net.dense_output_transitions(p).iter().copied())
            .chain(source_transitions.iter().copied())
            .collect::<HashSet<Transition>>()
            .into_iter()
            .map(|t| (initial_idx, t))
            .collect();

        let mut seen = HashMap::new();
        seen.insert(initial_marking, initial_idx);

        let state_space = StateGraph { net, initial_idx, graph, seen };

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
    pub fn pop_frontier(&mut self) -> Option<(NodeIndex, Transition)> {
        match self.order {
            ExplorationOrder::BreadthFirst => self.frontier.pop_front(),
            ExplorationOrder::DepthFirst => self.frontier.pop_back(),
        }
    }

    /// Whether a transition is enabled at the marking stored in `node`.
    pub fn is_enabled(&self, node: NodeIndex, t: Transition) -> bool {
        let marking = &self.state_space.graph[node];
        self.state_space.net.dense_input_places(t).iter().all(|&p| marking[p].at_least_one())
    }

    /// Compute the marking that results from firing `t` at `node`.
    ///
    /// Caller must ensure the transition is enabled.
    pub fn fire(&self, node: NodeIndex, t: Transition) -> Marking<T> {
        let mut result = self.state_space.graph[node].clone();
        for &p in self.state_space.net.dense_input_places(t) {
            result[p].decrement();
        }
        for &p in self.state_space.net.dense_output_places(t) {
            result[p].increment();
        }
        result
    }

    /// Register a marking in the graph.
    ///
    /// If already seen, adds an edge and returns `(existing_index, false)`.
    /// If new, adds the node, seeds the frontier with all potentially enabled
    /// transitions, adds the edge, and returns `(new_index, true)`.
    pub fn register(
        &mut self,
        from: NodeIndex,
        over: Transition,
        marking: Marking<T>,
    ) -> (NodeIndex, bool) {
        if let Some(&idx) = self.state_space.seen.get(&marking) {
            self.state_space.graph.add_edge(from, idx, over);
            return (idx, false);
        }

        let idx = self.state_space.graph.add_node(marking.clone());
        self.state_space.graph.add_edge(from, idx, over);

        // seed frontier with all transitions that could possibly be enabled at this marking
        self.state_space.net
            .places()
            .filter(|&p| marking[p].at_least_one())
            .flat_map(|p| self.state_space.net.dense_output_transitions(p).iter().copied())
            .chain(self.source_transitions.iter().copied())
            .collect::<HashSet<Transition>>()
            .into_iter()
            .for_each(|t| self.frontier.push_back((idx, t)));

        self.state_space.seen.insert(marking, idx);

        (idx, true)
    }
}

#[derive(Debug, Clone)]
pub(super) struct StateGraph<'a, T: TokenOps> {
    /// Reference to the net.
    pub net: &'a Net,
    /// Reference to the graph's initial node, for pathfinding.
    pub initial_idx: NodeIndex,
    /// The underlying graph structure. Nodes are markings, edges are transitions.
    pub graph: Graph<Marking<T>, Transition>,
    /// A hash table of seen markings to their node indices in the graph,
    /// for O(1) lookup.
    pub seen: HashMap<Marking<T>, NodeIndex>,
}

/// A fully explored state space.
impl<T: TokenOps> StateGraph<'_, T> {
    /// Reference to the marking at a given node.
    pub fn marking_at(&self, idx: NodeIndex) -> &Marking<T> {
        &self.graph[idx]
    }

    /// Find a path from initial to target using A*.
    pub fn path_to(&self, target: NodeIndex) -> Option<Box<[Transition]>> {
        if target == self.initial_idx {
            return Some(Box::new([]));
        }
        let (_, node_path) = petgraph::algo::astar(
            &self.graph,
            self.initial_idx,
            |n| n == target,
            |_| 1u32,
            |_| 0u32,
        )?;
        let transition_path = node_path
            .array_windows()
            .map(|&[m1_idx, m2_idx]| {
                self.graph.find_edge(m1_idx, m2_idx).expect("edge must exist")
            })
            .map(|edge_idx| self.graph[edge_idx])
            .collect();
        Some(transition_path)
    }

    /// Node indices with no outgoing edges (deadlocked states).
    pub fn deadlock_indices(&self) -> impl Iterator<Item = NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .next()
                    .is_none()
            })
    }
}
