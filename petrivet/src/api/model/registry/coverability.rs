//! The coverability decider list (M3).
//!
//! Reproduces [`analyze_coverability`](crate::system::PetriNet::analyze_coverability)
//! as a [`Driver`] over two deciders:
//! 1. [`TrivialCover`] — `m₀ ≥ target` (cert-gated by the empty firing word over a
//!    `Coverable` query).
//! 2. [`ExhaustiveCoverability`] — delegates to the full cascade (the exact covering
//!    sub-invariant refuter and the Karp–Miller coverability graph).
//!
//! Coverability is *total* (Karp–Miller always terminates), so the driver never
//! abstains. The exhaustive positive routes through [`BareBooleanCert`] rather than
//! [`FiringSequenceCert`] because an ω-coverability witness's finite firing word
//! does not by itself cover the target (the pumping cycle the checker would need is
//! C7 future work); the trivial cover, whose witness is the initial marking itself,
//! is genuinely cert-gated.

use super::{BareBooleanCert, Budget, CostClass, Decide, DefaultPolicy, Driver};
use crate::class::NetClass;
use crate::marking::Marking;
use crate::model::accept::{accept, Decided};
use crate::model::checkers::FiringSequenceCert;
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::model::verdict::{Abstention, Verdict};
use crate::net::Net;
use crate::state_space::OmegaMarking;
use crate::system::PetriNet;

use crate::api::system::coverability::{CoverabilityProof, CoverabilityResult, NonCoverabilityProof};

type CProof = CoverabilityProof;
type CRefutation = NonCoverabilityProof;

/// Arm 1 — `m₀` already covers the target. Cert-gated: it proposes the empty
/// firing word, and [`accept`] promotes it only when [`FiringSequenceCert`] (over
/// the `Coverable` query) confirms `m₀ ≥ target`.
struct TrivialCover;

impl Decide<CProof, CRefutation> for TrivialCover {
    fn name(&self) -> &'static str {
        "coverability::trivial"
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
    ) -> Verdict<CProof, CRefutation> {
        let proof = CoverabilityProof {
            firing_sequence: Box::new([]),
            covering_marking: OmegaMarking::from(m0.clone()),
        };
        let cert = FiringSequenceCert { sequence: Box::new([]) };
        accept(Decided::prove(proof), &cert, net, m0, query)
    }
}

/// Arm 2+ — the exhaustive decider: delegates to the full
/// [`analyze_coverability`](crate::system::PetriNet::analyze_coverability) cascade
/// (the exact covering sub-invariant refuter, then the Karp–Miller graph). Because
/// it *is* the cascade, the driver reproduces it exactly.
struct ExhaustiveCoverability;

impl Decide<CProof, CRefutation> for ExhaustiveCoverability {
    fn name(&self) -> &'static str {
        "coverability::exhaustive"
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
    ) -> Verdict<CProof, CRefutation> {
        let Query::Coverable(target) = query else {
            return Verdict::inconclusive(Abstention::NotApplicable);
        };
        let sys = PetriNet::new(net, m0.clone());
        match sys.analyze_coverability(target.clone()) {
            CoverabilityResult::Coverable(proof) => {
                let cert = BareBooleanCert::prove_yes();
                accept(Decided::prove(proof), &cert, net, m0, query)
            }
            CoverabilityResult::Uncoverable(proof) => {
                let cert = BareBooleanCert::prove_no();
                accept(Decided::refute(proof), &cert, net, m0, query)
            }
        }
    }
}

/// The default coverability driver: trivial cover, then the exhaustive cascade.
#[must_use]
pub fn coverability_driver() -> Driver<CProof, CRefutation, DefaultPolicy> {
    Driver::new(
        vec![Box::new(TrivialCover), Box::new(ExhaustiveCoverability)],
        DefaultPolicy,
    )
}

#[cfg(test)]
mod tests {
    use super::coverability_driver;
    use crate::builder::NetBuilder;
    use crate::marking::Marking;
    use crate::model::query::Query;
    use crate::model::registry::Budget;
    use crate::prelude::PetriNet;

    /// `[REGRESS]` (unit) — the driver's pole equals `analyze_coverability` on a
    /// covered target, an uncoverable target, and an unbounded ω-coverable target.
    #[test]
    fn driver_reproduces_cascade_pole() {
        let driver = coverability_driver();

        // Two-place cycle, one token: covers {p0:1}, cannot cover {p0:1, p1:1}.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid cycle");
        let m0: Marking<u32> = [(p0, 1)].into();
        let sys = PetriNet::new(&net, m0.clone());

        for target in [
            Into::<Marking<u32>>::into([(p0, 1)]),
            [(p0, 1), (p1, 1)].into(),
        ] {
            let v = driver.decide(&net, &m0, &Query::Coverable(target.clone()), Budget::unbounded());
            assert_eq!(
                v.is_proven(),
                sys.is_coverable(target.clone()),
                "driver vs cascade coverable for {target:?}",
            );
            assert!(!v.is_inconclusive(), "coverability is total");
        }

        // Unbounded producer: {p1:5} is coverable by pumping.
        let mut b = NetBuilder::new();
        let [q0, q1] = b.add_places();
        let [u0] = b.add_transitions();
        b.add_arc((q0, u0));
        b.add_arc((u0, q0));
        b.add_arc((u0, q1));
        let pump = b.build().expect("valid pump");
        let pm0: Marking<u32> = [(q0, 1)].into();
        let psys = PetriNet::new(&pump, pm0.clone());
        let target: Marking<u32> = [(q1, 5)].into();
        let v = driver.decide(&pump, &pm0, &Query::Coverable(target.clone()), Budget::unbounded());
        assert!(v.is_proven(), "pumped target is coverable");
        assert_eq!(v.is_proven(), psys.is_coverable(target));
    }

    /// `[PROP]` — the accepted pole is invariant under the two admissible orders.
    #[test]
    fn accepted_pole_invariant_over_orderings() {
        let driver = coverability_driver();
        let deciders = driver.deciders();

        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid cycle");
        let m0: Marking<u32> = [(p0, 1)].into();

        for target in [
            Into::<Marking<u32>>::into([(p0, 1)]),
            [(p0, 1), (p1, 1)].into(),
        ] {
            let query = Query::Coverable(target);
            let class = net.class();
            // Both orders of the two admissible deciders.
            let mut poles = Vec::new();
            for order in [[0usize, 1], [1, 0]] {
                let mut verdict_proven = None;
                for &i in &order {
                    if !deciders[i].admissible(class) {
                        continue;
                    }
                    let v = deciders[i].run(&net, &m0, &query, Budget::unbounded());
                    if !v.is_inconclusive() {
                        verdict_proven = Some(v.is_proven());
                        break;
                    }
                }
                poles.push(verdict_proven);
            }
            assert_eq!(poles[0], poles[1], "pole invariant under decider order");
        }
    }
}
