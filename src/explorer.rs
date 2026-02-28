//! Shared exploration core for reachability and coverability graph construction.
//!
//! This module is crate-private. Users interact with [`CoverabilityGraph`](crate::coverability)
//! and [`ReachabilityGraph`](crate::reachability) instead.

use crate::marking::{Marking, Omega};
use crate::net::{Net, Transition};
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

/// Operations on a token count needed for state space exploration.
///
/// Implemented for `u32` (reachability) and `Omega` (coverability).
pub(crate) trait TokenOps: Clone + Eq + Hash + Default {
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
/// Borrows the [`Net`] for its lifetime — the graph cannot outlive the net
/// it explores.
#[derive(Debug, Clone)]
pub(crate) struct ExplorerCore<'a, T: TokenOps> {
    pub graph: Graph<Marking<T>, Transition>,
    pub seen: HashMap<Marking<T>, NodeIndex>,
    pub frontier: VecDeque<(NodeIndex, Transition)>,
    pub net: &'a Net,
    pub initial: NodeIndex,
    pub(crate) order: ExplorationOrder,
    /// Transitions with empty presets — always enabled, must be seeded
    /// for every new node since the frontier optimization would miss them.
    pub(crate) source_transitions: Box<[Transition]>,
}

impl<'a, T: TokenOps> ExplorerCore<'a, T> {
    /// Returns a reference to the underlying petgraph.
    pub fn graph(&self) -> &Graph<Marking<T>, Transition> {
        &self.graph
    }

    /// Create a new explorer from a net reference and initial marking.
    ///
    /// Seeds the frontier with source transitions (empty preset, always
    /// enabled) plus transitions whose presets overlap with the support
    /// of the initial marking.
    pub fn new(net: &'a Net, initial_marking: Marking<T>, order: ExplorationOrder) -> Self {
        let mut graph = Graph::new();
        let mut seen = HashMap::new();
        let mut frontier = VecDeque::new();

        let source_transitions: Box<[Transition]> = net
            .transitions()
            .filter(|&t| net.preset_t(t).is_empty())
            .collect();

        let root = graph.add_node(initial_marking.clone());

        net.places()
            .filter(|&p| initial_marking[p].at_least_one())
            .flat_map(|p| net.postset_p(p).iter().copied())
            .chain(source_transitions.iter().copied())
            .collect::<HashSet<Transition>>()
            .into_iter()
            .for_each(|t| frontier.push_back((root, t)));

        seen.insert(initial_marking, root);

        Self { graph, seen, frontier, net, initial: root, order, source_transitions }
    }

    /// Change the exploration order for subsequent steps.
    pub fn set_exploration_order(&mut self, order: ExplorationOrder) {
        self.order = order;
    }

    pub fn exploration_order(&self) -> ExplorationOrder {
        self.order
    }

    /// Pop the next `(NodeIndex, Transition)` from the frontier.
    pub fn pop(&mut self) -> Option<(NodeIndex, Transition)> {
        match self.order {
            ExplorationOrder::BreadthFirst => self.frontier.pop_front(),
            ExplorationOrder::DepthFirst => self.frontier.pop_back(),
        }
    }

    /// Whether a transition is enabled at the marking stored in `node`.
    pub fn is_enabled(&self, node: NodeIndex, t: Transition) -> bool {
        let marking = &self.graph[node];
        self.net.preset_t(t).iter().all(|&p| marking[p].at_least_one())
    }

    /// Compute the marking that results from firing `t` at `node`.
    ///
    /// Caller must ensure the transition is enabled.
    pub fn fire(&self, node: NodeIndex, t: Transition) -> Marking<T> {
        let mut result = self.graph[node].clone();
        for &p in self.net.preset_t(t) {
            result[p].decrement();
        }
        for &p in self.net.postset_t(t) {
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
        if let Some(&idx) = self.seen.get(&marking) {
            self.graph.add_edge(from, idx, over);
            return (idx, false);
        }

        let idx = self.graph.add_node(marking.clone());
        self.graph.add_edge(from, idx, over);

        // seed frontier with all transitions that could possibly be enabled at this marking
        self.net
            .places()
            .filter(|&p| marking[p].at_least_one())
            .flat_map(|p| self.net.postset_p(p).iter().copied())
            .chain(self.source_transitions.iter().copied())
            .collect::<HashSet<Transition>>()
            .into_iter()
            .for_each(|t| self.frontier.push_back((idx, t)));

        self.seen.insert(marking, idx);

        (idx, true)
    }

    /// Reference to the marking at a given node.
    pub fn marking_at(&self, idx: NodeIndex) -> &Marking<T> {
        &self.graph[idx]
    }

    /// Whether the frontier is empty (exploration complete).
    pub fn is_fully_explored(&self) -> bool {
        self.frontier.is_empty()
    }

    /// Find a path from initial to target using A*.
    pub fn path_to(&self, target: NodeIndex) -> Option<Vec<Transition>> {
        if target == self.initial {
            return Some(Vec::new());
        }
        let (_, node_path) = petgraph::algo::astar(
            &self.graph,
            self.initial,
            |n| n == target,
            |_| 1u32,
            |_| 0u32,
        )?;
        let mut transitions = Vec::with_capacity(node_path.len() - 1);
        for pair in node_path.windows(2) {
            let edge = self.graph.find_edge(pair[0], pair[1])?;
            transitions.push(self.graph[edge]);
        }
        Some(transitions)
    }

    /// Node indices with no outgoing edges (deadlocked states).
    pub fn deadlock_indices(&self) -> Vec<NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .next()
                    .is_none()
            })
            .collect()
    }
}
