//! Reachability graph construction and queries.
//!
//! Two types model the lifecycle of a reachability graph:
//!
//! - [`ReachabilityExplorer`]: an incremental exploration handle. Works for
//!   any net (bounded or not). The user drives exploration step by step and is
//!   responsible for termination.
//!
//! - [`ReachabilityGraph`]: a fully explored, finite reachability graph. This
//!   type is a proof that exploration terminated, which implies boundedness.
//!   Exact analysis methods (liveness, deadlock-freedom) live here.
//!
//! # Recommended workflow
//!
//! For unknown nets, use the coverability graph first (it always terminates):
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::System;
//! //!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! let net = b.build().unwrap();
//! let sys = System::new(net, [1, 0]);
//!
//! // 1. Build coverability graph (always terminates)
//! let cg = sys.build_coverability_graph();
//!
//! // 2. If bounded, promote to reachability graph for exact analysis
//! if let Ok(rg) = cg.into_reachability_graph() {
//!     assert!(rg.is_deadlock_free());
//!     assert!(rg.is_live());
//! } else {
//!    println!("Net is unbounded");
//! }
//! ```
//!
//! For bounded nets where you know exploration will terminate, use
//! [`ReachabilityGraph::build`] directly. For unbounded nets or when you
//! need fine-grained control, use [`ReachabilityExplorer`].

use crate::analysis::model::LivenessLevel;
use crate::marking::{Marking, Omega};
use crate::net::{Net, Transition};
use crate::state_space::explorer::StateGraph;
use crate::state_space::{explorer::StateSpaceExplorer, CoverabilityExplorer, CoverabilityGraph, ExplorationOrder};
use crate::system::System;

/// An incremental exploration handle for a Petri net's reachability graph.
///
/// Works for any net (bounded or unbounded). For unbounded nets, the frontier
/// never empties - the caller must impose their own termination condition.
///
/// Once exploration is complete (`is_fully_explored()` returns `true`), convert
/// to a [`ReachabilityGraph`] for exact analysis.
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::system::System;
/// use petrivet::{ReachabilityExplorer, ReachabilityGraph, ExplorationOrder};
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p0)); b.add_arc((t0, p1));
/// let net = b.build().unwrap();
/// let sys = System::new(net, [1, 0]);
///
/// // Explore an unbounded net incrementally, stopping after 50 states
/// let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
/// while explorer.state_count() < 50 {
///     if explorer.explore_next().is_none() { break; }
/// }
/// assert!(explorer.state_count() >= 50);
/// assert!(!explorer.is_fully_explored()); // unbounded → never finishes
/// ```
pub struct ReachabilityExplorer<'a> {
    core: StateSpaceExplorer<'a, u32>,
}

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

impl<'a> ReachabilityExplorer<'a> {
    /// Create an unexplored explorer from a system.
    #[must_use]
    pub fn new(sys: &'a System<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        let net = sys.net().as_ref();
        let marking = sys.current_marking().clone();
        Self {
            core: StateSpaceExplorer::new(net, marking, order),
        }
    }

    /// Advance exploration by one step.
    ///
    /// Returns `None` when the frontier is exhausted (fully explored).
    pub fn explore_next(&mut self) -> Option<ReachabilityStep> {
        loop {
            let (src_idx, t) = self.core.pop_frontier()?;
            if !self.core.is_enabled(src_idx, t) {
                continue;
            }
            let new_marking = self.core.fire(src_idx, t);
            let (_, is_new) = self.core.register(src_idx, t, new_marking.clone());
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
    /// **Warning: infinite** for unbounded nets.
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    /// use petrivet::{ReachabilityExplorer, ExplorationOrder};
    /// use petrivet::marking::Marking;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// let sys = System::new(net, [1, 0]);
    ///
    /// let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
    ///
    /// // Search for a specific marking
    /// let target = Marking::from([0u32, 1]);
    /// let found = explorer.iter().any(|s| s.marking == target);
    /// assert!(found);
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

    /// Current exploration order.
    #[must_use]
    pub fn exploration_order(&self) -> ExplorationOrder {
        self.core.order
    }

    /// Change the exploration order for subsequent steps.
    pub fn set_exploration_order(&mut self, order: ExplorationOrder) {
        self.core.order = order;
    }

    /// Whether the frontier is empty (no more states to explore).
    #[must_use]
    pub fn is_fully_explored(&self) -> bool {
        self.core.is_fully_explored()
    }

    /// Number of distinct markings discovered so far.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.core.state_space.graph.node_count()
    }

    /// Number of edges (transition firings) discovered so far.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.core.state_space.graph.edge_count()
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        self.core.state_space.marking_at(self.core.state_space.initial_idx)
    }

    /// Whether `target` has been discovered so far.
    #[must_use]
    pub fn is_reachable(&self, target: &Marking) -> bool {
        self.core.state_space.seen.contains_key(target)
    }

    /// Returns a firing sequence from the initial marking to `target`,
    /// among states discovered so far.
    #[must_use]
    pub fn path_to(&self, target: &Marking) -> Option<Box<[Transition]>> {
        let &target_idx = self.core.state_space.seen.get(target)?;
        self.core.state_space.path_to(target_idx)
    }

    /// Whether a marking has been discovered so far.
    #[must_use]
    pub fn contains(&self, marking: &Marking) -> bool {
        self.core.state_space.seen.contains_key(marking)
    }

    /// Iterator over all discovered markings.
    pub fn states(&self) -> impl Iterator<Item = &Marking> {
        self.core.state_space.graph.node_weights()
    }

    /// All discovered markings as a collected vector.
    #[must_use]
    pub fn markings(&self) -> Vec<&Marking> {
        self.core.state_space.graph.node_weights().collect()
    }
}

impl std::fmt::Debug for ReachabilityExplorer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachabilityExplorer")
            .field("state_count", &self.state_count())
            .field("edge_count", &self.edge_count())
            .field("is_fully_explored", &self.is_fully_explored())
            .finish()
    }
}

/// A fully explored, finite reachability graph.
///
/// This type is a proof that exploration terminated (the net is bounded under
/// this initial marking). Exact analysis methods - liveness levels, deadlock
/// detection - are available here. Methods like [`liveness_levels`](Self::liveness_levels)
/// return owned results; callers should store them if repeated access is needed.
///
/// The reachability graph is infinite for unbounded systems. For unknown systems,
/// prefer building a [`CoverabilityExplorer`] first (always terminates, decides
/// coverability and boundedness), then attempt to promote to a `ReachabilityGraph`,
/// which succeeds if and only if the net is bounded.
///
/// Construct via:
/// - [`ReachabilityGraph::build`] (convenience; does not terminate for unbounded nets)
/// - [`TryFrom<ReachabilityExplorer>`] (succeeds if frontier is exhausted)
/// - [`TryFrom<CoverabilityGraph>`] / [`CoverabilityGraph::into_reachability_graph`]
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::system::System;
/// use petrivet::LivenessLevel;
/// use petrivet::{ReachabilityGraph, ExplorationOrder};
/// use petrivet::marking::Marking;
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap();
/// let sys = System::new(net, [1, 0]);
///
/// let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
///
/// // Query the graph
/// assert_eq!(rg.state_count(), 2);
/// assert!(rg.is_reachable(&Marking::from([0u32, 1])));
/// assert!(rg.is_deadlock_free());
///
/// // Liveness analysis
/// let levels = rg.liveness_levels();
/// assert!(levels.iter().all(|&l| l == LivenessLevel::L4));
/// ```
pub struct ReachabilityGraph<'a> {
    state_space: StateGraph<'a, u32>,
}

impl<'a> ReachabilityGraph<'a> {
    /// Build a fully explored reachability graph from a system.
    ///
    /// **Does not terminate** for unbounded nets - `explore_all()` runs
    /// until the frontier is exhausted, which never happens if the state
    /// space is infinite. For unknown nets, prefer the coverability graph
    /// path or use [`ReachabilityExplorer`] with manual termination.
    #[must_use]
    pub fn build(sys: &'a System<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        // todo: rewrite
        let mut explorer = ReachabilityExplorer::new(sys, order);
        explorer.explore_all(); // WARNING: does not terminate for unbounded nets!
        // explore_all() returned, so the frontier is exhausted,
        // so is_fully_explored() is true, so conversion to ReachabilityGraph is infallible.
        ReachabilityGraph {
            state_space: explorer.core.state_space
        }
    }

    /// Number of distinct reachable markings.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.state_space.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.state_space.graph.edge_count()
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        self.state_space.marking_at(self.state_space.initial_idx)
    }

    /// Whether `target` is reachable from the initial marking.
    #[must_use]
    pub fn is_reachable(&self, target: &Marking) -> bool {
        self.state_space.seen.contains_key(target)
    }

    /// Returns a firing sequence from the initial marking to `target`.
    ///
    /// When built with BFS, this is a minimal firing sequence.
    #[must_use]
    pub fn path_to(&self, target: &Marking) -> Option<Box<[Transition]>> {
        self.state_space.seen.get(target)
            .and_then(|&target_idx| self.state_space.path_to(target_idx))
    }

    /// Whether a marking exists in the graph.
    #[must_use]
    pub fn contains(&self, marking: &Marking) -> bool {
        self.state_space.seen.contains_key(marking)
    }

    /// Iterator over all reachable markings.
    pub fn states(&self) -> impl Iterator<Item = &Marking> {
        self.state_space.graph.node_weights()
    }

    /// All reachable markings as a collected vector.
    #[must_use]
    pub fn markings(&self) -> Vec<&Marking> {
        self.state_space.graph.node_weights().collect()
    }

    /// All markings with no enabled transitions.
    #[must_use]
    pub fn deadlocks(&self) -> impl Iterator<Item = &Marking> {
        self.state_space
            .deadlock_indices()
            .map(|idx| self.state_space.marking_at(idx))
    }

    /// Whether every reachable marking has at least one enabled transition.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.state_space.deadlock_indices().next().is_none()
    }

    /// Computes liveness levels for all transitions in a single pass.
    ///
    /// SCC-based decision procedure for bounded nets ([Murata 1989 §V-C](crate::literature#v-c--liveness-via-reachability-graph-sccs)):
    /// - L0 (dead): `t` does not label any edge.
    /// - L1: `t` labels at least one edge.
    /// - L3 (≡L2 for bounded): `t` labels an edge within some non-trivial SCC.
    /// - L4 (live): `t` labels an edge in **every** terminal SCC.
    ///
    /// Returns an owned `Box<[LivenessLevel]>` indexed by transition index.
    /// Store the result if you need to query it multiple times.
    #[must_use]
    pub fn liveness_levels(&self) -> Box<[LivenessLevel]> {
        use petgraph::visit::EdgeRef;

        let n_transitions = self.state_space.net.transition_count();
        let graph = &self.state_space.graph;
        let sccs = petgraph::algo::kosaraju_scc(graph);

        if sccs.is_empty() || n_transitions == 0 {
            return vec![LivenessLevel::L0; n_transitions].into_boxed_slice();
        }

        let mut node_to_scc = vec![0usize; graph.node_count()];
        for (scc_id, scc) in sccs.iter().enumerate() {
            for &node in scc {
                node_to_scc[node.index()] = scc_id;
            }
        }

        let n_scc = sccs.len();
        let mut has_external_edge = vec![false; n_scc];
        let mut scc_is_nontrivial = vec![false; n_scc];
        let mut scc_has_t: Vec<Vec<bool>> = vec![vec![false; n_transitions]; n_scc];
        let mut t_fires_anywhere = vec![false; n_transitions];

        for edge in graph.edge_references() {
            let t = *edge.weight();
            let src_scc = node_to_scc[edge.source().index()];
            let dst_scc = node_to_scc[edge.target().index()];

            t_fires_anywhere[t.idx] = true;

            if src_scc == dst_scc {
                scc_is_nontrivial[src_scc] = true;
                scc_has_t[src_scc][t.idx] = true;
            } else {
                has_external_edge[src_scc] = true;
            }
        }

        let terminal_sccs: Vec<usize> = (0..n_scc)
            .filter(|&i| !has_external_edge[i])
            .collect();

        let mut levels = vec![LivenessLevel::L0; n_transitions];
        for t_idx in 0..n_transitions {
            if !t_fires_anywhere[t_idx] {
                continue;
            }

            let in_all_terminal = terminal_sccs.iter().all(|&s| scc_has_t[s][t_idx]);
            if in_all_terminal {
                levels[t_idx] = LivenessLevel::L4;
            } else if (0..n_scc).any(|s| scc_is_nontrivial[s] && scc_has_t[s][t_idx]) {
                levels[t_idx] = LivenessLevel::L3;
            } else {
                levels[t_idx] = LivenessLevel::L1;
            }
        }

        levels.into_boxed_slice()
    }

    /// Convenience: checks L4-liveness for all transitions.
    ///
    /// Computes liveness levels internally. If you also need per-transition
    /// levels, call [`liveness_levels`](Self::liveness_levels) once and
    /// inspect the result instead.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.liveness_levels().iter().all(|&l| l == LivenessLevel::L4)
    }
}

/// Convert a fully explored explorer into a `ReachabilityGraph`.
///
/// Fails if the explorer's frontier is not exhausted.
impl<'a> TryFrom<ReachabilityExplorer<'a>> for ReachabilityGraph<'a> {
    type Error = ReachabilityExplorer<'a>;

    fn try_from(explorer: ReachabilityExplorer<'a>) -> Result<Self, Self::Error> {
        if !explorer.is_fully_explored() {
            return Err(explorer);
        }
        Ok(ReachabilityGraph {
            state_space: explorer.core.state_space,
        })
    }
}

/// Converts the coverability graph into a `ReachabilityGraph` if it is bounded
/// (contains no markings with ω). This is a "promotion" operation that preserves
/// the graph structure but unwraps all markings from `Marking<Omega>` to `Marking<u32>`.
///
/// If the coverability graph contains unbounded markings, the conversion fails
/// and returns the unchanged argument for further inspection.
///
/// Fails if the coverability graph is unbounded (contains any ω markings).
impl<'a> TryFrom<CoverabilityGraph<'a>> for ReachabilityGraph<'a> {
    type Error = CoverabilityGraph<'a>;

    fn try_from(cg: CoverabilityGraph<'a>) -> Result<Self, Self::Error> {
        if !cg.is_bounded() {
            return Err(cg);
        }

        let graph = cg.state_space.graph.map(
            |_idx, omega_marking| unwrap_omega_marking_to_u32(omega_marking),
            |_src, &t| t,
        );
        let seen = cg.state_space.seen
            .into_iter()
            .map(|(marking, idx)| {
                (unwrap_omega_marking_to_u32(&marking), idx)
            })
            .collect();

        Ok(ReachabilityGraph {
            state_space: StateGraph {
                net: cg.state_space.net,
                initial_idx: cg.state_space.initial_idx,
                graph,
                seen,
            },
        })
    }
}

fn unwrap_omega_marking_to_u32(om: &Marking<Omega>) -> Marking<u32> {
    om.iter()
        .map(|o| match o {
            Omega::Finite(n) => *n,
            Omega::Unbounded => panic!("unwrap_omega_marking called on unbounded graph"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    use crate::net::class::NetClass;

    fn m(val: impl Into<Marking>) -> Marking {
        val.into()
    }

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0
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

    /// Unbounded: t0 feeds back to p0 and also produces to p1
    fn unbounded_producer() -> System<Net> {
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
        assert_eq!(rg.deadlocks().count(), 1);
    }

    #[test]
    fn concurrent_enabling() {
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

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(rg.is_reachable(&m([1, 1, 0])));
        assert!(rg.is_reachable(&m([0, 1, 1])));
        assert!(rg.is_reachable(&m([1, 0, 1])));
        assert!(rg.is_reachable(&m([0, 0, 2])));
        assert!(rg.is_deadlock_free());
    }

    #[test]
    fn diamond_confluence() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p0, t1));
        b.add_arc((t1, p2));
        b.add_arc((p1, t2));
        b.add_arc((p2, t2));
        b.add_arc((t2, p3));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::FreeChoice);
        let sys = System::new(net, [2, 0, 0, 0]);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert_eq!(rg.state_count(), 7);
        assert!(rg.is_reachable(&m([2, 0, 0, 0])));
        assert!(rg.is_reachable(&m([0, 1, 1, 0])));
        assert!(rg.is_reachable(&m([0, 0, 0, 1])));
        assert!(!rg.is_reachable(&m([0, 0, 0, 2])));
    }

    #[test]
    fn self_loop_single_state() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::Circuit);
        let sys = System::new(net, [1]);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert_eq!(rg.state_count(), 1);
        assert_eq!(rg.edge_count(), 1);
        assert!(rg.is_deadlock_free());
    }

    #[test]
    fn multi_token_state_count() {
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
        let sys = System::new(net, [3, 0, 0]);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert_eq!(rg.state_count(), 10);
        assert!(rg.is_deadlock_free());
    }

    #[test]
    fn bfs_dfs_same_states() {
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
        let sys = System::new(net, [2, 0, 0]);

        let rg_bfs = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let rg_dfs = ReachabilityGraph::build(&sys, ExplorationOrder::DepthFirst);

        assert_eq!(rg_bfs.state_count(), rg_dfs.state_count());
        for marking in rg_bfs.markings() {
            assert!(rg_dfs.is_reachable(marking));
        }
    }

    #[test]
    fn mutex_mutual_exclusion() {
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
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        let sys = System::new(net, [1, 0, 0, 1, 0, 0, 1]);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(rg.is_deadlock_free());
        for marking in rg.markings() {
            assert!(
                marking[crit1] == 0 || marking[crit2] == 0,
                "mutual exclusion violated at {marking}"
            );
        }
        assert_eq!(rg.state_count(), 8);
    }

    #[test]
    fn path_produces_valid_firing_sequence() {
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
        let sys = System::new(net, [1, 0, 0]);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let target = m([0, 0, 1]);
        let path = rg.path_to(&target).expect("reachable");

        let mut replay = sys;
        for t in &path {
            replay.try_fire(*t).expect("path should be valid");
        }
        assert_eq!(replay.current_marking(), &target);
    }

    #[test]
    fn dining_philosophers_deadlock() {
        let n = 3;
        let mut b = NetBuilder::new();
        let forks: Vec<_> = (0..n).map(|_| b.add_place()).collect();
        let thinking: Vec<_> = (0..n).map(|_| b.add_place()).collect();
        let has_left: Vec<_> = (0..n).map(|_| b.add_place()).collect();
        let eating: Vec<_> = (0..n).map(|_| b.add_place()).collect();
        let take_left: Vec<_> = (0..n).map(|_| b.add_transition()).collect();
        let take_right: Vec<_> = (0..n).map(|_| b.add_transition()).collect();
        let release: Vec<_> = (0..n).map(|_| b.add_transition()).collect();

        for i in 0..n {
            let right_fork = forks[(i + 1) % n];
            b.add_arc((thinking[i], take_left[i]));
            b.add_arc((forks[i], take_left[i]));
            b.add_arc((take_left[i], has_left[i]));
            b.add_arc((has_left[i], take_right[i]));
            b.add_arc((right_fork, take_right[i]));
            b.add_arc((take_right[i], eating[i]));
            b.add_arc((eating[i], release[i]));
            b.add_arc((release[i], thinking[i]));
            b.add_arc((release[i], forks[i]));
            b.add_arc((release[i], right_fork));
        }

        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        let mut initial = vec![0u32; 4 * n];
        for i in 0..n {
            initial[forks[i].index()] = 1;
            initial[thinking[i].index()] = 1;
        }
        let sys = System::new(net, initial);

        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert!(!rg.is_deadlock_free());
    }

    #[test]
    fn edge_count_cycle() {
        let sys = two_place_cycle();
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);

        assert_eq!(rg.state_count(), 2);
        assert_eq!(rg.edge_count(), 2);
    }

    #[test]
    fn cycle_all_transitions_l4() {
        let sys = two_place_cycle();
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let levels = rg.liveness_levels();
        assert!(levels.iter().all(|&l| l == LivenessLevel::L4));
        assert!(rg.is_live());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let sys = System::new(two_place_cycle().into_parts().0, [0u32, 0]);
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let levels = rg.liveness_levels();
        assert!(levels.iter().all(|&l| l == LivenessLevel::L0));
        assert!(!rg.is_live());
    }

    #[test]
    fn absorbing_branch_mixed_liveness() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        b.add_arc((p2, t2)); b.add_arc((t2, p0));
        let net = b.build().unwrap();
        let sys = System::new(net, [1u32, 0, 0]);
        let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
        let levels = rg.liveness_levels();

        assert_eq!(levels[t0.idx], LivenessLevel::L1);
        assert_eq!(levels[t1.idx], LivenessLevel::L3);
        assert_eq!(levels[t2.idx], LivenessLevel::L3);
        assert!(!rg.is_live());
    }

    #[test]
    fn mutex_all_l4() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap();
        let sys = System::new(net, [1u32, 0, 0, 1, 0, 0, 1]);
        let cg = sys.build_coverability_graph();
        let rg = cg.into_reachability_graph().unwrap();
        let levels = rg.liveness_levels();

        assert!(levels.iter().all(|&l| l == LivenessLevel::L4));
        assert!(rg.is_live());
    }

    #[test]
    fn limited_exploration_by_state_count() {
        let sys = unbounded_producer();
        let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);

        while explorer.state_count() < 10 {
            if explorer.explore_next().is_none() { break; }
        }
        assert!(!explorer.is_fully_explored());
        assert!(explorer.state_count() >= 10);
    }

    #[test]
    fn iter_take() {
        let sys = unbounded_producer();
        let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);

        let steps: Vec<_> = explorer.iter().take(5).collect();
        assert_eq!(steps.len(), 5);
        assert!(!explorer.is_fully_explored());
    }

    #[test]
    fn step_by_step() {
        let sys = two_place_cycle();
        let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);

        assert_eq!(explorer.state_count(), 1);
        let mut count = 0;
        while let Some(_step) = explorer.explore_next() {
            count += 1;
        }
        assert!(count > 0);
        assert_eq!(explorer.state_count(), 2);
        assert!(explorer.is_fully_explored());

        let rg = ReachabilityGraph::try_from(explorer).expect("fully explored");
        assert!(rg.is_deadlock_free());
    }

    #[test]
    fn source_transitions_explored() {
        let mut b = NetBuilder::new();
        let [p0] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((t0, p0));
        let net = b.build().expect("valid net");
        let sys = System::new(net, [0]);

        let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
        let step = explorer.explore_next().expect("source transition should fire");
        assert!(step.is_new);
        assert_eq!(step.marking, m([1]));
    }

    #[test]
    fn explorer_try_into_fails_when_incomplete() {
        let sys = unbounded_producer();
        let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
        explorer.iter().take(3).for_each(drop);
        assert!(ReachabilityGraph::try_from(explorer).is_err());
    }
}
