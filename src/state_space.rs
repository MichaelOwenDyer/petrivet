//! State space exploration for Petri net systems.
//!
//! Build the reachability graph from an initial marking, then query it for
//! reachability, deadlocks, and firing sequences.
//!
//! ```ignore
//! use petrivet::state_space::{StateSpace, Limits};
//!
//! let ss = StateSpace::build_bfs(&sys, Limits::default());
//! assert!(ss.is_complete());
//! if let Some(path) = ss.path_to(&target) {
//!     println!("Reachable via: {path:?}");
//! }
//! ```

use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::system::System;
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::{HashMap, VecDeque};

/// Bounds on state space exploration.
///
/// Both fields default to `None` (no limit). Use the builder-style setters
/// or struct literal syntax.
#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    /// Stop after discovering this many states.
    pub max_states: Option<usize>,
    /// Don't explore states deeper than this many firing steps from the initial marking.
    pub max_depth: Option<usize>,
}

impl Limits {
    /// No limits — explore the full state space.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_max_states(mut self, n: usize) -> Self {
        self.max_states = Some(n);
        self
    }

    #[must_use]
    pub fn with_max_depth(mut self, d: usize) -> Self {
        self.max_depth = Some(d);
        self
    }
}

/// The reachability graph of a Petri net system.
///
/// Nodes are markings, edges are labeled with the transition that was fired.
/// Built via [`build_bfs`](Self::build_bfs) or [`build_dfs`](Self::build_dfs),
/// then queried for reachability, deadlocks, etc.
pub struct StateSpace {
    graph: Graph<StateNode, Transition>,
    initial: NodeIndex,
    seen: HashMap<Marking, NodeIndex>,
    complete: bool,
}

#[derive(Debug, Clone)]
struct StateNode {
    marking: Marking,
    depth: u32,
}

/// Whether to pop from the front (BFS) or back (DFS) of the frontier.
#[derive(Debug, Clone, Copy)]
enum Order { Fifo, Lifo }

impl StateSpace {
    /// Build the reachability graph using breadth-first exploration.
    ///
    /// BFS guarantees that `path_to` returns a shortest firing sequence.
    #[must_use]
    pub fn build_bfs(sys: &System<impl AsRef<Net>>, limits: Limits) -> Self {
        Self::explore(sys, limits, Order::Fifo)
    }

    /// Build the reachability graph using depth-first exploration.
    #[must_use]
    pub fn build_dfs(sys: &System<impl AsRef<Net>>, limits: Limits) -> Self {
        Self::explore(sys, limits, Order::Lifo)
    }

    fn explore(sys: &System<impl AsRef<Net>>, limits: Limits, order: Order) -> Self {
        let net = sys.as_ref();
        let m0 = sys.marking().clone();

        let mut graph = Graph::new();
        let mut seen = HashMap::new();
        let mut frontier: VecDeque<NodeIndex> = VecDeque::new();

        let root = graph.add_node(StateNode { marking: m0.clone(), depth: 0 });
        seen.insert(m0, root);
        frontier.push_back(root);

        let mut complete = true;

        while let Some(src_idx) = match order {
            Order::Fifo => frontier.pop_front(),
            Order::Lifo => frontier.pop_back(),
        } {
            let src = &graph[src_idx];
            let src_depth = src.depth;
            let src_marking = src.marking.clone();

            if limits.max_depth.is_some_and(|d| src_depth as usize >= d) {
                complete = false;
                continue;
            }

            for t in net.transitions() {
                if !net.preset_t(t).iter().all(|&p| src_marking[p] >= 1) {
                    continue;
                }

                let mut new_marking = src_marking.clone();
                for &p in net.preset_t(t) {
                    new_marking[p] -= 1;
                }
                for &p in net.postset_t(t) {
                    new_marking[p] += 1;
                }

                let dst_idx = if let Some(&idx) = seen.get(&new_marking) {
                    // update the depth if we found a shorter path to an already-seen marking
                    let dst = &mut graph[idx];
                    if src_depth + 1 < dst.depth {
                        dst.depth = src_depth + 1;
                    }
                    idx
                } else {
                    if limits.max_states.is_some_and(|m| seen.len() >= m) {
                        complete = false;
                        continue;
                    }
                    let idx = graph.add_node(StateNode {
                        marking: new_marking.clone(),
                        depth: src_depth + 1,
                    });
                    seen.insert(new_marking, idx);
                    frontier.push_back(idx);
                    idx
                };

                graph.add_edge(src_idx, dst_idx, t);
            }
        }

        Self { graph, initial: root, seen, complete }
    }

    /// Whether exploration completed without hitting any limits.
    ///
    /// If `true`, the graph contains every reachable marking — queries give
    /// definitive answers. If `false`, a negative query result is inconclusive.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Number of distinct reachable markings discovered.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// The initial marking (root of the graph).
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        &self.graph[self.initial].marking
    }

    /// Returns a firing sequence from the initial marking to `target`, if
    /// `target` was discovered during exploration.
    ///
    /// When built with BFS, this is a shortest firing sequence.
    #[must_use]
    pub fn path_to(&self, target: &Marking) -> Option<Vec<Transition>> {
        let &target_idx = self.seen.get(target)?;
        if target_idx == self.initial {
            return Some(Vec::new());
        }

        // BFS back from initial to target on the directed graph
        let (_, node_path) = petgraph::algo::astar(
            &self.graph,
            self.initial,
            |n| n == target_idx,
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

    /// Whether every discovered state has at least one enabled transition.
    ///
    /// Only definitive if [`is_complete`](Self::is_complete) returns `true`.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.deadlocks().is_empty()
    }

    /// Returns references to all discovered markings with no enabled transitions.
    #[must_use]
    pub fn deadlocks(&self) -> Vec<&Marking> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .next()
                    .is_none()
            })
            .map(|idx| &self.graph[idx].marking)
            .collect()
    }

    /// Returns all discovered markings.
    #[must_use]
    pub fn markings(&self) -> Vec<&Marking> {
        self.graph.node_weights().map(|n| &n.marking).collect()
    }

    /// Whether `target` was discovered during exploration.
    #[must_use]
    pub fn contains(&self, target: &Marking) -> bool {
        self.seen.contains_key(target)
    }
}

impl<N: AsRef<Net>> AsRef<Net> for System<N> {
    fn as_ref(&self) -> &Net {
        self.net().as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    use crate::net::class::ClassifiedNet;
    use crate::net::Place;

    fn m(val: impl Into<Marking>) -> Marking { val.into() }

    /// p0 -> t0 -> p1 -> t1 -> p0 (two-place cycle)
    fn two_place_cycle() -> System<ClassifiedNet> {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().expect("valid net");
        System::new(net, [1, 0])
    }

    /// Self-loop with 0 initial tokens — immediate deadlock
    fn deadlock_net() -> System<ClassifiedNet> {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        let net = b.build().expect("valid net");
        System::new(net, [0])
    }

    /// p0 feeds t0 which produces back to p0 and also to p1 (unbounded growth)
    fn unbounded_net() -> System<ClassifiedNet> {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        System::new(net, [1, 0])
    }

    #[test]
    fn full_exploration_bfs() {
        let sys = two_place_cycle();
        let ss = StateSpace::build_bfs(&sys, Limits::none());

        assert!(ss.is_complete());
        assert_eq!(ss.state_count(), 2);
        assert!(ss.contains(&m([1, 0])));
        assert!(ss.contains(&m([0, 1])));
        assert!(ss.is_deadlock_free());
    }

    #[test]
    fn full_exploration_dfs() {
        let sys = two_place_cycle();
        let ss = StateSpace::build_dfs(&sys, Limits::none());

        assert!(ss.is_complete());
        assert_eq!(ss.state_count(), 2);
        assert!(ss.is_deadlock_free());
    }

    #[test]
    fn path_to_reachable() {
        let sys = two_place_cycle();
        let ss = StateSpace::build_bfs(&sys, Limits::none());

        let path = ss.path_to(&m([0, 1])).expect("should be reachable");
        assert_eq!(path.len(), 1);

        // Path to initial marking is empty
        let path = ss.path_to(&m([1, 0])).expect("initial is trivially reachable");
        assert!(path.is_empty());
    }

    #[test]
    fn path_to_unreachable() {
        let sys = two_place_cycle();
        let ss = StateSpace::build_bfs(&sys, Limits::none());

        assert!(ss.path_to(&m([1, 1])).is_none());
    }

    #[test]
    fn deadlock_detected() {
        let sys = deadlock_net();
        let ss = StateSpace::build_bfs(&sys, Limits::none());

        assert!(ss.is_complete());
        assert_eq!(ss.state_count(), 1);
        assert!(!ss.is_deadlock_free());
        assert_eq!(ss.deadlocks().len(), 1);
    }

    #[test]
    fn max_states_limit() {
        let sys = unbounded_net();
        let ss = StateSpace::build_bfs(&sys, Limits::none().with_max_states(10));

        assert!(!ss.is_complete());
        assert_eq!(ss.state_count(), 10);
    }

    #[test]
    fn max_depth_limit() {
        let sys = unbounded_net();
        let ss = StateSpace::build_bfs(&sys, Limits::none().with_max_depth(5));

        assert!(!ss.is_complete());
        for marking in ss.markings() {
            // p1 accumulates tokens: depth d means p1 has d tokens
            assert_eq!(marking[Place::from_index(1)], 5);
        }
    }
}
