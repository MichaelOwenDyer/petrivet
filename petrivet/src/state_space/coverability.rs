//! Coverability graph construction and queries.
//!
//! The coverability graph (Karp-Miller tree) always terminates, even for
//! unbounded nets. Places that can grow without bound are represented by
//! [`Omega::Unbounded`].
//!
//! References:
//! - [Primer, Proposition 3.23](crate::literature#proposition-323--finiteness-of-the-coverability-trees-and-graphs) (termination)
//! - [Primer, Proposition 3.27](crate::literature#proposition-327--all-that-can-be-checked-on-a-coverability-graph) (coverability characterization)
//! - [Murata 1989, §V-A](crate::literature#v-a--the-coverability-tree) (coverability tree properties)
//! - [Esparza Lecture Notes, Theorem 3.2.5](crate::literature#theorem-325--coverability-graph-terminates) (termination, supplementary)
//! - [Esparza Lecture Notes, Theorem 3.2.8](crate::literature#theorem-328--coverability-characterization) (correctness, supplementary)
//!
//! # Usage
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::net::system::System;
//! use petrivet::{CoverabilityGraph, ExplorationOrder};
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0] = b.add_transitions();
//! b.add_arc((p0, t0));
//! b.add_arc((t0, p0));
//! b.add_arc((t0, p1));
//! let net = b.build().expect("valid net");
//! let sys = System::new(net, [1, 0]);
//! let cg = sys.build_coverability_graph();
//! assert!(!cg.is_bounded());
//! ```

use crate::analysis::model::CoverabilityProof;
use crate::net::marking::{IdxMarking, IdxOmegaMarking, Omega};
use crate::net::{Net};
use crate::state_space::explorer::StateGraph;
use crate::state_space::ReachabilityGraph;
use crate::state_space::{explorer::StateSpaceExplorer, ExplorationOrder};
use crate::net::system::System;
use crate::{OmegaMarking, Place, Transition};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::HashSet;
use std::fmt;
use crate::net::idx::TransitionIdx;

/// The coverability graph of a Petri net system.
///
/// Built by iteratively exploring reachable markings with ω-acceleration:
/// when a new marking strictly covers an ancestor, the growing components
/// are replaced with ω. This guarantees termination even for unbounded nets.
#[derive(Clone)]
pub struct CoverabilityExplorer<'a> {
    explorer: StateSpaceExplorer<'a, Omega>,
}

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

pub(crate) struct CoverabilityStepIdx {
    /// The transition that was fired.
    pub transition_idx: TransitionIdx,
    /// The resulting marking (may contain ω after acceleration).
    pub marking: IdxOmegaMarking,
    /// Whether this marking was newly discovered (vs. already seen).
    pub is_new: bool,
}

impl<'a> CoverabilityExplorer<'a> {
    /// Create a new coverability explorer for a system and exploration order.
    #[must_use]
    pub fn new<N: AsRef<Net>>(sys: &'a System<N>, order: ExplorationOrder) -> Self {
        let net = sys.net();
        let omega_marking = IdxOmegaMarking::from(sys.core.current_marking.clone());
        Self {
            explorer: StateSpaceExplorer::new(net, omega_marking, order),
        }
    }

    /// Current exploration order.
    #[must_use]
    pub const fn exploration_order(&self) -> ExplorationOrder {
        self.explorer.order
    }

    /// Change the exploration order for subsequent steps.
    pub const fn set_exploration_order(&mut self, order: ExplorationOrder) {
        self.explorer.order = order;
    }

    /// Advance exploration by one step.
    ///
    /// Pops a frontier entry, fires the transition if enabled, applies
    /// ω-acceleration, and registers the result. Returns `None` when the
    /// frontier is exhausted (graph fully explored).
    pub fn explore_next(&mut self) -> Option<CoverabilityStep> {
        self.explore_next_inner().map(|step_idx| {
            CoverabilityStep {
                transition: self.explorer.state_space.net.index_to_transition[step_idx.transition_idx],
                marking: self.explorer.state_space.net.to_marking(step_idx.marking),
                is_new: step_idx.is_new,
            }
        })
    }

    pub(crate) fn explore_next_inner(&mut self) -> Option<CoverabilityStepIdx> {
        loop {
            let (src_node_idx, transition_idx) = self.explorer.pop_frontier()?;
            if !self.explorer.is_enabled(src_node_idx, transition_idx) {
                continue;
            }
            let mut marking = self.explorer.fire(src_node_idx, transition_idx);
            self.omega_accelerate(&mut marking, src_node_idx);
            let is_new = self.explorer.register(src_node_idx, transition_idx, marking.clone());
            return Some(CoverabilityStepIdx {
                transition_idx,
                marking,
                is_new,
            });
        }
    }

    /// Consume the explorer and drive exploration to completion.
    ///
    /// This materializes a completed coverability graph with the guarantee
    /// that `is_fully_explored()` is true.
    #[must_use]
    pub fn build_coverability_graph(mut self) -> CoverabilityGraph<'a> {
        while self.explore_next().is_some() {}
        CoverabilityGraph {
            state_space: self.explorer.state_space,
        }
    }

    /// Drive exploration looking for a reachability graph, bailing out as
    /// soon as ω-acceleration introduces an unbounded component.
    ///
    /// On success the system is bounded, and we return the fully explored
    /// reachability graph. On failure the system is unbounded; we return
    /// the partially explored coverability explorer so the caller can
    /// inspect what was discovered before the first ω.
    ///
    /// # Errors
    /// Returns `Err(self_partial)` as soon as any explored marking contains ω.
    /// The returned explorer has its frontier preserved, so exploration can be
    /// resumed if desired.
    #[allow(clippy::result_large_err)]
    pub fn build_reachability_or_coverability(mut self) -> Result<ReachabilityGraph<'a>, Self> {
        while let Some(step) = self.explore_next_inner() {
            if !step.marking.is_finite() {
                return Err(self);
            }
        }
        let cg = CoverabilityGraph {
            state_space: self.explorer.state_space,
        };
        cg.into_reachability_graph().map_err(|_| {
            unreachable!("ω-free CG must promote successfully; ω is detected per-step above")
        })
    }

    /// Returns an iterator that drives exploration step by step.
    ///
    /// Each call to `next()` fires one transition (with ω-acceleration)
    /// and returns the step. The iterator ends when the frontier is
    /// exhausted (Karp-Miller guarantees termination).
    pub fn explore_iter(&mut self) -> impl Iterator<Item = CoverabilityStep> + '_ {
        std::iter::from_fn(move || self.explore_next())
    }

    /// Whether exploration has completed (frontier is empty).
    #[must_use]
    pub fn is_fully_explored(&self) -> bool {
        self.explorer.is_fully_explored()
    }

    /// Number of distinct ω-markings discovered so far.
    #[must_use]
    pub fn marking_count(&self) -> usize {
        self.explorer.state_space.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.explorer.state_space.graph.edge_count()
    }

    /// Karp–Miller acceleration: if any ancestor of `src` (including `src`
    /// itself) carries a marking strictly smaller than `new_marking`,
    /// promote each strictly-greater component of `new_marking` to ω.
    ///
    /// This follows the predecessor-on-a-path formulation of [Primer,
    /// Algorithm 3.18](crate::literature) (the lecture notes give a
    /// strictly weaker condition that quantifies over all paths to `src`,
    /// but Proposition 3.23 shows both formulations yield valid
    /// coverability graphs).
    fn omega_accelerate(&self, new_marking: &mut IdxOmegaMarking, src: NodeIndex) {
        let graph = &self.explorer.state_space.graph;
        let mut stack = vec![src];
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        while let Some(predecessor_node) = stack.pop() {
            if !visited.insert(predecessor_node) {
                continue;
            }
            let ancestor_marking = self.explorer.state_space.marking_at(predecessor_node);
            if ancestor_marking < new_marking {
                for (component, prev) in new_marking.iter_mut().zip(ancestor_marking.iter()) {
                    if *component > *prev {
                        *component = Omega::Unbounded;
                    }
                }
            }
            for incoming_edge in graph.edges_directed(
                predecessor_node,
                petgraph::Direction::Incoming
            ) {
                stack.push(incoming_edge.source());
            }
        }
    }

    /// The initial ω-marking.
    #[must_use]
    pub fn initial_marking(&self) -> OmegaMarking {
        let marking = self.explorer.state_space.marking_at(self.explorer.state_space.initial_idx).clone();
        self.explorer.state_space.net.to_marking(marking)
    }

    /// All ω-markings discovered so far which enable no transitions.
    pub fn deadlocks(&self) -> impl Iterator<Item = OmegaMarking> {
        self.explorer.state_space
            .deadlock_indices()
            .map(|idx| self.explorer.state_space.marking_at(idx))
            .cloned()
            .map(|marking| self.explorer.state_space.net.to_marking(marking))
    }

    /// Advances exploration until a marking covering `target` is found,
    /// and returns the marking and a firing sequence from the initial marking to it.
    /// **Note**: this will not consider already-discovered markings.
    pub fn find_cover(&mut self, target: OmegaMarking) -> Option<CoverabilityProof> {
        let target = self.explorer.state_space.net.to_idx_marking(target);
        self.find_cover_inner(&target).map(|(marking, firing_sequence)| {
            CoverabilityProof {
                firing_sequence: firing_sequence.into_iter()
                    .map(|t_idx| self.explorer.state_space.net.index_to_transition[t_idx])
                    .collect(),
                covering_marking: self.explorer.state_space.net.to_marking(marking),
            }
        })
    }

    pub(crate) fn find_cover_inner(&mut self, target: &IdxOmegaMarking) -> Option<(IdxOmegaMarking, Box<[TransitionIdx]>)> {
        while let Some(CoverabilityStepIdx { marking, .. }) = self.explore_next_inner() {
            if marking < *target {
                continue;
            }
            let firing_sequence = self.explorer.state_space
                .path_from_initial_to(self.explorer.state_space.seen[&marking])
                .expect("marking is in graph");
            return Some((marking, firing_sequence));
        }
        None
    }
}

impl fmt::Debug for CoverabilityExplorer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoverabilityExplorer")
            .field("states", &self.marking_count())
            .field("edges", &self.edge_count())
            .field("frontier", &self.explorer.frontier_count())
            .finish()
    }
}

/// A fully explored coverability graph with an explicit completion proof.
#[derive(Clone)]
pub struct CoverabilityGraph<'a> {
    pub(super) state_space: StateGraph<'a, Omega>, // todo: make private
}

impl<'a> CoverabilityGraph<'a> {
    /// Build the coverability graph for a system in one shot.
    pub fn new(system: &'a System<impl AsRef<Net>>) -> Self {
        CoverabilityExplorer::new(system, ExplorationOrder::BreadthFirst).build_coverability_graph()
    }

    /// Number of distinct markings in the coverability graph.
    #[must_use]
    pub fn marking_count(&self) -> usize {
        self.state_space.graph.node_count()
    }

    pub(crate) fn markings_inner(&self) -> impl Iterator<Item = &IdxOmegaMarking> {
        self.state_space.graph.node_weights()
    }

    /// Iterator over all distinct markings in the coverability graph.
    pub fn markings(&self) -> impl Iterator<Item = OmegaMarking> {
        self.markings_inner()
            .cloned()
            .map(|marking| self.state_space.net.to_marking(marking))
    }

    /// Whether the given ω-marking has been discovered.
    ///
    /// **Note**: this checks for exact presence, not coverability.
    /// For coverability queries, use `cover()`.
    #[must_use]
    pub fn contains_marking(&self, marking: OmegaMarking) -> bool {
        self.state_space.seen.contains_key(&self.state_space.net.to_idx_marking(marking))
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.state_space.graph.edge_count()
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> OmegaMarking {
        let marking = self.state_space.marking_at(self.state_space.initial_idx).clone();
        self.state_space.net.to_marking(marking)
    }

    /// Whether the net is bounded: no ω appears in any discovered marking.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.state_space.graph.node_weights().all(IdxMarking::is_finite)
    }

    /// Upper bound on the token count for each place across all discovered markings.
    #[must_use]
    pub fn place_bounds(&self) -> Box<[(Place, Omega)]> {
        let place_bounds: IdxOmegaMarking = self.markings_inner().fold(
            IdxOmegaMarking::zeros(self.state_space.net.place_count()),
            IdxOmegaMarking::ceil,
        );
        self.state_space.net.places()
            .zip(place_bounds)
            .collect()
    }

    /// Upper bound on the token count for a given place across all
    /// discovered markings. Returns `Omega::Unbounded` if the place is
    /// unbounded.
    #[must_use]
    pub fn place_bound(&self, p: Place) -> Omega {
        self.state_space.net.place_index(p).map_or(
            Omega::Finite(0),
            |&p_idx| {
                self.markings_inner()
                    .map(|marking| marking[p_idx])
                    .max()
                    .unwrap_or(Omega::Finite(0))
            },
        )
    }

    /// Tries to find an omega marking which covers the provided omega marking.
    ///
    /// # Panics
    ///
    /// Panics if no path can be found from the initial marking to the covering marking,
    /// which should never happen since the marking was discovered during exploration.
    #[must_use]
    pub fn cover(&self, target: OmegaMarking) -> Option<CoverabilityProof> {
        let target = self.state_space.net.to_idx_marking(target);
        self.state_space
            .graph
            .node_indices()
            .map(|idx| (idx, self.state_space.marking_at(idx)))
            .find(|&(_, marking)| marking >= &target)
            .map(|(idx, marking)| {
                let firing_sequence = self.state_space
                    .path_from_initial_to(idx)
                    .expect("marking is in graph")
                    .into_iter()
                    .map(|t_idx| self.state_space.net.index_to_transition[t_idx])
                    .collect();
                let covering_marking = self.state_space.net.to_marking(marking.clone());
                CoverabilityProof {
                    firing_sequence,
                    covering_marking,
                }
            })
    }

    /// All discovered markings that have no enabled transitions.
    pub fn deadlocks(&self) -> impl Iterator<Item = OmegaMarking> {
        self.state_space
            .deadlock_indices()
            .map(|idx| self.state_space.marking_at(idx))
            .cloned()
            .map(|marking| self.state_space.net.to_marking(marking))
    }

    /// Whether the graph contains no deadlocks.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.state_space.deadlock_indices().next().is_none()
    }

    /// Promote to a [`ReachabilityGraph`] if the system is bounded.
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
        ReachabilityGraph::try_from(self)
    }
}

impl fmt::Debug for CoverabilityGraph<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoverabilityGraph")
            .field("states", &self.marking_count())
            .field("edges", &self.transition_count())
            .field("bounded", &self.is_bounded())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{builder::NetBuilder, class::NetClass, Net};

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0 (bounded)
    fn two_place_cycle() -> (System<Net>, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid net");
        (net.with_marking([(p0, 1)]), p0, p1)
    }

    /// Unbounded: t0 consumes from p0 and produces to both p0 and p1
    fn unbounded_producer() -> (System<Net>, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arcs((p0, t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        (net.with_marking([(p0, 1)]), p0, p1)
    }

    /// Self-loop with 0 tokens: immediate deadlock
    fn deadlock_net() -> System<Net> {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arcs((p0, t0, p0));
        let net = b.build().expect("valid net");
        net.with_marking([])
    }

    #[test]
    fn bounded_net_fully_explored() {
        let (sys, _p0, _p1) = two_place_cycle();
        let cg = sys.build_coverability_graph();

        assert!(cg.is_bounded());
        assert_eq!(cg.marking_count(), 2);
        assert!(cg.is_deadlock_free());
    }

    #[test]
    fn unbounded_net_has_omega() {
        let (sys, p0, p1) = unbounded_producer();
        let cg = sys.build_coverability_graph();

        assert!(!cg.is_bounded());
        assert_eq!(cg.place_bound(p0), Omega::Finite(1));
        assert_eq!(cg.place_bound(p1), Omega::Unbounded);
    }

    #[test]
    fn coverability_check() {
        use Omega::Finite;
        let (sys, p0, p1) = two_place_cycle();
        let cg = sys.build_coverability_graph();

        assert!(cg.cover([(p0, Finite(1))].into()).is_some());
        assert!(cg.cover([(p1, Finite(1))].into()).is_some());
        assert!(cg.cover([(p0, Finite(1)), (p1, Finite(1))].into()).is_none());
    }

    #[test]
    fn deadlock_detected() {
        let sys = deadlock_net();
        let cg = sys.build_coverability_graph();

        assert!(!cg.is_deadlock_free());
        assert_eq!(cg.deadlocks().count(), 1);
    }

    #[test]
    fn step_by_step_exploration() {
        let (sys, _p0, _p1) = two_place_cycle();
        let mut cg = sys.explore_coverability(ExplorationOrder::BreadthFirst);

        assert!(!cg.is_fully_explored());
        assert_eq!(cg.marking_count(), 1);

        let mut steps = 0;
        while let Some(step) = cg.explore_next() {
            steps += 1;
            assert!(!step.marking.as_ref().iter().any(|&(_, o)| !o.is_finite()));
        }
        assert!(cg.is_fully_explored());
        assert!(steps > 0);
        assert_eq!(cg.marking_count(), 2);
    }

    #[test]
    fn early_termination_unbounded() {
        let (sys, _, _) = unbounded_producer();
        let mut cg = sys.explore_coverability(ExplorationOrder::BreadthFirst);

        while let Some(step) = cg.explore_next() {
            if step.marking.as_ref().iter().any(|&(_, o)| o == Omega::Unbounded) {
                break;
            }
        }
        let cg = cg.build_coverability_graph();
        assert!(!cg.is_bounded());
    }

    #[test]
    fn promotion_bounded() {
        let (sys, _p0, p1) = two_place_cycle();
        let cg = sys.build_coverability_graph();
        let rg = cg.into_reachability_graph().expect("should be bounded");

        assert_eq!(rg.state_count(), 2);
        assert!(rg.is_reachable([(p1, 1)].into()));
    }

    #[test]
    fn promotion_unbounded_returns_err() {
        let (sys, _, _) = unbounded_producer();
        let cg = sys.build_coverability_graph();
        let result = cg.into_reachability_graph();
        assert!(result.is_err());
    }

    #[test]
    fn rg_or_cg_short_circuits_on_unbounded() {
        let (sys, _, _) = unbounded_producer();
        match sys.build_reachability_or_coverability() {
            Ok(_) => panic!("unbounded net must short-circuit"),
            Err(cg) => assert!(!cg.is_fully_explored(), "frontier preserved on bail-out"),
        }
    }

    #[test]
    fn rg_or_cg_completes_for_bounded() {
        let (sys, _p0, p1) = two_place_cycle();
        let rg = sys.build_reachability_or_coverability()
            .expect("bounded net must yield reachability graph");
        assert_eq!(rg.state_count(), 2);
        assert!(rg.is_reachable([(p1, 1)].into()));
    }

    #[test]
    fn switch_order_mid_exploration() {
        let (sys, _p0, _p1) = two_place_cycle();
        let mut cg = sys.explore_coverability(ExplorationOrder::BreadthFirst);
        cg.explore_next();
        cg.set_exploration_order(ExplorationOrder::DepthFirst);
        let cg = cg.build_coverability_graph();
        assert_eq!(cg.marking_count(), 2);
    }

    /// Connected net with two unbounded places: both should get ω.
    ///
    /// ```text
    /// p0 → t0 → p0, p1       (p1 grows unboundedly)
    /// p0 → t1 → p0, p2       (p2 grows unboundedly)
    /// ```
    #[test]
    fn multiple_omega_places() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        b.add_arc((p0, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p2));
        let net = b.build().expect("valid net");
        let sys = net.with_marking([(p0, 1)]);
        let cg = sys.build_coverability_graph();

        assert!(!cg.is_bounded());
        assert_eq!(cg.place_bound(p0), Omega::Finite(1));
        assert_eq!(cg.place_bound(p1), Omega::Unbounded);
        assert_eq!(cg.place_bound(p2), Omega::Unbounded);
    }

    /// CG of a bounded net: CG→RG promotion preserves state and edge counts.
    #[test]
    fn promotion_preserves_graph_structure() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));
        b.add_arc((p2, t2));
        b.add_arc((t2, p0));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::Circuit);
        let sys = net.with_marking([(p0, 2)]);

        let cg = sys.build_coverability_graph();
        assert!(cg.is_bounded());
        let cg_states = cg.marking_count();
        let cg_edges = cg.transition_count();

        let rg = cg.into_reachability_graph().expect("bounded");
        assert_eq!(rg.state_count(), cg_states);
        assert_eq!(rg.transition_count(), cg_edges);
        for marking in rg.markings() {
            assert_eq!(marking.total_tokens(), 2);
        }
    }

    /// Connected net with concurrent enabling: both sub-cycles share `p_shared`.
    /// Tests that transitions enabled from pre-existing tokens are explored.
    #[test]
    fn concurrent_enabling_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1, p_shared] = b.add_places();
        let [t0, t1, t2, t3] = b.add_transitions();
        b.add_arcs((p0, t0, p_shared, t2, p0));
        b.add_arcs((p1, t1, p_shared, t3, p1));
        let net = b.build().expect("valid net");
        let sys = net.with_marking([(p0, 1), (p1, 1)]);
        let cg = sys.build_coverability_graph();

        assert!(cg.is_bounded());
        assert!(cg.is_deadlock_free());
    }

    /// A net where omega acceleration fires on multiple places simultaneously:
    /// t0: p0 → p0, p1, p2
    #[test]
    fn multi_place_acceleration() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        b.add_arc((t0, p2));
        let net = b.build().expect("valid net");
        let sys = net.with_marking([(p0, 1)]);
        let cg = sys.build_coverability_graph();

        assert!(!cg.is_bounded());
        assert_eq!(cg.place_bound(p1), Omega::Unbounded);
        assert_eq!(cg.place_bound(p2), Omega::Unbounded);

        // use Omega::Finite;
        assert!(cg.cover([(p0, 1.into()), (p1, 100.into()), (p2, 100.into())].into()).is_some());
    }

    /// BFS and DFS produce same coverability results for bounded nets.
    #[test]
    fn bfs_dfs_same_coverability() {
        let (sys, _p0, _p1) = two_place_cycle();
        let cg_bfs = sys.explore_coverability(ExplorationOrder::BreadthFirst).build_coverability_graph();
        let cg_dfs = sys.explore_coverability(ExplorationOrder::DepthFirst).build_coverability_graph();

        assert_eq!(cg_bfs.marking_count(), cg_dfs.marking_count());
        assert_eq!(cg_bfs.is_bounded(), cg_dfs.is_bounded());
    }

    /// Mutex via coverability: mutual exclusion verified over all coverable markings.
    #[test]
    fn mutex_bounded_via_coverability() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arcs((idle1, t_req1, wait1, t_enter1, crit1, t_exit1, idle1));

        b.add_arcs((idle2, t_req2, wait2, t_enter2, crit2, t_exit2, idle2));

        b.add_arc((mutex, t_enter1));
        b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2));
        b.add_arc((t_exit2, mutex));

        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        let sys = net.with_marking([(idle1, 1), (idle2, 1), (mutex, 1)]);
        let cg = sys.build_coverability_graph();

        assert!(cg.is_bounded());
        assert!(cg.is_deadlock_free());
        let zero = Omega::Finite(0);
        for marking in cg.markings() {
            let c1 = marking.get(crit1).copied().unwrap_or(zero);
            let c2 = marking.get(crit2).copied().unwrap_or(zero);
            assert!(
                c1 == zero || c2 == zero,
                "mutual exclusion violated: {marking:?}",
            );
        }

        assert!(cg.cover([(crit1, 1.into()), (crit2, 1.into())].into()).is_none());

        let rg = cg.into_reachability_graph().expect("bounded");
        assert_eq!(rg.state_count(), 8);
    }
}
