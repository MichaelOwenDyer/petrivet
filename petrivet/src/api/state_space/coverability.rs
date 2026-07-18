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
//! use petrivet::builder::NetBuilder;
//! use petrivet::system::PetriNet;
//! use petrivet::state_space::{CoverabilityGraph, ExplorationOrder};
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0] = b.add_transitions();
//! b.add_arc((p0, t0));
//! b.add_arc((t0, p0));
//! b.add_arc((t0, p1));
//! let net = b.build().expect("valid net");
//! let sys = PetriNet::new(net, [(p0, 1)]);
//! let cg = sys.build_coverability_graph();
//! assert!(!cg.is_bounded());
//! ```

use ahash::HashMap;
use crate::boundedness::{Boundedness, K};
use crate::core::marking::IdxMarking;
use crate::core::state_space::coverability::IdxOmegaMarking;
pub use crate::core::state_space::coverability::Omega;
use crate::core::state_space::{DenseStateGraph, ExploreNext};
use crate::marking::Marking;
use crate::net::Place;
use crate::state_space::reachability::ReachabilityGraph;
use crate::state_space::{StateGraph, StateGraphExplorer};

/// An ω-marking: a marking where token counts can either be a finite number or `ω`.
///
/// `ω` represents an unbounded token count, i.e. an arbitrarily large finite number of tokens.
/// Used in coverability analysis to mark places that can grow without bound.
///
/// See also: Karp-Miller coverability graph.
pub type OmegaMarking = Marking<Omega>;

impl OmegaMarking {
    /// Returns true if any token count in this marking is unbounded (ω).
    #[must_use]
    pub fn has_omega(&self) -> bool {
        self.support.iter().any(|(_, tokens)| tokens.is_omega())
    }

    /// Returns true if all token counts in this marking are finite (no ω).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        !self.has_omega()
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
    pub fn try_build_reachability_graph(mut self) -> Result<ReachabilityGraph<'a>, Self> {
        while let Some((_transition_idx, node_idx, is_new)) = self.core.explore_next() {
            if is_new && self.core.state_space.marking_at(node_idx).has_omega() {
                // short-circuit if we encounter a marking with ω
                return Err(self);
            }
        }
        ReachabilityGraph::try_from(CoverabilityGraph {
            state_space: self.core.state_space,
            mapping: self.mapping,
        }).map_err(|_| {
            unreachable!("ω-free CG must promote successfully; ω would have been detected above")
        })
    }
}

impl std::fmt::Debug for CoverabilityExplorer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoverabilityExplorer")
            .field("markings", &self.marking_count())
            .field("transitions", &self.transition_count())
            .field("frontier", &self.core.frontier_count())
            .finish()
    }
}

/// A fully-explored Karp-Miller coverability graph of a Petri net.
pub type CoverabilityGraph<'a> = StateGraph<'a, Omega>;

impl<'a> CoverabilityGraph<'a> {
    /// Returns true if any marking in the coverability graph is unbounded (contains ω).
    ///
    /// If this returns `true`, the system is unbounded and the [`ReachabilityGraph`] is infinite.
    #[must_use]
    pub fn has_omega_marking(&self) -> bool {
        self.state_space.graph.node_weights().any(IdxOmegaMarking::has_omega)
    }

    /// Returns true if every marking in the coverability graph is finite (contains no ω).
    ///
    /// If this returns `true`, the system is bounded and the coverability graph is exactly
    /// the [`ReachabilityGraph`].
    ///
    /// If this returns `false`, the system is unbounded and the reachability graph is infinite.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        !self.state_space.graph.node_weights().any(IdxOmegaMarking::has_omega)
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
    pub fn try_into_reachability_graph(self) -> Result<ReachabilityGraph<'a>, Self> {
        ReachabilityGraph::try_from(self)
    }

    /// Returns the maximum value of each place across all markings in the coverability graph.
    /// Places that were marked as Omega are unbounded.
    #[must_use]
    pub fn place_bounds(&self) -> HashMap<Place, Boundedness> {
        let mut markings = self.state_space.markings();
        let initial_marking = markings.next().expect("always at least one marking in the graph");
        let bounds = markings.fold(
            initial_marking.iter().copied().collect::<Vec<_>>(),
            |mut bounds_so_far, next_marking| {
                bounds_so_far.iter_mut()
                    .zip(next_marking.iter())
                    .for_each(|(bound, &next)| {
                        *bound = core::cmp::max(*bound, next);
                    });
                bounds_so_far
            },
        ).into_iter().map(|o| match o {
            Omega::Finite(tokens) => Boundedness::Bounded(tokens as K),
            Omega::Unbounded => Boundedness::Unbounded,
        });
        self.mapping.places().zip(bounds).collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{Net, NetBuilder, NetClass, PetriNet, Place};
    use crate::state_space::ExplorationOrder;

    /// Two-place cycle: p0 → t0 → p1 → t1 → p0 (bounded)
    fn two_place_cycle() -> (PetriNet<Net>, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid net");
        (PetriNet::new(net, [(p0, 1)]), p0, p1)
    }

    /// Unbounded: t0 consumes from p0 and produces to both p0 and p1
    fn unbounded_producer() -> (PetriNet<Net>, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arcs((p0, t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        (PetriNet::new(net, [(p0, 1)]), p0, p1)
    }

    /// Self-loop with 0 tokens: immediate deadlock
    fn deadlock_net() -> PetriNet<Net> {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0] = b.add_transitions();
        b.add_arcs((p0, t0, p0));
        let net = b.build().expect("valid net");
        PetriNet::new(net, [])
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
        use crate::state_space::Omega::Finite;
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
            assert!(!step.marking.has_omega());
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
            if step.marking.has_omega() {
                break;
            }
        }
        let cg = cg.build_graph();
        assert!(!cg.is_bounded());
    }

    #[test]
    fn promotion_bounded() {
        let (sys, _p0, _p1) = two_place_cycle();
        let cg = sys.build_coverability_graph();
        let rg = cg.try_into_reachability_graph().expect("should be bounded");

        assert_eq!(rg.marking_count(), 2);
    }

    #[test]
    fn promotion_unbounded_returns_err() {
        let (sys, _, _) = unbounded_producer();
        let cg = sys.build_coverability_graph();
        let result = cg.try_into_reachability_graph();
        assert!(result.is_err());
    }

    #[test]
    fn rg_or_cg_short_circuits_on_unbounded() {
        let (sys, _, _) = unbounded_producer();
        match sys.try_build_reachability_graph() {
            Ok(_) => panic!("unbounded net must short-circuit"),
            Err(cg) => assert!(!cg.is_fully_explored(), "frontier preserved on bail-out"),
        }
    }

    #[test]
    fn rg_or_cg_completes_for_bounded() {
        let (sys, _p0, _p1) = two_place_cycle();
        let rg = sys.try_build_reachability_graph()
            .expect("bounded net must yield reachability graph");
        assert_eq!(rg.marking_count(), 2);
    }

    #[test]
    fn switch_order_mid_exploration() {
        let (sys, _p0, _p1) = two_place_cycle();
        let mut cg = sys.explore_coverability(ExplorationOrder::BreadthFirst);
        cg.explore_next();
        cg.set_exploration_order(ExplorationOrder::DepthFirst);
        let cg = cg.build_graph();
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

        let rg = cg.try_into_reachability_graph().expect("bounded");
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
        let cg_bfs = sys.explore_coverability(ExplorationOrder::BreadthFirst).build_graph();
        let cg_dfs = sys.explore_coverability(ExplorationOrder::DepthFirst).build_graph();

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

        let rg = cg.try_into_reachability_graph().expect("bounded");
        assert_eq!(rg.marking_count(), 8);
    }
}