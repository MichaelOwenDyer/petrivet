//! The liveness decider list (M3).
//!
//! Reproduces the **three-valued** [`try_liveness`](crate::system::PetriNet::try_liveness)
//! (live / non-live / cannot-decide), not the boolean `is_live` collapse, as a
//! [`Driver`] over three deciders:
//! 1. [`FreeChoiceCHCLive`] — free-choice nets via the Commoner–Hack criterion,
//!    routed through the real [`SiphonTrapCoverCert`] (the certifying ledger row).
//! 2. [`ClassLiveness`] — the circuit / state-machine / marked-graph efficient
//!    analyses (trusted-base rows, [`BareBooleanCert`]).
//! 3. [`ExhaustiveLiveness`] — delegates to the full
//!    [`try_liveness`](crate::system::PetriNet::try_liveness), abstaining
//!    (`Inconclusive`) when liveness cannot be decided (an unbounded net).
//!
//! The verdict pole maps `try_liveness`: `Some(live) → Proven`,
//! `Some(non-live) → Refuted`, `None → Inconclusive`. The proof and refutation
//! payloads are both the [`LivenessAnalysis`] (its per-transition levels witness
//! both poles).

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

use crate::api::system::liveness::LivenessAnalysis;

type LProof = LivenessAnalysis;
type LRefutation = LivenessAnalysis;

/// Free-choice liveness via the Commoner–Hack criterion, certified by the real
/// [`SiphonTrapCoverCert`] (claim [`Live`](SiphonTrapClaim::Live), which `accept`
/// gates to free-choice). When CHC holds with a non-empty siphon/trap cover the
/// verdict is genuinely certifying; an empty cover (a net with no proper siphon)
/// falls through to the exhaustive decider.
struct FreeChoiceCHCLive;

impl Decide<LProof, LRefutation> for FreeChoiceCHCLive {
    fn name(&self) -> &'static str {
        "liveness::free_choice_chc"
    }
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }
    fn cost_class(&self) -> CostClass {
        CostClass::Polynomial
    }
    fn admissible(&self, class: NetClass) -> bool {
        matches!(class, NetClass::FreeChoice)
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        _query: &Query,
        _budget: Budget,
    ) -> Verdict<LProof, LRefutation> {
        let sys = PetriNet::new(net, m0.clone());
        let Ok(pairs) = sys.commoner_hack_criterion() else {
            return Verdict::inconclusive(Abstention::NoEfficientProcedure);
        };
        // CHC holds on a free-choice net ⇒ live (Commoner–Hack). Thread the cover
        // into the certifying cert; `accept` re-derives the universal independently.
        let cover = pairs.iter().map(|p| (p.siphon.clone(), p.trap.clone())).collect();
        let cert = SiphonTrapCoverCert { claim: SiphonTrapClaim::Live, cover };
        let Some(analysis) = sys.try_liveness() else {
            return Verdict::inconclusive(Abstention::NoEfficientProcedure);
        };
        accept(Decided::prove(analysis), &cert, net, m0, &Query::Trivial)
    }
}

/// Circuit / state-machine / marked-graph liveness via the efficient per-class
/// analysis (the ledger's bare-boolean rows). Always decides for these classes
/// (`try_liveness` is `Some`): `Proven` if every transition is L4, else `Refuted`.
struct ClassLiveness;

impl Decide<LProof, LRefutation> for ClassLiveness {
    fn name(&self) -> &'static str {
        "liveness::structural_class"
    }
    fn polarity(&self) -> Polarity {
        Polarity::Exact
    }
    fn cost_class(&self) -> CostClass {
        CostClass::NearLinear
    }
    fn admissible(&self, class: NetClass) -> bool {
        matches!(class, NetClass::Circuit | NetClass::StateMachine | NetClass::MarkedGraph)
    }
    fn run(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        _query: &Query,
        _budget: Budget,
    ) -> Verdict<LProof, LRefutation> {
        let sys = PetriNet::new(net, m0.clone());
        let Some(analysis) = sys.try_liveness() else {
            return Verdict::inconclusive(Abstention::NoEfficientProcedure);
        };
        if analysis.is_live() {
            accept(Decided::prove(analysis), &BareBooleanCert::prove_yes(), net, m0, &Query::Trivial)
        } else {
            accept(Decided::refute(analysis), &BareBooleanCert::prove_no(), net, m0, &Query::Trivial)
        }
    }
}

/// The exhaustive decider: delegates to the three-valued
/// [`try_liveness`](crate::system::PetriNet::try_liveness). `None` (liveness
/// undecidable on an unbounded net) maps to `Inconclusive` — never a fabricated
/// non-live verdict (A5).
struct ExhaustiveLiveness;

impl Decide<LProof, LRefutation> for ExhaustiveLiveness {
    fn name(&self) -> &'static str {
        "liveness::exhaustive"
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
    ) -> Verdict<LProof, LRefutation> {
        let sys = PetriNet::new(net, m0.clone());
        match sys.try_liveness() {
            Some(analysis) if analysis.is_live() => {
                accept(Decided::prove(analysis), &BareBooleanCert::prove_yes(), net, m0, &Query::Trivial)
            }
            Some(analysis) => {
                accept(Decided::refute(analysis), &BareBooleanCert::prove_no(), net, m0, &Query::Trivial)
            }
            None => Verdict::inconclusive(Abstention::Unbounded),
        }
    }
}

/// The default liveness driver: free-choice CHC, the structural class analyses,
/// then the exhaustive (three-valued) `try_liveness`.
#[must_use]
pub fn liveness_driver() -> Driver<LProof, LRefutation, DefaultPolicy> {
    Driver::new(
        vec![
            Box::new(FreeChoiceCHCLive),
            Box::new(ClassLiveness),
            Box::new(ExhaustiveLiveness),
        ],
        DefaultPolicy,
    )
}

#[cfg(test)]
mod tests {
    use super::liveness_driver;
    use crate::builder::NetBuilder;
    use crate::marking::Marking;
    use crate::model::query::Query;
    use crate::model::registry::Budget;
    use crate::prelude::PetriNet;

    /// The verdict the three-valued `try_liveness` maps to: `Some(true)` proven,
    /// `Some(false)` refuted, `None` inconclusive.
    fn cascade_pole(sys: &PetriNet<&crate::net::Net>) -> &'static str {
        match sys.try_liveness().map(|a| a.is_live()) {
            Some(true) => "proven",
            Some(false) => "refuted",
            None => "inconclusive",
        }
    }

    fn driver_pole(net: &crate::net::Net, m0: &Marking<u32>) -> &'static str {
        let v = liveness_driver().decide(net, m0, &Query::Trivial, Budget::unbounded());
        if v.is_proven() {
            "proven"
        } else if v.is_refuted() {
            "refuted"
        } else {
            "inconclusive"
        }
    }

    /// `[REGRESS]` (unit) — the driver pole equals `try_liveness`'s on a live cycle,
    /// a dead (unmarked) cycle, a live free-choice net, and an unbounded net.
    #[test]
    fn driver_reproduces_cascade_pole() {
        // Live circuit (marked).
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let cycle = b.build().expect("valid cycle");
        let live: Marking<u32> = [(p0, 1)].into();
        assert_eq!(driver_pole(&cycle, &live), "proven");
        assert_eq!(driver_pole(&cycle, &live), cascade_pole(&PetriNet::new(&cycle, live.clone())));

        // Dead circuit (unmarked) — refuted.
        let dead: Marking<u32> = [].into();
        assert_eq!(driver_pole(&cycle, &dead), "refuted");
        assert_eq!(driver_pole(&cycle, &dead), cascade_pole(&PetriNet::new(&cycle, dead.clone())));

        // Live free-choice net (Esparza Fig 5.3).
        let mut b = NetBuilder::new();
        let [s1, s2, s3, s4, s5, s6, s7, s8] = b.add_places();
        let [t1, t2, t3, t4, t5, t6, t7] = b.add_transitions();
        b.add_arc((s1, t1)); b.add_arc((s2, t1));
        b.add_arc((s1, t2)); b.add_arc((s2, t2));
        b.add_arc((t1, s3)); b.add_arc((t1, s4));
        b.add_arc((t2, s5)); b.add_arc((t2, s6));
        b.add_arc((s3, t3)); b.add_arc((t3, s7));
        b.add_arc((s4, t4)); b.add_arc((t4, s8));
        b.add_arc((s5, t5)); b.add_arc((t5, s7));
        b.add_arc((s6, t6)); b.add_arc((t6, s8));
        b.add_arc((s7, t7)); b.add_arc((s8, t7));
        b.add_arc((t7, s1)); b.add_arc((t7, s2));
        let fc = b.build().expect("valid fc");
        assert_eq!(fc.class(), NetClass::FreeChoice);
        let fcm0: Marking<u32> = [(s1, 1), (s2, 1)].into();
        assert_eq!(driver_pole(&fc, &fcm0), "proven");
        assert_eq!(driver_pole(&fc, &fcm0), cascade_pole(&PetriNet::new(&fc, fcm0.clone())));

        // Unbounded net — inconclusive (liveness undecidable).
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((p2, t1)); b.add_arc((t1, p2));
        let unb = b.build().expect("valid net");
        let um0: Marking<u32> = [(p0, 1), (p2, 1)].into();
        assert_eq!(driver_pole(&unb, &um0), "inconclusive");
        assert_eq!(driver_pole(&unb, &um0), cascade_pole(&PetriNet::new(&unb, um0.clone())));
    }

    use crate::class::NetClass;

    /// `[PROP]` — over the admissible orderings the accepted pole is invariant, on a
    /// live circuit (two admissible deciders: structural-class + exhaustive).
    #[test]
    fn accepted_pole_invariant_over_orderings() {
        let driver = liveness_driver();
        let deciders = driver.deciders();
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid cycle");
        let class = net.class();

        for m0 in [Into::<Marking<u32>>::into([(p0, 1)]), [].into()] {
            let mut poles = Vec::new();
            // All permutations of the three deciders.
            for order in [
                [0usize, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0],
            ] {
                let mut pole = None;
                for &i in &order {
                    if !deciders[i].admissible(class) {
                        continue;
                    }
                    let v = deciders[i].run(&net, &m0, &Query::Trivial, Budget::unbounded());
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
