//! The boundedness decider list (M3).
//!
//! Reproduces [`is_bounded`](crate::system::PetriNet::is_bounded) — the boolean
//! boundedness property the ledger tracks — as a [`Driver`] over two deciders:
//! 1. [`ConservativeClassBounded`] — the efficient structural positive
//!    (`is_efficiently_bounded() == Some(true)`: a conservative class or a
//!    strongly-connected marked graph).
//! 2. [`ExhaustiveBoundedness`] — delegates to the full
//!    [`is_bounded`](crate::system::PetriNet::is_bounded) (the exact structural
//!    sub-invariant then the exact coverability graph), which is *total*.
//!
//! Both route through [`BareBooleanCert`]: the ledger classifies the boundedness
//! decider bare-boolean because, although the exact `BoundednessSubinvariantCert`
//! checker exists (M5), the live decider does not yet *emit* the exact witness —
//! routing it through `accept` with the emitted sub-invariant is the A4 completion
//! (M6, on the S-component cover B3). Using `BareBooleanCert` here is the honest
//! match: it does not inflate `f` (the ledger row stays bare-boolean).

use super::{BareBooleanCert, Budget, CostClass, Decide, DefaultPolicy, Driver};
use crate::class::NetClass;
use crate::marking::Marking;
use crate::model::accept::{accept, Decided};
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::model::verdict::{Abstention, Verdict};
use crate::net::Net;
use crate::system::PetriNet;

/// The witness that the net is bounded — the method that established it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BoundednessProof {
    /// A conservative class (circuit / state machine) or a strongly-connected
    /// marked graph — structurally bounded under every marking.
    ConservativeClass,
    /// The exact positive P-sub-invariant or the exact coverability graph.
    Structural,
}

/// The refutation that the net is unbounded (the exact coverability graph found an
/// ω-place). A compact pumping witness is C7 future work; this is the marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnboundedWitness;

/// The efficient structural positive: `is_efficiently_bounded() == Some(true)`.
struct ConservativeClassBounded;

impl Decide<BoundednessProof, UnboundedWitness> for ConservativeClassBounded {
    fn name(&self) -> &'static str {
        "boundedness::conservative_class"
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
    ) -> Verdict<BoundednessProof, UnboundedWitness> {
        let sys = PetriNet::new(net, m0.clone());
        if sys.is_efficiently_bounded() == Some(true) {
            let cert = BareBooleanCert::prove_yes();
            accept(Decided::prove(BoundednessProof::ConservativeClass), &cert, net, m0, query)
        } else {
            Verdict::inconclusive(Abstention::NoEfficientProcedure)
        }
    }
}

/// The exhaustive decider: delegates to the *total*
/// [`is_bounded`](crate::system::PetriNet::is_bounded).
struct ExhaustiveBoundedness;

impl Decide<BoundednessProof, UnboundedWitness> for ExhaustiveBoundedness {
    fn name(&self) -> &'static str {
        "boundedness::exhaustive"
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
    ) -> Verdict<BoundednessProof, UnboundedWitness> {
        let sys = PetriNet::new(net, m0.clone());
        if sys.is_bounded() {
            let cert = BareBooleanCert::prove_yes();
            accept(Decided::prove(BoundednessProof::Structural), &cert, net, m0, query)
        } else {
            let cert = BareBooleanCert::prove_no();
            accept(Decided::refute(UnboundedWitness), &cert, net, m0, query)
        }
    }
}

/// The default boundedness driver: the efficient structural positive, then the
/// exhaustive (total) `is_bounded`.
#[must_use]
pub fn boundedness_driver() -> Driver<BoundednessProof, UnboundedWitness, DefaultPolicy> {
    Driver::new(
        vec![Box::new(ConservativeClassBounded), Box::new(ExhaustiveBoundedness)],
        DefaultPolicy,
    )
}

#[cfg(test)]
mod tests {
    use super::boundedness_driver;
    use crate::builder::NetBuilder;
    use crate::model::query::Query;
    use crate::model::registry::Budget;

    fn decide(net: &crate::net::Net, m0: impl Into<crate::marking::Marking<u32>>) -> bool {
        let driver = boundedness_driver();
        let m0 = m0.into();
        driver
            .decide(net, &m0, &Query::Trivial, Budget::unbounded())
            .is_proven()
    }

    /// `[REGRESS]` (unit) — the driver's pole equals `is_bounded` on a bounded
    /// circuit and an unbounded producer.
    #[test]
    fn driver_reproduces_cascade_pole() {
        // Bounded conservative cycle.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let cycle = b.build().expect("valid cycle");
        let csys = cycle.with_initial_marking([(p0, 1)]);
        assert_eq!(decide(&cycle, [(p0, 1)]), csys.is_bounded());
        assert!(decide(&cycle, [(p0, 1)]), "the cycle is bounded");

        // UnboundedWitness producer.
        let mut b = NetBuilder::new();
        let [q0, q1] = b.add_places();
        let [u0] = b.add_transitions();
        b.add_arc((q0, u0));
        b.add_arc((u0, q0));
        b.add_arc((u0, q1));
        let pump = b.build().expect("valid pump");
        let psys = pump.with_initial_marking([(q0, 1)]);
        assert_eq!(decide(&pump, [(q0, 1)]), psys.is_bounded());
        assert!(!decide(&pump, [(q0, 1)]), "the producer is unbounded");
    }

    /// `[PROP]` — the accepted pole is invariant under the two admissible orders.
    #[test]
    fn accepted_pole_invariant_over_orderings() {
        let driver = boundedness_driver();
        let deciders = driver.deciders();
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid cycle");
        let m0: crate::marking::Marking<u32> = [(p0, 1)].into();
        let class = net.class();
        let mut poles = Vec::new();
        for order in [[0usize, 1], [1, 0]] {
            let mut proven = None;
            for &i in &order {
                if !deciders[i].admissible(class) {
                    continue;
                }
                let v = deciders[i].run(&net, &m0, &Query::Trivial, Budget::unbounded());
                if !v.is_inconclusive() {
                    proven = Some(v.is_proven());
                    break;
                }
            }
            poles.push(proven);
        }
        assert_eq!(poles[0], poles[1], "pole invariant under decider order");
    }
}
