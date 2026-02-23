//! Reachability graph construction and queries.
//!
//! The reachability graph enumerates all markings reachable from the initial
//! marking. For bounded nets this is finite; for unbounded nets it grows
//! without bound — use [`explore_next`](ReachabilityGraph::explore_next) or
//! [`iter`](ReachabilityGraph::iter) with your own termination condition.
//!
//! The recommended workflow for unknown nets is to build a
//! [`CoverabilityGraph`](CoverabilityGraph) first. If it
//! turns out bounded, promote it to a `ReachabilityGraph` via
//! [`into_reachability_graph`](CoverabilityGraph::into_reachability_graph)
//! at near-zero cost.

use crate::explorer::{ExplorationOrder, ExplorerCore};
use crate::marking::{Marking, Omega};
use crate::net::{Net, Transition};
use crate::system::System;
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::HashMap;
use crate::coverability::CoverabilityGraph;

/// A single step in reachability graph exploration.
#[derive(Debug, Clone)]
pub struct ReachabilityStep {
    /// The transition that was fired.
    pub transition: Transition,
    /// The resulting marking.
    pub marking: Marking,
    /// Whether this marking was newly discovered.
    pub is_new: bool,
}

/// The reachability graph of a Petri net system.
///
/// Nodes are concrete markings (`Marking<u32>`), edges are labeled with the
/// transition that was fired. Queries give exact answers (no ω approximation).
///
/// Use [`build`](Self::build) for one-shot construction (bounded nets only),
/// or [`new`](Self::new) + [`explore_next`](Self::explore_next) /
/// [`iter`](Self::iter) for step-by-step control with your own termination
/// condition.
pub struct ReachabilityGraph<'a> {
    core: ExplorerCore<'a, u32>,
}

impl<'a> ReachabilityGraph<'a> {
    /// Create an unexplored reachability graph from a system.
    #[must_use]
    pub fn new(sys: &'a System<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        let net = sys.net().as_ref();
        let marking = sys.marking().clone();
        Self {
            core: ExplorerCore::new(net, marking, order),
        }
    }

    /// Build a fully explored reachability graph from a system.
    ///
    /// **Warning: does not terminate** for unbounded nets. Use the
    /// coverability graph first to check boundedness, or drive exploration
    /// manually via [`explore_next`](Self::explore_next) / [`iter`](Self::iter).
    #[must_use]
    pub fn build(sys: &'a System<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        let mut rg = Self::new(sys, order);
        rg.explore_all();
        rg
    }

    /// Change the exploration order for subsequent steps.
    pub fn set_exploration_order(&mut self, order: ExplorationOrder) {
        self.core.set_order(order);
    }

    /// Current exploration order.
    #[must_use]
    pub fn exploration_order(&self) -> ExplorationOrder {
        self.core.order()
    }

    /// Advance exploration by one step.
    ///
    /// Returns `None` when the frontier is exhausted (fully explored).
    /// For unbounded nets, the frontier never empties — the caller is
    /// responsible for termination (e.g. checking [`state_count`](Self::state_count)).
    pub fn explore_next(&mut self) -> Option<ReachabilityStep> {
        loop {
            let (src, t) = self.core.pop()?;
            if !self.core.is_enabled(src, t) {
                continue;
            }
            let new_marking = self.core.fire(src, t);
            let (_, is_new) = self.core.register(src, t, new_marking.clone());
            return Some(ReachabilityStep {
                transition: t,
                marking: new_marking,
                is_new,
            });
        }
    }

    /// Returns an iterator that drives exploration step by step.
    ///
    /// Each call to `next()` fires one transition and returns the step.
    /// The iterator ends when the frontier is exhausted.
    ///
    /// Use standard iterator combinators for termination:
    ///
    /// ```ignore
    /// // Explore up to 1000 steps
    /// for step in rg.iter().take(1000) { /* ... */ }
    ///
    /// // Find a specific marking
    /// let found = rg.iter().any(|s| s.marking == target);
    /// ```
    pub fn iter(&mut self) -> impl Iterator<Item = ReachabilityStep> + '_ {
        std::iter::from_fn(move || self.explore_next())
    }

    /// Explore until the frontier is exhausted.
    ///
    /// **Warning: does not terminate** for unbounded nets.
    pub fn explore_all(&mut self) {
        while self.explore_next().is_some() {}
    }

    /// Whether the frontier is empty (no more states to explore).
    #[must_use]
    pub fn is_fully_explored(&self) -> bool {
        self.core.is_fully_explored()
    }

    /// Number of distinct markings discovered.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.core.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.core.graph.edge_count()
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        self.core.marking_at(self.core.initial)
    }

    /// Whether `target` was discovered during exploration.
    #[must_use]
    pub fn is_reachable(&self, target: &Marking) -> bool {
        self.core.seen.contains_key(target)
    }

    /// Returns a firing sequence from the initial marking to `target`.
    ///
    /// When built with BFS (and no order switching), this is a shortest
    /// firing sequence.
    #[must_use]
    pub fn path_to(&self, target: &Marking) -> Option<Vec<Transition>> {
        let &target_idx = self.core.seen.get(target)?;
        self.core.path_to(target_idx)
    }

    /// All discovered markings with no enabled transitions.
    #[must_use]
    pub fn deadlocks(&self) -> Vec<&Marking> {
        self.core
            .deadlock_indices()
            .iter()
            .map(|&idx| self.core.marking_at(idx))
            .collect()
    }

    /// Whether all discovered markings have at least one enabled transition.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.core.deadlock_indices().is_empty()
    }

    /// All discovered markings.
    #[must_use]
    pub fn markings(&self) -> Vec<&Marking> {
        self.core.graph.node_weights().collect()
    }

    /// Whether a marking has been discovered.
    #[must_use]
    pub fn contains(&self, marking: &Marking) -> bool {
        self.core.seen.contains_key(marking)
    }

    /// Convert a fully explored, bounded coverability graph into a
    /// reachability graph.
    ///
    /// This is called by
    /// [`CoverabilityGraph::into_reachability_graph`](CoverabilityGraph::into_reachability_graph).
    /// All `Omega::Finite(k)` values are unwrapped to `k`.
    pub(crate) fn from_coverability(cg: CoverabilityGraph<'a>) -> Self {
        let cg_core = cg.into_core();
        let order = cg_core.order;
        let cg_initial = cg_core.initial;
        let net = cg_core.net;

        let source_transitions: Box<[Transition]> = net
            .transitions()
            .filter(|&t| net.preset_t(t).is_empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let mut graph: Graph<Marking<u32>, Transition> =
            Graph::with_capacity(cg_core.graph.node_count(), cg_core.graph.edge_count());
        let mut index_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut seen: HashMap<Marking<u32>, NodeIndex> = HashMap::new();

        for old_idx in cg_core.graph.node_indices() {
            let u32_marking = omega_marking_to_u32(&cg_core.graph[old_idx]);
            let new_idx = graph.add_node(u32_marking.clone());
            index_map.insert(old_idx, new_idx);
            seen.insert(u32_marking, new_idx);
        }

        for edge in cg_core.graph.edge_indices() {
            let (src, dst) = cg_core.graph.edge_endpoints(edge).unwrap();
            let t = cg_core.graph[edge];
            graph.add_edge(index_map[&src], index_map[&dst], t);
        }

        let initial = index_map[&cg_initial];

        Self {
            core: ExplorerCore {
                graph,
                seen,
                frontier: std::collections::VecDeque::new(),
                net,
                initial,
                order,
                source_transitions,
            },
        }
    }
}

/// Unwrap an `OmegaMarking` with all-finite components to a `Marking<u32>`.
fn omega_marking_to_u32(om: &Marking<Omega>) -> Marking<u32> {
    om.iter()
        .map(|o| match o {
            Omega::Finite(n) => *n,
            Omega::Unbounded => unreachable!("called on bounded graph"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    use crate::net::class::ClassifiedNet;

    fn m(val: impl Into<Marking>) -> Marking {
        val.into()
    }

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0
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

    /// Unbounded: t0 feeds back to p0 and also produces to p1
    fn unbounded_producer() -> System<ClassifiedNet> {
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
    fn full_exploration() {
        let sys = two_place_cycle();
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(rg.is_fully_explored());
        assert_eq!(rg.state_count(), 2);
        assert!(rg.is_reachable(&m([1, 0])));
        assert!(rg.is_reachable(&m([0, 1])));
        assert!(!rg.is_reachable(&m([1, 1])));
        assert!(rg.is_deadlock_free());
    }

    #[test]
    fn path_to_reachable() {
        let sys = two_place_cycle();
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        let path = rg.path_to(&m([0, 1])).expect("reachable");
        assert_eq!(path.len(), 1);

        let path = rg.path_to(&m([1, 0])).expect("initial");
        assert!(path.is_empty());
    }

    #[test]
    fn path_to_unreachable() {
        let sys = two_place_cycle();
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        assert!(rg.path_to(&m([1, 1])).is_none());
    }

    #[test]
    fn limited_exploration_by_state_count() {
        let sys = unbounded_producer();
        let mut rg = ReachabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);

        while rg.state_count() < 10 {
            if rg.explore_next().is_none() { break; }
        }
        assert!(!rg.is_fully_explored());
        assert!(rg.state_count() >= 10);
    }

    #[test]
    fn iter_take() {
        let sys = unbounded_producer();
        let mut rg = ReachabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);

        let steps: Vec<_> = rg.iter().take(5).collect();
        assert_eq!(steps.len(), 5);
        assert!(!rg.is_fully_explored());
    }

    #[test]
    fn step_by_step() {
        let sys = two_place_cycle();
        let mut rg = ReachabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);

        assert_eq!(rg.state_count(), 1);
        let mut count = 0;
        while let Some(_step) = rg.explore_next() {
            count += 1;
        }
        assert!(count > 0);
        assert_eq!(rg.state_count(), 2);
        assert!(rg.is_fully_explored());
    }

    #[test]
    fn deadlock_detected() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        let net = b.build().expect("valid net");
        let sys = System::new(net, [0]);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        assert!(!rg.is_deadlock_free());
        assert_eq!(rg.deadlocks().len(), 1);
    }

    #[test]
    fn source_transitions_explored() {
        let mut b = NetBuilder::new();
        let [p0] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((t0, p0));
        let net = b.build().expect("valid net");
        let sys = System::new(net, [0]);

        let mut rg = ReachabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);
        let step = rg.explore_next().expect("source transition should fire");
        assert!(step.is_new);
        assert_eq!(step.marking, m([1]));
    }
}
