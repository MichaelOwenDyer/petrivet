//! The reachability decider list (M3) — the reference property.
//!
//! Reproduces [`analyze_reachability`](crate::system::PetriNet::analyze_reachability)
//! as a [`Driver`] over four deciders, in cascade order:
//! 1. [`IdentityReachable`] — `m₀ == target` (cert-gated by the empty firing word).
//! 2. [`StateMachineConservationRefuter`] — S-net token-sum mismatch (cert-gated).
//! 3. [`LiveStateMachineReachable`] — strongly-connected S-net, equal sums (trusted).
//! 4. [`ExhaustiveReachability`] — delegates to the full cascade for the remaining
//!    arms (live-marked-graph realization, the exact marking-equation refuter, and
//!    the state-space search), so exact reproduction is *by construction*.
//!
//! The cheap deciders (1–2) route through their C1 certificate, which re-derives
//! the property — so a wrong proposal fails `check` and collapses to `Inconclusive`,
//! and decider 4 (the cascade) then decides correctly. Decider 3 rests on a sound
//! theorem (5.1.5) and carries a [`BareBooleanCert`] (the ledger's trusted-base
//! row). The accepted *pole* is therefore order-invariant (the `[PROP]` gate).

use super::{BareBooleanCert, Budget, CostClass, Decide, DefaultPolicy, Driver};
use crate::class::NetClass;
use crate::marking::Marking;
use crate::model::accept::{accept, Decided};
use crate::model::checkers::{FiringSequenceCert, TokenConservationCert};
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::model::verdict::{Abstention, Verdict};
use crate::net::Net;
use crate::system::PetriNet;

use crate::api::system::reachability::{ReachabilityProof, ReachabilityResult, UnreachabilityProof};

/// The reachability proof / refutation payload types.
type RProof = ReachabilityProof;
type RRefutation = UnreachabilityProof;

/// Arm 1 — the empty firing word reaches the target iff `m₀ == target`. Fully
/// cert-gated: it always proposes the empty word and [`accept`] promotes it to
/// `Proven` only when the [`FiringSequenceCert`] replays to the target.
struct IdentityReachable;

impl Decide<RProof, RRefutation> for IdentityReachable {
    fn name(&self) -> &'static str {
        "reachability::identity"
    }
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }
    fn cost_class(&self) -> CostClass {
        CostClass::Constant
    }
    fn admissible(&self, _class: NetClass) -> bool {
        true
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        query: &Query,
        _budget: Budget,
    ) -> Verdict<RProof, RRefutation> {
        let cert = FiringSequenceCert { sequence: Box::new([]) };
        accept(
            Decided::prove(ReachabilityProof::FiringSequence(Box::new([]))),
            &cert,
            net,
            m0,
            query,
        )
    }
}

/// Arm 2 — an S-net conserves tokens, so a target with a different token sum is
/// unreachable. Cert-gated: [`TokenConservationCert`] re-derives the S-net
/// precondition and the sum mismatch from primitives; an equal-sum target fails
/// `check` and falls through.
struct StateMachineConservationRefuter;

impl Decide<RProof, RRefutation> for StateMachineConservationRefuter {
    fn name(&self) -> &'static str {
        "reachability::state_machine_conservation"
    }
    fn polarity(&self) -> Polarity {
        Polarity::ProveNo
    }
    fn cost_class(&self) -> CostClass {
        CostClass::Constant
    }
    fn admissible(&self, class: NetClass) -> bool {
        class.is_state_machine()
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        query: &Query,
        _budget: Budget,
    ) -> Verdict<RProof, RRefutation> {
        let cert = TokenConservationCert;
        accept(
            Decided::refute(UnreachabilityProof::StateMachineTokenConservation),
            &cert,
            net,
            m0,
            query,
        )
    }
}

/// Arm 3 — in a strongly-connected S-net any equal-token-sum marking is reachable
/// (Theorem 5.1.5). A trusted-base structural decider: it checks the exact
/// precondition itself and carries a [`BareBooleanCert`] (no compact replayable
/// witness is emitted — the ledger's bare-boolean reachability row).
struct LiveStateMachineReachable;

impl Decide<RProof, RRefutation> for LiveStateMachineReachable {
    fn name(&self) -> &'static str {
        "reachability::live_state_machine"
    }
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }
    fn cost_class(&self) -> CostClass {
        CostClass::Constant
    }
    fn admissible(&self, class: NetClass) -> bool {
        class.is_state_machine()
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        query: &Query,
        _budget: Budget,
    ) -> Verdict<RProof, RRefutation> {
        let Query::Reachable(target) = query else {
            return Verdict::inconclusive(Abstention::NotApplicable);
        };
        let sys = PetriNet::new(net, m0.clone());
        let sums_equal = m0.total_tokens_wide() == target.total_tokens_wide();
        if net.class().is_state_machine() && sys.is_strongly_connected() && sums_equal {
            let marking_sum = u32::try_from(m0.total_tokens_wide()).unwrap_or(u32::MAX);
            let cert = BareBooleanCert::prove_yes();
            accept(
                Decided::prove(ReachabilityProof::LiveStateMachineTokenConservation { marking_sum }),
                &cert,
                net,
                m0,
                query,
            )
        } else {
            Verdict::inconclusive(Abstention::NoEfficientProcedure)
        }
    }
}

/// Arm 4+ — the exhaustive decider: delegates to the full
/// [`analyze_reachability`](crate::system::PetriNet::analyze_reachability) cascade,
/// covering the live-marked-graph realization, the exact marking-equation refuter,
/// and the state-space search. Because it *is* the cascade, the driver reproduces
/// it exactly for every case the cheap deciders abstain on. Positive firing-word
/// proofs route through [`FiringSequenceCert`] (genuinely certifying); the other
/// structural proofs and the negative search through [`BareBooleanCert`].
struct ExhaustiveReachability;

impl Decide<RProof, RRefutation> for ExhaustiveReachability {
    fn name(&self) -> &'static str {
        "reachability::exhaustive"
    }
    fn polarity(&self) -> Polarity {
        Polarity::Exact
    }
    fn cost_class(&self) -> CostClass {
        CostClass::StateSpace
    }
    fn admissible(&self, _class: NetClass) -> bool {
        true
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        query: &Query,
        _budget: Budget,
    ) -> Verdict<RProof, RRefutation> {
        let Query::Reachable(target) = query else {
            return Verdict::inconclusive(Abstention::NotApplicable);
        };
        let sys = PetriNet::new(net, m0.clone());
        match sys.analyze_reachability(target) {
            ReachabilityResult::Reachable(ReachabilityProof::FiringSequence(word)) => {
                let cert = FiringSequenceCert { sequence: word.clone() };
                accept(
                    Decided::prove(ReachabilityProof::FiringSequence(word)),
                    &cert,
                    net,
                    m0,
                    query,
                )
            }
            ReachabilityResult::Reachable(other) => {
                // A structural positive proof without a replayable witness — the
                // trusted-base routing (in practice the cheap deciders catch these).
                let cert = BareBooleanCert::prove_yes();
                accept(Decided::prove(other), &cert, net, m0, query)
            }
            ReachabilityResult::Unreachable(proof) => {
                let cert = BareBooleanCert::prove_no();
                accept(Decided::refute(proof), &cert, net, m0, query)
            }
            ReachabilityResult::Inconclusive => Verdict::inconclusive(Abstention::Unbounded),
        }
    }
}

/// The default reachability driver: the four deciders in cascade order.
///
/// Under the [`DefaultPolicy`], its [`decide`](Driver::decide) reproduces
/// [`analyze_reachability`](crate::system::PetriNet::analyze_reachability) exactly
/// (the S2 contract; proven by the `[REGRESS]` gate in `mcc-tests`).
#[must_use]
pub fn reachability_driver() -> Driver<RProof, RRefutation, DefaultPolicy> {
    Driver::new(
        vec![
            Box::new(IdentityReachable),
            Box::new(StateMachineConservationRefuter),
            Box::new(LiveStateMachineReachable),
            Box::new(ExhaustiveReachability),
        ],
        DefaultPolicy,
    )
}

#[cfg(test)]
mod tests {
    use super::{reachability_driver, Budget, Decide};
    use crate::builder::NetBuilder;
    use crate::marking::Marking;
    use crate::model::query::Query;
    use crate::model::verdict::{Abstention, Verdict};
    use crate::net::Net;
    use crate::prelude::Place;

    /// Run a permuted admissible-decider order with "first decided wins" — the
    /// invariant the `[PROP]` order-independence gate quantifies over.
    fn decide_in_order(
        deciders: &[Box<dyn Decide<super::RProof, super::RRefutation>>],
        order: &[usize],
        net: &Net,
        m0: &Marking<u32>,
        query: &Query,
    ) -> Verdict<super::RProof, super::RRefutation> {
        let class = net.class();
        for &i in order {
            if !deciders[i].admissible(class) {
                continue;
            }
            let v = deciders[i].run(net, m0, query, Budget::unbounded());
            if !v.is_inconclusive() {
                return v;
            }
        }
        Verdict::inconclusive(Abstention::NoEfficientProcedure)
    }

    /// All permutations of `0..n` (Heap's algorithm, no external crate).
    fn permutations(n: usize) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        let mut a: Vec<usize> = (0..n).collect();
        let mut c = vec![0usize; n];
        out.push(a.clone());
        let mut i = 0;
        while i < n {
            if c[i] < i {
                if i % 2 == 0 {
                    a.swap(0, i);
                } else {
                    a.swap(c[i], i);
                }
                out.push(a.clone());
                c[i] += 1;
                i = 0;
            } else {
                c[i] = 0;
                i += 1;
            }
        }
        out
    }

    fn pole(v: &Verdict<super::RProof, super::RRefutation>) -> &'static str {
        if v.is_proven() {
            "proven"
        } else if v.is_refuted() {
            "refuted"
        } else {
            "inconclusive"
        }
    }

    /// `p0 → t0 → p1 → t1 → p0`, a circuit (state machine). Returns the net and
    /// its two places.
    fn two_place_cycle() -> (Net, Place, Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        (b.build().expect("valid cycle"), p0, p1)
    }

    /// A general (non-state-machine) net for the non-structural fallback.
    fn general_net() -> (Net, Place, Place, Place) {
        let mut b = NetBuilder::new();
        let [pa, pb, pc, pd, pe] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((pa, t0));
        b.add_arc((t0, pb));
        b.add_arc((pa, t1));
        b.add_arc((pc, t1));
        b.add_arc((t1, pd));
        b.add_arc((pc, t2));
        b.add_arc((t2, pe));
        (b.build().expect("valid general net"), pa, pb, pc)
    }

    /// `[REGRESS]` (unit) — the default driver reproduces `analyze_reachability`'s
    /// verdict pole on representative built nets (the corpus-level gate is in
    /// `mcc-tests/tests/cascade_baseline.rs`).
    #[test]
    fn driver_reproduces_cascade_pole() {
        let driver = reachability_driver();

        // Circuit: reachable (redistribute the token), identity, and unreachable
        // (different token sum) targets.
        let (net, p0, p1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        let m0: Marking<u32> = [(p0, 1)].into();
        for (target, expect_reachable) in [
            ([(p1, 1)].into(), true),
            ([(p0, 1)].into(), true),
            ([(p0, 2)].into(), false),
        ] {
            let target: Marking<u32> = target;
            let v = driver.decide(&net, &m0, &Query::Reachable(target.clone()), Budget::unbounded());
            assert_eq!(
                v.is_proven(),
                sys.is_reachable(&target),
                "driver vs cascade reachable for {target:?}",
            );
            assert_eq!(v.is_proven(), expect_reachable, "expected reachability for {target:?}");
            assert!(!v.is_inconclusive(), "bounded circuit always decides");
        }

        // General net: structural fallback to the state space.
        let (gnet, pa, pb, pc) = general_net();
        let gsys = gnet.with_initial_marking([(pa, 1), (pc, 1)]);
        let gm0: Marking<u32> = [(pa, 1), (pc, 1)].into();
        let reachable: Marking<u32> = [(pb, 1), (pc, 1)].into();
        let v = driver.decide(&gnet, &gm0, &Query::Reachable(reachable.clone()), Budget::unbounded());
        assert!(v.is_proven(), "general reachable target decided by the state space");
        assert_eq!(v.is_proven(), gsys.is_reachable(&reachable));
    }

    /// `[PROP]` — over every permutation of the admissible deciders, the accepted
    /// verdict pole is invariant (the policy-independence soundness theorem).
    #[test]
    fn accepted_pole_invariant_over_admissible_orderings() {
        let driver = reachability_driver();
        let deciders = driver.deciders();
        let n = deciders.len();

        let (net, p0, p1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let queries: [Marking<u32>; 3] =
            [[(p1, 1)].into(), [(p0, 2)].into(), [(p0, 1)].into()];

        for target in queries {
            let query = Query::Reachable(target.clone());
            let baseline = decide_in_order(deciders, &(0..n).collect::<Vec<_>>(), &net, &m0, &query);
            for perm in permutations(n) {
                let v = decide_in_order(deciders, &perm, &net, &m0, &query);
                assert_eq!(
                    pole(&v),
                    pole(&baseline),
                    "verdict pole must be invariant under decider order {perm:?} for {target:?}",
                );
            }
        }
    }
}
