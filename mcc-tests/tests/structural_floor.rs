//! G4a — the structural-coverage floor, run over the in-tree PNML corpus.
//!
//! This test measures `f_struct` (the fraction of property queries decided by
//! the polynomial structural tier) over the 15 PNML fixtures committed under
//! `petrivet/tests/fixtures/`, and asserts a committed baseline reproduces.
//!
//! It is the **starting line**, not the thesis result: the coverage the
//! *current* structural tier achieves before any Epic-B generator lands. Run it
//! with `--nocapture` to see the full per-(net, property) tier table:
//!
//! ```text
//! cargo test -p mcc-tests --test structural_floor -- --nocapture
//! ```

use mcc_tests::floor::{measure_corpus, FloorReport, Tier};
use std::path::Path;

/// The committed corpus: every `.pnml` under `petrivet/tests/fixtures/`.
/// Loading them all (and recording which convert) IS the reproducing baseline
/// of "all property verdicts over the corpus".
fn load_corpus() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("petrivet")
        .join("tests")
        .join("fixtures");
    let mut corpus: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("could not read fixtures dir {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "pnml"))
        .filter_map(|p| {
            let name = p.file_stem()?.to_str()?.to_string();
            let pnml = std::fs::read_to_string(&p).ok()?;
            Some((name, pnml))
        })
        .collect();
    // Deterministic order so the printed table and any snapshot are stable.
    corpus.sort_by(|a, b| a.0.cmp(&b.0));
    corpus
}

#[test]
fn structural_coverage_floor_reproduces() {
    let corpus = load_corpus();
    assert!(
        corpus.len() >= 15,
        "expected at least the 15 committed PNML fixtures, found {}",
        corpus.len()
    );

    let rows = measure_corpus(&corpus);
    let report = FloorReport::from_rows(&rows);

    // Print the per-(net, property) tier table and the floor report. This is the
    // human-readable artifact (run with --nocapture).
    println!("\n=== structural-coverage floor (G4a) — per-query tiers ===");
    for row in &rows {
        println!("  {:<48} {:<16} {}", row.model, row.property.to_string(), row.tier);
    }
    println!("\n{report}\n");

    // ---- The committed baseline (reproducibility gate [REGRESS]) ------------
    //
    // These numbers are the floor as measured against the committed corpus. They
    // are the STARTING LINE: when Epic-B structural generators land, the
    // `structural` count rises and this baseline is intentionally updated (with
    // the milestone delta recorded). A *fall* in structural coverage, or a
    // change in the corpus size, fails this test — exactly the regression guard
    // G4a requires.
    //
    // Corpus: 15 fixtures × 3 query-free structural properties = 45 queries.
    // 3 fixtures carry non-unit-weight arcs and are conversion-rejected (A7/E7),
    // contributing 9 conversion-rejected queries.
    let expected = FloorReport {
        structural: EXPECTED_STRUCTURAL,
        choice_class: EXPECTED_CHOICE_CLASS,
        not_structural: EXPECTED_NOT_STRUCTURAL,
        conversion_rejected: EXPECTED_CONVERSION_REJECTED,
    };
    assert_eq!(
        report, expected,
        "structural-coverage floor changed.\n  measured: {report:#?}\n  baseline: {expected:#?}\n\
         If this is an intended improvement (e.g. a new structural decider), update the \
         EXPECTED_* constants and record the milestone delta."
    );

    // Sanity: the all-corpus denominator is exactly 3 properties per fixture.
    assert_eq!(report.all_corpus_total(), corpus.len() * 3);
    // No query is left unclassified.
    assert_eq!(rows.len(), corpus.len() * 3);
    // The 3 weighted-arc fixtures account for all conversion-rejected rows.
    assert_eq!(
        rows.iter().filter(|r| r.tier == Tier::ConversionRejected).count(),
        EXPECTED_CONVERSION_REJECTED,
    );
}

// ---- Committed baseline constants (the G4a floor) --------------------------
//
// Measured 2026-06-20 over the 15 committed PNML fixtures × 3 query-free
// structural properties = 45 queries. THE STARTING LINE.
//
//   structural-decided : 6   (producer_consumer, train_tracks — both MarkedGraph)
//   choice-class (CHC) : 6   (swimming-pool, traffic — AsymmetricChoice; B7 exp.)
//   not-structural     : 24  (8 General nets: CopsAndRobbers ×2, aslink, champagne,
//                              mainframe, medleyB01, philo, token-ring)
//   conversion-rejected: 9   (3 weighted-arc fixtures: CopsAndRobbers-Circular-L015,
//                              CopsAndRobbers-Heawood, ZombiesAndSurvivors — A7/E7)
//
//   f_struct (in-scope   denom 36): 0.167
//   f_struct (all-corpus denom 45): 0.133
//
// These are intentionally LOW: before any Epic-B generator lands, only the two
// cheap T-net (MarkedGraph) fixtures decide structurally. Epic B (M6–M8) makes
// the choice-class (CHC) tier efficient and pushes General nets into structural
// coverage; M12 reports the raised number as the thesis headline. When that
// happens these constants are updated WITH the recorded milestone delta — a
// fall, or a corpus-size change, fails this regression guard.
const EXPECTED_STRUCTURAL: usize = 6;
const EXPECTED_CHOICE_CLASS: usize = 6;
const EXPECTED_NOT_STRUCTURAL: usize = 24;
const EXPECTED_CONVERSION_REJECTED: usize = 9;
