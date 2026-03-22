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
//! use petrivet::system::System;
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
use crate::marking::{Marking, Omega, OmegaMarking};
use crate::net::{Net, Place};
use crate::state_space::explorer::StateGraph;
use crate::state_space::ReachabilityGraph;
use crate::state_space::{explorer::StateSpaceExplorer, ExplorationOrder};
use crate::system::System;
use crate::{PlaceKey, PlaceMap, TransitionKey};
use petgraph::graph::NodeIndex;
use std::fmt;

/// The coverability graph of a Petri net system.
///
/// Built by iteratively exploring reachable markings with ω-acceleration:
/// when a new marking strictly covers an ancestor, the growing components
/// are replaced with ω. This guarantees termination even for unbounded nets.
#[derive(Clone)]
pub struct CoverabilityExplorer<'a> {
    explorer: StateSpaceExplorer<'a, Omega>,
    /// The highest token count observed for each place
    /// across all discovered markings so far.
    place_bounds: Box<[Omega]>,
}

/// A single step in coverability graph exploration.
#[derive(Debug, Clone)]
pub struct CoverabilityStep {
    /// The transition that was fired.
    pub transition: TransitionKey,
    /// The resulting marking (may contain ω after acceleration).
    pub marking: OmegaMarking,
    /// Whether this marking was newly discovered (vs. already seen).
    pub is_new: bool,
}

impl<'a> CoverabilityExplorer<'a> {
    /// Create a new coverability explorer for a system and exploration order.
    #[must_use]
    pub fn new<N: AsRef<Net>>(sys: &'a System<N>, order: ExplorationOrder) -> Self {
        let net = sys.net().as_ref();
        let omega_marking = OmegaMarking::from(sys.current_marking());
        Self {
            explorer: StateSpaceExplorer::new(net, omega_marking, order),
            place_bounds: net.places().map(|_| Omega::Finite(0)).collect(),
        }
    }

    /// Current exploration order.
    #[must_use]
    pub fn exploration_order(&self) -> ExplorationOrder {
        self.explorer.order
    }

    /// Change the exploration order for subsequent steps.
    pub fn set_exploration_order(&mut self, order: ExplorationOrder) {
        self.explorer.order = order;
    }

    /// Advance exploration by one step.
    ///
    /// Pops a frontier entry, fires the transition if enabled, applies
    /// ω-acceleration, and registers the result. Returns `None` when the
    /// frontier is exhausted (graph fully explored).
    pub fn explore_next(&mut self) -> Option<CoverabilityStep> {
        loop {
            let (src_idx, dense_t) = self.explorer.pop_frontier()?;
            if !self.explorer.is_enabled(src_idx, dense_t) {
                continue;
            }
            let mut marking = self.explorer.fire(src_idx, dense_t);
            self.omega_accelerate(&mut marking, src_idx);
            self.update_place_bounds(&marking);
            let (_, is_new) = self.explorer.register(src_idx, dense_t, marking.clone());
            let transition_key = self.explorer.state_space.net.transition_key(dense_t);
            return Some(CoverabilityStep {
                transition: transition_key,
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
            place_bounds: self.place_bounds
        }
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

    /// Iterator over all distinct ω-markings reached so far.
    pub fn markings(&self) -> impl Iterator<Item = &OmegaMarking> {
        self.explorer.state_space.graph.node_weights()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.explorer.state_space.graph.edge_count()
    }

    /// Karp-Miller acceleration: if any previously seen marking is strictly
    /// smaller than `new_marking` AND lies on a path to `src`, replace the
    /// components where `new_marking` is strictly greater with ω.
    fn omega_accelerate(&self, new_marking: &mut OmegaMarking, src: NodeIndex) {
        for (seen_marking, &seen_idx) in &self.explorer.state_space.seen {
            if seen_marking < new_marking
                && petgraph::algo::has_path_connecting(&self.explorer.state_space.graph, seen_idx, src, None)
            {
                for (component, prev) in new_marking.iter_mut().zip(seen_marking.iter()) {
                    if *component > *prev {
                        *component = Omega::Unbounded;
                    }
                }
            }
        }
    }

    fn update_place_bounds(&mut self, marking: &OmegaMarking) {
        for (component, bound) in marking.iter().zip(self.place_bounds.iter_mut()) {
            if *component > *bound {
                *bound = *component;
            }
        }
    }

    /// The initial ω-marking.
    #[must_use]
    pub fn initial_marking(&self) -> &OmegaMarking {
        self.explorer.state_space.marking_at(self.explorer.state_space.initial_idx)
    }

    /// All ω-markings discovered so far which enable no transitions.
    #[must_use]
    pub fn deadlocks(&self) -> impl Iterator<Item = &OmegaMarking> {
        self.explorer.state_space
            .deadlock_indices()
            .map(|idx| self.explorer.state_space.marking_at(idx))
    }

    /// Advances exploration until a marking covering `target` is found,
    /// and returns the marking and a firing sequence from the initial marking to it.
    /// **Note**: this will not consider already-discovered markings.
    pub fn find_cover(&mut self, target: &OmegaMarking) -> Option<CoverabilityProof> {
        while let Some(step) = self.explore_next() {
            if step.marking >= *target {
                let path = self.explorer.state_space.path_to(self.explorer.state_space.seen[&step.marking]).expect("marking is in graph");
                return Some(CoverabilityProof {
                    firing_sequence: path,
                    covering_marking: step.marking,
                });
            }
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
    #[expect(dead_code)] // currently unused but may be useful for quick access in future queries
    place_bounds: Box<[Omega]>, // precompute place bounds for quick access
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

    /// Iterator over all distinct markings in the coverability graph.
    pub fn markings(&self) -> impl Iterator<Item = &OmegaMarking> {
        self.state_space.graph.node_weights()
    }

    /// Whether the given ω-marking has been discovered.
    ///
    /// **Note**: this checks for exact presence, not coverability.
    /// For coverability queries, use `cover()`.
    #[must_use]
    pub fn contains_marking(&self, marking: &OmegaMarking) -> bool {
        self.state_space.seen.contains_key(marking)
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.state_space.graph.edge_count()
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &OmegaMarking {
        self.state_space.marking_at(self.state_space.initial_idx)
    }

    /// Whether the net is bounded: no ω appears in any discovered marking.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.state_space.graph.node_weights().all(Marking::is_finite)
    }

    /// Upper bound on the token count for each place across all discovered markings.
    #[must_use]
    pub fn place_bounds(&self) -> PlaceMap<Omega> {
        self.state_space
            .net
            .places()
            .map(|p| self.place_bound_dense(p))
            .collect()
    }

    /// Upper bound on the token count for a given place across all
    /// discovered markings. Returns `Omega::Unbounded` if the place is
    /// unbounded.
    #[must_use]
    pub fn place_bound(&self, p: PlaceKey) -> Omega {
        let dense = self.state_space.net.dense_place(p);
        self.place_bound_dense(dense)
    }

    /// Dense-index version for internal use.
    fn place_bound_dense(&self, p: Place) -> Omega {
        self.state_space
            .graph
            .node_weights()
            .map(|m| m[p])
            .max()
            .unwrap_or_default()
    }

    /// Tries to find an omega marking which covers the provided omega marking.
    ///
    /// # Panics
    ///
    /// Panics if no path can be found from the initial marking to the covering marking,
    /// which should never happen since the marking was discovered during exploration.
    #[must_use]
    pub fn cover(&self, target: &OmegaMarking) -> Option<CoverabilityProof> {
        self.state_space
            .graph
            .node_indices()
            .map(|idx| (idx, self.state_space.marking_at(idx)))
            .find(|&(_, marking)| marking >= target)
            .map(|(idx, marking)| {
                let firing_sequence = self.state_space.path_to(idx).expect("marking is in graph");
                let covering_marking = marking.clone();
                CoverabilityProof {
                    firing_sequence,
                    covering_marking,
                }
            })
    }

    /// All discovered markings that have no enabled transitions.
    pub fn deadlocks(&self) -> impl Iterator<Item = &OmegaMarking> {
        self.state_space
            .deadlock_indices()
            .map(|idx| self.state_space.marking_at(idx))
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
            .field("edges", &self.edge_count())
            .field("bounded", &self.is_bounded())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::{builder::NetBuilder, class::NetClass, Net};

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0 (bounded)
    fn two_place_cycle() -> System<Net> {
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
    fn unbounded_producer() -> (System<Net>, PlaceKey, PlaceKey) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        (System::new(net, [1, 0]), p0, p1)
    }

    /// Self-loop with 0 tokens: immediate deadlock
    fn deadlock_net() -> System<Net> {
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
        let sys = two_place_cycle();
        let cg = sys.build_coverability_graph();

        use Omega::Finite;
        assert!(cg.cover(&OmegaMarking::from([Finite(1), Finite(0)])).is_some());
        assert!(cg.cover(&OmegaMarking::from([Finite(0), Finite(1)])).is_some());
        assert!(cg.cover(&OmegaMarking::from([Finite(1), Finite(1)])).is_none());
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
        let sys = two_place_cycle();
        let mut cg = CoverabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);

        assert!(!cg.is_fully_explored());
        assert_eq!(cg.marking_count(), 1);

        let mut steps = 0;
        while let Some(step) = cg.explore_next() {
            steps += 1;
            assert!(!step.marking.iter().any(|o| !o.is_finite()));
        }
        assert!(cg.is_fully_explored());
        assert!(steps > 0);
        assert_eq!(cg.marking_count(), 2);
    }

    #[test]
    fn early_termination_unbounded() {
        let (sys, _, _) = unbounded_producer();
        let mut cg = CoverabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);

        while let Some(step) = cg.explore_next() {
            if step.marking.iter().any(|&o| o == Omega::Unbounded) {
                break;
            }
        }
        let cg = cg.build_coverability_graph();
        assert!(!cg.is_bounded());
    }

    #[test]
    fn promotion_bounded() {
        let sys = two_place_cycle();
        let cg = sys.build_coverability_graph();
        let rg = cg.into_reachability_graph().expect("should be bounded");

        assert_eq!(rg.state_count(), 2);
        assert!(rg.is_reachable(&Marking::from([0, 1])));
    }

    #[test]
    fn promotion_unbounded_returns_err() {
        let (sys, _, _) = unbounded_producer();
        let cg = sys.build_coverability_graph();
        let result = cg.into_reachability_graph();
        assert!(result.is_err());
    }

    #[test]
    fn switch_order_mid_exploration() {
        let sys = two_place_cycle();
        let mut cg = CoverabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
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
        let sys = System::new(net, [1, 0, 0]);
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
        let sys = System::new(net, [2, 0, 0]);

        let cg = sys.build_coverability_graph();
        assert!(cg.is_bounded());
        let cg_states = cg.marking_count();
        let cg_edges = cg.edge_count();

        let rg = cg.into_reachability_graph().expect("bounded");
        assert_eq!(rg.state_count(), cg_states);
        assert_eq!(rg.edge_count(), cg_edges);
        for marking in rg.markings() {
            assert_eq!(marking.total_tokens(), 2);
        }
    }

    /// Connected net with concurrent enabling: both sub-cycles share p_shared.
    /// Tests that transitions enabled from pre-existing tokens are explored.
    #[test]
    fn concurrent_enabling_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1, p_shared] = b.add_places();
        let [t0, t1, t2, t3] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p_shared));
        b.add_arc((p_shared, t2));
        b.add_arc((t2, p0));
        b.add_arc((p1, t1));
        b.add_arc((t1, p_shared));
        b.add_arc((p_shared, t3));
        b.add_arc((t3, p1));
        let net = b.build().expect("valid net");
        let sys = System::new(net, [1, 1, 0]);
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
        let sys = System::new(net, [1, 0, 0]);
        let cg = sys.build_coverability_graph();

        assert!(!cg.is_bounded());
        assert_eq!(cg.place_bound(p1), Omega::Unbounded);
        assert_eq!(cg.place_bound(p2), Omega::Unbounded);

        // use Omega::Finite;
        // assert!(cg.cover(&OmegaMarking::from([Finite(1), Finite(100), Finite(100)])));
    }

    /// BFS and DFS produce same coverability results for bounded nets.
    #[test]
    fn bfs_dfs_same_coverability() {
        let sys = two_place_cycle();
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

        b.add_arc((idle1, t_req1));
        b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1));
        b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1));
        b.add_arc((t_exit1, idle1));

        b.add_arc((idle2, t_req2));
        b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2));
        b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2));
        b.add_arc((t_exit2, idle2));

        b.add_arc((mutex, t_enter1));
        b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2));
        b.add_arc((t_exit2, mutex));

        let net = b.build().expect("valid net");
        let crit1 = net.dense_place(crit1);
        let crit2 = net.dense_place(crit2);
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        let sys = System::new(net, [1, 0, 0, 1, 0, 0, 1]);
        let cg = sys.build_coverability_graph();

        assert!(cg.is_bounded());
        assert!(cg.is_deadlock_free());
        for marking in cg.markings() {
            assert!(
                marking[crit1] <= Omega::Finite(0) || marking[crit2] <= Omega::Finite(0),
                "mutual exclusion violated"
            );
        }

        // todo: OmegaMarking from nums
        // assert!(cg.cover(&OmegaMarking::from([0, 0, 1, 0, 0, 1, 0])).is_none());

        let rg = cg.into_reachability_graph().expect("bounded");
        assert_eq!(rg.state_count(), 8);
    }
}
