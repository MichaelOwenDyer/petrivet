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
//! use petrivet::net::system::PetriNet;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! let net = b.build().unwrap();
//! let sys = PetriNet::new(net, [1, 0]);
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
use crate::net::marking::{IdxMarking, Omega};
use crate::net::system::PetriNet;
use crate::net::{Net, Transition};
use crate::state_space::explorer::StateGraph;
use crate::state_space::{explorer::StateSpaceExplorer, CoverabilityGraph, ExplorationOrder};
use crate::{Marking, Place};
use crate::net::idx::TransitionIdx;

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
/// use petrivet::net::system::PetriNet;
/// use petrivet::{ReachabilityExplorer, ReachabilityGraph, ExplorationOrder};
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0] = b.add_transitions();
/// b.add_arcs((p0, t0));
/// b.add_arc((t0, p0));
/// b.add_arc((t0, p1));
/// let net = b.build().unwrap();
/// let sys = PetriNet::new(net, [1, 0]);
///
/// // Explore an unbounded net incrementally, stopping after 50 states
/// let mut explorer = sys.explore_reachability(ExplorationOrder::BreadthFirst);
/// explorer.iter().take(50).for_each(|s| println!("{:#?}", s.marking));
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
    pub fn new(sys: &'a PetriNet<impl AsRef<Net>>, order: ExplorationOrder) -> Self {
        let net = sys.core.net();
        let marking = sys.core.current_marking.clone();
        Self {
            core: StateSpaceExplorer::new(net, marking, order),
        }
    }

    /// Advance exploration by one step.
    ///
    /// Returns `None` when the frontier is exhausted (fully explored).
    pub(crate) fn explore_next_inner(&mut self) -> Option<(TransitionIdx, IdxMarking, bool)> {
        loop {
            let (src_idx, t_idx) = self.core.pop_frontier()?;
            if !self.core.is_enabled(src_idx, t_idx) {
                continue;
            }
            let new_marking = self.core.fire(src_idx, t_idx);
            let is_new = self.core.register(src_idx, t_idx, new_marking.clone());
            return Some((t_idx, new_marking, is_new));
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
            let is_new = self.core.register(src_idx, t, new_marking.clone());
            return Some(ReachabilityStep {
                transition: self.core.state_space.net.ordered_transitions[t],
                marking: self.core.state_space.net.to_marking(new_marking),
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
    /// use petrivet::net::system::PetriNet;
    /// use petrivet::{ReachabilityExplorer, ExplorationOrder};
    /// use petrivet::net::marking::IdxMarking;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// let sys = PetriNet::new(net, [1, 0]);
    ///
    /// let mut explorer = sys.explore_reachability(ExplorationOrder::BreadthFirst);
    ///
    /// // Search for a specific marking
    /// let target = IdxMarking::from([0u32, 1]);
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
    pub const fn exploration_order(&self) -> ExplorationOrder {
        self.core.order
    }

    /// Change the exploration order for subsequent steps.
    pub const fn set_exploration_order(&mut self, order: ExplorationOrder) {
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
    pub fn initial_marking(&self) -> Marking {
        let inner = self.core.state_space.marking_at(self.core.state_space.initial_idx).clone();
        self.core.state_space.net.to_marking(inner)
    }

    /// Whether `target` has been discovered so far.
    #[must_use]
    pub fn is_reachable(&self, target: Marking) -> bool {
        let target = self.core.state_space.net.to_idx_marking(target);
        self.core.state_space.seen.contains_key(&target)
    }

    /// Returns a firing sequence from the initial marking to `target`,
    /// among states discovered so far.
    #[must_use]
    pub fn path_to(&self, target: Marking) -> Option<Box<[Transition]>> {
        let target = self.core.state_space.net.to_idx_marking(target);
        let &target_idx = self.core.state_space.seen.get(&target)?;
        self.core.state_space.path_from_initial_to(target_idx).map(|path| {
            path.into_iter()
                .map(|t_idx| self.core.state_space.net.ordered_transitions[t_idx])
                .collect()
        })
    }

    /// Whether a marking has been discovered so far.
    #[must_use]
    pub fn contains(&self, marking: Marking) -> bool {
        let marking = self.core.state_space.net.to_idx_marking(marking);
        self.core.state_space.seen.contains_key(&marking)
    }

    /// Iterator over all discovered markings.
    pub fn states(&self) -> impl Iterator<Item = Marking> + '_ {
        self.core.state_space.graph
            .node_weights()
            .cloned()
            .map(|marking| self.core.state_space.net.to_marking(marking))
    }

    /// Drive exploration until either:
    ///
    /// - `predicate` returns `true` for some reachable marking — in which
    ///   case `true` is returned immediately, or
    /// - the frontier is exhausted (the entire reachability graph has been
    ///   explored without the predicate ever firing) — in which case
    ///   `false` is returned.
    ///
    /// **Does not terminate on unbounded nets.** Callers must rule that
    /// out before calling — typically via
    /// [`Net::is_structurally_bounded`](crate::Net::is_structurally_bounded)
    /// or by going through the coverability path first.
    ///
    /// This is the kernel for short-circuiting global-property questions
    /// of the form "is there a reachable marking such that …". On nets
    /// where the witness is shallow it is dramatically cheaper than
    /// building the full reachability graph and querying it afterwards;
    /// on nets where the answer is "no" the cost is identical (a full
    /// exploration).
    pub(crate) fn any_marking_satisfies(
        &mut self,
        mut predicate: impl FnMut(&IdxMarking) -> bool,
    ) -> bool {
        let initial = self.core.state_space.initial_marking();
        if predicate(initial) {
            return true;
        }
        while let Some((_t_idx, marking, is_new)) = self.explore_next_inner() {
            if is_new && predicate(&marking) {
                return true;
            }
        }
        false
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
/// use petrivet::net::system::PetriNet;
/// use petrivet::{ReachabilityGraph, ExplorationOrder};
/// use petrivet::net::marking::IdxMarking;
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap();
/// let sys = PetriNet::new(net, [1, 0]);
///
/// let rg = ReachabilityGraph::build(&sys);
///
/// // Query the graph
/// assert_eq!(rg.state_count(), 2);
/// assert!(rg.is_reachable(&IdxMarking::from([0u32, 1])));
/// assert!(rg.is_deadlock_free());
/// assert!(rg.is_live());
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
    pub fn build(sys: &'a PetriNet<impl AsRef<Net>>) -> Self {
        let mut explorer = sys.explore_reachability(ExplorationOrder::BreadthFirst);
        explorer.explore_all(); // WARNING: does not terminate for unbounded nets!
        // explore_all() returned, so the frontier is exhausted,
        // so is_fully_explored() is true, so conversion to ReachabilityGraph is infallible.
        ReachabilityGraph {
            state_space: explorer.core.state_space,
        }
    }

    /// Number of distinct reachable markings.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.state_space.graph.node_count()
    }

    /// Number of edges (transition firings) in the graph.
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.state_space.graph.edge_count()
    }

    /// The initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> Marking {
        let inner = self.state_space.marking_at(self.state_space.initial_idx).clone();
        self.state_space.net.to_marking(inner)
    }

    /// Whether `target` is reachable from the initial marking.
    #[must_use]
    pub fn is_reachable(&self, target: Marking) -> bool {
        let target = self.state_space.net.to_idx_marking(target);
        self.state_space.seen.contains_key(&target)
    }

    /// Returns a firing sequence from the initial marking to `target`.
    ///
    /// When built with BFS, this is a minimal firing sequence.
    #[must_use]
    pub fn path_to(&self, target: Marking) -> Option<Box<[Transition]>> {
        let target = self.state_space.net.to_idx_marking(target);
        self.path_to_marking(&target)
    }

    pub(crate) fn path_to_marking(&self, target: &IdxMarking) -> Option<Box<[Transition]>> {
        self.state_space.seen.get(target)
            .and_then(|&target_idx| self.state_space.path_from_initial_to(target_idx))
            .map(|path| {
                path.into_iter()
                    .map(|t_idx| self.state_space.net.ordered_transitions[t_idx])
                    .collect()
            })
    }

    /// Whether a marking exists in the graph.
    #[must_use]
    pub fn contains(&self, marking: Marking) -> bool {
        let marking = self.state_space.net.to_idx_marking(marking);
        self.state_space.seen.contains_key(&marking)
    }

    pub(crate) fn markings_inner(&self) -> impl Iterator<Item = &IdxMarking<u32>> {
        self.state_space.graph.node_weights()
    }

    /// Iterator over all reachable markings.
    pub fn markings(&self) -> impl Iterator<Item = Marking> {
        self.markings_inner()
            .cloned()
            .map(|marking| self.state_space.net.to_marking(marking))
    }

    /// All markings with no enabled transitions.
    pub fn deadlocks(&self) -> impl Iterator<Item = Marking> {
        self.state_space
            .deadlock_indices()
            .map(|idx| self.state_space.marking_at(idx))
            .cloned()
            .map(|marking| self.state_space.net.to_marking(marking))
    }

    /// Whether every reachable marking has at least one enabled transition.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.state_space.deadlock_indices().next().is_none()
    }

    #[must_use]
    pub fn liveness(&self) -> Box<[(Transition, LivenessLevel)]> {
        self.state_space.net.transitions()
            .zip(self.liveness_levels())
            .collect()
    }

    /// Upper bound on the token count for each place across all discovered markings.
    #[must_use]
    pub fn place_bounds(&self) -> Box<[(Place, u32)]> {
        let place_bounds: IdxMarking = self.markings_inner().fold(
            IdxMarking::zeros(self.state_space.net.place_count()),
            IdxMarking::ceil,
        );
        self.state_space.net.places()
            .zip(place_bounds)
            .collect()
    }

    /// Upper bound on the token count for a given place across all
    /// discovered markings. Returns `Omega::Unbounded` if the place is
    /// unbounded.
    #[must_use]
    pub fn place_bound(&self, p: Place) -> u32 {
        self.state_space.net.place_index(p).map_or(
            0,
            |&idx| {
                self.markings_inner()
                    .map(|marking| marking[idx])
                    .max()
                    .unwrap_or(0)
            },
        )
    }

    /// The largest single-place bound across the entire net.
    ///
    /// Equal to the maximum number of tokens that any *one* place can hold
    /// in any reachable marking. This is the value the Model Checking Contest
    /// reports as `STATE_SPACE MAX_TOKEN_IN_PLACE`.
    #[must_use]
    pub fn max_token_in_any_place(&self) -> u32 {
        self.markings_inner()
            .flat_map(|m| m.iter().copied())
            .max()
            .unwrap_or(0)
    }

    /// The largest total token count across all reachable markings.
    ///
    /// For each reachable marking, sum the tokens over all places, then
    /// take the maximum over all markings. This is the value the Model
    /// Checking Contest reports as `STATE_SPACE MAX_TOKEN_PER_MARKING`.
    #[must_use]
    pub fn max_token_per_marking(&self) -> u32 {
        self.markings_inner()
            .map(|m| m.iter().copied().sum::<u32>())
            .max()
            .unwrap_or(0)
    }

    /// Computes liveness levels for all transitions in a single pass.
    ///
    /// SCC-based decision procedure for bounded nets ([Murata 1989 §V-C](crate::literature#v-c--liveness-via-reachability-graph-sccs)):
    /// - L0 (dead): `t` does not label any edge.
    /// - L1: `t` labels at least one edge.
    /// - L3 (≡L2 for bounded): `t` labels an edge within some non-trivial SCC.
    /// - L4 (live): `t` labels an edge in **every** terminal SCC.
    ///
    /// Returns a dense `TransitionMap<LivenessLevel>` indexed by transition index.
    /// Store the result if you need to query it multiple times.
    ///
    /// To get per-key results, use [`PetriNet::analyze_liveness`] which returns a
    /// [`LivenessAnalysis`] with key-based access via
    /// [`transition_level`](crate::analysis::model::LivenessAnalysis::transition_level).
    #[must_use]
    pub(crate) fn liveness_levels(&self) -> Box<[LivenessLevel]> {
        use petgraph::visit::EdgeRef;

        let n_transitions = self.state_space.net.transition_count() as usize;
        let graph = &self.state_space.graph;
        // todo: replace with Tarjan's algorithm for better performance
        let sccs = petgraph::algo::kosaraju_scc(graph);

        if sccs.is_empty() || n_transitions == 0 {
            return std::iter::repeat_n(LivenessLevel::L0, n_transitions).collect();
        }

        let mut node_to_scc = vec![0usize; graph.node_count()];
        for (scc_id, scc) in sccs.iter().enumerate() {
            for &node in scc {
                node_to_scc[node.index()] = scc_id;
            }
        }

        let n_scc = sccs.len();
        let mut has_external_edge = vec![false; n_scc].into_boxed_slice();
        let mut scc_is_nontrivial = vec![false; n_scc].into_boxed_slice();
        let mut scc_has_t = vec![vec![false; n_transitions].into_boxed_slice(); n_scc].into_boxed_slice();
        let mut t_fires_anywhere = vec![false; n_transitions].into_boxed_slice();

        for edge in graph.edge_references() {
            let t = *edge.weight();
            let src_scc = node_to_scc[edge.source().index()];
            let dst_scc = node_to_scc[edge.target().index()];

            t_fires_anywhere[t] = true;

            if src_scc == dst_scc {
                scc_is_nontrivial[src_scc] = true;
                scc_has_t[src_scc][t] = true;
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

    /// Whether some reachable marking has no enabled transitions.
    ///
    /// This is the answer to `EF deadlock`, used by the MCC
    /// `ReachabilityDeadlock` examination: TRUE iff a deadlock is reachable.
    /// Equivalent to `!is_deadlock_free()`, exposed under this name to
    /// match the formula it answers.
    #[must_use]
    pub fn has_reachable_deadlock(&self) -> bool {
        !self.is_deadlock_free()
    }

    /// Whether every transition appears on some edge of the reachability graph.
    ///
    /// This answers `∀t EF is-fireable(t)`, used by the MCC `QuasiLiveness`
    /// examination. A transition that never fires (L0) is the only obstacle.
    #[must_use]
    pub fn is_quasi_live(&self) -> bool {
        self.liveness_levels().iter().all(|&l| l != LivenessLevel::L0)
    }

    /// Whether every reachable marking puts at most one token in every place.
    ///
    /// This answers `∀p AG tokens-count(p) ≤ 1`, used by the MCC `OneSafe`
    /// examination. Equivalent to `max_token_in_any_place() <= 1`, exposed
    /// under this name to match the formula it answers.
    #[must_use]
    pub fn is_one_safe(&self) -> bool {
        self.max_token_in_any_place() <= 1
    }

    /// Whether some place holds the same token count in every reachable marking.
    ///
    /// This answers `∃p ∃x AG tokens-count(p) = x`, used by the MCC
    /// `StableMarking` examination. Note: the constant `x` is allowed to be
    /// zero, so a place that's never marked still counts as "stable".
    #[must_use]
    pub fn has_stable_place(&self) -> bool {
        let n_places = self.state_space.net.place_count() as usize;
        if n_places == 0 {
            return true;
        }

        let mut markings = self.markings_inner();
        let Some(first) = markings.next() else {
            // No reachable markings at all: every place is vacuously stable.
            return true;
        };
        let baseline: Vec<u32> = first.iter().copied().collect();
        // Per-place flag: is the count still equal to its first-observed value?
        let mut still_stable: Vec<bool> = vec![true; n_places];
        for marking in markings {
            for (p_idx, token_count) in marking.iter().copied().enumerate() {
                if still_stable[p_idx] && token_count != baseline[p_idx] {
                    still_stable[p_idx] = false;
                }
            }
        }
        still_stable.into_iter().any(|b| b)
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

fn unwrap_omega_marking_to_u32(om: &IdxMarking<Omega>) -> IdxMarking<u32> {
    om.iter()
        .map(|o| match o {
            Omega::Finite(n) => *n,
            Omega::Unbounded => panic!("unwrap_omega_marking called on unbounded graph"),
        })
        .collect()
}