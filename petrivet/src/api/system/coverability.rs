use crate::core::analysis::exact_matrix::{self, CoveringInvariantExact};
use crate::core::coverability::find_candidate_covering_parikh_vector;
use crate::core::state_space::coverability::IdxOmegaMarking;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;
use crate::state_space::{ExplorationOrder, OmegaMarking};

#[derive(Debug, Clone)]
pub enum CoverabilityResult {
    /// The target marking is coverable from M₀.
    Coverable(CoverabilityProof),
    /// The target marking is not coverable from M₀.
    Uncoverable(NonCoverabilityProof),
}

impl CoverabilityResult {
    /// Whether the target is coverable.
    #[must_use]
    pub const fn is_coverable(&self) -> bool {
        matches!(self, Self::Coverable(_))
    }

    /// Whether the target is not coverable.
    #[must_use]
    pub const fn is_uncoverable(&self) -> bool {
        matches!(self, Self::Uncoverable(_))
    }
}

/// Proof that a marking is coverable.
///
/// For bounded nets, the coverability graph contains only finite markings, so the
/// returned `covering_marking` is a reachable marking.
///
/// For unbounded nets, the coverability graph may contain ω-markings. An ω-marking
/// that covers the target is still a valid proof of coverability, but it may not be
/// a reachable marking itself. Instead, it represents the existence of reachable
/// markings that can exceed any finite threshold on its ω-places.
///
/// A node of the coverability graph covers the target.
///
/// The witness firing sequence reaches a node in the coverability graph. The
/// node marking may contain ω.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CoverabilityProof {
    /// A transition firing sequence from `M₀` to a marking `M` which covers the target.
    pub firing_sequence: Vec<Transition>,
    /// The node marking M″ with M″ ≥ target (may contain ω).
    pub covering_marking: OmegaMarking,
}

/// Various methods to demonstrate that a marking is not coverable
/// in a given system.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NonCoverabilityProof {
    /// A non-negative place weighting `y` satisfies `yᵀ·C ≤ 0` — the weighted
    /// token count `y·M` can never increase across a firing — while the target's
    /// weighted count strictly exceeds the initial marking's
    /// (`y·target > y·M₀`). No reachable marking can therefore cover the target.
    /// The weighting is found exactly, over the rationals, from the incidence
    /// matrix's left kernel; no floating point or external solver is trusted.
    PlaceInvariant,
    /// An SMT solver proved the target is not coverable.
    ///
    /// Reserved and not currently produced. The SMT candidate search
    /// (`find_candidate_covering_parikh_vector`) returns `None` for three
    /// distinct reasons — the solver proved the constraints unsatisfiable, the
    /// solver returned neither Sat nor Unsat, or a Sat model whose values did
    /// not extract into `u32` — and the current API cannot distinguish a genuine
    /// UNSAT from the other two. A bare `None` is therefore not a sound
    /// uncoverability verdict; the negative verdict rests on [`Self::PlaceInvariant`]
    /// (exact, over ℚ) or on the exhausted coverability graph instead. This
    /// variant is retained for a future path that surfaces a genuine, checkable
    /// UNSAT core from the solver.
    Smt(), // todo: add unsat core, and only then produce this from a genuine UNSAT
    /// Full coverability graph explored; target not covered.
    ExhaustiveSearch,
}

/// If we have proof of coverability, the target is coverable.
impl From<CoverabilityProof> for CoverabilityResult {
    fn from(value: CoverabilityProof) -> Self {
        CoverabilityResult::Coverable(value)
    }
}

/// If we have proof of non-coverability, the target is not coverable.
impl From<NonCoverabilityProof> for CoverabilityResult {
    fn from(value: NonCoverabilityProof) -> Self {
        CoverabilityResult::Uncoverable(value)
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns whether `target` is coverable from the initial marking.
    pub fn is_coverable(&self, target: impl Into<Marking<u32>>) -> bool {
        self.analyze_coverability(target.into()).is_coverable()
    }

    /// Analyzes coverability of a target marking with structured evidence.
    ///
    /// A marking `target` is **coverable** if there exists a reachable marking `M`
    /// such that `M(p) >= target(p)` for every place `p`.
    ///
    /// Strategy (ascending cost):
    /// 1. Trivial: if `M₀ >= target`, return immediately.
    /// 2. LP covering equation (necessary): if infeasible, `target` is uncoverable.
    /// 3. ILP covering equation (stronger necessary): if infeasible, uncoverable.
    /// 4. Coverability graph (Karp–Miller): always terminates; exact.
    ///
    /// References:
    /// - [Murata 1989, §V-A](crate::literature#v-a--the-coverability-tree) (coverability tree properties)
    /// - [Primer, Proposition 3.23](crate::literature#proposition-323--finiteness-of-the-coverability-trees-and-graphs) (termination)
    /// - [Primer, Proposition 3.27](crate::literature#proposition-327--all-that-can-be-checked-on-a-coverability-graph) (coverability via Cov(N))
    /// - [Primer, Proposition 4.3](crate::literature#proposition-43--state-equation) (necessary condition underpinning LP/ILP filters)
    /// - [Esparza Lecture Notes, Theorem 3.2.5](crate::literature#theorem-325--coverability-graph-terminates) (termination, supplementary)
    /// - [Esparza Lecture Notes, Theorem 3.2.8](crate::literature#theorem-328--coverability-characterization) (correctness, supplementary)
    #[must_use]
    pub fn analyze_coverability(&self, target: Marking<u32>) -> CoverabilityResult {
        let target_idx_marking = self.mapping.decode(target.clone());

        if self.marking >= target_idx_marking {
            return CoverabilityProof {
                firing_sequence: Vec::new(),
                covering_marking: self.mapping.encode(IdxOmegaMarking::from(self.marking.clone())),
            }.into();
        }

        // The SMT candidate search stays as the first pass: it prunes candidate
        // covering markings against violated S-invariants and initially-marked
        // traps, and a surviving candidate can later seed the exploration.
        // todo: use potentially coverable marking as hint for state space exploration
        if find_candidate_covering_parikh_vector(
            &self.net.as_ref().dense_net,
            &self.marking,
            &target_idx_marking,
        ).is_none() {
            // `None` from the candidate search is evidence of uncoverability, but
            // not yet a verdict. It conflates three cases: the solver proved the
            // constraints unsatisfiable; the solver returned neither Sat nor
            // Unsat; and a Sat model whose values did not extract into `u32`.
            // Only the first is an uncoverability argument, and even it rests on
            // an external solver. The verdict therefore comes from an exact
            // rational certificate computed against this net's own incidence
            // matrix: a non-negative place weighting `y` with `yᵀ·C ≤ 0` (the
            // weighted token count can never increase across a firing) and
            // `y·target > y·M₀` (the target demands more weight than any
            // reachable marking can hold). When exact arithmetic cannot certify
            // — an integer-only infeasibility, or i128 overflow — fall through
            // to the coverability graph, which decides exactly.
            match exact_matrix::covering_invariant_exact(
                &self.net.as_ref().dense_net,
                &self.marking,
                &target_idx_marking,
            ) {
                CoveringInvariantExact::Uncoverable { .. } => {
                    return NonCoverabilityProof::PlaceInvariant.into();
                }
                CoveringInvariantExact::NotCertified | CoveringInvariantExact::Overflowed => {}
            }
        }

        // todo: backwards coverability
        let mut explorer = self.explore_coverability(ExplorationOrder::BreadthFirst);
        explorer
            .find_cover(OmegaMarking::from(target))
            .map_or_else(
                || NonCoverabilityProof::ExhaustiveSearch.into(),
                |cover| {
                    let firing_sequence = explorer.find_path_from_initial(cover.clone()).unwrap();
                    CoverabilityProof {
                        firing_sequence,
                        covering_marking: cover,
                    }.into()
                }
            )
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::prelude::PetriNet;
    use crate::state_space::Omega;
    use crate::system::coverability::{CoverabilityProof, CoverabilityResult, NonCoverabilityProof};

    #[test]
    fn coverability_initial_marking_covers() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        let res = sys.analyze_coverability([(p0, 1)].into());
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable(CoverabilityProof { firing_sequence, covering_marking }) => {
                assert_eq!(covering_marking, [(p0, 1.into())].into());
                assert_eq!(firing_sequence.len(), 0);
            }
            _ => panic!("expected InitialMarking proof"),
        }
    }

    #[test]
    fn coverability_uncoverable_certified_by_place_invariant() {
        // Two-place cycle with one token: p0 + p1 = 1 in every reachable marking
        // (the weighting y = (1, 1) never gains weight across a firing), while
        // covering (1, 1) would need weight 2. The verdict must carry that exact
        // invariant as its evidence, not depend on a solver failing.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1)]);

        let res = sys.analyze_coverability([(p0, 1), (p1, 1)].into());
        assert!(res.is_uncoverable());
        assert!(matches!(
            res,
            CoverabilityResult::Uncoverable(NonCoverabilityProof::PlaceInvariant)
        ));
    }

    #[test]
    fn coverability_unbounded_omega_witness() {
        // Unbounded producer: t0 consumes p0 and produces p0 and p1.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        let res = sys.analyze_coverability([(p0, 1), (p1, 10)].into());
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable(CoverabilityProof { covering_marking, .. }) => {
                // p0 stays 1; p1 becomes ω in the coverability graph.
                assert_eq!(covering_marking.get(p0), Omega::Finite(1));
                assert!(covering_marking.get(p1) >= Omega::Finite(100_000));
            }
            _ => panic!("expected coverability-graph proof"),
        }
    }
}