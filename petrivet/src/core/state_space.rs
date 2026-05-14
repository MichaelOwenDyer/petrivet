use crate::core::marking::{IdxMarking, IdxOmegaMarking};
use crate::core::{DenseNet, TransitionIdx};
use crate::api::marking::Omega;
use petgraph::graph::NodeIndex;
use petgraph::prelude::EdgeRef;
use petgraph::Graph;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

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
///
/// Implemented for `u32` (reachability) and `Omega` (coverability).
pub trait TokenOps: Clone + Copy + Eq + Ord + Hash + Default {
    fn zero() -> Self;
    fn one() -> Self;
    fn at_least_one(&self) -> bool;
    fn increment(&mut self);
    fn decrement(&mut self);
}

impl TokenOps for u32 {
    fn zero() -> Self { 0 }
    fn one() -> Self { 1 }
    fn at_least_one(&self) -> bool { *self >= 1 }
    fn increment(&mut self) { *self += 1; }
    fn decrement(&mut self) { *self -= 1; }
}

impl TokenOps for Omega {
    fn zero() -> Self { Omega::Finite(0) }
    fn one() -> Self { Omega::Finite(1) }
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
        let mut graph = Graph::new();
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

impl DenseStateGraphExplorer<'_, u32> {
    /// Advance exploration by one step.
    ///
    /// Returns `None` when the frontier is exhausted (fully explored).
    ///
    /// The second tuple element is the graph [`NodeIndex`] of the marking
    /// reached by firing the transition (new or existing).
    pub fn explore_next(&mut self) -> Option<(TransitionIdx, NodeIndex, bool)> {
        loop {
            let (src_idx, t_idx) = self.pop_frontier()?;
            if !self.is_enabled(src_idx, t_idx) {
                continue;
            }
            let new_marking = self.fire(src_idx, t_idx);
            let (is_new, node_idx) = self.register(src_idx, t_idx, new_marking);
            return Some((t_idx, node_idx, is_new));
        }
    }

    /// Drive exploration until either:
    ///
    /// - `predicate` returns `true` for some reachable marking — in which
    ///   case the marking is returned immediately, or
    /// - the frontier is exhausted (the entire reachability graph has been
    ///   explored without the predicate ever firing) — in which case
    ///   `None` is returned.
    ///
    /// **Does not terminate on unbounded nets.** Callers must rule that
    /// out before calling — typically via
    /// [`Net::is_structurally_bounded`](crate::Net::is_structurally_bounded)
    /// or by going through the coverability path first.
    pub fn search(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking<u32>) -> bool,
    ) -> Option<&IdxMarking<u32>> {
        for &node in self.state_space.seen.values() {
            if predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        while let Some((_t_idx, node, is_new)) = self.explore_next() {
            if is_new && predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        None
    }
}

impl DenseStateGraphExplorer<'_, Omega> {
    pub fn explore_next(&mut self) -> Option<(TransitionIdx, NodeIndex, bool)> {
        /// Karp–Miller acceleration: if any ancestor of `src` (including `src`
        /// itself) carries a marking strictly smaller than `new_marking`,
        /// promote each strictly-greater component of `new_marking` to ω.
        fn omega_accelerate(
            state_space: &DenseStateGraph<'_, Omega>,
            new_marking: &mut IdxOmegaMarking, src: NodeIndex
        ) {
            let mut stack = vec![src];
            let mut visited: HashSet<NodeIndex> = HashSet::new();
            while let Some(predecessor_node) = stack.pop() {
                if !visited.insert(predecessor_node) {
                    continue;
                }
                let ancestor_marking = state_space.marking_at(predecessor_node);
                if ancestor_marking < new_marking {
                    for (component, prev) in new_marking.iter_mut().zip(ancestor_marking.iter()) {
                        if *component > *prev {
                            *component = Omega::Unbounded;
                        }
                    }
                }
                for incoming_edge in state_space.graph.edges_directed(
                    predecessor_node,
                    petgraph::Direction::Incoming
                ) {
                    stack.push(incoming_edge.source());
                }
            }
        }

        loop {
            let (src_node_idx, transition_idx) = self.pop_frontier()?;
            if !self.is_enabled(src_node_idx, transition_idx) {
                continue;
            }
            let mut marking = self.fire(src_node_idx, transition_idx);
            omega_accelerate(&self.state_space, &mut marking, src_node_idx);
            let (is_new, node_idx) = self.register(src_node_idx, transition_idx, marking.clone());
            return Some((transition_idx, node_idx, is_new));
        }
    }

    /// Drive exploration until either:
    ///
    /// - `predicate` returns `true` for some reachable marking — in which
    ///   case the marking is returned immediately, or
    /// - the frontier is exhausted (the entire reachability graph has been
    ///   explored without the predicate ever firing) — in which case
    ///   `None` is returned.
    pub fn find(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking<Omega>) -> bool,
    ) -> Option<&IdxOmegaMarking> {
        for &node in self.state_space.seen.values() {
            if predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        while let Some((_t_idx, node, is_new)) = self.explore_next() {
            if is_new && predicate(self.state_space.marking_at(node)) {
                return Some(self.state_space.marking_at(node));
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct DenseStateGraph<'a, T: TokenOps> {
    /// Reference to the net.
    pub net: &'a DenseNet,
    /// Reference to the graph's initial node, for pathfinding.
    pub initial_idx: NodeIndex,
    /// The underlying graph structure. Nodes are markings, edges are transitions.
    pub graph: Graph<IdxMarking<T>, TransitionIdx>,
    /// A hash table of seen markings to their node indices in the graph,
    /// for O(1) lookup.
    pub seen: HashMap<IdxMarking<T>, NodeIndex>,
}

/// A fully explored state space.
impl<T: TokenOps> DenseStateGraph<'_, T> {
    pub fn markings(&self) -> impl Iterator<Item = &IdxMarking<T>> + '_ {
        self.graph.node_weights()
    }

    /// Reference to the marking at a given node.
    pub fn marking_at(&self, idx: NodeIndex) -> &IdxMarking<T> {
        &self.graph[idx]
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