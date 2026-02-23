//! Coverability graph construction and queries.
//!
//! The coverability graph (Karp-Miller tree) always terminates, even for
//! unbounded nets. Places that can grow without bound are represented by
//! [`Omega::Unbounded`](Omega::Unbounded).
//!
//! # Usage
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::System;
//! use petrivet::coverability::CoverabilityGraph;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0] = b.add_transitions();
//! b.add_arc((p0, t0));
//! b.add_arc((t0, p0));
//! b.add_arc((t0, p1));
//! let net = b.build().expect("valid net");
//! let sys = System::new(net, [1, 0]);
//!
//! let cg = CoverabilityGraph::build(&sys);
//! assert!(!cg.is_bounded());
//! ```

use crate::explorer::{ExplorationOrder, ExplorerCore};
use crate::marking::{Marking, Omega, OmegaMarking};
use crate::net::{Net, Transition};
use crate::reachability::ReachabilityGraph;
use crate::system::System;
use petgraph::graph::NodeIndex;

/// A single step in coverability graph exploration.
#[derive(Debug, Clone)]
pub struct CoverabilityStep {
    /// The transition that was fired.
    pub transition: Transition,
    /// The resulting marking (may contain ω after acceleration).
    pub marking: OmegaMarking,
    /// Whether this marking was newly discovered (vs. already seen).
    pub is_new: bool,
}

/// The coverability graph of a Petri net system.
///
/// Built by iteratively exploring reachable markings with ω-acceleration:
/// when a new marking strictly covers an ancestor, the growing components
/// are replaced with ω. This guarantees termination even for unbounded nets.
///
/// Use [`build`](Self::build) for one-shot construction, or [`new`](Self::new) +
/// [`explore_next`](Self::explore_next) / [`iter`](Self::iter) for step-by-step
/// control.
pub struct CoverabilityGraph<'a> {
    core: ExplorerCore<'a, Omega>,
}

impl std::fmt::Debug for CoverabilityGraph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoverabilityGraph")
            .field("states", &self.state_count())
            .field("edges", &self.edge_count())
            .field("fully_explored", &self.is_fully_explored())
            .field("bounded", &self.is_bounded())
            .finish()
    }
}

impl<'a> CoverabilityGraph<'a> {
    /// Create an unexplored coverability graph from a system.
    ///
    /// Only the initial marking is present. Call [`explore_next`](Self::explore_next)
    /// or [`explore_all`](Self::explore_all) to drive exploration.
    #[must_use]
    pub fn new(sys: &'a System<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        let net = sys.net().as_ref();
        let omega_marking: OmegaMarking = sys.marking().into();
        Self {
            core: ExplorerCore::new(net, omega_marking, order),
        }
    }

    /// Build a fully explored coverability graph from a system.
    #[must_use]
    pub fn build(sys: &'a System<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        let mut cg = Self::new(sys, order);
        cg.explore_all();
        cg
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
    /// Pops a frontier entry, fires the transition if enabled, applies
    /// ω-acceleration, and registers the result. Returns `None` when the
    /// frontier is exhausted (graph fully explored).
    pub fn explore_next(&mut self) -> Option<CoverabilityStep> {
        loop {
            let (src, t) = self.core.pop()?;
            if !self.core.is_enabled(src, t) {
                continue;
            }
            let mut new_marking = self.core.fire(src, t);
            self.omega_accelerate(&mut new_marking, src);
            let (_, is_new) = self.core.register(src, t, new_marking.clone());
            return Some(CoverabilityStep {
                transition: t,
                marking: new_marking,
                is_new,
            });
        }
    }

    /// Returns an iterator that drives exploration step by step.
    ///
    /// Each call to `next()` fires one transition (with ω-acceleration)
    /// and returns the step. The iterator ends when the frontier is
    /// exhausted (Karp-Miller guarantees termination).
    pub fn iter(&mut self) -> impl Iterator<Item = CoverabilityStep> + '_ {
        std::iter::from_fn(move || self.explore_next())
    }

    /// Explore until the frontier is exhausted.
    pub fn explore_all(&mut self) {
        while self.explore_next().is_some() {}
    }

    /// Whether exploration has completed (frontier is empty).
    #[must_use]
    pub fn is_fully_explored(&self) -> bool {
        self.core.is_fully_explored()
    }

    /// Karp-Miller acceleration: if any previously seen marking is strictly
    /// smaller than `new_marking` AND lies on a path to `src`, replace the
    /// components where `new_marking` is strictly greater with ω.
    fn omega_accelerate(&self, new_marking: &mut OmegaMarking, src: NodeIndex) {
        for (seen_marking, &seen_idx) in &self.core.seen {
            if seen_marking < new_marking
                && petgraph::algo::has_path_connecting(&self.core.graph, seen_idx, src, None)
            {
                for (component, prev) in new_marking.iter_mut().zip(seen_marking.iter()) {
                    if *component > *prev {
                        *component = Omega::Unbounded;
                    }
                }
            }
        }
    }

    /// Number of distinct markings discovered so far.
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
    pub fn initial_marking(&self) -> &OmegaMarking {
        self.core.marking_at(self.core.initial)
    }

    /// Whether the net is bounded: no ω appears in any discovered marking.
    ///
    /// Definitive only after [`is_fully_explored`](Self::is_fully_explored)
    /// returns `true`.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.core.graph.node_weights().all(Marking::is_finite)
    }

    /// Upper bound on the token count for a given place across all
    /// discovered markings. Returns `Omega::Unbounded` if the place is
    /// unbounded.
    #[must_use]
    pub fn place_bound(&self, p: crate::net::Place) -> Omega {
        self.core
            .graph
            .node_weights()
            .map(|m| m[p])
            .max()
            .unwrap_or_default()
    }

    /// Whether `target` is coverable: a discovered marking M ≥ target exists.
    #[must_use]
    pub fn is_coverable(&self, target: &Marking) -> bool {
        let omega_target: OmegaMarking = target.into();
        self.core
            .graph
            .node_weights()
            .any(|m| *m >= omega_target)
    }

    /// Whether a marking (possibly containing ω) has been discovered.
    #[must_use]
    pub fn contains(&self, marking: &OmegaMarking) -> bool {
        self.core.seen.contains_key(marking)
    }

    /// All discovered markings that have no enabled transitions.
    #[must_use]
    pub fn deadlocks(&self) -> Vec<&OmegaMarking> {
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
    pub fn markings(&self) -> Vec<&OmegaMarking> {
        self.core
            .graph
            .node_weights()
            .collect()
    }

    /// Consume and return the inner explorer core (used by `ReachabilityGraph`
    /// for the promotion conversion).
    pub(crate) fn into_core(self) -> ExplorerCore<'a, Omega> {
        self.core
    }

    /// Promote to a [`ReachabilityGraph`] if the net is bounded.
    ///
    /// When the coverability graph contains no ω, it is exactly the
    /// reachability graph. This conversion is O(n) in the number of states
    /// (unwrapping `Omega::Finite(k)` → `k`).
    ///
    /// # Errors
    /// Returns `Err(self)` if any marking contains ω, so you don't lose
    /// the coverability graph.
    #[allow(clippy::result_large_err)]
    pub fn into_reachability_graph(self) -> Result<ReachabilityGraph<'a>, Self> {
        if !self.is_bounded() {
            return Err(self);
        }
        Ok(ReachabilityGraph::from_coverability(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    use crate::net::class::ClassifiedNet;
    use crate::net::Place;

    fn m(val: impl Into<Marking>) -> Marking {
        val.into()
    }

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0 (bounded)
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

    /// Unbounded: t0 consumes from p0 and produces to both p0 and p1
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

    /// Self-loop with 0 tokens: immediate deadlock
    fn deadlock_net() -> System<ClassifiedNet> {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        let net = b.build().expect("valid net");
        System::new(net, [0])
    }

    #[test]
    fn bounded_net_fully_explored() {
        let sys = two_place_cycle();
        let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(cg.is_fully_explored());
        assert!(cg.is_bounded());
        assert_eq!(cg.state_count(), 2);
        assert!(cg.is_deadlock_free());
    }

    #[test]
    fn unbounded_net_has_omega() {
        let sys = unbounded_producer();
        let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(cg.is_fully_explored());
        assert!(!cg.is_bounded());
        assert_eq!(cg.place_bound(Place::from_index(0)), Omega::Finite(1));
        assert_eq!(cg.place_bound(Place::from_index(1)), Omega::Unbounded);
    }

    #[test]
    fn coverability_check() {
        let sys = two_place_cycle();
        let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(cg.is_coverable(&m([1, 0])));
        assert!(cg.is_coverable(&m([0, 1])));
        assert!(!cg.is_coverable(&m([1, 1])));
    }

    #[test]
    fn deadlock_detected() {
        let sys = deadlock_net();
        let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(cg.is_fully_explored());
        assert!(!cg.is_deadlock_free());
        assert_eq!(cg.deadlocks().len(), 1);
    }

    #[test]
    fn step_by_step_exploration() {
        let sys = two_place_cycle();
        let mut cg = CoverabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);

        assert!(!cg.is_fully_explored());
        assert_eq!(cg.state_count(), 1);

        let mut steps = 0;
        while let Some(step) = cg.explore_next() {
            steps += 1;
            assert!(!step.marking.iter().any(|o| !o.is_finite()));
        }
        assert!(cg.is_fully_explored());
        assert!(steps > 0);
        assert_eq!(cg.state_count(), 2);
    }

    #[test]
    fn early_termination_unbounded() {
        let sys = unbounded_producer();
        let mut cg = CoverabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);

        while let Some(step) = cg.explore_next() {
            if step.marking.iter().any(|&o| o == Omega::Unbounded) {
                break;
            }
        }
        assert!(!cg.is_bounded());
    }

    #[test]
    fn promotion_bounded() {
        let sys = two_place_cycle();
        let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let rg = cg.into_reachability_graph().expect("should be bounded");

        assert_eq!(rg.state_count(), 2);
        assert!(rg.is_reachable(&m([0, 1])));
    }

    #[test]
    fn promotion_unbounded_returns_err() {
        let sys = unbounded_producer();
        let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let result = cg.into_reachability_graph();
        assert!(result.is_err());
    }

    #[test]
    fn switch_order_mid_exploration() {
        let sys = two_place_cycle();
        let mut cg = CoverabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);
        cg.explore_next();
        cg.set_exploration_order(ExplorationOrder::DepthFirst);
        cg.explore_all();
        assert!(cg.is_fully_explored());
        assert_eq!(cg.state_count(), 2);
    }
}
