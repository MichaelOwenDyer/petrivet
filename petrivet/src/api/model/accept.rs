//! The trust boundary in code: [`Decided`] and [`accept`].
//!
//! `accept` is the **only** public path to [`Verdict::Proven`] / `Refuted`. A
//! decider hands it a candidate [`Decided`] token together with the
//! certificate that is supposed to justify it; `accept` runs the certificate's
//! `check` against the original net and yields the decided verdict only if the
//! check passes, otherwise [`Verdict::Inconclusive`]. There is no syntactic path
//! from a failed check to a decided verdict.
//!
//! `Decided` carries a *private* inner enum and is constructible only through
//! the `pub(crate)` constructors [`Decided::prove`] / [`Decided::refute`]. This
//! makes an un-checked `Proven`/`Refuted` unrepresentable even *inside* the
//! `model` module: code cannot write `Verdict::Proven(_)` from a bare payload
//! without first minting a `Decided` (crate-internal) and then passing it
//! through `accept` (the public gate). Module privacy alone would not stop a
//! relocated `From` impl or a future careless decider from forging a verdict;
//! the private token does. This is the doctrine-#4 realization of the firewall:
//! the compiler refuses the un-witnessed verdict before any test runs.

use crate::marking::Marking;
use crate::net::Net;
use crate::model::certificate::Certificate;
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::model::verdict::{Abstention, Proof, Refutation, Verdict};

/// A crate-internal candidate verdict awaiting certification.
///
/// A `Decided` is *not* yet a `Verdict`: it is the un-checked candidate a
/// decider proposes. The only way to turn it into a public `Proven`/`Refuted`
/// is to pass it through [`accept`], which gates on the certificate. The inner
/// representation is private, so the candidate cannot be unwrapped into a
/// verdict by any route other than the gate.
#[derive(Debug, Clone)]
pub struct Decided<P, N>(VerdictCore<P, N>);

/// The private candidate representation. Has no public constructor and no public
/// accessor; it exists only to be threaded from a `pub(crate)` constructor
/// through [`accept`].
//
// `dead_code`: the non-test constructors of these variants are the M2 deciders,
// which route their candidate verdicts through `Decided::prove`/`refute` and
// `accept`. The scaffolding is built ahead of its consumer (doctrine #7 — the
// blueprint is real); the M1 tests exercise the full path. Remove this allow
// when the first decider lands.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum VerdictCore<P, N> {
    Proven(P),
    Refuted(N),
}

impl<P, N> Decided<P, N> {
    /// Propose a positive candidate. Crate-internal: a candidate is not a
    /// verdict until it passes [`accept`].
    #[allow(dead_code)] // consumer is the M2 deciders; see the type's note above.
    pub(crate) const fn prove(proof: P) -> Self {
        Self(VerdictCore::Proven(proof))
    }

    /// Propose a negative candidate. Crate-internal: a candidate is not a
    /// verdict until it passes [`accept`].
    #[allow(dead_code)] // consumer is the M2 deciders; see the type's note above.
    pub(crate) const fn refute(refutation: N) -> Self {
        Self(VerdictCore::Refuted(refutation))
    }

    /// Promote a *checked* candidate to the public verdict. Private: reachable
    /// only from [`accept`], after the certificate has passed its `check`. This
    /// is the **sole** site that fills the private field of [`Proof`] /
    /// [`Refutation`], so it is the only producer of a decided `Verdict`.
    fn into_verdict(self) -> Verdict<P, N> {
        match self.0 {
            VerdictCore::Proven(p) => Verdict::Proven(Proof(p)),
            VerdictCore::Refuted(n) => Verdict::Refuted(Refutation(n)),
        }
    }

    /// The pole this candidate claims: [`Polarity::ProveYes`] for a positive
    /// (`prove`) candidate, [`Polarity::ProveNo`] for a negative (`refute`) one.
    /// [`accept`] requires the certificate's
    /// [`polarity`](crate::model::certificate::Certificate::polarity) to agree with
    /// this before promoting the candidate — so a positive witness cannot mint a
    /// `Refuted`, nor a refutation a `Proven`.
    const fn polarity(&self) -> Polarity {
        match &self.0 {
            VerdictCore::Proven(_) => Polarity::ProveYes,
            VerdictCore::Refuted(_) => Polarity::ProveNo,
        }
    }
}

/// Gate a candidate verdict on its certificate, against the original net.
///
/// This function is the trusted computing base in code: it is the sole public
/// constructor of [`Verdict::Proven`] / `Refuted`. If `cert.check(net, m0,
/// query)` returns `true`, the candidate is promoted to the corresponding
/// decided verdict; otherwise the result is
/// `Verdict::Inconclusive(Abstention::NotApplicable)` — a rejected candidate
/// collapses to an honest abstention, **never** to the opposite verdict.
///
/// # The polarity-coherence gate (A6), enforced
///
/// `accept` gates on the check's *pass/fail* **and** on polarity coherence: the
/// certificate's [`polarity`](Certificate::polarity) must agree with the
/// candidate's variant. A caller that pairs a *positive* (reachability)
/// certificate with a `Decided::refute(_)` candidate gets `Inconclusive`, never
/// `Refuted` — even if the certificate `check`s `true`, because the witness
/// establishes the *positive* property, not the refutation the candidate claims.
/// Symmetrically, a refutation cannot mint a `Proven`. The coherence test
/// short-circuits *before* `check`, so a misused certificate's (possibly
/// expensive) check is never run, and a mismatch collapses to the same honest
/// `Inconclusive(NotApplicable)` as a failed check — never the opposite verdict.
///
/// This closes the A6 latent gap: previously the C1 checkers were each internally
/// pole-correct (a [`FiringSequenceCert`](crate::model::FiringSequenceCert) only
/// `check`s a `Reachable`/`Coverable` target), so no *wired* path minted a
/// wrong-signed verdict — but nothing at the boundary *enforced* it, so a future
/// decider pairing a passing checker with the wrong-signed candidate would have.
/// Now the `accept` boundary enforces it structurally. See `foundational-design.md`
/// §F3‴ / §F5, BACKLOG A6.
// No `#[must_use]` here: the returned `Verdict` already carries it, and a
// second attribute is redundant (clippy::double_must_use).
pub fn accept<P, N, C: Certificate>(
    candidate: Decided<P, N>,
    cert: &C,
    net: &Net,
    m0: &Marking<u32>,
    query: &Query,
) -> Verdict<P, N> {
    // Polarity coherence: the certificate must witness the SAME pole the candidate
    // claims. An `Exact` certificate (decides both poles) is compatible with either
    // candidate; a `ProveYes`/`ProveNo` certificate must match exactly. Checked
    // before `check` so a misused certificate's check is never run.
    let coherent = matches!(cert.polarity(), Polarity::Exact)
        || cert.polarity() == candidate.polarity();
    if coherent && cert.check(net, m0, query) {
        candidate.into_verdict()
    } else {
        Verdict::inconclusive(Abstention::NotApplicable)
    }
}

#[cfg(test)]
mod tests {
    use super::{accept, Decided};
    use crate::marking::Marking;
    use crate::model::certificate::Certificate;
    use crate::model::frontier::Polarity;
    use crate::model::query::Query;
    use crate::model::verdict::Abstention;
    use crate::prelude::{Net, NetBuilder, PetriNet, Transition};

    /// A test certificate that replays a firing sequence against the *original*
    /// net and accepts iff it reaches the `Reachable` query's target. This is a
    /// genuine original-net check (replay through `fire`/`is_enabled`), so it
    /// exercises the real `accept` plumbing — including that the *query* is
    /// load-bearing.
    struct FiringSequenceCert {
        sequence: Box<[Transition]>,
    }

    impl Certificate for FiringSequenceCert {
        fn polarity(&self) -> Polarity {
            Polarity::ProveYes
        }
        fn check(&self, net: &Net, m0: &Marking<u32>, query: &Query) -> bool {
            let Query::Reachable(target) = query else {
                return false;
            };
            let mut sys = PetriNet::new(net, m0.clone());
            for &t in &self.sequence {
                if sys.try_fire(t).is_err() {
                    return false;
                }
            }
            sys.marking() == *target
        }
    }

    /// A certificate that always rejects — stands in for a corrupted witness.
    /// Declares `ProveYes` (a corrupted positive witness); since it never passes
    /// `check`, its pole is immaterial to the failing-check tests.
    struct AlwaysReject;
    impl Certificate for AlwaysReject {
        fn polarity(&self) -> Polarity {
            Polarity::ProveYes
        }
        fn check(&self, _net: &Net, _m0: &Marking<u32>, _query: &Query) -> bool {
            false
        }
    }

    /// A `ProveNo` certificate whose `check` always passes — stands in for a valid
    /// refutation witness, so the coherence gate can be exercised on the negative
    /// pole (where `AlwaysReject` cannot, since it never passes `check`).
    struct AlwaysAcceptNo;
    impl Certificate for AlwaysAcceptNo {
        fn polarity(&self) -> Polarity {
            Polarity::ProveNo
        }
        fn check(&self, _net: &Net, _m0: &Marking<u32>, _query: &Query) -> bool {
            true
        }
    }

    /// Two-place cycle p0 -> t0 -> p1 -> t1 -> p0.
    fn two_place_cycle() -> (Net, crate::prelude::Place, Transition, crate::prelude::Place, Transition)
    {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().expect("valid net");
        (net, p0, t0, p1, t1)
    }

    /// Replay a firing sequence from `m0` and return the marking it reaches.
    /// Used to derive the *true* target a sequence reaches, so these tests do
    /// not hardcode a target (and so are robust to the net's internal index
    /// assignment).
    fn marking_after(net: &Net, m0: &Marking<u32>, seq: &[Transition]) -> Marking<u32> {
        let mut sys = PetriNet::new(net, m0.clone());
        for &t in seq {
            sys.try_fire(t).expect("sequence transition should be enabled");
        }
        sys.marking()
    }

    /// The plumbing property: a passing check is the ONLY path to `Proven`.
    #[test]
    fn accept_passing_check_yields_proven() {
        let (net, p0, t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let reached = marking_after(&net, &m0, &[t0]);
        let query = Query::Reachable(reached);
        let cert = FiringSequenceCert { sequence: Box::new([t0]) };

        let candidate: Decided<&str, &str> = Decided::prove("reaches the target");
        let verdict = accept(candidate, &cert, &net, &m0, &query);
        assert!(verdict.is_proven());
        assert_eq!(verdict.proof(), Some(&"reaches the target"));
    }

    /// A failing check yields `Inconclusive(NotApplicable)`, NEVER the opposite
    /// verdict. A corrupted certificate is rejected.
    #[test]
    fn accept_failing_check_yields_inconclusive_not_refuted() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let query = Query::Reachable([(p1, 1)].into());

        let candidate: Decided<&str, &str> = Decided::prove("forged");
        let verdict = accept(candidate, &AlwaysReject, &net, &m0, &query);
        assert!(verdict.is_inconclusive());
        assert_eq!(verdict.abstention(), Some(Abstention::NotApplicable));
        // Crucially: a rejected positive candidate did NOT become a Refuted.
        assert!(!verdict.is_refuted());
    }

    /// The same firewall holds for a negative candidate: a failed check
    /// collapses to `Inconclusive`, never flips to `Proven`.
    #[test]
    fn accept_failing_check_does_not_flip_refute_to_prove() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let query = Query::Reachable([(p1, 1)].into());

        let candidate: Decided<&str, &str> = Decided::refute("forged refutation");
        let verdict = accept(candidate, &AlwaysReject, &net, &m0, &query);
        assert!(verdict.is_inconclusive());
        assert!(!verdict.is_proven());
    }

    /// THE LOAD-BEARING GATE: a `FiringSequence` witness valid for one query
    /// FAILS when checked against a *different* query — the target is part of
    /// what is checked, so a witness cannot be reused across queries.
    ///
    /// This is the M1 `[PROP]` gate, discharged by *quantifying over a generated
    /// family* of perturbations rather than two hand-picked targets (the prior
    /// weakness flagged by the adversarial review). The invariant proven is
    /// universal over the family: for every target marking that differs from the
    /// one the sequence actually reaches, `accept` is `Inconclusive` — only the
    /// single correct target yields `Proven`.
    #[test]
    fn firing_sequence_fails_against_a_different_query() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = FiringSequenceCert { sequence: Box::new([t0]) };

        // The actual marking [t0] reaches — the unique query this witness answers.
        let reached = marking_after(&net, &m0, &[t0]);
        let right = Query::Reachable(reached.clone());
        let v_right = accept(Decided::<&str, &str>::prove("ok"), &cert, &net, &m0, &right);
        assert!(v_right.is_proven(), "the correct target must be accepted");

        // Generated family of perturbed targets: for each place in {p0, p1} and
        // each token delta in 1..=8 (added on top of the reached count, then
        // again removed below the reached count where possible), build a target
        // that DIFFERS from `reached` and assert the witness is rejected. The
        // universal claim is "accept is Inconclusive whenever target != reached".
        let mut perturbed_cases = 0_u32;
        for place in [p0, p1] {
            let base = reached.get(place);
            // Deltas in both directions so we exercise more-than and fewer-than.
            let deltas: [i64; 16] = [
                1, 2, 3, 4, 5, 6, 7, 8, -1, -2, -3, -4, -5, -6, -7, -8,
            ];
            for delta in deltas {
                let Ok(new_count) = u32::try_from(i64::from(base) + delta) else {
                    continue; // negative target on this place: skip, not representable.
                };
                if new_count == base {
                    continue; // not a perturbation.
                }
                let perturbed: Marking<u32> = reached
                    .clone()
                    .into_iter()
                    .filter(|(p, _)| *p != place)
                    .chain(std::iter::once((place, new_count)))
                    .collect();
                assert_ne!(perturbed, reached, "the perturbed target must differ");
                let wrong = Query::Reachable(perturbed);
                let v =
                    accept(Decided::<&str, &str>::prove("ok"), &cert, &net, &m0, &wrong);
                assert!(
                    v.is_inconclusive(),
                    "a witness must be rejected for every target != reached (place {place:?}, count {new_count})",
                );
                perturbed_cases += 1;
            }
        }
        assert!(
            perturbed_cases >= 8,
            "the generated family must be non-trivial (got {perturbed_cases})",
        );

        // And a query-free `Trivial` query is also rejected by a reachability
        // witness — it answers a reachability question, not a global one.
        let trivial = Query::Trivial;
        let v_trivial =
            accept(Decided::<&str, &str>::prove("ok"), &cert, &net, &m0, &trivial);
        assert!(v_trivial.is_inconclusive());
    }

    /// Quantified plumbing property over every (candidate-polarity × check-outcome)
    /// combination with a *positive* (`ProveYes`) certificate: a decided verdict
    /// appears iff the check passed **and** the candidate's pole matches the
    /// certificate's. A positive certificate can never mint a `Refuted`.
    #[test]
    fn accept_is_check_gated_for_all_polarity_outcome_combinations() {
        let (net, p0, t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let query = Query::Reachable(marking_after(&net, &m0, &[t0]));
        let passing = FiringSequenceCert { sequence: Box::new([t0]) }; // ProveYes

        // Passing check, positive cert, positive candidate -> Proven (coherent).
        let v = accept(Decided::<u8, u8>::prove(1), &passing, &net, &m0, &query);
        assert!(v.is_proven() && !v.is_refuted());
        // Passing check, positive cert, NEGATIVE candidate -> Inconclusive: the A6
        // coherence gate refuses to mint a `Refuted` from a positive witness.
        let v = accept(Decided::<u8, u8>::refute(2), &passing, &net, &m0, &query);
        assert!(v.is_inconclusive() && !v.is_refuted() && !v.is_proven());
        // Failing check, positive candidate -> Inconclusive.
        let v = accept(Decided::<u8, u8>::prove(3), &AlwaysReject, &net, &m0, &query);
        assert!(v.is_inconclusive());
        // Failing check, negative candidate -> Inconclusive.
        let v = accept(Decided::<u8, u8>::refute(4), &AlwaysReject, &net, &m0, &query);
        assert!(v.is_inconclusive());
    }

    /// The A6 polarity-coherence gate, both directions, against *passing* checks:
    /// a certificate's witnessed pole must match the candidate's, or `accept`
    /// abstains — never minting the wrong-signed verdict. Uses a passing `ProveYes`
    /// cert and a passing `ProveNo` cert so the gate (not the check) is the only
    /// thing that can reject the mismatches.
    #[test]
    fn accept_enforces_polarity_coherence() {
        let (net, p0, t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let reach = Query::Reachable(marking_after(&net, &m0, &[t0]));
        let yes = FiringSequenceCert { sequence: Box::new([t0]) }; // ProveYes, passes
        let no = AlwaysAcceptNo; // ProveNo, passes

        // Matched poles + passing check -> the decided verdict.
        assert!(accept(Decided::<u8, u8>::prove(1), &yes, &net, &m0, &reach).is_proven());
        let v = accept(Decided::<u8, u8>::refute(2), &no, &net, &m0, &Query::Trivial);
        assert!(v.is_refuted());

        // Mismatched poles + passing check -> Inconclusive, never the wrong verdict.
        // Positive cert with a refute candidate:
        let v = accept(Decided::<u8, u8>::refute(3), &yes, &net, &m0, &reach);
        assert!(v.is_inconclusive() && !v.is_refuted());
        // Negative cert with a prove candidate:
        let v = accept(Decided::<u8, u8>::prove(4), &no, &net, &m0, &Query::Trivial);
        assert!(v.is_inconclusive() && !v.is_proven());
    }
}
