//! M2 · C7 — the **checkable frontier**: a per-(property × polarity) map of
//! where compact, independently-checkable certificates exist, the complexity of
//! checking each, and where complexity theory forbids one (the wall).
//!
//! Certificate strength is sharply non-uniform, and mapping it is a thesis
//! deliverable. This module is the *machine-readable* form of that map: a table
//! ([`frontier_table`]) of [`FrontierEntry`] rows, each naming a property, a
//! polarity (prove-yes / prove-no), the witness shape, the **checker complexity**
//! ([`CheckComplexity`]), and whether a checker exists **today** (M2) or is
//! deferred to where its soundness precondition lands. The thesis renders this as
//! the per-property × polarity table; the test in this module pins it as a
//! `[REGRESS]` baseline so a future change to the frontier is a *recorded*
//! change, not a silent one.
//!
//! This is a *map*, not a measurement of the corpus: it states, per cell, the
//! known complexity-theoretic standing of the certificate. The corpus
//! *coverage* number (`f_struct`) is the floor (G4a, `mcc-tests`); the certifying
//! *fraction* (`f`) is the ledger (C5, [`super::ledger`]). The frontier says
//! which certificates are even *possible* and how dearly they check.

/// A property the analyzer decides. (Query-dependent properties —
/// reachability/coverability — and query-free global properties — boundedness,
/// liveness, deadlock-freedom.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Property {
    /// Is a given target marking reachable from `M₀`?
    Reachability,
    /// Is a given target marking coverable from `M₀`?
    Coverability,
    /// Is the net bounded (a finite reachable state space)?
    Boundedness,
    /// Is every transition live (L4)?
    Liveness,
    /// Is no reachable marking a total deadlock?
    DeadlockFreedom,
}

/// The polarity of a verdict: the direction the certificate witnesses.
///
/// Surfacing polarity is the A6 latent-structure finding: it is implicit in the
/// split proof enums (`ReachabilityProof` vs. `UnreachabilityProof`) and the
/// prove-NO-only LP filters. A decider/certificate is `ProveYes` (a positive
/// witness), `ProveNo` (a refutation), or `Exact` (decides both poles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Polarity {
    /// Witnesses that the property *holds* (a positive proof).
    ProveYes,
    /// Witnesses that the property *fails* (a refutation).
    ProveNo,
    /// Decides both poles exactly.
    Exact,
}

/// The complexity of *checking* a certificate against the original net — the
/// only complexity that bears on the trusted base (the generator's cost is
/// outside it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckComplexity {
    /// Near-linear in the net / witness size: a firing-sequence replay, one
    /// dot product, a token-sum comparison.
    NearLinear,
    /// Polynomial: checking an *exhibited* cover (e.g. a siphon/trap cover —
    /// linear per pair, even though enumerating all siphons is hard).
    Polynomial,
    /// No known compact checkable certificate — the wall. The honest witness is
    /// worst-case super-polynomial (a cutting-plane / VeriPB-shaped derivation)
    /// or unknown to exist at all.
    Wall,
}

impl CheckComplexity {
    /// A short human label for the table.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NearLinear => "near-linear",
            Self::Polynomial => "polynomial",
            Self::Wall => "wall (no compact certificate)",
        }
    }
}

/// One cell of the checkable-frontier map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierEntry {
    /// The property this row is about.
    pub property: Property,
    /// The polarity (which pole the certificate witnesses).
    pub polarity: Polarity,
    /// The witness shape, named (e.g. "firing word", "Farkas P-semiflow").
    pub witness: &'static str,
    /// The complexity of checking the witness against the original net.
    pub check: CheckComplexity,
    /// Whether a checker exists in the tree **today** (M2). `false` means the
    /// certificate is mapped but its checker is deferred — the `note` says to
    /// where (a milestone) and why.
    pub checker_built: bool,
    /// The milestone/precondition note for a deferred checker, or the theorem the
    /// built checker rests on.
    pub note: &'static str,
}

/// The checkable-frontier table: the full per-(property × polarity) map.
///
/// Built rows (M2) and deferred rows (with their milestone) are both present, so
/// the map is honest about the wall and the runway. The `[REGRESS]` test in this
/// module pins the table.
#[must_use]
// This is a flat declarative data table (15 cells), not control-flow logic;
// splitting it would scatter the map across helpers for no clarity gain.
#[allow(clippy::too_many_lines)]
pub fn frontier_table() -> Vec<FrontierEntry> {
    use CheckComplexity::{NearLinear, Polynomial, Wall};
    use Polarity::{ProveNo, ProveYes};
    use Property::{Boundedness, Coverability, DeadlockFreedom, Liveness, Reachability};
    vec![
        // --- Reachability ---
        FrontierEntry {
            property: Reachability,
            polarity: ProveYes,
            witness: "firing word (replay)",
            check: NearLinear,
            checker_built: true,
            note: "FiringSequenceCert: replay σ from M₀, reached == target",
        },
        FrontierEntry {
            property: Reachability,
            polarity: ProveYes,
            witness: "marking-equation integer Parikh vector",
            check: NearLinear,
            checker_built: true,
            note: "ParikhVectorCert: recompute M₀+C·σ over ℤ (sufficient on MG/FC; \
                   Thm 5.2.7 / 5.3.18)",
        },
        FrontierEntry {
            property: Reachability,
            polarity: ProveNo,
            witness: "state-machine token conservation",
            check: NearLinear,
            checker_built: true,
            note: "TokenConservationCert: re-derive |•t|=|t•|=1 ∀t and sum(M₀)≠sum(target)",
        },
        FrontierEntry {
            property: Reachability,
            polarity: ProveNo,
            witness: "Farkas P-semiflow (separating place invariant)",
            check: NearLinear,
            checker_built: false,
            note: "M5/B1a: needs an EXACT-rational separating invariant \
                   (y·C=0 ∧ y·(m'−m₀)≠0); an f64 dual is not emittable",
        },
        FrontierEntry {
            property: Reachability,
            polarity: ProveNo,
            witness: "ILP-only infeasibility (cutting-plane derivation)",
            check: Wall,
            checker_built: false,
            note: "the wall: integer-only infeasibility has no single Farkas dual \
                   (VeriPB-shaped, worst-case super-polynomial)",
        },
        // --- Coverability ---
        FrontierEntry {
            property: Coverability,
            polarity: ProveYes,
            witness: "firing word to a finite covering marking (replay)",
            check: NearLinear,
            checker_built: true,
            note: "FiringSequenceCert on Query::Coverable: reached ≥ target (finite case)",
        },
        FrontierEntry {
            property: Coverability,
            polarity: ProveYes,
            witness: "ω-covering marking with pumping cycle (lasso)",
            check: NearLinear,
            checker_built: false,
            note: "C7 witness enrichment: the ω-witness lacks its pumping cycle; \
                   the finite firing word checks, the ω lasso does not yet",
        },
        FrontierEntry {
            property: Coverability,
            polarity: ProveNo,
            witness: "Farkas P-semiflow (separating place invariant)",
            check: NearLinear,
            checker_built: false,
            note: "M5/B1a: shares the exact-rational dual precondition with \
                   unreachability",
        },
        // --- Boundedness ---
        FrontierEntry {
            property: Boundedness,
            polarity: ProveYes,
            witness: "positive place sub-invariant y (yᵀC ≤ 0)",
            check: NearLinear,
            checker_built: true,
            note: "BoundednessSubinvariantCert (A4/M5): re-verify y > 0 and yᵀ·C ≤ 0 \
                   over ℚ exactly; the structural bound is ⌊(y·M₀)/y[p]⌋. The exact \
                   witness replaces the f64 vector on the decider path at M6.",
        },
        FrontierEntry {
            property: Boundedness,
            polarity: ProveNo,
            witness: "Karp–Miller self-covering lasso (unboundedness)",
            check: NearLinear,
            checker_built: false,
            note: "C7 witness enrichment: the ω-witness lacks the self-covering \
                   cycle the checker needs",
        },
        // --- Liveness ---
        FrontierEntry {
            property: Liveness,
            polarity: ProveYes,
            witness: "free-choice siphon/trap cover (Commoner–Hack)",
            check: Polynomial,
            checker_built: true,
            note: "SiphonTrapCoverCert (Live claim): re-derive the CHC universal from \
                   primitives, then GATE the liveness reading to free-choice (the \
                   universal is sufficient for liveness only on FC; Thm 5.17). \
                   Enumeration is the generator's cost.",
        },
        FrontierEntry {
            property: Liveness,
            polarity: ProveYes,
            witness: "marked-graph circuit-token cover",
            check: Polynomial,
            checker_built: false,
            note: "C7 witness enrichment: LivenessMethod::MarkedGraph carries no \
                   circuit-token data yet",
        },
        FrontierEntry {
            property: Liveness,
            polarity: Polarity::Exact,
            witness: "general (non-free-choice) liveness",
            check: Wall,
            checker_built: false,
            note: "the wall: no known compact checkable certificate for general \
                   liveness — the standout open, high-novelty target",
        },
        // --- Deadlock-freedom ---
        FrontierEntry {
            property: DeadlockFreedom,
            polarity: ProveYes,
            witness: "siphon/trap cover (sufficient, general nets)",
            check: Polynomial,
            checker_built: true,
            note: "SiphonTrapCoverCert (DeadlockFree claim): every minimal siphon holds \
                   a marked trap ⇒ deadlock-free on ANY class (no fence; sufficient; \
                   the converse is unsound and excluded)",
        },
        FrontierEntry {
            property: DeadlockFreedom,
            polarity: ProveNo,
            witness: "reachable deadlock marking (firing word to a dead marking)",
            check: NearLinear,
            checker_built: false,
            note: "M3+: a firing word to a marking enabling no transition; the \
                   witness is not yet emitted by a structural decider",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{frontier_table, CheckComplexity, Polarity};

    /// `[REGRESS]` — the frontier map is pinned. Its *shape* (how many cells, how
    /// many checkers built, where the wall is) is a recorded baseline; a change
    /// to the map must update this test deliberately, never drift silently. This
    /// is the C7 deliverable in its machine-checkable form.
    #[test]
    fn frontier_map_matches_committed_baseline() {
        let table = frontier_table();

        // Total cells mapped.
        assert_eq!(table.len(), 15, "frontier cell count changed (update baseline)");

        // Checker-bearing cells. The five C1/A4 checker *types* (FiringSequence,
        // Parikh, TokenConservation, SiphonTrapCover, BoundednessSubinvariant)
        // cover seven frontier *cells*: FiringSequenceCert serves both
        // reachability (Reachable) and coverability (finite Coverable), and
        // SiphonTrapCoverCert serves both liveness and deadlock-freedom.
        let built = table.iter().filter(|e| e.checker_built).count();
        assert_eq!(built, 7, "built-checker cell count changed (update baseline)");

        // Of the built cells: 5 near-linear (firing-word reachability, Parikh,
        // token conservation, firing-word coverability, boundedness sub-invariant)
        // and 2 polynomial (the two siphon/trap cover cells — liveness and
        // deadlock-freedom).
        let built_near_linear = table
            .iter()
            .filter(|e| e.checker_built && e.check == CheckComplexity::NearLinear)
            .count();
        let built_polynomial = table
            .iter()
            .filter(|e| e.checker_built && e.check == CheckComplexity::Polynomial)
            .count();
        assert_eq!(built_near_linear, 5, "built near-linear checker count changed");
        assert_eq!(built_polynomial, 2, "built polynomial checker count changed");

        // The wall is explicitly mapped (general liveness; ILP-only infeasibility).
        let walls = table
            .iter()
            .filter(|e| e.check == CheckComplexity::Wall)
            .count();
        assert_eq!(walls, 2, "wall-cell count changed (the stated hardness boundary)");

        // No built checker sits on the wall — by construction, the wall has no
        // compact certificate to check.
        assert!(
            table
                .iter()
                .all(|e| !(e.checker_built && e.check == CheckComplexity::Wall)),
            "a checker must never be marked built on a wall cell"
        );

        // Every entry's note is non-empty (the map is self-documenting).
        assert!(table.iter().all(|e| !e.note.is_empty()));

        // Polarity coverage: every polarity appears somewhere in the map.
        for polarity in [Polarity::ProveYes, Polarity::ProveNo, Polarity::Exact] {
            assert!(
                table.iter().any(|e| e.polarity == polarity),
                "polarity {polarity:?} missing from the frontier map"
            );
        }
    }
}
