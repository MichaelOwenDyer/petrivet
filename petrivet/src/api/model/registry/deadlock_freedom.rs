//! The deadlock-freedom decider list (M3).
//!
//! Reproduces [`is_deadlock_free`](crate::system::PetriNet::is_deadlock_free) as a
//! [`Driver`] over three deciders:
//! 1. [`CHCDeadlockFree`] — the Commoner–Hack marked-trap cover (sufficient for
//!    deadlock-freedom on **any** class), routed through the real
//!    [`SiphonTrapCoverCert`] with the [`DeadlockFree`](SiphonTrapClaim::DeadlockFree)
//!    claim (the certifying ledger row, no class fence).
//! 2. [`LivenessImpliesDeadlockFree`] — `is_efficiently_live() == Some(true)`
//!    (liveness implies deadlock-freedom; trusted-base, [`BareBooleanCert`]).
//! 3. [`ExhaustiveDeadlockFreedom`] — delegates to the exhaustive
//!    [`deadlocks`](crate::system::PetriNet::deadlocks) search; the only path that
//!    can *refute* (it exhibits a reachable deadlock marking as the witness).
//!
//! The structural path (deciders 1–2) **never** refutes: a non-live net is not
//! thereby deadlock-*ful* (the A2-family invariant — no fabricated negative). Only
//! the exhaustive search produces a `Refuted`, carrying the deadlock marking.

use super::{BareBooleanCert, Budget, CostClass, Decide, DefaultPolicy, Driver};
use crate::class::NetClass;
use crate::marking::Marking;
use crate::model::accept::{accept, Decided};
use crate::model::checkers::{SiphonTrapClaim, SiphonTrapCoverCert};
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::model::verdict::{Abstention, Verdict};
use crate::net::Net;
use crate::system::PetriNet;

/// The witness that the system is deadlock-free. The structural deciders carry no
/// compact certificate beyond the (re-derived) siphon/trap cover; the marker
/// records the positive verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeadlockFreeWitness;

/// The refutation: a reachable deadlock marking (enables no transition).
type DlRefutation = Marking<u32>;

/// Commoner–Hack deadlock-freedom (sufficient on any class), certified by the real
/// [`SiphonTrapCoverCert`] with the [`DeadlockFree`](SiphonTrapClaim::DeadlockFree)
/// claim. An empty cover (no proper siphon) falls through to the exhaustive search.
struct CHCDeadlockFree;

impl Decide<DeadlockFreeWitness, DlRefutation> for CHCDeadlockFree {
    fn name(&self) -> &'static str {
        "deadlock_freedom::commoner_hack"
    }
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }
    fn cost_class(&self) -> CostClass {
        CostClass::Polynomial
    }
    fn admissible(&self, _class: NetClass) -> bool {
        true
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        _query: &Query,
        _budget: Budget,
    ) -> Verdict<DeadlockFreeWitness, DlRefutation> {
        let sys = PetriNet::new(net, m0.clone());
        let Ok(pairs) = sys.commoner_hack_criterion() else {
            return Verdict::inconclusive(Abstention::NoEfficientProcedure);
        };
        let cover = pairs.iter().map(|p| (p.siphon.clone(), p.trap.clone())).collect();
        let cert = SiphonTrapCoverCert { claim: SiphonTrapClaim::DeadlockFree, cover };
        accept(Decided::prove(DeadlockFreeWitness), &cert, net, m0, &Query::Trivial)
    }
}

/// Liveness implies deadlock-freedom: if the efficient liveness path certifies the
/// net live, it is deadlock-free. Trusted-base ([`BareBooleanCert`]); never
/// consumes the negative pole of liveness (no fabricated deadlock).
struct LivenessImpliesDeadlockFree;

impl Decide<DeadlockFreeWitness, DlRefutation> for LivenessImpliesDeadlockFree {
    fn name(&self) -> &'static str {
        "deadlock_freedom::liveness_implies"
    }
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }
    fn cost_class(&self) -> CostClass {
        CostClass::NearLinear
    }
    fn admissible(&self, _class: NetClass) -> bool {
        true
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        _query: &Query,
        _budget: Budget,
    ) -> Verdict<DeadlockFreeWitness, DlRefutation> {
        let sys = PetriNet::new(net, m0.clone());
        if sys.is_efficiently_live() == Some(true) {
            accept(
                Decided::prove(DeadlockFreeWitness),
                &BareBooleanCert::prove_yes(),
                net,
                m0,
                &Query::Trivial,
            )
        } else {
            Verdict::inconclusive(Abstention::NoEfficientProcedure)
        }
    }
}

/// The exhaustive decider: delegates to the [`deadlocks`](crate::system::PetriNet::deadlocks)
/// search. No reachable deadlock ⇒ `Proven`; the first reachable deadlock is the
/// `Refuted` witness. This is the only deadlock-freedom path that can refute.
struct ExhaustiveDeadlockFreedom;

impl Decide<DeadlockFreeWitness, DlRefutation> for ExhaustiveDeadlockFreedom {
    fn name(&self) -> &'static str {
        "deadlock_freedom::exhaustive"
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
        _query: &Query,
        _budget: Budget,
    ) -> Verdict<DeadlockFreeWitness, DlRefutation> {
        let sys = PetriNet::new(net, m0.clone());
        sys.deadlocks().next().map_or_else(
            || {
                accept(
                    Decided::prove(DeadlockFreeWitness),
                    &BareBooleanCert::prove_yes(),
                    net,
                    m0,
                    &Query::Trivial,
                )
            },
            |deadlock| {
                accept(
                    Decided::refute(deadlock),
                    &BareBooleanCert::prove_no(),
                    net,
                    m0,
                    &Query::Trivial,
                )
            },
        )
    }
}

/// The default deadlock-freedom driver: the certifying CHC cover, the
/// liveness-implies shortcut, then the exhaustive (refuting) search.
#[must_use]
pub fn deadlock_freedom_driver() -> Driver<DeadlockFreeWitness, DlRefutation, DefaultPolicy> {
    Driver::new(
        vec![
            Box::new(CHCDeadlockFree),
            Box::new(LivenessImpliesDeadlockFree),
            Box::new(ExhaustiveDeadlockFreedom),
        ],
        DefaultPolicy,
    )
}

#[cfg(test)]
mod tests {
    use super::deadlock_freedom_driver;
    use crate::builder::NetBuilder;
    use crate::marking::Marking;
    use crate::model::query::Query;
    use crate::model::registry::Budget;
    use crate::prelude::PetriNet;

    fn driver_pole(net: &crate::net::Net, m0: &Marking<u32>) -> &'static str {
        let v = deadlock_freedom_driver().decide(net, m0, &Query::Trivial, Budget::unbounded());
        if v.is_proven() {
            "proven"
        } else if v.is_refuted() {
            "refuted"
        } else {
            "inconclusive"
        }
    }

    /// `[REGRESS]` (unit) — the driver pole equals `is_deadlock_free` on a live
    /// (deadlock-free) cycle and a net that can reach a total deadlock.
    #[test]
    fn driver_reproduces_cascade_pole() {
        // Live cycle: deadlock-free.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let cycle = b.build().expect("valid cycle");
        let live: Marking<u32> = [(p0, 1)].into();
        let csys = PetriNet::new(&cycle, live.clone());
        assert!(csys.is_deadlock_free());
        assert_eq!(driver_pole(&cycle, &live), "proven");

        // A net that reaches a deadlock: a free choice at p0 where the t1 branch
        // drains into a sink p2 (no outgoing transition), so the system can stop.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p0, t1));
        b.add_arc((t0, p1));
        b.add_arc((p1, t0));
        b.add_arc((t1, p2));
        let dnet = b.build().expect("valid net");
        let dm0: Marking<u32> = [(p0, 1)].into();
        let dsys = PetriNet::new(&dnet, dm0.clone());
        let expect = if dsys.is_deadlock_free() { "proven" } else { "refuted" };
        assert_eq!(driver_pole(&dnet, &dm0), expect, "driver vs is_deadlock_free");
        assert_eq!(expect, "refuted", "this net can reach a total deadlock");
    }

    /// `[PROP]` — the accepted pole is invariant under every ordering of the three
    /// admissible deciders, on the deadlock-free cycle and the deadlocking net.
    #[test]
    fn accepted_pole_invariant_over_orderings() {
        let driver = deadlock_freedom_driver();
        let deciders = driver.deciders();

        // (net, m0) pairs: a deadlock-free cycle and a deadlocking net.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let cycle = b.build().expect("valid cycle");

        let mut b = NetBuilder::new();
        let [q0, q1, q2] = b.add_places();
        let [u0, u1] = b.add_transitions();
        b.add_arc((q0, u0));
        b.add_arc((q0, u1));
        b.add_arc((u0, q1));
        b.add_arc((q1, u0));
        b.add_arc((u1, q2));
        let dnet = b.build().expect("valid net");

        let cases: [(&crate::net::Net, Marking<u32>); 2] =
            [(&cycle, [(p0, 1)].into()), (&dnet, [(q0, 1)].into())];

        for (net, m0) in cases {
            let class = net.class();
            let mut poles = Vec::new();
            for order in [
                [0usize, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0],
            ] {
                let mut pole = None;
                for &i in &order {
                    if !deciders[i].admissible(class) {
                        continue;
                    }
                    let v = deciders[i].run(net, &m0, &Query::Trivial, Budget::unbounded());
                    if !v.is_inconclusive() {
                        pole = Some(v.is_proven());
                        break;
                    }
                }
                poles.push(pole);
            }
            assert!(poles.windows(2).all(|w| w[0] == w[1]), "pole invariant under order");
        }
    }
}
