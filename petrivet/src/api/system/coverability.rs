use crate::core::analysis::rackoff;
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

        // todo: use potentially coverable marking as hint for state space exploration
        let Some(_potentially_reachable_cover) = find_candidate_covering_parikh_vector(
            &self.net.as_ref().dense_net,
            &self.marking,
            &target_idx_marking,
        ) else {
            return CoverabilityResult::Uncoverable(NonCoverabilityProof::ExhaustiveSearch);
        };

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

// Rackoff depth bound.
impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns Rackoff's a priori upper bound on the length of a shortest
    /// covering firing sequence for `target`, or `None` if the bound does
    /// not fit in `u128`.
    ///
    /// If `target` is coverable — from *any* initial marking, in particular
    /// the current one — then it is coverable by a firing sequence of length
    /// at most this bound. Searching for a cover deeper than the bound is
    /// therefore provably useless. The bound is doubly exponential in the
    /// number of places, so `None` (overflow) is the common case on all but
    /// the smallest nets; `None` means "not representable in `u128`", never
    /// "no bound exists".
    ///
    /// This method only computes the bound; it does not decide coverability
    /// (use [`analyze_coverability`](Self::analyze_coverability) for that).
    /// In particular, concluding uncoverability from a depth-limited search
    /// is sound only if the search exhaustively enumerates every distinct
    /// marking reachable within the bound.
    ///
    /// References:
    /// - [Esparza Lecture Notes, Theorem 3.2.9](crate::literature#theorem-329--rackoff-coverability-depth-bound) (the bound; original result Rackoff 1978)
    /// - [Esparza Lecture Notes, Lemma 3.2.12](crate::literature#lemma-3212--length-of-shortest-i-covering-sequences) (the recurrence computed here)
    #[must_use]
    pub fn rackoff_coverability_depth_bound(&self, target: impl Into<Marking<u32>>) -> Option<u128> {
        let target_idx_marking = self.mapping.decode(target.into());
        rackoff::coverability_depth_bound(&self.net.as_ref().dense_net, &target_idx_marking)
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

    #[test]
    fn rackoff_bound_two_place_cycle() {
        // k = 2, target (1,1): n = 3; f(1) = 4; f(2) = (3·4)^2 + 4 = 148.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1)]);

        assert_eq!(sys.rackoff_coverability_depth_bound([(p0, 1), (p1, 1)]), Some(148));
    }

    #[test]
    fn rackoff_bound_caps_witness_length() {
        // Falsifiable check of the bound's claim on a tiny bounded net:
        // any covering firing sequence found by exact analysis must fit
        // within the a priori bound.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1)]);

        // k = 2, target (0,1): n = 2; f(2) = (2·3)^2 + 3 = 39.
        let bound = sys.rackoff_coverability_depth_bound([(p1, 1)]).unwrap();
        assert_eq!(bound, 39);

        match sys.analyze_coverability([(p1, 1)].into()) {
            CoverabilityResult::Coverable(CoverabilityProof { firing_sequence, .. }) => {
                // The shortest cover fires t0 once; it must respect the bound.
                assert!(u128::try_from(firing_sequence.len()).unwrap() <= bound);
            }
            CoverabilityResult::Uncoverable(_) => panic!("(0,1) is coverable from (1,0)"),
        }
    }

    #[test]
    fn rackoff_bound_overflows_to_none_on_five_places() {
        // The bound is doubly exponential: five places with a unit target
        // already exceed u128, and the function must abstain with `None`
        // rather than saturate.
        let mut b = NetBuilder::new();
        let ps: Vec<_> = (0..5).map(|_| b.add_place()).collect();
        for window in ps.windows(2) {
            let t = b.add_transition();
            b.add_arc((window[0], t));
            b.add_arc((t, window[1]));
        }
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(ps[0], 1)]);

        assert_eq!(sys.rackoff_coverability_depth_bound([(ps[4], 1)]), None);
    }
}