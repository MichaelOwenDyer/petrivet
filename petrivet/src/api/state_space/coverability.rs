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
//! use petrivet::api::builder::NetBuilder;
//! use petrivet::api::net::system::PetriNet;
//! use petrivet::{CoverabilityGraph, ExplorationOrder};
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0] = b.add_transitions();
//! b.add_arc((p0, t0));
//! b.add_arc((t0, p0));
//! b.add_arc((t0, p1));
//! let net = b.build().expect("valid net");
//! let sys = PetriNet::new(net, [1, 0]);
//! let cg = sys.build_coverability_graph();
//! assert!(!cg.is_bounded());
//! ```
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
//! use petrivet::api::builder::NetBuilder;
//! use petrivet::api::net::system::PetriNet;
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

use crate::core::state_space::coverability::IdxOmegaMarking;
use crate::marking::Marking;
use crate::state_space::{ExplorationOrder, ExplorationStep, ReachabilityGraph, StateGraph, StateGraphExplorer};
use crate::{Net, PetriNet};
use std::fmt;

pub use crate::core::state_space::coverability::Omega;

/// An ω-marking: a marking where token counts can either be a finite number or `ω`.
///
/// `ω` represents an unbounded token count, i.e. an arbitrarily large finite number of tokens.
/// Used in coverability analysis to mark places that can grow without bound.
///
/// See also: Karp-Miller coverability graph.
pub type OmegaMarking = Marking<Omega>;

impl OmegaMarking {
    /// Returns true if all token counts in this marking are finite (no ω).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.support.iter().all(|(_, o)| o.is_finite())
    }

    /// Returns true if any token count in this marking is unbounded (ω).
    #[must_use]
    pub fn is_unbounded(&self) -> bool {
        self.support.iter().any(|(_, o)| o.is_unbounded())
    }

    /// Returns true if all token counts in this marking are finite
    /// and less than or equal to `b`.
    #[must_use]
    pub fn is_b_bounded(&self, b: u32) -> bool {
        self.support.iter().all(|(_, o)| o.is_b_bounded(b))
    }
}

impl From<Marking<u32>> for OmegaMarking {
    fn from(value: Marking<u32>) -> Self {
        value.into_iter()
            .map(|(p, t)| (p, Omega::Finite(t)))
            .collect()
    }
}

/// An incremental exploration handle for constructing a coverability graph of a Petri net.
///
/// Unlike [`ReachabilityExplorer`], this is guaranteed to terminate, even for unbounded nets.
pub type CoverabilityExplorer<'a> = StateGraphExplorer<'a, Omega>;

impl<'a> CoverabilityExplorer<'a> {
    /// Advance exploration by one step.
    ///
    /// Fires a transition, applies ω-acceleration, and returns the result.
    /// Returns `None` when the frontier is exhausted (graph fully explored).
    pub fn explore_next(&mut self) -> Option<ExplorationStep<Omega>> {
        self.core.explore_next().map(|(transition_idx, node_idx, is_new)| {
            let idx_marking = self.core.state_space.marking_at(node_idx);
            ExplorationStep {
                transition: self.mapping.transition(transition_idx),
                marking: self.mapping.marking(idx_marking.clone()),
                is_new,
            }
        })
    }

    /// Advances exploration until a marking covering `target` is found,
    /// and returns the marking and a firing sequence from the initial marking to it.
    ///
    /// Already-known nodes in the explorer's graph are checked first, then the
    /// frontier is advanced until a witness appears or exploration finishes.
    pub fn find_cover(&mut self, target: OmegaMarking) -> Option<OmegaMarking> {
        let target_idx_marking = self.mapping.idx_marking(target);
        self.core
            .find(|idx_marking| *idx_marking >= target_idx_marking)
            .map(|idx_marking| self.mapping.marking(idx_marking.clone()))
    }

    /// Consume the explorer and drive exploration to completion.
    ///
    /// This materializes a completed coverability graph with the guarantee
    /// that `is_fully_explored()` is true.
    #[must_use]
    pub fn build_coverability_graph(mut self) -> CoverabilityGraph<'a> {
        while self.core.explore_next().is_some() {}
        CoverabilityGraph {
            state_space: self.core.state_space,
            mapping: self.mapping,
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
        while let Some((_transition_idx, node_idx, _is_new)) = self.core.explore_next() {
            if !self.core.state_space.marking_at(node_idx).is_finite() {
                // short-circuit if we encounter a marking with ω
                return Err(self);
            }
        }
        let cg = CoverabilityGraph {
            state_space: self.core.state_space,
            mapping: self.mapping,
        };
        cg.into_reachability_graph().map_err(|_| {
            unreachable!("ω-free CG must promote successfully; ω would have been detected above")
        })
    }

    /// Returns an iterator that drives exploration step by step.
    ///
    /// Each call to `next()` fires one transition (with ω-acceleration)
    /// and returns the step. The iterator ends when the frontier is
    /// exhausted (Karp-Miller guarantees termination).
    pub fn explore_iter(&mut self) -> impl Iterator<Item = ExplorationStep<Omega>> + '_ {
        std::iter::from_fn(move || self.explore_next())
    }
}

impl fmt::Debug for CoverabilityExplorer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoverabilityExplorer")
            .field("markings", &self.marking_count())
            .field("transitions", &self.transition_count())
            .field("frontier", &self.core.frontier_count())
            .finish()
    }
}

pub type CoverabilityGraph<'a> = StateGraph<'a, Omega>;

impl<'a> CoverabilityGraph<'a> {
    /// Build the coverability graph for a system in one shot.
    pub fn new(system: &'a PetriNet<impl AsRef<Net>>) -> Self {
        CoverabilityExplorer::new(system, ExplorationOrder::BreadthFirst).build_coverability_graph()
    }

    /// Whether the net is bounded: no ω appears in any discovered marking.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.state_space.graph.node_weights().all(IdxOmegaMarking::is_finite)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::builder::NetBuilder;
    use crate::api::class::NetClass;
    use crate::api::net::{Net, Place};

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0 (bounded)
    fn two_place_cycle() -> (PetriNet<Net>, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid net");
        (net.with_initial_marking([(p0, 1)]), p0, p1)
    }

    /// Unbounded: t0 consumes from p0 and produces to both p0 and p1
    fn unbounded_producer() -> (PetriNet<Net>, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arcs((p0, t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        (net.with_initial_marking([(p0, 1)]), p0, p1)
    }

    /// Self-loop with 0 tokens: immediate deadlock
    fn deadlock_net() -> PetriNet<Net> {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arcs((p0, t0, p0));
        let net = b.build().expect("valid net");
        net.with_initial_marking([])
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
        use crate::api::state_space::coverability::Omega::Finite;
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
            assert!(!step.marking.iter().any(|(_, o)| o.is_unbounded()));
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
            if step.marking.iter().any(|(_, o)| o.is_unbounded()) {
                break;
            }
        }
        let cg = cg.build_coverability_graph();
        assert!(!cg.is_bounded());
    }

    #[test]
    fn promotion_bounded() {
        let (sys, _p0, _p1) = two_place_cycle();
        let cg = sys.build_coverability_graph();
        let rg = cg.into_reachability_graph().expect("should be bounded");

        assert_eq!(rg.marking_count(), 2);
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
        assert_eq!(rg.marking_count(), 2);
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
        let sys = net.with_initial_marking([(p0, 1)]);
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
        let sys = net.with_initial_marking([(p0, 2)]);

        let cg = sys.build_coverability_graph();
        assert!(cg.is_bounded());
        let cg_states = cg.marking_count();
        let cg_edges = cg.transition_count();

        let rg = cg.into_reachability_graph().expect("bounded");
        assert_eq!(rg.marking_count(), cg_states);
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
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
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
        let sys = net.with_initial_marking([(p0, 1)]);
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
        let sys = net.with_initial_marking([(idle1, 1), (idle2, 1), (mutex, 1)]);
        let cg = sys.build_coverability_graph();

        assert!(cg.is_bounded());
        assert!(cg.is_deadlock_free());
        let zero = Omega::Finite(0);
        for marking in cg.markings() {
            let c1 = marking.get(crit1);
            let c2 = marking.get(crit2);
            assert!(
                c1 == zero || c2 == zero,
                "mutual exclusion violated: {marking:?}",
            );
        }

        assert!(cg.cover([(crit1, 1.into()), (crit2, 1.into())].into()).is_none());

        let rg = cg.into_reachability_graph().expect("bounded");
        assert_eq!(rg.marking_count(), 8);
    }
}