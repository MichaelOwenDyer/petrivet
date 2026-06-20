use crate::core::analysis::exact_matrix::{self, CoveringInvariantExact};
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
    pub firing_sequence: Box<[Transition]>,
    /// The node marking M″ with M″ ≥ target (may contain ω).
    pub covering_marking: OmegaMarking,
}

/// Various methods to demonstrate that a marking is not coverable
/// in a given system.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NonCoverabilityProof {
    /// The LP marking equation (rational relaxation) is infeasible.
    /// Some S-invariant is violated.
    MarkingEquationNoRationalSolution,
    /// The ILP marking equation (integer) is infeasible.
    /// Stronger than LP: no integer firing count vector exists.
    MarkingEquationNoIntegerSolution,
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
    /// Whether `target` is coverable from the initial marking.
    ///
    /// Delegates to [`analyze_coverability`](Self::analyze_coverability).
    pub fn is_coverable(&self, target: Marking<u32>) -> bool {
        self.analyze_coverability(target).is_coverable()
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
        let target_idx_marking = self.mapping.idx_marking(target.clone());

        if self.marking >= target_idx_marking {
            return CoverabilityProof {
                firing_sequence: Box::new([]),
                covering_marking: self.mapping.marking(IdxOmegaMarking::from(self.marking.clone())),
            }.into();
        }

        // M3 — the negative `Uncoverable` verdict must rest on an EXACT
        // certificate, not on the `f64` microlp covering LP merely failing to find
        // a (rational or integer) solution. A spurious floating "infeasible" at a
        // degenerate vertex would be a silent false `Uncoverable` the firewall
        // cannot catch (there is no positive object to check on the negative path)
        // — the same B1a hole the reachability arm closed, now closed for
        // coverability. The exact guard `covering_invariant_exact` re-derives a
        // *sufficient* uncoverability certificate over ℚ — a non-negative place
        // sub-invariant `y` with `yᵀ·C ≤ 0` and `yᵀ·target > yᵀ·M₀`. On anything it
        // cannot certify exactly (NotCertified / Overflowed) we escalate to the
        // exact, always-terminating Karp–Miller coverability graph rather than
        // fabricating a verdict from f64. (The f64 covering LP is no longer on the
        // verdict path at all — it may suggest, never decide.)
        match exact_matrix::covering_invariant_exact(
            &self.dense_net,
            &self.marking,
            &target_idx_marking,
        ) {
            // Exactly confirmed uncoverable by a non-negative place sub-invariant.
            CoveringInvariantExact::Uncoverable { .. } => {
                return NonCoverabilityProof::MarkingEquationNoRationalSolution.into();
            }
            // Not exactly certified — do NOT fabricate `Uncoverable`; decide by the
            // exact coverability graph below.
            CoveringInvariantExact::NotCertified | CoveringInvariantExact::Overflowed => {}
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
    fn coverability_uncoverable_detected_by_lp() {
        // Two-place cycle with one token: cannot cover (1,1).
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
            CoverabilityResult::Uncoverable(NonCoverabilityProof::MarkingEquationNoRationalSolution)
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

    /// M3 — the negative `Uncoverable` verdict now rests on an EXACT place
    /// sub-invariant, not on the `f64` covering LP failing. This pins the
    /// falsifying invariant (the coverability analogue of the reachability B1a
    /// falsifier): a target that IS coverable must NEVER be reported `Uncoverable`,
    /// even at a degenerate vertex where the f64 simplex is prone to a spurious
    /// "infeasible". An unbounded producer can cover any threshold on its pumped
    /// place; the exact guard must not certify uncoverability, and the verdict must
    /// be `Coverable` (decided exactly by the coverability graph).
    #[test]
    fn m3_coverable_target_never_reported_uncoverable() {
        // Unbounded producer pumping p1; p0 conserved at 1.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1)]);
        // {p1:5} is coverable (pump t0 five times). Must not be `Uncoverable`.
        let res = sys.analyze_coverability([(p1, 5)].into());
        assert!(
            !res.is_uncoverable(),
            "M3: a coverable target must never be reported Uncoverable (got {res:?})"
        );
        assert!(res.is_coverable(), "the pumped target is in fact coverable");
    }

    /// ADVERSARIAL (exact lens, post-repair-1) — a near-boundary / degenerate
    /// coverable target must never be reported `Uncoverable` by a spurious f64
    /// "infeasible". This is the coverability twin of the reachability B1a
    /// falsifier built on a deliberately ill-conditioned net: several near-parallel
    /// place-conservation constraints meeting at a degenerate vertex, plus an
    /// unbounded pump so the target is genuinely coverable. The public verdict must
    /// agree with the exhaustive oracle (the Karp–Miller coverability graph), which
    /// is exact and always terminating. Since M3, `is_coverable` decides via the
    /// exact `covering_invariant_exact` guard then the coverability graph — no f64
    /// on the verdict path — so a spurious float infeasibility can never appear.
    #[test]
    fn adversarial_degenerate_coverable_target_not_reported_uncoverable() {
        // p3 is fed by two distinct transitions (degeneracy) and a pump grows pX
        // without bound, so any threshold on pX is coverable.
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3, px] = b.add_places();
        let [t0, t1, t2, t3, pump] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p2)); b.add_arc((t1, p3));
        b.add_arc((p2, t2)); b.add_arc((t2, p3));   // second producer of p3
        b.add_arc((p3, t3)); b.add_arc((t3, p0));
        // pump: p0 -> p0, px  (always re-enabled while p0 holds a token → unbounded px)
        b.add_arc((p0, pump)); b.add_arc((pump, p0)); b.add_arc((pump, px));
        let net = b.build().expect("valid net");
        let sys = PetriNet::new(net, [(p0, 1)]);
        // {px:7} is coverable (pump seven times). It must not be Uncoverable, and
        // the public verdict must match the oracle.
        let res = sys.analyze_coverability([(px, 7)].into());
        assert!(
            !res.is_uncoverable(),
            "exact lens: a coverable target at a degenerate vertex must never be \
             reported Uncoverable (got {res:?})"
        );
        assert!(sys.is_coverable([(px, 7)].into()), "px is pumped without bound");
    }

    /// M3 — the exact guard genuinely certifies uncoverability where a sound
    /// place sub-invariant exists (so capability is preserved, not just soundness).
    /// A two-place conservative cycle (P-semiflow `(1,1)`) cannot cover any target
    /// whose weighted sum exceeds the conserved token count.
    #[test]
    fn m3_exact_guard_certifies_uncoverable_via_subinvariant() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1)]);
        // {p0:1, p1:1} needs weighted sum 2 > 1 conserved: exactly uncoverable.
        let res = sys.analyze_coverability([(p0, 1), (p1, 1)].into());
        assert!(matches!(
            res,
            CoverabilityResult::Uncoverable(NonCoverabilityProof::MarkingEquationNoRationalSolution)
        ));
    }
}