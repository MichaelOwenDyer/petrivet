use crate::core::liveness::LivenessLevel;
use crate::core::marking::IdxMarking;
use crate::core::state_space::DenseStateGraph;
use crate::model::{LivenessAnalysis, LivenessMethod};
use crate::state_space::{CoverabilityGraph, ExplorationOrder, ExplorationStep, Omega, StateGraph, StateGraphExplorer};
use crate::{Net, PetriNet};

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
/// use petrivet::{NetBuilder, PetriNet};
/// use petrivet::state_space::ExplorationOrder;
/// use petrivet::state_space::reachability::{ReachabilityExplorer, ReachabilityGraph};
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
pub type ReachabilityExplorer<'a> = StateGraphExplorer<'a, u32>;

impl<'a> ReachabilityExplorer<'a> {
    /// Advance exploration by one step.
    ///
    /// Pops a frontier entry, fires the transition if enabled, applies
    /// ω-acceleration, and registers the result. Returns `None` when the
    /// frontier is exhausted (graph fully explored).
    pub fn explore_next(&mut self) -> Option<ExplorationStep<u32>> {
        self.core.explore_next().map(|(transition_idx, node_idx, is_new)| {
            let idx_marking = self.core.state_space.marking_at(node_idx);
            ExplorationStep {
                transition: self.mapping.transition(transition_idx),
                marking: self.mapping.marking(idx_marking.clone()),
                is_new,
            }
        })
    }

    /// Returns an iterator that drives exploration step by step.
    ///
    /// Each call to `next()` fires one transition and returns the step.
    /// The iterator ends when the frontier is exhausted.
    ///
    /// **Warning: infinite** for unbounded nets.
    ///
    /// ```
    /// use petrivet::builder::NetBuilder;
    /// use petrivet::net::system::PetriNet;
    /// use petrivet::{ReachabilityExplorer, ExplorationOrder};
    /// use petrivet::marking::IdxMarking;
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
    pub fn explore_iter(&mut self) -> impl Iterator<Item = ExplorationStep<u32>> + '_ {
        std::iter::from_fn(move || self.explore_next())
    }

    /// Explore until the frontier is exhausted.
    ///
    /// **Warning: does not terminate** for unbounded nets.
    pub fn explore_all(&mut self) {
        while self.core.explore_next().is_some() {}
    }
}

impl std::fmt::Debug for ReachabilityExplorer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachabilityExplorer")
            .field("markings", &self.marking_count())
            .field("transitions", &self.transition_count())
            .field("frontier", &self.core.frontier_count())
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
/// use petrivet::builder::NetBuilder;
/// use petrivet::net::system::PetriNet;
/// use petrivet::{ReachabilityGraph, ExplorationOrder};
/// use petrivet::marking::IdxMarking;
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
pub type ReachabilityGraph<'a> = StateGraph<'a, u32>;

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
            mapping: explorer.mapping,
        }
    }
}

impl ReachabilityGraph<'_> {
    /// Computes liveness levels for all transitions in a single pass.
    ///
    /// SCC-based decision procedure for bounded nets ([Murata 1989 §V-C](crate::literature#v-c--liveness-via-reachability-graph-sccs)):
    /// - L0 (dead): `t` does not label any edge.
    /// - L1: `t` labels at least one edge.
    /// - L3 (≡L2 for bounded): `t` labels an edge within some non-trivial SCC.
    /// - L4 (live): `t` labels an edge in **every** terminal SCC.
    pub(crate) fn liveness_levels(&self) -> impl Iterator<Item = LivenessLevel> {
        use petgraph::visit::EdgeRef;

        let transition_count = self.state_space.net.transition_count() as usize;
        let graph = &self.state_space.graph;
        let sccs = petgraph::algo::tarjan_scc(graph);

        let mut node_to_scc = vec![0usize; graph.node_count()];
        for (scc_id, scc) in sccs.iter().enumerate() {
            for &node in scc {
                node_to_scc[node.index()] = scc_id;
            }
        }

        let scc_count = sccs.len();
        let mut has_external_edge = vec![false; scc_count].into_boxed_slice();
        let mut scc_is_nontrivial = vec![false; scc_count].into_boxed_slice();
        let mut scc_has_t = vec![vec![false; transition_count].into_boxed_slice(); scc_count].into_boxed_slice();
        let mut t_fires_anywhere = vec![false; transition_count].into_boxed_slice();

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

        (0..transition_count).map(move |t_idx| {
            if !t_fires_anywhere[t_idx] {
                LivenessLevel::L0
            } else if (0..scc_count).all(|scc_idx| has_external_edge[scc_idx] || scc_has_t[scc_idx][t_idx]) {
                LivenessLevel::L4
            } else if (0..scc_count).any(|scc_idx| scc_is_nontrivial[scc_idx] && scc_has_t[scc_idx][t_idx]) {
                LivenessLevel::L3
            } else {
                LivenessLevel::L1
            }
        })
    }

    /// Returns all transitions in the Petri net with their associated liveness levels.
    #[must_use]
    pub fn liveness(&self) -> LivenessAnalysis {
        let levels = self.mapping
            .transitions()
            .zip(self.liveness_levels())
            .collect();
        LivenessAnalysis {
            levels,
            method: LivenessMethod::ReachabilityGraph,
        }
    }

    /// Returns true if all transitions in the Petri net are live.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.liveness_levels().all(|l| l == LivenessLevel::L4)
    }

    /// Whether every transition appears on some edge of the reachability graph,
    /// i.e. no transition in the Petri net is dead.
    #[must_use]
    pub fn is_quasi_live(&self) -> bool {
        self.liveness_levels().all(|l| l != LivenessLevel::L0)
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
            mapping: explorer.mapping,
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
        // if any marking contains ω, the system is unbounded
        // and the reachability graph would be infinite
        if !cg.is_bounded() {
            return Err(cg);
        }

        // todo: this creates a new graph, can we reuse the old one instead?
        let graph = cg.state_space.graph.map(
            |_idx, omega_marking| unwrap_omega_marking_to_u32(omega_marking.clone()),
            |_src, &t| t,
        );
        let seen = cg.state_space.seen
            .into_iter()
            .map(|(marking, idx)| {
                (unwrap_omega_marking_to_u32(marking), idx)
            })
            .collect();
        let reachable_state_space = DenseStateGraph {
            net: cg.state_space.net,
            initial_idx: cg.state_space.initial_idx,
            graph,
            seen,
        };

        Ok(ReachabilityGraph {
            state_space: reachable_state_space,
            mapping: cg.mapping,
        })
    }
}

fn unwrap_omega_marking_to_u32(om: IdxMarking<Omega>) -> IdxMarking<u32> {
    om.into_iter()
        .map(|o| o.finite().expect("ω cannot be converted to u32"))
        .collect()
}