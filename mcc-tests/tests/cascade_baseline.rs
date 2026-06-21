//! M3 `[REGRESS]` — the default-policy decider registry reproduces the cascade.
//!
//! The standing invariant S2 (no decisiveness regression) in its corpus form: for
//! the PNML fixtures under `petrivet/tests/fixtures/`, the default driver for each
//! property returns the **same verdict pole** (Proven / Refuted / Inconclusive) as
//! the hand-coded cascade method it wraps. The drivers run *alongside* the cascade
//! (M3 is purely additive), and this test captures both columns in the same run —
//! so the comparison IS the baseline; no separate committed snapshot is needed.
//!
//! # Why two tiers of coverage
//!
//! Driver↔cascade equivalence is *structural* — the exhaustive decider delegates to
//! the very cascade method under test, so a divergence can only come from a cheap
//! structural decider, which the comparison catches. The exhaustive path is correct
//! by construction; running it over every fixture would only re-pay its (worst-case
//! exponential) state-space cost on the large MCC nets, with no extra assurance.
//! So:
//! - **Identity reachability / coverability** (`m₀` covers/reaches itself) is `O(net)`
//!   and is compared on **every** fixture — exercising `from_pnml`, the driver
//!   plumbing, and the cheap deciders across the whole corpus.
//! - The **query-free properties** (boundedness, liveness, deadlock-freedom), which
//!   fall through to state-space / Commoner–Hack on nets without a structural
//!   shortcut, are compared **full-cascade on the small fixtures only** (≤ a node
//!   cap), so the gate is fast and cannot hang on a large or state-explosive net
//!   (e.g. dining philosophers). The state-space equivalence is additionally pinned
//!   by the per-property unit `[REGRESS]` tests on small built nets.
//!
//! The deadlock-freedom comparison is further gated on boundedness, because the
//! cascade's `deadlocks()` search explores the (infinite) reachable set on an
//! unbounded net.

use petrivet::model::{
    boundedness_driver, coverability_driver, deadlock_freedom_driver, liveness_driver,
    reachability_driver, Budget, Query,
};
use petrivet::prelude::{Marking, Net, PetriNet};
use std::path::Path;

/// Node cap for the full-cascade comparison: above this, only the `O(net)` identity
/// queries run. Sized to include the small fixtures (producer_consumer … token-ring)
/// and exclude the large MCC nets and the state-explosive `philo` (60 nodes).
const FULL_CASCADE_NODE_CAP: usize = 33;

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
    corpus.sort_by(|a, b| a.0.cmp(&b.0));
    corpus
}

/// Approximate node count from the PNML text (place + transition element tags), a
/// cheap size heuristic for the cap — exactness is not required, only a bound.
fn structural_size(pnml: &str) -> usize {
    pnml.matches("<place ").count() + pnml.matches("<transition ").count()
}

/// `"proven" | "refuted" | "inconclusive"` for a verdict, so poles compare across
/// the heterogeneous `Verdict<P, N>` payload types.
fn pole<P, N>(v: &petrivet::model::Verdict<P, N>) -> &'static str {
    if v.is_proven() {
        "proven"
    } else if v.is_refuted() {
        "refuted"
    } else {
        "inconclusive"
    }
}

#[test]
fn default_drivers_reproduce_the_cascade_over_the_corpus() {
    let corpus = load_corpus();
    let mut identity_checked = 0_usize;
    let mut full_checked = 0_usize;
    let budget = Budget::unbounded();

    for (name, pnml) in &corpus {
        let Ok(sys): Result<PetriNet<Net>, _> = PetriNet::from_pnml(pnml) else {
            continue; // a fixture the converter rejects (the M0 fidelity gates)
        };
        let net: &Net = &sys.net;
        let m0: Marking<u32> = sys.marking();

        // --- Identity reachability / coverability — O(net), every fixture. ---
        let v = reachability_driver().decide(net, &m0, &Query::Reachable(m0.clone()), budget);
        assert_eq!(
            v.is_proven(),
            sys.is_reachable(&m0),
            "{name}: reachability(m0) driver vs is_reachable",
        );
        let v = coverability_driver().decide(net, &m0, &Query::Coverable(m0.clone()), budget);
        assert_eq!(
            v.is_proven(),
            sys.is_coverable(m0.clone()),
            "{name}: coverability(m0) driver vs is_coverable",
        );
        identity_checked += 1;

        // --- Query-free properties: full cascade on the small fixtures only. ---
        if structural_size(pnml) > FULL_CASCADE_NODE_CAP {
            continue;
        }

        let bounded = sys.is_bounded();
        let v = boundedness_driver().decide(net, &m0, &Query::Trivial, budget);
        assert_eq!(v.is_proven(), bounded, "{name}: boundedness driver vs is_bounded");
        assert!(!v.is_inconclusive(), "{name}: boundedness is total");

        let cascade_live = match sys.try_liveness().map(|a| a.is_live()) {
            Some(true) => "proven",
            Some(false) => "refuted",
            None => "inconclusive",
        };
        let v = liveness_driver().decide(net, &m0, &Query::Trivial, budget);
        assert_eq!(pole(&v), cascade_live, "{name}: liveness driver vs try_liveness");

        if bounded {
            let v = deadlock_freedom_driver().decide(net, &m0, &Query::Trivial, budget);
            assert_eq!(
                v.is_proven(),
                sys.is_deadlock_free(),
                "{name}: deadlock-freedom driver vs is_deadlock_free",
            );
        }
        full_checked += 1;
    }

    assert!(
        identity_checked >= 5,
        "expected the identity cross-check on several fixtures, got {identity_checked}",
    );
    assert!(
        full_checked >= 3,
        "expected the full-cascade comparison on several small fixtures, got {full_checked}",
    );
    println!(
        "M3 [REGRESS]: identity reachability/coverability on {identity_checked} fixtures; \
         full cascade on {full_checked} small fixtures (≤ {FULL_CASCADE_NODE_CAP} nodes)",
    );
}
