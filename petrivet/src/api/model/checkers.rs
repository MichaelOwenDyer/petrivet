//! M2 · C1 — the per-certificate checkers, each validating against the
//! **original** net.
//!
//! This module is the signature contribution made concrete: a small set of
//! [`Certificate`] implementors whose `check` re-establishes a property against
//! the original `(net, m0, query)`, **sharing no code with the generators**
//! beyond primitive net access (`fire`/`is_enabled`, per-transition pre/post
//! sets, token sums). Each checker is the *only* code whose correctness is
//! load-bearing for the verdict it gates; the generator that produced the
//! witness may be arbitrarily buggy without affecting soundness, because the
//! candidate only becomes a [`Verdict::Proven`](crate::model::Verdict)/`Refuted`
//! after passing through [`accept`](crate::model::accept) with one of these.
//!
//! # What is checkable now (M2) versus the wall (C7)
//!
//! These checkers cover the witnesses that are already *structured* and
//! near-linear- or polynomial-checkable with no exact-rational machinery:
//!
//! - [`FiringSequenceCert`] — positive reachability/coverability by **replay**.
//!   Near-linear: fire the sequence from `M₀` and compare. (Witness:
//!   [`ReachabilityProof::FiringSequence`](crate::model::ReachabilityProof),
//!   [`CoverabilityProof::firing_sequence`](crate::model::CoverabilityProof) in
//!   the finite case.)
//! - [`ParikhVectorCert`] — a marking-equation **integer** solution checked by
//!   recomputing `M₀ + C·σ` over ℤ. Exact, no LP. (Witness: the
//!   `LiveMarkedGraphMarkingEquationIntegerSolution` /
//!   `LiveBoundedFreeChoiceMarkingEquationWithTrapCheck` Parikh maps.)
//! - [`TokenConservationCert`] — a **negative** reachability verdict for state
//!   machines: re-verify the net is a state machine (`|•t| = |t•| = 1` for every
//!   transition) and that `M₀` and the target have different token sums.
//!   (Witness: `UnreachabilityProof::StateMachineTokenConservation`.)
//! - [`SiphonTrapCoverCert`] — a free-choice-liveness / general-deadlock-freedom
//!   witness. `check` re-derives the Commoner–Hack **universal** from primitives
//!   (every minimal siphon holds a marked trap) and applies a polarity/class fence
//!   from the cert's [`SiphonTrapClaim`]: `Live` is sound only on free-choice nets
//!   (so it is gated there), `DeadlockFree` is sound on any class. The exhibited
//!   cover is advisory. Polynomial in the *exhibited* cover plus the re-enumeration
//!   (the generator bears the exponential siphon enumeration — C7's "polynomial"
//!   tier; the re-derivation moves the universal into the trusted accept path).
//!
//! Deferred to where their soundness precondition lands (recorded in the C7
//! frontier map, [`super::frontier`]):
//!
//! - The Farkas **P-invariant** negative checker for
//!   `MarkingEquationNoRationalSolution` needs an *exact-rational* separating
//!   invariant (`y·C = 0 ∧ y·(m'−m₀) ≠ 0` in exact arithmetic) — that is B0/B1a
//!   at M5; a raw `f64` dual is not emittable as a certificate (foundational
//!   design §1, §4.1). M2 does **not** mint a checker over `f64`.
//! - The ω-coverability witness needs its **pumping cycle** (lasso) and the
//!   marked-graph witness its **circuit-token** data before either checks; both
//!   are the C7 witness-enrichment items.
//! - General (non-free-choice) liveness has no known compact checkable
//!   certificate — the wall.
//!
//! # The original-net discipline, enforced
//!
//! Every checker below takes `(net, m0, query)` and reads only the public,
//! primitive net surface ([`Net::transition_preset`], [`Net::transition_postset`],
//! [`PetriNet::try_fire`], [`Marking::get`]). None of them calls a solver, builds
//! a state space, or re-runs a generator. That is what makes the checker — and
//! not the generator — the trusted code, and what lets a certificate from a
//! *different* procedure (or a lifted one, Epic F) validate identically.

use crate::core::analysis::rational::Rational;
use crate::marking::Marking;
use crate::model::certificate::Certificate;
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::net::{Net, Place, Transition};
use crate::prelude::PetriNet;
use ahash::{HashMap, HashSet};

/// Replay a firing sequence from `m0`, returning the reached marking, or `None`
/// if any transition in the sequence was not enabled when its turn came (an
/// invalid witness). Uses only `PetriNet::try_fire` — pure simulation against the
/// original net.
fn replay(net: &Net, m0: &Marking<u32>, sequence: &[Transition]) -> Option<Marking<u32>> {
    let mut sys = PetriNet::new(net, m0.clone());
    for &t in sequence {
        sys.try_fire(t).ok()?;
    }
    Some(sys.marking())
}

/// Whether `cover` ≥ `target` place-wise over the **union** of their supports.
/// The public `Marking<u32>` does not implement `PartialOrd` (only the internal
/// index marking does), so coverage is computed explicitly through `get`, which
/// returns 0 off-support. A place absent from `cover` but present in `target`
/// (with a positive count) fails coverage, as it must.
fn covers(cover: &Marking<u32>, target: &Marking<u32>) -> bool {
    target
        .support()
        .all(|p| cover.get(p) >= target.get(p))
}

/// A positive reachability/coverability witness: a transition firing sequence.
///
/// `check` replays the sequence from `M₀` against the **original** net and
/// accepts iff:
/// - for [`Query::Reachable(target)`](Query) — the reached marking *equals* the
///   target;
/// - for [`Query::Coverable(target)`](Query) — the reached marking *covers* the
///   target (`≥` place-wise).
///
/// A `Query::Trivial` is rejected: a firing-sequence witness answers a
/// reachability/coverability question, not a global property. This is the
/// load-bearing "the query is part of what is checked" discipline.
#[derive(Debug, Clone)]
pub struct FiringSequenceCert {
    /// The transition sequence claimed to reach (or cover) the target from `M₀`.
    pub sequence: Box<[Transition]>,
}

impl Certificate for FiringSequenceCert {
    /// Positive: a realized firing sequence witnesses that the target is
    /// reachable/coverable.
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }

    fn check(&self, net: &Net, m0: &Marking<u32>, query: &Query) -> bool {
        let Some(reached) = replay(net, m0, &self.sequence) else {
            return false;
        };
        match query {
            Query::Reachable(target) => reached == *target,
            Query::Coverable(target) => covers(&reached, target),
            // A firing-sequence witness does not answer a query-free property.
            // (`Query` is `#[non_exhaustive]` for downstream crates, but within
            // this crate the match is exhaustive — no wildcard needed.)
            Query::Trivial => false,
        }
    }
}

/// A marking-equation **integer** solution (a Parikh vector `σ`): for each
/// transition, the number of times it fires.
///
/// `check` accepts a `Reachable` target iff it can **realize** `σ` as an actual
/// firing sequence against the original net — that is, fire the transitions in
/// *some* order, each exactly `σ(t)` times, with every step enabled, ending on a
/// marking that equals the target.
///
/// # Why realization, not the bare state equation (the BLOCKER 1 soundness fix)
///
/// The state equation `M_target = M₀ + C·σ` is only a *necessary* condition for
/// reachability, not a sufficient one. It is *sufficient* on **live** marked
/// graphs (Theorem 5.2.7) and **live-and-bounded** free-choice nets with the
/// accompanying trap check (Theorem 5.3.18) — but a *non-live* net of the same
/// class can satisfy the equation for a target that is genuinely unreachable
/// (the σ requires firing a dead transition). A class fence alone therefore
/// mints a false `Proven` (verified counterexample: a bounded, strongly-connected
/// but non-live marked graph where `M₀ + C·σ = target` yet the oracle reports
/// `Unreachable`).
///
/// Re-deriving the missing precondition — marked-graph liveness (every circuit
/// marked) or free-choice liveness (Commoner–Hack) — would require the net's
/// *liveness-analysis machinery* (circuit enumeration; the CHC siphon/trap
/// engine), which is **not** primitive net access. Per the trust contract, the
/// checker must instead establish the property from primitives alone, or abstain.
///
/// **Realization does exactly that, soundly and without any class assumption.** A
/// successfully replayed `σ` *is* a firing sequence reaching the target, so it
/// certifies reachability on *any* net — there is nothing left to trust. The
/// replay uses only `try_fire`/`is_enabled` (pure simulation against the original
/// net), shares no code with the generator, and never touches `f64` or unchecked
/// `i64` (it compares actual `u32` markings, closing the silent-overflow direction
/// the bare equation accumulation had). It *preserves capability* precisely where
/// the theorems hold: on a live marked graph / live-bounded free-choice net the
/// equation is realizable, so a valid witness still validates. Where realization
/// gets stuck (a wrong order, or a genuinely unrealizable σ) the checker returns
/// `false` (abstain) — never a false positive.
///
/// # Realization strategy and its completeness
///
/// `check` greedily fires any still-needed transition that is currently enabled,
/// repeating until all counts are exhausted (accept, if the reached marking equals
/// the target) or no needed transition is enabled (reject). For the *persistent*
/// classes this certificate targets — marked graphs (always persistent) and
/// free-choice nets (cluster-persistent) — a greedy maximal strategy realizes the
/// Parikh vector whenever any realization exists, so capability is not lost on the
/// theorem-backed cases. On a general net greedy replay is sound but may not be
/// complete; that is the honest abstention posture, not a soundness defect.
#[derive(Debug, Clone)]
pub struct ParikhVectorCert {
    /// The firing-count vector `σ`: transitions to fire counts. Transitions
    /// absent from the map fire zero times.
    pub firing_counts: HashMap<Transition, u32>,
}

impl ParikhVectorCert {
    /// Try to realize the Parikh vector `σ` as a firing sequence from `m0`,
    /// returning the reached marking on success, or `None` if it could not be
    /// fully realized (some needed transition was never enabled before its count
    /// was exhausted). Pure simulation against the original net via `try_fire`.
    ///
    /// Greedy maximal strategy: while any transition with remaining count is
    /// enabled, fire it (decrementing its remaining count). Terminates because the
    /// total remaining count strictly decreases on each successful fire.
    fn realize(&self, net: &Net, m0: &Marking<u32>) -> Option<Marking<u32>> {
        let mut remaining: HashMap<Transition, u32> = self
            .firing_counts
            .iter()
            .filter(|(_, c)| **c > 0)
            .map(|(t, c)| (*t, *c))
            .collect();
        let mut sys = PetriNet::new(net, m0.clone());
        while !remaining.is_empty() {
            // Find a still-needed, currently-enabled transition.
            let Some(t) = remaining
                .keys()
                .copied()
                .find(|&t| sys.is_enabled(t))
            else {
                // No needed transition is enabled: σ cannot be realized from here.
                return None;
            };
            sys.try_fire(t).ok()?;
            let count = remaining.get_mut(&t).expect("t is a key");
            *count -= 1;
            if *count == 0 {
                remaining.remove(&t);
            }
        }
        Some(sys.marking())
    }
}

impl Certificate for ParikhVectorCert {
    /// Positive: realizing the Parikh vector as a firing sequence witnesses that
    /// the target is reachable.
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }

    fn check(&self, net: &Net, m0: &Marking<u32>, query: &Query) -> bool {
        let Query::Reachable(target) = query else {
            return false;
        };
        // Accept iff σ realizes as an actual firing sequence ending at the target.
        // This is a self-contained reachability proof against the original net —
        // sound on *any* class, with no liveness assumption to trust. (See the
        // type doc for why the bare state equation + class fence was unsound.)
        self.realize(net, m0)
            .is_some_and(|reached| reached == *target)
    }
}

/// A **negative** reachability witness for state machines: token conservation.
///
/// In a state machine (S-net) every transition has exactly one input and one
/// output place (`|•t| = |t•| = 1`), so firing preserves the total token count.
/// Therefore any target with a token sum different from `M₀`'s is unreachable —
/// regardless of liveness or connectivity (Theorem 5.1.5 corollary).
///
/// `check` re-establishes *both* halves against the original net: it confirms the
/// net really is a state machine (every transition has unit pre/post sets) and
/// that the target's token sum differs from `M₀`'s. It does **not** trust a
/// `NetClass` tag for this: the structural precondition is re-derived from the
/// pre/post sets directly, so the checker shares nothing with the classifier.
#[derive(Debug, Clone)]
pub struct TokenConservationCert;

impl Certificate for TokenConservationCert {
    /// Negative: a token-sum mismatch witnesses that the target is *un*reachable —
    /// a refutation, not a positive proof.
    fn polarity(&self) -> Polarity {
        Polarity::ProveNo
    }

    fn check(&self, net: &Net, m0: &Marking<u32>, query: &Query) -> bool {
        let Query::Reachable(target) = query else {
            return false;
        };
        // Re-derive the state-machine precondition directly: every transition
        // has exactly one input and one output place. (An isolated transition
        // with empty pre/post would change nothing, but the textbook S-net
        // definition requires unit pre/post; we require it here to match the
        // theorem the verdict rests on.)
        let is_state_machine = net.transitions().all(|t| {
            net.transition_preset(&t).count() == 1 && net.transition_postset(&t).count() == 1
        });
        if !is_state_machine {
            return false;
        }
        // Token conservation: different sums ⇒ the target is unreachable. The sums
        // are accumulated in `u64` (M·minor: `Marking::total_tokens` sums `u32` and
        // would wrap in release; here, on the *verdict* path, a wide accumulator
        // removes the wrap. A wrap would be the *safe* direction for this negative
        // certificate — it could only make two genuinely-different sums compare
        // equal, demoting a valid refutation to abstention — but a verdict path
        // must not depend on that, so we widen).
        total_tokens_u64(m0) != total_tokens_u64(target)
    }
}

/// Total token count of a `Marking<u32>` accumulated in `u64`, so the sum cannot
/// wrap in release (the M·minor widening for the verdict path; cf.
/// [`Marking::total_tokens`], which sums in `u32`).
fn total_tokens_u64(m: &Marking<u32>) -> u64 {
    m.support().map(|p| u64::from(m.get(p))).sum()
}

/// A free-choice-liveness / general-deadlock-freedom witness: an *exhibited*
/// cover of (siphon, trap) pairs where each trap is marked under `M₀`.
///
/// By the Commoner–Hack criterion, a free-choice system is live iff **every**
/// proper siphon contains a trap marked under `M₀`; for general nets the same
/// condition is *sufficient* for deadlock-freedom. The generator bears the
/// (worst-case exponential) enumeration of minimal siphons; the certificate
/// records the resulting finite cover.
///
/// # The universal is re-established, not assumed (the BLOCKER 2 soundness fix)
///
/// The Commoner–Hack criterion is a **universal**: *every* (minimal) siphon must
/// contain a marked trap. Validating only the *exhibited* pairs is not enough — an
/// **incomplete** cover that omits the one offending siphon (whose maximal trap is
/// empty or unmarked) would pass per-pair while the net actually deadlocks,
/// minting a false `Proven` liveness/deadlock-freedom verdict (verified
/// counterexample: a non-live net carrying one genuinely marked siphon-trap pair
/// `{p0,p1}` whose cover omits the unmarked offender siphon `{q0,q1}`; the oracle
/// reports `is_live == false`).
///
/// `check` therefore **re-enumerates the minimal siphons of the original net** and
/// confirms each one contains a non-empty trap marked under `M₀` — re-deriving the
/// universal independently, rather than trusting the cover to be complete. Minimal-
/// siphon enumeration and the maximal-trap-in-a-set computation are *core net
/// access* under the C1 trust exception (they are siphon/trap structure, not the
/// liveness/boundedness decision the generator makes); the checker assembles them
/// into the universal itself, sharing no decision code with the generator. Cost is
/// the (worst-case exponential) re-enumeration — the same structural object the
/// generator computed — which is the honest price of re-establishing a *universal*
/// from primitives. The exhibited cover is additionally validated for internal
/// consistency, but the verdict rests on the independently re-derived universal.
///
/// For each exhibited `(siphon, trap)` pair, `check` re-verifies, against the
/// **original** net:
/// 1. `siphon` is a siphon: `•siphon ⊆ siphon•` (every transition producing into
///    the set also consumes from it);
/// 2. `trap ⊆ siphon`;
/// 3. `trap` is a trap: `trap• ⊆ •trap` (every transition consuming from the set
///    also produces into it);
/// 4. `trap` is marked under `M₀` (some place in `trap` holds a token).
///
/// # The liveness ⇄ deadlock-freedom polarity gate (the second BLOCKER 2 fix)
///
/// The Commoner–Hack universal is *sufficient for liveness only on free-choice
/// nets*; on a **general (non-free-choice) net it proves deadlock-freedom only**
/// (the repo's own [`commoner_hack_criterion`](crate::core::analysis::siphon_trap::commoner_hack_criterion)
/// records this — Murata Thm 12, Primer Thm 5.17). A cert that re-established the
/// universal but did not distinguish *which* verdict it backs would mint a false
/// `Proven` **liveness** on a non-free-choice net where the universal holds yet the
/// net is not live. That net exists and is small: an oracle-confirmed `General`
/// 4-place net (`bounded`, `deadlock_free`, every minimal siphon marked-trap-
/// covered, but `is_live == false`) — pinned by
/// `siphon_trap_cover_rejects_liveness_on_a_non_free_choice_net`.
///
/// The fix carries the verdict's *meaning* in the type: [`SiphonTrapClaim`] makes
/// the dangerous conflation "CHC passed on a general net, therefore live"
/// **unrepresentable**. `check` applies the correct class fence:
/// - [`SiphonTrapClaim::Live`] additionally requires the original net be
///   free-choice ([`NetClass::is_free_choice`](crate::class::NetClass::is_free_choice),
///   a structural tag recomputable from pre/post cardinalities — *not* liveness
///   machinery). On a non-free-choice net a `Live` claim is rejected (abstain).
/// - [`SiphonTrapClaim::DeadlockFree`] is accepted on any class: the universal is
///   sufficient for deadlock-freedom unconditionally.
///
/// `query` is [`Query::Trivial`]: liveness and deadlock-freedom are query-free
/// global properties. A reachability/coverability query is rejected.
#[derive(Debug, Clone)]
pub struct SiphonTrapCoverCert {
    /// Which verdict this cover certifies. The class fence `check` applies depends
    /// on it: `Live` is gated to free-choice nets, `DeadlockFree` is class-agnostic.
    pub claim: SiphonTrapClaim,
    /// The exhibited cover: each entry is a siphon together with a trap inside it
    /// that is claimed to be marked under `M₀`. **Advisory / audit only** — the
    /// verdict rests on the independently re-derived universal, not on this list
    /// (which a generator could leave incomplete). Validated for internal
    /// consistency so a malformed pair can only *reject*, never make `check` accept.
    pub cover: Box<[(HashSet<Place>, HashSet<Place>)]>,
}

/// What a [`SiphonTrapCoverCert`] certifies.
///
/// This fixes the class fence `check` applies, making the polarity-ambiguous
/// conflation (one Commoner–Hack pass backing *both* free-choice liveness and
/// general deadlock-freedom) unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SiphonTrapClaim {
    /// Liveness. Sound **only on free-choice nets** (Commoner–Hack); `check` gates
    /// the verdict to [`NetClass::is_free_choice`](crate::class::NetClass::is_free_choice)
    /// and abstains otherwise.
    Live,
    /// Deadlock-freedom. The marked-trap-cover universal is sufficient on **any**
    /// class, so `check` applies no class fence.
    DeadlockFree,
}

impl SiphonTrapCoverCert {
    /// Whether `places` is a siphon in `net`: `•places ⊆ places•`. Equivalently,
    /// every transition that has an output place in the set also has an input
    /// place in the set.
    fn is_siphon(net: &Net, places: &HashSet<Place>) -> bool {
        net.transitions().all(|t| {
            let produces_into = net.transition_postset(&t).any(|p| places.contains(&p));
            let consumes_from = net.transition_preset(&t).any(|p| places.contains(&p));
            // •places ⊆ places•: if t produces into the set (t ∈ •places), then
            // t must also consume from it (t ∈ places•).
            !produces_into || consumes_from
        })
    }

    /// Whether `places` is a trap in `net`: `places• ⊆ •places`. Every
    /// transition that consumes from the set also produces into it.
    fn is_trap(net: &Net, places: &HashSet<Place>) -> bool {
        net.transitions().all(|t| {
            let consumes_from = net.transition_preset(&t).any(|p| places.contains(&p));
            let produces_into = net.transition_postset(&t).any(|p| places.contains(&p));
            // places• ⊆ •places: if t consumes from the set (t ∈ places•), then
            // t must also produce into it (t ∈ •places).
            !consumes_from || produces_into
        })
    }

    /// Whether `trap` holds a token under `m0`.
    fn is_marked(m0: &Marking<u32>, trap: &HashSet<Place>) -> bool {
        trap.iter().any(|&p| m0.get(p) > 0)
    }

    /// Re-establish the Commoner–Hack **universal** from primitives: every minimal
    /// siphon of the original net contains a non-empty trap marked under `M₀`.
    ///
    /// This is the load-bearing soundness gate. It re-enumerates the minimal
    /// siphons (C1 core-net-access exception) and, for each, computes the maximal
    /// trap inside it ([`maximal_trap_in`](crate::core::analysis::siphon_trap::maximal_trap_in))
    /// and checks that trap is non-empty and marked — sharing no decision code
    /// with the generator's `commoner_hack_criterion`. Returns `false` if any
    /// minimal siphon has no marked trap (the deadlock witness the universal
    /// forbids).
    fn every_minimal_siphon_has_a_marked_trap(net: &Net, m0: &Marking<u32>) -> bool {
        use crate::core::analysis::siphon_trap::{maximal_trap_in, minimal_siphons};
        let dense = &net.dense_net;
        let m0_idx = net.mapping.idx_marking(m0.clone());
        minimal_siphons(dense).iter().all(|siphon| {
            let trap = maximal_trap_in(dense, siphon.clone());
            // The maximal trap inside this siphon must be non-empty and marked.
            !trap.is_empty() && trap.iter().any(|&p_idx| m0_idx[p_idx] > 0)
        })
    }
}

impl Certificate for SiphonTrapCoverCert {
    /// Positive: a marked-trap cover of every minimal siphon witnesses that the
    /// net *is* live (free-choice) or deadlock-free — both poles of
    /// [`SiphonTrapClaim`] are positive properties.
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }

    fn check(&self, net: &Net, m0: &Marking<u32>, query: &Query) -> bool {
        if !matches!(query, Query::Trivial) {
            return false;
        }
        if self.cover.is_empty() {
            // An empty cover certifies nothing.
            return false;
        }
        // Internal consistency of the exhibited cover: each pair is a genuine
        // siphon containing a genuine marked trap. (Necessary, but NOT sufficient:
        // the cover could be incomplete — see below.)
        let exhibited_pairs_are_valid = self.cover.iter().all(|(siphon, trap)| {
            Self::is_siphon(net, siphon)
                && trap.iter().all(|p| siphon.contains(p))
                && !trap.is_empty()
                && Self::is_trap(net, trap)
                && Self::is_marked(m0, trap)
        });
        if !exhibited_pairs_are_valid {
            return false;
        }
        // The soundness gate: re-establish the UNIVERSAL independently — every
        // minimal siphon of the original net contains a marked trap. An incomplete
        // exhibited cover cannot mint a false `Proven`, because this is re-derived
        // from primitives, not read off the (possibly partial) cover.
        if !Self::every_minimal_siphon_has_a_marked_trap(net, m0) {
            return false;
        }
        // The polarity/class fence: the universal is sufficient for liveness ONLY on
        // free-choice nets; on a general net it proves deadlock-freedom only. A
        // `Live` claim on a non-free-choice net is therefore rejected (abstain) —
        // this closes the false-Proven-liveness hole on the oracle-confirmed
        // non-free-choice counterexample. `is_free_choice` is a structural tag over
        // pre/post cardinalities, recomputable from primitives, not the net's
        // liveness-analysis machinery.
        match self.claim {
            SiphonTrapClaim::Live => net.class().is_free_choice(),
            SiphonTrapClaim::DeadlockFree => true,
        }
    }
}

/// A4 — a **structural-boundedness** witness: an exact positive place
/// sub-invariant `y`.
///
/// A net is structurally bounded (bounded under *every* initial marking) iff
/// there is a positive place weighting `y > 0` with `yᵀ·C ≤ 0` — the weighted
/// token count `y·M` cannot increase across any firing, so it is bounded by its
/// initial value, giving the per-place bound `M[p] ≤ ⌊(y·M₀) / y[p]⌋`
/// (Murata Table 5; Primer Prop. 4.12).
///
/// The witness is **exact** ([`Rational`] weights): an `f64` weight vector is not
/// a certificate (a near-boundary `yᵀ·C` could be a small positive that rounds to
/// 0). The f64 LP (`find_positive_place_subvariant`) may *suggest* `y`; this
/// checker re-verifies, against the original net and in exact arithmetic, that
/// every weight is strictly positive and `yᵀ·C ≤ 0` on every transition column.
///
/// **M5 wiring (no longer dead code):** the positive structural-boundedness answer
/// of [`PetriNet::is_bounded`](crate::prelude::PetriNet::is_bounded) is now routed
/// through this exact certificate — the f64 LP suggests `y`, this checker validates
/// it exactly, and a failed validation falls back to the exact coverability graph
/// (`PetriNet::structurally_bounded_exact`). The verdict no longer rests on the
/// `f64` `PositivePlaceSubvariant`. The remaining A4 completion — routing the
/// *live* `is_efficiently_bounded` free-choice decider through an emitted
/// certificate (the S-component cover, B3) — is M6.
///
/// `query` is [`Query::Trivial`] — structural boundedness is a query-free
/// property.
#[derive(Debug, Clone)]
pub struct BoundednessSubinvariantCert {
    /// The exact place weighting `y`, indexed by [`Place`]. A place absent from
    /// the map is treated as weight 0 — but the positivity check requires every
    /// place of the net to be present with a strictly positive weight, so a
    /// missing place fails the check (a *positive* sub-invariant covers the net).
    pub weights: HashMap<Place, Rational>,
}

impl Certificate for BoundednessSubinvariantCert {
    /// Positive: a positive place sub-invariant witnesses that the net *is*
    /// structurally bounded.
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }

    fn check(&self, net: &Net, _m0: &Marking<u32>, query: &Query) -> bool {
        if !matches!(query, Query::Trivial) {
            return false;
        }
        // y must be strictly positive on every place of the net (a *positive*
        // place sub-invariant — the whole-net structural-boundedness witness).
        for p in net.places() {
            match self.weights.get(&p) {
                Some(w) if *w > Rational::ZERO => {}
                _ => return false,
            }
        }
        // yᵀ·C ≤ 0 on every transition column, exactly. C[p][t] is +1 for p∈t•,
        // −1 for p∈•t (weight-1 arcs); so the column sum is
        // Σ_{p∈t•} y[p] − Σ_{p∈•t} y[p].
        for t in net.transitions() {
            let mut delta = Rational::ZERO;
            for p in net.transition_postset(&t) {
                let Some(w) = self.weights.get(&p) else { return false };
                let Ok(acc) = delta.checked_add(*w) else { return false };
                delta = acc;
            }
            for p in net.transition_preset(&t) {
                let Some(w) = self.weights.get(&p) else { return false };
                let Ok(acc) = delta.checked_sub(*w) else { return false };
                delta = acc;
            }
            // yᵀ·C[·][t] must be ≤ 0.
            if delta > Rational::ZERO {
                return false;
            }
        }
        true
    }
}

/// A4 (per-place) — a **per-place structural-boundedness** witness: an exact
/// *semi-positive* place sub-invariant `y` with `y ≥ 0`, `y[place] ≥ 1`, and
/// `yᵀ·C ≤ 0`.
///
/// A single place `p` is structurally bounded (bounded under *every* initial
/// marking) iff there is a semi-positive place weighting `y ≥ 0` with `y[p] ≥ 1`
/// and `yᵀ·C ≤ 0`: the weighted token count `y·M` cannot increase across any
/// firing, so `y[p]·M[p] ≤ y·M ≤ y·M₀` and `p` is bounded by `⌊(y·M₀) / y[p]⌋`
/// (Murata Table 5; the per-place specialization of the whole-net positive
/// sub-invariant). This is the per-place twin of [`BoundednessSubinvariantCert`],
/// relaxing the positivity requirement to *the witness place only*.
///
/// The witness is **exact** ([`Rational`] weights): an `f64` weight vector is not
/// a certificate (a near-boundary `yᵀ·C` could be a small positive that rounds to
/// 0). The f64 LP (`find_semipositive_place_subvariant`) may *suggest* `y`; this
/// checker re-verifies, against the original net and in exact arithmetic, that
/// every weight is `≥ 0`, the witness place's weight is `≥ 1`, and `yᵀ·C ≤ 0` on
/// every transition column.
///
/// `query` is [`Query::Trivial`] — structural boundedness is a query-free
/// property.
#[derive(Debug, Clone)]
pub struct PlaceBoundednessSubinvariantCert {
    /// The place whose structural boundedness is certified. Its weight must be
    /// strictly positive (`≥ 1`); every other weight is `≥ 0`.
    pub place: Place,
    /// The exact place weighting `y`, indexed by [`Place`]. A place absent from
    /// the map is treated as weight 0 (the semi-positive default); the witness
    /// place must be present with a weight `≥ 1`.
    pub weights: HashMap<Place, Rational>,
}

impl Certificate for PlaceBoundednessSubinvariantCert {
    /// Positive: a semi-positive place sub-invariant witnesses that the witness
    /// place *is* structurally bounded.
    fn polarity(&self) -> Polarity {
        Polarity::ProveYes
    }

    fn check(&self, net: &Net, _m0: &Marking<u32>, query: &Query) -> bool {
        if !matches!(query, Query::Trivial) {
            return false;
        }
        // The witness place must belong to the net and carry a weight ≥ 1.
        if !net.places().any(|p| p == self.place) {
            return false;
        }
        match self.weights.get(&self.place) {
            Some(w) if *w >= Rational::ONE => {}
            _ => return false,
        }
        // Every other place's weight must be ≥ 0 (semi-positive). A place absent
        // from the map defaults to 0, which is admissible here (unlike the
        // whole-net positive cert). A present weight must be non-negative.
        for w in self.weights.values() {
            if *w < Rational::ZERO {
                return false;
            }
        }
        // yᵀ·C ≤ 0 on every transition column, exactly. The column sum is
        // Σ_{p∈t•} y[p] − Σ_{p∈•t} y[p] (weight-1 arcs); a place absent from the
        // map contributes 0.
        for t in net.transitions() {
            let mut delta = Rational::ZERO;
            for p in net.transition_postset(&t) {
                if let Some(w) = self.weights.get(&p) {
                    let Ok(acc) = delta.checked_add(*w) else { return false };
                    delta = acc;
                }
            }
            for p in net.transition_preset(&t) {
                if let Some(w) = self.weights.get(&p) {
                    let Ok(acc) = delta.checked_sub(*w) else { return false };
                    delta = acc;
                }
            }
            if delta > Rational::ZERO {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoundednessSubinvariantCert, FiringSequenceCert, ParikhVectorCert,
        PlaceBoundednessSubinvariantCert, SiphonTrapClaim, SiphonTrapCoverCert,
        TokenConservationCert,
    };
    use crate::model::certificate::Certificate;
    use crate::model::query::Query;
    use crate::prelude::{Marking, Net, NetBuilder, Place, Transition};
    use ahash::{HashMap, HashSet};

    /// Two-place cycle p0 -> t0 -> p1 -> t1 -> p0 (a state machine / circuit).
    fn two_place_cycle() -> (Net, Place, Transition, Place, Transition) {
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

    /// A marked graph: t0 forks into p0,p2; both join at t1; t1 returns to start.
    /// `t0: {pa} -> {p0, p2}`, `t1: {p0, p2} -> {pa}`.
    fn marked_graph() -> (Net, [Place; 3], [Transition; 2]) {
        let mut b = NetBuilder::new();
        let [pa, p0, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((pa, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p2));
        b.add_arc((p0, t1));
        b.add_arc((p2, t1));
        b.add_arc((t1, pa));
        let net = b.build().expect("valid net");
        (net, [pa, p0, p2], [t0, t1])
    }

    // --- FiringSequenceCert ---

    #[test]
    fn firing_sequence_accepts_the_reached_target() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = FiringSequenceCert { sequence: Box::new([t0]) };
        // Firing t0 moves the token p0 -> p1.
        let query = Query::Reachable([(p1, 1)].into());
        assert!(cert.check(&net, &m0, &query));
    }

    #[test]
    fn firing_sequence_rejects_a_wrong_target() {
        let (net, p0, t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = FiringSequenceCert { sequence: Box::new([t0]) };
        // The sequence does not reach [(p0,1)] (the token left p0).
        let query = Query::Reachable([(p0, 1)].into());
        assert!(!cert.check(&net, &m0, &query));
    }

    #[test]
    fn firing_sequence_rejects_an_unfireable_sequence() {
        let (net, p0, _t0, _p1, t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        // t1 is not enabled from [(p0,1)] (its input p1 is empty).
        let cert = FiringSequenceCert { sequence: Box::new([t1]) };
        let query = Query::Reachable([(p0, 1)].into());
        assert!(!cert.check(&net, &m0, &query));
    }

    #[test]
    fn firing_sequence_covers_for_a_coverable_query() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = FiringSequenceCert { sequence: Box::new([t0]) };
        // Reaches [(p1,1)], which covers the empty/zero target trivially and the
        // (p1,1) target exactly.
        assert!(cert.check(&net, &m0, &Query::Coverable([(p1, 1)].into())));
        // Does NOT cover (p1, 2): only one token reached.
        assert!(!cert.check(&net, &m0, &Query::Coverable([(p1, 2)].into())));
    }

    #[test]
    fn firing_sequence_rejects_trivial_query() {
        let (net, p0, t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = FiringSequenceCert { sequence: Box::new([t0]) };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    // --- ParikhVectorCert ---

    #[test]
    fn parikh_accepts_a_valid_integer_solution() {
        let (net, places, [t0, t1]) = marked_graph();
        let [pa, p0, p2] = places;
        let m0: Marking<u32> = [(pa, 1)].into();
        // Fire t0 once: pa -> {p0, p2}. The reached marking is {p0:1, p2:1}.
        let mut counts = HashMap::default();
        counts.insert(t0, 1);
        let cert = ParikhVectorCert { firing_counts: counts };
        let query = Query::Reachable([(p0, 1), (p2, 1)].into());
        assert!(cert.check(&net, &m0, &query));
        // t1 fired zero times is implicit; its inclusion at zero changes nothing.
        let mut counts0 = HashMap::default();
        counts0.insert(t0, 1);
        counts0.insert(t1, 0);
        let cert0 = ParikhVectorCert { firing_counts: counts0 };
        assert!(cert0.check(&net, &m0, &query));
    }

    #[test]
    fn parikh_rejects_a_wrong_displacement() {
        let (net, places, [t0, _t1]) = marked_graph();
        let [pa, p0, p2] = places;
        let m0: Marking<u32> = [(pa, 1)].into();
        let mut counts = HashMap::default();
        counts.insert(t0, 1);
        let cert = ParikhVectorCert { firing_counts: counts };
        // The true reached marking is {p0:1, p2:1}; claiming {p0:1} alone fails.
        assert!(!cert.check(&net, &m0, &Query::Reachable([(p0, 1)].into())));
        // And a target that double-counts p0 fails.
        assert!(!cert.check(&net, &m0, &Query::Reachable([(p0, 2), (p2, 1)].into())));
    }

    #[test]
    fn parikh_rejects_non_reachable_query() {
        let (net, places, [t0, _t1]) = marked_graph();
        let [pa, _p0, _p2] = places;
        let m0: Marking<u32> = [(pa, 1)].into();
        let mut counts = HashMap::default();
        counts.insert(t0, 1);
        let cert = ParikhVectorCert { firing_counts: counts };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
        assert!(!cert.check(&net, &m0, &Query::Coverable([(pa, 1)].into())));
    }

    /// BLOCKER 1 — the soundness fix, pinned. The marking equation is only
    /// *necessary* in general: a Parikh vector whose endpoint equation
    /// `M₀ + C·σ = target` balances to a non-negative target can still be
    /// **unrealizable** (no firing order enables every needed step), so the target
    /// is genuinely unreachable. The old checker accepted on a class fence alone
    /// and would mint a false `Proven`; the realization-based checker rejects,
    /// because σ never realizes.
    ///
    /// Net (free-choice): `u0: pa→pb`, `u1: {pb,pd}→{pc,pd}` (u1 needs and returns
    /// `pd`). From `m0={pa:1}` with no `pd`, u0 fires (→`{pb:1}`) but u1 can never
    /// fire (its input `pd` is never marked). σ=(u0:1,u1:1) balances the endpoint
    /// equation to `{pc:1}` (pa−1, pb +1−1=0, pc +1, pd +1−1=0), yet `{pc:1}` is
    /// **unreachable** — the library oracle agrees.
    #[test]
    fn parikh_rejects_an_unrealizable_balanced_witness() {
        let mut b = NetBuilder::new();
        let [pa, pb, pc, pd] = b.add_places();
        let [u0, u1] = b.add_transitions();
        b.add_arc((pa, u0)); b.add_arc((u0, pb));
        b.add_arc((pb, u1)); b.add_arc((pd, u1)); b.add_arc((u1, pc)); b.add_arc((u1, pd));
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(pa, 1)].into();
        let target: Marking<u32> = [(pc, 1)].into();
        let mut counts = HashMap::default();
        counts.insert(u0, 1);
        counts.insert(u1, 1);
        let cert = ParikhVectorCert { firing_counts: counts };
        // The endpoint equation balances, but σ is unrealizable → reject.
        assert!(
            !cert.check(&net, &m0, &Query::Reachable(target.clone())),
            "an unrealizable Parikh vector must not certify reachability"
        );
        // The library oracle confirms the target is genuinely unreachable.
        let sys = net.with_initial_marking([(pa, 1)]);
        assert!(!sys.is_reachable(&target), "oracle: pc=1 is unreachable");
    }

    /// BLOCKER 1 — the decisive oracle counterexample, pinned. A bounded,
    /// strongly-connected but **non-live** marked graph: the `pa↔pb` circuit is
    /// unmarked, so `t0`/`t1` are dead; only the `pc/pd` circuit holds a token.
    /// The marking equation `{pd:1} + C·(t0:1,t1:1) = {pc:1}` balances, but
    /// `{pc:1}` is genuinely unreachable (the oracle reports `Unreachable`). The
    /// old class-fenced checker minted a false `Proven`; the realization-based
    /// checker rejects.
    #[test]
    fn parikh_rejects_nonlive_marked_graph_unreachable_target() {
        let mut b = NetBuilder::new();
        let [pa, pb, pc, pd] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        // circuit A: t0→pa→t1→pb→t0  (left UNMARKED → t0,t1 dead → not live)
        b.add_arc((t0, pa)); b.add_arc((pa, t1)); b.add_arc((t1, pb)); b.add_arc((pb, t0));
        // circuit B: t1→pc→t2→pd→t0  (token on pd)
        b.add_arc((t1, pc)); b.add_arc((pc, t2)); b.add_arc((t2, pd)); b.add_arc((pd, t0));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), crate::class::NetClass::MarkedGraph);
        let m0: Marking<u32> = [(pd, 1)].into();
        let target: Marking<u32> = [(pc, 1)].into();
        let sys = net.with_initial_marking([(pd, 1)]);
        assert!(sys.is_bounded(), "oracle: the net is bounded");
        assert!(!sys.is_live(), "oracle: the unmarked pa↔pb circuit makes it non-live");
        assert!(!sys.is_reachable(&target), "oracle: pc=1 is unreachable");
        // σ=(t0:1,t1:1) balances the endpoint equation but is unrealizable (both
        // are dead): the checker must reject, not mint a false Proven.
        let mut counts = HashMap::default();
        counts.insert(t0, 1);
        counts.insert(t1, 1);
        let cert = ParikhVectorCert { firing_counts: counts };
        assert!(
            !cert.check(&net, &m0, &Query::Reachable(target)),
            "BLOCKER 1: a balanced σ on a non-live marked graph must not certify an \
             unreachable target"
        );
    }

    /// Capability is preserved (and soundly *extended*): a Parikh witness that
    /// genuinely realizes is accepted even on a `General`-class net, because a
    /// realized σ *is* a firing sequence — a self-contained reachability proof with
    /// no class assumption to trust.
    #[test]
    fn parikh_accepts_a_realizable_witness_on_a_general_net() {
        let mut b = NetBuilder::new();
        let [pa, pb, pc, pd, pe] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((pa, t0)); b.add_arc((t0, pb));
        b.add_arc((pa, t1)); b.add_arc((pc, t1)); b.add_arc((t1, pd));
        b.add_arc((pc, t2)); b.add_arc((t2, pe));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), crate::class::NetClass::General);
        let m0: Marking<u32> = [(pa, 1), (pc, 1)].into();
        // σ = {t0: 1}: fire t0 once → {pb:1, pc:1}. A genuine firing sequence.
        let mut counts = HashMap::default();
        counts.insert(t0, 1);
        let cert = ParikhVectorCert { firing_counts: counts };
        let target: Marking<u32> = [(pb, 1), (pc, 1)].into();
        assert!(
            cert.check(&net, &m0, &Query::Reachable(target.clone())),
            "a realizable Parikh witness certifies reachability on any class"
        );
        // The oracle agrees.
        let sys = net.with_initial_marking([(pa, 1), (pc, 1)]);
        assert!(sys.is_reachable(&target));
        let _ = (pd, pe);
    }

    // --- TokenConservationCert ---

    #[test]
    fn token_conservation_accepts_a_sum_mismatch_on_a_state_machine() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = TokenConservationCert;
        // The cycle conserves one token; a target with two tokens is unreachable.
        assert!(cert.check(&net, &m0, &Query::Reachable([(p0, 2)].into())));
        // A zero-token target also differs in sum (1 != 0): unreachable.
        assert!(cert.check(&net, &m0, &Query::Reachable([].into())));
    }

    #[test]
    fn token_conservation_rejects_an_equal_sum() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = TokenConservationCert;
        // [(p1,1)] has the same sum (1): token conservation does NOT prove it
        // unreachable, so the checker must reject (it is in fact reachable).
        assert!(!cert.check(&net, &m0, &Query::Reachable([(p1, 1)].into())));
    }

    #[test]
    fn token_conservation_rejects_a_non_state_machine() {
        // A marked graph: t0 has two output places, so |t0•| = 2 ≠ 1: not an
        // S-net. Token conservation does not apply; reject even on a sum mismatch.
        let (net, places, _ts) = marked_graph();
        let [pa, _p0, _p2] = places;
        let m0: Marking<u32> = [(pa, 1)].into();
        let cert = TokenConservationCert;
        // Sum of m0 is 1; target sum 5 differs — but the net is not an S-net, so
        // the conservation theorem does not license the verdict.
        assert!(!cert.check(&net, &m0, &Query::Reachable([(pa, 5)].into())));
    }

    // --- SiphonTrapCoverCert ---

    /// A live two-place cycle: the whole place set {p0, p1} is both a siphon and
    /// a trap, and it is marked (p0 holds the token). This is a valid liveness /
    /// deadlock-freedom cover.
    #[test]
    fn siphon_trap_cover_accepts_a_marked_self_covering_set() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut set = HashSet::default();
        set.insert(p0);
        set.insert(p1);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(set.clone(), set)]),
        };
        assert!(cert.check(&net, &m0, &Query::Trivial));
    }

    #[test]
    fn siphon_trap_cover_rejects_an_unmarked_trap() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        // No tokens anywhere: the trap is unmarked, so it does NOT certify
        // liveness (the net is in fact dead).
        let m0: Marking<u32> = [].into();
        let mut set = HashSet::default();
        set.insert(p0);
        set.insert(p1);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(set.clone(), set)]),
        };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    #[test]
    fn siphon_trap_cover_rejects_a_non_siphon() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        // {p1} alone is not a siphon in the cycle: t0 produces into p1
        // (t0 ∈ •{p1}) but does not consume from {p1} (•t0 = {p0}), so
        // •{p1} ⊄ {p1}•. A cover claiming {p1} as both siphon and trap must be
        // rejected at the is-siphon step.
        let mut just_p1 = HashSet::default();
        just_p1.insert(p1);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(just_p1.clone(), just_p1)]),
        };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    #[test]
    fn siphon_trap_cover_rejects_an_empty_cover() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([]),
        };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    #[test]
    fn siphon_trap_cover_rejects_a_non_trivial_query() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut set = HashSet::default();
        set.insert(p0);
        set.insert(p1);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(set.clone(), set)]),
        };
        assert!(!cert.check(&net, &m0, &Query::Reachable([(p1, 1)].into())));
    }

    /// BLOCKER 2 — the incomplete-cover soundness fix, pinned. The Commoner–Hack
    /// criterion is a *universal*: EVERY minimal siphon must contain a marked trap.
    /// A cover that exhibits only the well-behaved siphon `{p0,p1}` (a genuine
    /// marked self-trap) while omitting the offending unmarked siphon would pass
    /// the per-pair checks, but the net is **not live**. The fix re-establishes the
    /// universal from primitives (re-enumerating the minimal siphons), so the
    /// incomplete cover is rejected.
    ///
    /// Net: the good marked cycle `pa↔pb` (via t0,t1), an UNMARKED cycle `qa→qb→qa`
    /// (via u0,u1), and a coupling that makes `t0` consume `qb`/`u1` produce `qb`
    /// so the q-cycle is a genuine siphon left unmarked — the offender. The oracle
    /// reports `is_live == false`.
    #[test]
    fn siphon_trap_cover_rejects_an_incomplete_cover() {
        let mut b = NetBuilder::new();
        let [pa, pb, qa, qb] = b.add_places();
        let [t0, t1, u0, u1] = b.add_transitions();
        // good marked cycle: pa→t0→pb→t1→pa
        b.add_arc((pa, t0)); b.add_arc((t0, pb)); b.add_arc((pb, t1)); b.add_arc((t1, pa));
        // unmarked cycle: qa→u0→qb→u1→qa
        b.add_arc((qa, u0)); b.add_arc((u0, qb)); b.add_arc((qb, u1)); b.add_arc((u1, qa));
        // coupling: t0 also needs qb; t1 returns a token to qb. With the q-cycle
        // unmarked, qb is empty → t0 dead → the net is not live, and {qa,qb} is an
        // unmarked siphon (the offender the cover omits).
        b.add_arc((qb, t0)); b.add_arc((t1, qb));
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(pa, 1)].into(); // q-cycle unmarked
        let sys = net.with_initial_marking([(pa, 1)]);
        assert!(!sys.is_live(), "oracle: the net is not live (unmarked q-siphon)");

        // The exhibited pair {pa,pb} is a genuine siphon and a genuine marked trap;
        // it passes the per-pair checks. But the cover is INCOMPLETE.
        let mk = |ps: &[Place]| -> HashSet<Place> { ps.iter().copied().collect() };
        let good = mk(&[pa, pb]);
        assert!(
            SiphonTrapCoverCert::is_siphon(&net, &good)
                && SiphonTrapCoverCert::is_trap(&net, &good)
                && SiphonTrapCoverCert::is_marked(&m0, &good),
            "the exhibited pair is individually valid"
        );
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(good.clone(), good)]),
        };
        // The universal re-establishment catches the omitted unmarked siphon.
        assert!(
            !cert.check(&net, &m0, &Query::Trivial),
            "BLOCKER 2: an incomplete cover must not certify liveness on a non-live net"
        );
    }

    /// The complement to BLOCKER 2: on a genuinely live net, a complete cover
    /// (here the single self-covering set of the live cycle) is still ACCEPTED —
    /// the universal holds, so capability is preserved.
    #[test]
    fn siphon_trap_cover_accepts_when_the_universal_holds() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_live(), "oracle: the marked cycle is live");
        let mut set = HashSet::default();
        set.insert(p0);
        set.insert(p1);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(set.clone(), set)]),
        };
        assert!(
            cert.check(&net, &m0, &Query::Trivial),
            "a complete cover on a live net is accepted"
        );
    }

    /// An oracle-confirmed non-free-choice net where the Commoner–Hack universal
    /// holds (every minimal siphon has a marked trap), the net is bounded and
    /// deadlock-free, but it is **not live**. The universal is sufficient for
    /// liveness *only on free-choice nets* (Murata Thm 12 / Primer Thm 5.17, as the
    /// repo's own `commoner_hack_criterion` records); on a general net it proves
    /// deadlock-freedom only.
    ///
    /// Without the class fence, a `SiphonTrapCoverCert` whose universal holds would
    /// mint a false `Proven` **liveness** here — the cardinal sin. The `claim` +
    /// free-choice gate closes it: a [`SiphonTrapClaim::Live`] cert is **rejected**
    /// (the net is not free-choice), while a [`SiphonTrapClaim::DeadlockFree`] cert
    /// is **accepted** (the universal is sufficient for deadlock-freedom on any
    /// class, and the oracle confirms `is_deadlock_free`).
    ///
    /// Net (found by brute-force search against the oracle, then minimized):
    /// `t0: {p1,p3}→{p2}`, `t1: {p2,p3}→{p0,p1}`, `t2: {p2}→{p0,p1}`,
    /// `t3: {p0,p1}→{p1,p3}`; marked `{p0,p1,p3}`. Class `General` (`t1` and `t2`
    /// share input `p2` with different presets ⇒ not free-choice).
    #[test]
    fn siphon_trap_cover_rejects_liveness_on_a_non_free_choice_net() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        let [t0, t1, t2, t3] = b.add_transitions();
        b.add_arc((p1, t0)); b.add_arc((p3, t0)); b.add_arc((t0, p2));
        b.add_arc((p2, t1)); b.add_arc((p3, t1)); b.add_arc((t1, p0)); b.add_arc((t1, p1));
        b.add_arc((p2, t2)); b.add_arc((t2, p0)); b.add_arc((t2, p1));
        b.add_arc((p0, t3)); b.add_arc((p1, t3)); b.add_arc((t3, p1)); b.add_arc((t3, p3));
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(p0, 1), (p1, 1), (p3, 1)].into();

        // Oracle ground truth.
        assert!(!net.class().is_free_choice(), "the net is non-free-choice");
        let sys = net.with_initial_marking([(p0, 1), (p1, 1), (p3, 1)]);
        assert!(sys.is_bounded(), "oracle: bounded");
        assert!(sys.is_deadlock_free(), "oracle: deadlock-free");
        assert_eq!(
            sys.try_liveness().map(|l| l.is_live()),
            Some(false),
            "oracle: NOT live"
        );

        // The universal genuinely holds on this net — so the deadlock-freedom claim
        // is sound, and only the (unsound-here) liveness claim must be rejected.
        assert!(
            SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
            "the CHC universal holds (sufficient for deadlock-freedom on any class)"
        );

        // A genuine marked siphon-trap cover (the whole place set is a siphon and a
        // marked trap on this strongly-connected net), exhibited for both claims.
        let whole: HashSet<Place> = [p0, p1, p2, p3].into_iter().collect();
        let make = |claim| SiphonTrapCoverCert {
            claim,
            cover: Box::new([(whole.clone(), whole.clone())]),
        };

        // LIVENESS on a non-free-choice net: REJECTED (the cardinal-sin guard).
        assert!(
            !make(SiphonTrapClaim::Live).check(&net, &m0, &Query::Trivial),
            "BLOCKER 2 (polarity): a Live claim must be rejected on a non-free-choice \
             net even when the CHC universal holds — the net is not live"
        );
        // DEADLOCK-FREEDOM: ACCEPTED (sufficient on any class; the oracle agrees).
        assert!(
            make(SiphonTrapClaim::DeadlockFree).check(&net, &m0, &Query::Trivial),
            "a DeadlockFree claim is accepted: the universal is sufficient for \
             deadlock-freedom on any class"
        );

        // And the same Live cover is ACCEPTED once routed through accept only on a
        // free-choice net (capability preserved) — covered by
        // `siphon_trap_cover_accepts_when_the_universal_holds`.
    }

    /// ADVERSARIAL RE-REVIEW (independent): a fresh deadlocking net distinct from
    /// the existing fixtures. Two free-choice "lock" cycles share a guard place `g`
    /// that is consumed but never replenished while empty, so after the guard is
    /// spent the second cycle is dead: the net deadlocks. The second cycle's place
    /// set forms an UNMARKED siphon (the offender). A cover exhibiting only the
    /// first, genuinely-marked cycle as a self-trap omits the offender. The
    /// universal re-enumeration in `check` must catch it.
    #[test]
    fn adversarial_incomplete_cover_on_a_fresh_deadlocking_net() {
        let mut b = NetBuilder::new();
        // Cycle A (marked, live in isolation): a0 -> ta0 -> a1 -> ta1 -> a0
        // Cycle B (unmarked): b0 -> tb0 -> b1 -> tb1 -> b0
        // Coupling: tc drains B into A (b1 -> tc -> a0), making the net connected
        // while keeping {b0,b1} a genuine siphon (its producers tb0,tb1 also consume
        // from it; the new consumer tc does not violate the siphon property).
        let [a0, a1, b0, b1] = b.add_places();
        let [ta0, ta1, tb0, tb1, tc] = b.add_transitions();
        b.add_arc((a0, ta0)); b.add_arc((ta0, a1)); b.add_arc((a1, ta1)); b.add_arc((ta1, a0));
        b.add_arc((b0, tb0)); b.add_arc((tb0, b1)); b.add_arc((b1, tb1)); b.add_arc((tb1, b0));
        // tc synchronizes the two cycles: consumes a1 and b1, returns to a0 and b0.
        // This preserves BOTH {a0,a1} and {b0,b1} as siphons-and-traps (tc consumes
        // from each set and produces back into each set), while connecting the net.
        b.add_arc((a1, tc)); b.add_arc((b1, tc)); b.add_arc((tc, a0)); b.add_arc((tc, b0));
        let net = b.build().expect("valid net");
        // Mark only cycle A. Cycle B's place set {b0,b1} is a genuine siphon-and-trap
        // but UNMARKED → tb0,tb1,tc are dead → not live (and the unmarked siphon's
        // maximal trap is unmarked).
        let m0: Marking<u32> = [(a0, 1)].into();
        let sys = net.with_initial_marking([(a0, 1)]);
        assert!(!sys.is_live(), "oracle: cycle B is dead → not live");

        // The exhibited pair {a0,a1} is a genuine marked siphon-trap; it passes the
        // per-pair checks. But it OMITS the offending unmarked siphon {b0,b1}.
        let mk = |ps: &[Place]| -> HashSet<Place> { ps.iter().copied().collect() };
        let good = mk(&[a0, a1]);
        assert!(
            SiphonTrapCoverCert::is_siphon(&net, &good)
                && SiphonTrapCoverCert::is_trap(&net, &good)
                && SiphonTrapCoverCert::is_marked(&m0, &good),
            "the exhibited pair is individually valid"
        );
        // The universal genuinely FAILS (the unmarked B-siphon has no marked trap).
        assert!(
            !SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
            "the CHC universal must fail: {{b0,b1}} is an unmarked siphon"
        );
        // Both claims must be REJECTED on this net: Live (not free-choice-live) and
        // DeadlockFree (the universal that backs it does not hold).
        for claim in [SiphonTrapClaim::Live, SiphonTrapClaim::DeadlockFree] {
            let cert = SiphonTrapCoverCert { claim, cover: Box::new([(good.clone(), good.clone())]) };
            assert!(
                !cert.check(&net, &m0, &Query::Trivial),
                "an incomplete cover omitting the unmarked offender must be rejected ({claim:?})"
            );
        }
    }

    /// ADVERSARIAL RE-REVIEW (independent): the genuinely-live complement. Same
    /// synchronized two-cycle topology as the deadlocking case, but now BOTH cycles
    /// are marked → live → the universal holds → a complete cover is accepted,
    /// confirming the universal re-enumeration does not over-reject (capability
    /// preserved). The synchronizer makes the net non-free-choice, so a `Live` claim
    /// is fenced off (correctly — CHC backs liveness only on free-choice nets) while
    /// `DeadlockFree` is accepted. A separate free-choice instance below confirms a
    /// `Live` claim is accepted where it is sound.
    #[test]
    fn adversarial_complete_cover_on_a_fresh_live_net() {
        let mut b = NetBuilder::new();
        let [a0, a1, b0, b1] = b.add_places();
        let [ta0, ta1, tb0, tb1, tc] = b.add_transitions();
        b.add_arc((a0, ta0)); b.add_arc((ta0, a1)); b.add_arc((a1, ta1)); b.add_arc((ta1, a0));
        b.add_arc((b0, tb0)); b.add_arc((tb0, b1)); b.add_arc((b1, tb1)); b.add_arc((tb1, b0));
        b.add_arc((a1, tc)); b.add_arc((b1, tc)); b.add_arc((tc, a0)); b.add_arc((tc, b0));
        let net = b.build().expect("valid net");
        // Mark BOTH cycles → both are live; the universal holds.
        let m0: Marking<u32> = [(a0, 1), (b0, 1)].into();
        let sys = net.with_initial_marking([(a0, 1), (b0, 1)]);
        assert!(sys.is_live(), "oracle: both cycles marked → live");
        assert!(
            SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
            "the CHC universal holds"
        );
        let mk = |ps: &[Place]| -> HashSet<Place> { ps.iter().copied().collect() };
        // A complete cover: both cycles' place sets (each a marked self-trap-siphon).
        let cover: Box<[(HashSet<Place>, HashSet<Place>)]> = Box::new([
            (mk(&[a0, a1]), mk(&[a0, a1])),
            (mk(&[b0, b1]), mk(&[b0, b1])),
        ]);
        // DeadlockFree is sound on any class and the universal holds → accepted.
        let cert = SiphonTrapCoverCert { claim: SiphonTrapClaim::DeadlockFree, cover };
        assert!(
            cert.check(&net, &m0, &Query::Trivial),
            "a complete cover on a live net (universal holds) must be accepted for DeadlockFree"
        );
    }

    /// ADVERSARIAL RE-REVIEW (independent): a `Live` claim is ACCEPTED on a fresh
    /// free-choice live net (a 3-place circuit, distinct from the 2-place fixture),
    /// and even an incomplete cover is accepted because the verdict rests on the
    /// re-derived universal, not on the exhibited cover's completeness.
    #[test]
    fn adversarial_live_claim_accepted_on_a_fresh_free_choice_net() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p2));
        b.add_arc((p2, t2)); b.add_arc((t2, p0));
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(p0, 1)].into();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(net.class().is_free_choice(), "a circuit is free-choice");
        assert!(sys.is_live(), "oracle: a marked circuit is live");
        let mk = |ps: &[Place]| -> HashSet<Place> { ps.iter().copied().collect() };
        // The whole set is the single minimal siphon (and a marked trap).
        let whole = mk(&[p0, p1, p2]);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::Live,
            cover: Box::new([(whole.clone(), whole)]),
        };
        assert!(
            cert.check(&net, &m0, &Query::Trivial),
            "a Live claim on a live free-choice net must be accepted"
        );
    }

    /// ADVERSARIAL RE-REVIEW (independent, brute-force oracle): the load-bearing
    /// soundness claim of the BLOCKER 2 fix is that
    /// `every_minimal_siphon_has_a_marked_trap` re-establishes the UNIVERSAL "every
    /// proper siphon contains a marked trap" — i.e. that restricting to *minimal*
    /// siphons is sound (a marked trap inside a minimal siphon S' ⊆ S is also a
    /// marked trap inside S), and that the underlying `minimal_siphons`/
    /// `maximal_trap_in` primitives are correct.
    ///
    /// This cross-checks the checker's universal against a fully independent
    /// brute-force enumeration of *all* 2^|P|−1 non-empty place subsets, using only
    /// primitive pre/post-set access (sharing no code with `minimal_siphons`). For
    /// each net we assert: checker-universal == (every non-empty siphon contains a
    /// non-empty marked trap). They must agree on every net.
    // A battery of independent net cases in one test; the per-net blocks are the
    // body length, not a single complex routine.
    #[expect(clippy::too_many_lines)]
    #[test]
    fn brute_force_universal_matches_the_checker_on_a_battery_of_nets() {
        // Brute-force: is `set` a siphon? •set ⊆ set• (primitive access only).
        fn bf_is_siphon(net: &Net, set: &HashSet<Place>) -> bool {
            net.transitions().all(|t| {
                let produces = net.transition_postset(&t).any(|p| set.contains(&p));
                let consumes = net.transition_preset(&t).any(|p| set.contains(&p));
                !produces || consumes
            })
        }
        // Brute-force: is `set` a trap? set• ⊆ •set.
        fn bf_is_trap(net: &Net, set: &HashSet<Place>) -> bool {
            net.transitions().all(|t| {
                let consumes = net.transition_preset(&t).any(|p| set.contains(&p));
                let produces = net.transition_postset(&t).any(|p| set.contains(&p));
                !consumes || produces
            })
        }
        // Brute-force universal: every non-empty siphon contains a non-empty marked
        // trap. Enumerate ALL non-empty subsets; for each siphon, enumerate ALL its
        // non-empty subsets looking for a marked trap.
        fn bf_universal(net: &Net, m0: &Marking<u32>) -> bool {
            let places: Vec<Place> = net.places().collect();
            let n = places.len();
            assert!(n <= 12, "brute force only on small nets");
            let subset = |mask: u32| -> HashSet<Place> {
                (0..n).filter(|i| mask & (1 << i) != 0).map(|i| places[i]).collect()
            };
            for mask in 1u32..(1 << n) {
                let s = subset(mask);
                if !bf_is_siphon(net, &s) {
                    continue;
                }
                // Does some non-empty subset of s form a marked trap?
                let s_bits: Vec<u32> =
                    (0..n).filter(|i| mask & (1 << i) != 0).map(|i| 1u32 << i).collect();
                let k = s_bits.len();
                let mut has_marked_trap = false;
                'sub: for sub in 1u32..(1 << k) {
                    let mut tmask = 0u32;
                    for (j, &bit) in s_bits.iter().enumerate() {
                        if sub & (1 << j) != 0 {
                            tmask |= bit;
                        }
                    }
                    let t = subset(tmask);
                    if bf_is_trap(net, &t) && t.iter().any(|&p| m0.get(p) > 0) {
                        has_marked_trap = true;
                        break 'sub;
                    }
                }
                if !has_marked_trap {
                    return false; // an uncovered siphon ⇒ universal fails
                }
            }
            true
        }

        // 1. Live 2-cycle.
        {
            let (net, p0, _t0, _p1, _t1) = two_place_cycle();
            let m0: Marking<u32> = [(p0, 1)].into();
            assert_eq!(
                SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
                bf_universal(&net, &m0),
                "live 2-cycle"
            );
        }
        // 2. Dead 2-cycle (unmarked).
        {
            let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
            let m0: Marking<u32> = [].into();
            assert_eq!(
                SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
                bf_universal(&net, &m0),
                "dead 2-cycle"
            );
        }
        // 3. Marked graph.
        {
            let (net, places, _ts) = marked_graph();
            let [pa, _p0, _p2] = places;
            let m0: Marking<u32> = [(pa, 1)].into();
            assert_eq!(
                SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
                bf_universal(&net, &m0),
                "marked graph"
            );
        }
        // 4. Synchronized two-cycle (the deadlocking adversarial net), both markings.
        {
            let mut b = NetBuilder::new();
            let [a0, a1, b0, b1] = b.add_places();
            let [ta0, ta1, tb0, tb1, tc] = b.add_transitions();
            b.add_arc((a0, ta0)); b.add_arc((ta0, a1)); b.add_arc((a1, ta1)); b.add_arc((ta1, a0));
            b.add_arc((b0, tb0)); b.add_arc((tb0, b1)); b.add_arc((b1, tb1)); b.add_arc((tb1, b0));
            b.add_arc((a1, tc)); b.add_arc((b1, tc)); b.add_arc((tc, a0)); b.add_arc((tc, b0));
            let net = b.build().expect("valid net");
            for m0 in [
                Marking::<u32>::from([(a0, 1)]),          // B unmarked → universal fails
                Marking::<u32>::from([(a0, 1), (b0, 1)]), // both marked → universal holds
            ] {
                assert_eq!(
                    SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
                    bf_universal(&net, &m0),
                    "synchronized two-cycle"
                );
            }
        }
        // 5. The non-free-choice oracle counterexample net.
        {
            let mut b = NetBuilder::new();
            let [p0, p1, p2, p3] = b.add_places();
            let [t0, t1, t2, t3] = b.add_transitions();
            b.add_arc((p1, t0)); b.add_arc((p3, t0)); b.add_arc((t0, p2));
            b.add_arc((p2, t1)); b.add_arc((p3, t1)); b.add_arc((t1, p0)); b.add_arc((t1, p1));
            b.add_arc((p2, t2)); b.add_arc((t2, p0)); b.add_arc((t2, p1));
            b.add_arc((p0, t3)); b.add_arc((p1, t3)); b.add_arc((t3, p1)); b.add_arc((t3, p3));
            let net = b.build().expect("valid net");
            let m0: Marking<u32> = [(p0, 1), (p1, 1), (p3, 1)].into();
            assert_eq!(
                SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
                bf_universal(&net, &m0),
                "non-free-choice counterexample"
            );
        }
        // 6. Unbounded producer net (no proper siphon, vacuous universal).
        {
            let mut b = NetBuilder::new();
            let [p0, p1] = b.add_places();
            let [t0] = b.add_transitions();
            b.add_arc((p0, t0)); b.add_arc((t0, p0)); b.add_arc((t0, p1));
            let net = b.build().expect("valid net");
            let m0: Marking<u32> = [(p0, 1)].into();
            assert_eq!(
                SiphonTrapCoverCert::every_minimal_siphon_has_a_marked_trap(&net, &m0),
                bf_universal(&net, &m0),
                "unbounded producer"
            );
        }
    }

    /// Capability preserved: a `DeadlockFree` claim on a free-choice live net is
    /// accepted (the gate is a fence on the *liveness* reading only, never on
    /// deadlock-freedom).
    #[test]
    fn siphon_trap_cover_accepts_deadlock_freedom_on_a_free_choice_net() {
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut set = HashSet::default();
        set.insert(p0);
        set.insert(p1);
        let cert = SiphonTrapCoverCert {
            claim: SiphonTrapClaim::DeadlockFree,
            cover: Box::new([(set.clone(), set)]),
        };
        assert!(cert.check(&net, &m0, &Query::Trivial));
    }

    // --- BoundednessSubinvariantCert (A4) ---

    /// The two-place cycle is conservative: the all-ones weighting `y = (1, 1)`
    /// is a positive place sub-invariant (`yᵀ·C = 0 ≤ 0`), certifying structural
    /// boundedness. The exact checker accepts it.
    #[test]
    fn boundedness_subinvariant_accepts_a_conservative_weighting() {
        use crate::core::analysis::rational::Rational;
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE);
        weights.insert(p1, Rational::ONE);
        let cert = BoundednessSubinvariantCert { weights };
        assert!(cert.check(&net, &m0, &Query::Trivial));
        // And the library agrees the net is bounded.
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_bounded());
    }

    /// A weighting that is not positive on every place (p1 missing) is rejected —
    /// a *positive* sub-invariant must cover the whole net.
    #[test]
    fn boundedness_subinvariant_rejects_a_non_covering_weighting() {
        use crate::core::analysis::rational::Rational;
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE); // p1 omitted → not positive everywhere
        let cert = BoundednessSubinvariantCert { weights };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    /// A weighting that fails `yᵀ·C ≤ 0` on an unbounded net is rejected: the
    /// producer `t0: {} -> {p0}` makes `y[p0]·(+1) > 0` for any positive `y[p0]`,
    /// so no positive sub-invariant exists and the checker cannot be fooled.
    #[test]
    fn boundedness_subinvariant_rejects_on_an_unbounded_net() {
        use crate::core::analysis::rational::Rational;
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1)); // pumps p1 without bound
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(p0, 1)].into();
        // Try to certify boundedness with the all-ones weighting: yᵀ·C on t0 is
        // (p0: +1−1=0) + (p1: +1) = +1 > 0, so the check must reject.
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE);
        weights.insert(p1, Rational::ONE);
        let cert = BoundednessSubinvariantCert { weights };
        assert!(!cert.check(&net, &m0, &Query::Trivial), "p1 is unbounded; no positive sub-invariant");
    }

    /// Query discipline: a structural-boundedness witness answers a query-free
    /// property, so a reachability query is rejected.
    #[test]
    fn boundedness_subinvariant_rejects_a_non_trivial_query() {
        use crate::core::analysis::rational::Rational;
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE);
        weights.insert(p1, Rational::ONE);
        let cert = BoundednessSubinvariantCert { weights };
        assert!(!cert.check(&net, &m0, &Query::Reachable([(p0, 1)].into())));
    }

    // --- PlaceBoundednessSubinvariantCert (A4 per-place, §4.6) ---

    /// The per-place cert is *semi-positive*: a place is structurally bounded if
    /// some `y ≥ 0` with `y[place] ≥ 1` and `yᵀ·C ≤ 0`. On the two-place cycle the
    /// all-ones weighting witnesses p0's boundedness (and p1 could be 0 here, but
    /// all-ones is also valid). The exact checker accepts it.
    #[test]
    fn place_boundedness_subinvariant_accepts_a_semipositive_weighting() {
        use crate::core::analysis::rational::Rational;
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE);
        weights.insert(p1, Rational::ONE);
        let cert = PlaceBoundednessSubinvariantCert { place: p0, weights };
        assert!(cert.check(&net, &m0, &Query::Trivial));
    }

    /// The witness place's weight must be `≥ 1`: a weighting with `y[place] = 0`
    /// does not bound that place (it is not in the support), so the checker rejects.
    #[test]
    fn place_boundedness_subinvariant_rejects_zero_weight_on_the_witness_place() {
        use crate::core::analysis::rational::Rational;
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        // p0 (the witness place) omitted → defaults to 0 → must reject.
        weights.insert(p1, Rational::ONE);
        let cert = PlaceBoundednessSubinvariantCert { place: p0, weights };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    /// A near-boundary `yᵀ·C` is rejected exactly: on the unbounded producer net
    /// (t0 pumps p1), no semi-positive sub-invariant with `y[p1] ≥ 1` can satisfy
    /// `yᵀ·C ≤ 0` (the column for t0 forces `y[p1] ≤ 0`). The all-ones weighting
    /// gives `+1 > 0` and is rejected — the exact check cannot be fooled by an f64
    /// rounding.
    #[test]
    fn place_boundedness_subinvariant_rejects_on_an_unbounded_place() {
        use crate::core::analysis::rational::Rational;
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1)); // pumps p1 without bound
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE);
        weights.insert(p1, Rational::ONE);
        let cert = PlaceBoundednessSubinvariantCert { place: p1, weights };
        assert!(
            !cert.check(&net, &m0, &Query::Trivial),
            "p1 is unbounded; no semi-positive sub-invariant with y[p1] ≥ 1 satisfies yᵀ·C ≤ 0"
        );
    }

    /// A negative weight anywhere is rejected — the cert is *semi-positive*
    /// (`y ≥ 0`), not arbitrary.
    #[test]
    fn place_boundedness_subinvariant_rejects_a_negative_weight() {
        use crate::core::analysis::rational::Rational;
        let (net, p0, _t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let mut weights = HashMap::default();
        weights.insert(p0, Rational::ONE);
        weights.insert(p1, Rational::new(-1, 1).unwrap());
        let cert = PlaceBoundednessSubinvariantCert { place: p0, weights };
        assert!(!cert.check(&net, &m0, &Query::Trivial));
    }

    // --- C2/C4 at the accept gate: a passing check is the only path to a
    //     decided verdict; a failing one collapses to Inconclusive. These need
    //     the `pub(crate)` `Decided` constructors, so they live in-crate. ---

    use crate::model::accept::{accept, Decided};
    use crate::model::verdict::Abstention;

    /// Each C1 checker, threaded through the real `accept` gate: a valid witness
    /// promotes the candidate; the verdict carries the proof. This is the C2
    /// "checking as the gate" property for every checker at once.
    #[test]
    fn every_checker_promotes_a_valid_candidate_through_accept() {
        // FiringSequence (positive reachability).
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();
        let v = accept(
            Decided::<&str, &str>::prove("reached p1"),
            &FiringSequenceCert { sequence: Box::new([t0]) },
            &net,
            &m0,
            &Query::Reachable([(p1, 1)].into()),
        );
        assert!(v.is_proven());

        // TokenConservation (negative reachability).
        let v = accept(
            Decided::<&str, &str>::refute("sum 1 != 2"),
            &TokenConservationCert,
            &net,
            &m0,
            &Query::Reachable([(p0, 2)].into()),
        );
        assert!(v.is_refuted());

        // SiphonTrapCover (liveness on a free-choice circuit).
        let mut set = HashSet::default();
        set.insert(p0);
        set.insert(p1);
        let v = accept(
            Decided::<&str, &str>::prove("marked-trap cover"),
            &SiphonTrapCoverCert {
                claim: SiphonTrapClaim::Live,
                cover: Box::new([(set.clone(), set)]),
            },
            &net,
            &m0,
            &Query::Trivial,
        );
        assert!(v.is_proven());

        // Parikh (positive reachability on a marked graph).
        let (mg, places, [mt0, _mt1]) = marked_graph();
        let [pa, mp0, mp2] = places;
        let mg_m0: Marking<u32> = [(pa, 1)].into();
        let mut counts = HashMap::default();
        counts.insert(mt0, 1);
        let v = accept(
            Decided::<&str, &str>::prove("M₀+C·σ = target"),
            &ParikhVectorCert { firing_counts: counts },
            &mg,
            &mg_m0,
            &Query::Reachable([(mp0, 1), (mp2, 1)].into()),
        );
        assert!(v.is_proven());
    }

    /// A rejected witness collapses to `Inconclusive(NotApplicable)` at the gate,
    /// never to the opposite verdict — the firewall, for each checker.
    #[test]
    fn every_checker_rejects_an_invalid_candidate_at_the_gate() {
        let (net, p0, _t0, p1, t1) = two_place_cycle();
        let m0: Marking<u32> = [(p0, 1)].into();

        // FiringSequence with an unfireable sequence (t1 not enabled).
        let v = accept(
            Decided::<u8, u8>::prove(1),
            &FiringSequenceCert { sequence: Box::new([t1]) },
            &net,
            &m0,
            &Query::Reachable([(p1, 1)].into()),
        );
        assert_eq!(v.abstention(), Some(Abstention::NotApplicable));
        assert!(!v.is_refuted());

        // TokenConservation where the sums are equal (not a refutation).
        let v = accept(
            Decided::<u8, u8>::refute(2),
            &TokenConservationCert,
            &net,
            &m0,
            &Query::Reachable([(p1, 1)].into()),
        );
        assert!(v.is_inconclusive());
        assert!(!v.is_proven());
    }
}
