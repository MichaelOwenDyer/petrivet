//! M2 · A6 + C5 — the certifying audit and the **trusted-base ledger**.
//!
//! The firewall's strength is exactly the *certifying fraction* `f`: the share of
//! accepted verdicts that carry a checked certificate. Its dual is the
//! **trusted base** — the deciders that emit a bare `Option<bool>`/`bool` with no
//! checkable witness, so trust rests on the decider's own correctness. This
//! module is the A6 inventory of every fast (`is_efficiently_*`) decider, tagged
//! with its polarity and whether it is *certifying* (a C1 checker can re-validate
//! its verdict) or *bare-boolean* (in the trusted base, the residue to shrink).
//!
//! Two things ride on the inventory, per the §3 trust architecture:
//!
//! 1. **`f` is reported** — [`TrustedBaseLedger::certifying_fraction`].
//! 2. **The trusted base is non-increasing** — the `[REGRESS]` test pins the
//!    bare-boolean set's size; a milestone may *shrink* it (route a decider
//!    through a checker) but never grow it. This is the CI assertion the trust
//!    architecture (§3 rule 2) requires, in its in-tree form.
//!
//! This is an *inventory*, not a runtime gate: it records the standing of each
//! decider as a fact about the code, unit-testable and reproducible. Wiring the
//! `analyze_*` return paths to actually *route through* `accept` + a C1 checker
//! (so the firewall holds at runtime, not just in the ledger) is the C4
//! verify-on-return work; M2 builds the checkers and the ledger that names which
//! deciders they cover, and the test that holds the trusted base from growing.

use super::frontier::Polarity;

/// Whether a decider is certifying (a C1 checker re-validates its verdict) or
/// part of the bare-boolean trusted base (no checkable witness yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeciderKind {
    /// The verdict carries a witness that one of the C1 checkers
    /// ([`super::checkers`]) can re-validate against the original net. Outside
    /// the trusted base: a bug in this decider costs a rejected candidate, not an
    /// unsound verdict.
    Certifying,
    /// The verdict is a bare `bool`/`Option<bool>` with no checkable witness, so
    /// soundness rests on the decider's own correctness. **In the trusted base** —
    /// the residue A6 enumerates and later milestones shrink.
    BareBoolean,
}

/// One fast decider in the audit: a public `is_efficiently_*` (or
/// `efficient_*`) entry point, its polarity, and its certifying standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decider {
    /// The decider's public method name (the audit anchor).
    pub method: &'static str,
    /// The property it decides (human label).
    pub property: &'static str,
    /// The polarity of the verdict it can produce.
    pub polarity: Polarity,
    /// Certifying or bare-boolean (trusted base).
    pub kind: DeciderKind,
    /// The C1 checker that covers it (if certifying), or the milestone that will
    /// make it certifying (if bare-boolean) — keeping the runway with the work.
    pub note: &'static str,
}

/// The full trusted-base ledger: every fast decider, with `f` derivable from it.
#[derive(Debug, Clone)]
pub struct TrustedBaseLedger {
    /// The inventory rows.
    pub deciders: Vec<Decider>,
}

impl TrustedBaseLedger {
    /// The number of certifying deciders (covered by a C1 checker).
    #[must_use]
    pub fn certifying_count(&self) -> usize {
        self.deciders
            .iter()
            .filter(|d| d.kind == DeciderKind::Certifying)
            .count()
    }

    /// The number of bare-boolean deciders — the size of the trusted base. This
    /// is the quantity required to be **non-increasing** across milestones.
    #[must_use]
    pub fn trusted_base_size(&self) -> usize {
        self.deciders
            .iter()
            .filter(|d| d.kind == DeciderKind::BareBoolean)
            .count()
    }

    /// The certifying fraction `f`: certifying deciders over all deciders, in
    /// `[0, 1]`. The figure of merit of the firewall. Returns 0.0 for an empty
    /// inventory.
    ///
    /// Note this is `f` over the *decider inventory* (a structural figure), not
    /// over corpus *verdicts* (the empirical `f` of G6/M12 — which weights by how
    /// often each decider fires). The structural `f` is the one M2 can report
    /// without running the corpus; the empirical `f` is the rig's measurement.
    #[must_use]
    // The decider counts are single-digit, so the `usize → f64` casts cannot lose
    // precision in practice; the allow matches the in-crate precedent
    // (`siphon_trap.rs`). The same `as f64` ratio is used in `mcc-tests/floor.rs`.
    #[allow(clippy::cast_precision_loss)]
    pub fn certifying_fraction(&self) -> f64 {
        if self.deciders.is_empty() {
            0.0
        } else {
            self.certifying_count() as f64 / self.deciders.len() as f64
        }
    }
}

/// The committed trusted-base ledger as of M2.
///
/// Each `is_efficiently_*` / `efficient_*` decider on the public `PetriNet`
/// surface is enumerated. The *certifying* rows are those whose verdict shape a
/// C1 checker re-validates; the *bare-boolean* rows are the trusted base — the
/// residue Epic B's certified deciders (M6–M9) will shrink as each gains a
/// witness.
///
/// The polarity column is the A6 latent-structure surfacing: positive
/// reachability is `ProveYes`, state-machine token conservation is `ProveNo`,
/// and the boundedness/liveness predicates are `Exact` booleans (they answer the
/// yes/no question directly for their class).
#[must_use]
pub fn trusted_base() -> TrustedBaseLedger {
    use DeciderKind::{BareBoolean, Certifying};
    use Polarity::{Exact, ProveNo, ProveYes};
    TrustedBaseLedger {
        deciders: vec![
            // --- Reachability: the witnesses C1 covers. ---
            Decider {
                method: "is_efficiently_reachable (Circuit/live StateMachine, positive)",
                property: "reachability",
                polarity: ProveYes,
                kind: Certifying,
                note: "FiringSequenceCert / ParikhVectorCert re-validate a positive verdict",
            },
            Decider {
                method: "analyze_reachability → StateMachineTokenConservation",
                property: "reachability",
                polarity: ProveNo,
                kind: Certifying,
                note: "TokenConservationCert re-derives the S-net precondition + sum mismatch",
            },
            Decider {
                method: "analyze_reachability → MarkingEquation*NoSolution",
                property: "reachability",
                polarity: ProveNo,
                kind: BareBoolean,
                note: "M5/B1a: the negative verdict rests on f64 LP infeasibility; \
                       an EXACT Farkas P-invariant checker is the certifying replacement",
            },
            // --- Liveness. ---
            Decider {
                method: "is_efficiently_live (FreeChoice via CHC)",
                property: "liveness",
                polarity: ProveYes,
                kind: Certifying,
                note: "SiphonTrapCoverCert (Live claim) re-derives the CHC universal and \
                       gates the liveness reading to free-choice (sound only on FC)",
            },
            Decider {
                method: "is_efficiently_live (Circuit/StateMachine/MarkedGraph)",
                property: "liveness",
                polarity: Exact,
                kind: BareBoolean,
                note: "M6/M8: a bare bool from class+connectivity / unmarked-circuit; \
                       the S-component / circuit-token certificate is B3/B4",
            },
            // --- Boundedness. ---
            Decider {
                method: "is_efficiently_bounded (Circuit/StateMachine/MarkedGraph/AC-CHC)",
                property: "boundedness",
                polarity: Exact,
                kind: BareBoolean,
                note: "still bare-boolean: the A4 exact-sub-invariant CHECKER exists \
                       (BoundednessSubinvariantCert, M5), but the live decider does not yet \
                       EMIT the exact witness — routing it through accept is the A4 completion \
                       (M6, with the S-component cover B3). The ledger flips to Certifying then.",
            },
            // --- Deadlock-freedom. ---
            Decider {
                method: "is_efficiently_deadlock_free (liveness ⇒ DF, or CHC Ok)",
                property: "deadlock-freedom",
                polarity: ProveYes,
                kind: Certifying,
                note: "SiphonTrapCoverCert (DeadlockFree claim) re-derives the CHC \
                       marked-trap universal (sufficient on ANY class, no fence)",
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{trusted_base, DeciderKind};

    /// `[REGRESS]` / `[LINT]` — the trusted base is reported and **non-increasing**.
    ///
    /// This pins the size of the bare-boolean set (the trusted base) at its M2
    /// value. The standing invariant S3 forbids it to grow; a milestone may only
    /// *shrink* it by routing a decider through a C1 checker (which would *lower*
    /// this number and require a deliberate baseline update). A change that raises
    /// it fails here — the in-tree form of the §3 "trusted base non-increasing"
    /// CI assertion.
    #[test]
    fn trusted_base_is_reported_and_non_increasing() {
        // The committed M2 baseline. Shrinking (more certifying, fewer
        // bare-boolean) is allowed but must update these numbers deliberately;
        // GROWING the trusted base is an S3 violation and fails the assertion
        // below.
        const BASELINE_TRUSTED_BASE_SIZE: usize = 3;
        const BASELINE_CERTIFYING_COUNT: usize = 4;

        let ledger = trusted_base();

        assert!(
            ledger.trusted_base_size() <= BASELINE_TRUSTED_BASE_SIZE,
            "S3 violation: the bare-boolean trusted base grew from {BASELINE_TRUSTED_BASE_SIZE} \
             to {} — a decider lost its certificate or a new bare-boolean decider was added",
            ledger.trusted_base_size(),
        );
        // The certifying count must not silently regress either (S2-adjacent: a
        // checker should not vanish).
        assert!(
            ledger.certifying_count() >= BASELINE_CERTIFYING_COUNT,
            "the certifying-decider count regressed below the M2 baseline"
        );

        // `f` (structural certifying fraction) is computed and reported.
        let f = ledger.certifying_fraction();
        assert!((0.0..=1.0).contains(&f));
        // At M2: 4 certifying of 7 deciders ≈ 0.571. Pin it as a recorded result.
        assert!(
            (f - 4.0 / 7.0).abs() < 1e-9,
            "f (structural certifying fraction) = {f:.3}, expected {:.3}",
            4.0 / 7.0
        );

        // Print the ledger for the --nocapture record.
        println!(
            "trusted-base ledger (M2): {} certifying / {} bare-boolean / {} total; f = {:.3}",
            ledger.certifying_count(),
            ledger.trusted_base_size(),
            ledger.deciders.len(),
            f,
        );
        for d in &ledger.deciders {
            let tag = match d.kind {
                DeciderKind::Certifying => "CERT",
                DeciderKind::BareBoolean => "TRUST",
            };
            println!("  [{tag}] {} ({:?}) — {}", d.method, d.polarity, d.note);
        }
    }

    /// Every decider names either its covering checker or its certifying-milestone
    /// note — the runway is kept with the work (doctrine #8). No silent gaps.
    #[test]
    fn every_decider_carries_its_rationale() {
        for d in trusted_base().deciders {
            assert!(!d.note.is_empty(), "decider {} has no rationale note", d.method);
            assert!(!d.method.is_empty());
            assert!(!d.property.is_empty());
        }
    }
}
