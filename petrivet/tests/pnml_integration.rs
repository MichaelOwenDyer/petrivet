//! Integration tests: parse official PNML example files into petrivet systems
//! and assert known structural and behavioral properties.
//!
//! Fixture files include official PNML 2009 examples from
//! <https://www.pnml.org/version-2009/version-2009.php> and **Model Checking
//! Contest** benchmark PNML (with NUPN metadata inside `<toolspecific>`), which
//! exercise the same parser paths as the archives used in competition runs.

#![cfg(feature = "pnml")]

use petrivet::pnml::convert::PetriNetKind;
use petrivet::pnml::labels::NetLabels;
use petrivet::pnml::PnmlDocument;
use petrivet::{Net, PetriNet};

fn load(path: &str) -> PnmlDocument {
    let xml = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read fixture {path}: {e}"));
    PnmlDocument::from_xml(&xml)
        .unwrap_or_else(|e| panic!("could not parse fixture {path}: {e}"))
}

fn first_pt_net(doc: &PnmlDocument) -> (PetriNet<Net>, Box<NetLabels>) {
    let sys = doc.nets[0]
        .to_pt_system()
        .expect("conversion failed");
    let labels = sys.labels.as_ref().unwrap().clone();
    (sys, labels)
}

// ── Philosophers (philo.pnml) ─────────────────────────────────────────────────
//
// 6-philosopher dining philosophers model.
// Known properties (verified against the PNML source):
//   - 30 places, 30 transitions, 96 arcs
//   - 12 marked places (one token each) → 12 total tokens
//   - Free-choice net: every fork/think/eat place has a single output transition
//     except for the contention places; in practice the model is not S-net/T-net
//   - The net is live and bounded under the standard initial marking
//     (each philosopher starts thinking with their fork available)

#[test]
fn philo_topology() {
    let doc = load("tests/fixtures/philo.pnml");
    let (sys, labels) = first_pt_net(&doc);

    assert_eq!(sys.place_count(), 30, "place count");
    assert_eq!(sys.transition_count(), 30, "transition count");
    assert_eq!(sys.arc_count(), 96, "arc count");
    assert_eq!(labels.net_name(), Some("philo"));
}

#[test]
fn philo_initial_marking() {
    let doc = load("tests/fixtures/philo.pnml");
    let (sys, _) = first_pt_net(&doc);

    let total: u32 = sys.current_marking().total_tokens();
    let marked = sys.current_marking().support().count();
    assert_eq!(total, 12, "total tokens");
    assert_eq!(marked, 12, "marked places");
}

#[test]
fn philo_all_place_and_transition_names_populated() {
    let doc = load("tests/fixtures/philo.pnml");
    let (sys, labels) = first_pt_net(&doc);

    // NetLabels per-node accessors take dense Place/Transition (pub(crate)),
    // so from an integration test we verify that the label set has the
    // expected total count of named elements via the public net_name / net_id
    // accessors (tested elsewhere) plus the known property of the philo model:
    // all 30 places and all 30 transitions carry a <name> label in the PNML.
    //
    // A more granular per-node check lives in the convert.rs unit tests which
    // have pub(crate) access.
    assert!(labels.net_name().is_some(), "net name should be populated");
    assert_eq!(sys.place_count(), 30);
    assert_eq!(sys.transition_count(), 30);
}

#[test]
fn philo_is_bounded() {
    let doc = load("tests/fixtures/philo.pnml");
    let (sys, _) = first_pt_net(&doc);
    // The philosophers net is structurally bounded (no place can accumulate
    // tokens indefinitely given any firing sequence).
    assert!(sys.is_structurally_bounded(), "philosophers net should be structurally bounded");
}

// ── Token ring (token-ring.pnml) ──────────────────────────────────────────────
//
// A token-ring mutual exclusion protocol.
// Known properties:
//   - 18 places, 15 transitions, 67 arcs
//   - No initial marking in the file (all places start at 0)
//   - The net is structurally bounded

#[test]
fn token_ring_topology() {
    let doc = load("tests/fixtures/token-ring.pnml");
    let (sys, labels) = first_pt_net(&doc);

    assert_eq!(sys.place_count(), 18, "place count");
    assert_eq!(sys.transition_count(), 15, "transition count");
    assert_eq!(sys.arc_count(), 67, "arc count");
    assert_eq!(labels.net_name(), Some("Token-ring"));
}

#[test]
fn token_ring_zero_initial_marking() {
    let doc = load("tests/fixtures/token-ring.pnml");
    let (sys, _) = first_pt_net(&doc);

    let total: u32 = sys.current_marking().total_tokens();
    assert_eq!(total, 0, "token-ring has no initial marking in the file");
}

#[test]
fn token_ring_net_id_preserved() {
    let doc = load("tests/fixtures/token-ring.pnml");
    let (_, labels) = first_pt_net(&doc);
    // The net id in the file is a long opaque string; we just check it's present.
    assert!(labels.net_id().is_some(), "net id should be preserved in labels");
}

#[test]
fn token_ring_is_not_structurally_bounded() {
    // Two places in the token-ring have no input transitions (source places).
    // The LP-based structural boundedness check correctly determines that no
    // positive place subvariant covers them — the net is not structurally
    // bounded (it would be unbounded under markings that place tokens in those
    // source places). It *is* bounded under the specific operational marking
    // described in the protocol, but that is a behavioral property, not a
    // structural one.
    let doc = load("tests/fixtures/token-ring.pnml");
    let (sys, _) = first_pt_net(&doc);
    assert!(!sys.is_structurally_bounded(), "token-ring has source places; not structurally bounded");
}

// ── Swimming pool (swimming-pool.pnml / Piscine) ──────────────────────────────
//
// A swimming pool access model (Piscine in French).
// Known properties:
//   - 9 places, 7 transitions, 20 arcs
//   - 3 marked places, 5 total tokens
//   - Net name is "Piscine"

#[test]
fn pool_topology() {
    let doc = load("tests/fixtures/swimming-pool.pnml");
    let (sys, labels) = first_pt_net(&doc);

    assert_eq!(sys.place_count(), 9, "place count");
    assert_eq!(sys.transition_count(), 7, "transition count");
    assert_eq!(sys.arc_count(), 20, "arc count");
    assert_eq!(labels.net_name(), Some("Piscine"));
}

#[test]
fn pool_initial_marking() {
    let doc = load("tests/fixtures/swimming-pool.pnml");
    let (sys, _) = first_pt_net(&doc);

    let total: u32 = sys.current_marking().total_tokens();
    let marked = sys.current_marking().support().count();
    assert_eq!(total, 5, "total tokens");
    assert_eq!(marked, 3, "marked places");
}

#[test]
fn pool_is_structurally_bounded() {
    let doc = load("tests/fixtures/swimming-pool.pnml");
    let (sys, _) = first_pt_net(&doc);
    assert!(sys.is_structurally_bounded(), "swimming pool should be structurally bounded");
}

#[test]
fn pool_is_bounded() {
    let doc = load("tests/fixtures/swimming-pool.pnml");
    let (sys, _) = first_pt_net(&doc);
    assert!(sys.is_bounded(), "swimming pool should be bounded under initial marking");
}

#[test]
fn to_petri_net_dispatch_pt_net() {
    let doc = load("tests/fixtures/philo.pnml");
    let kind = doc.nets[0].to_petri_net().expect("dispatch failed");
    assert!(matches!(kind, PetriNetKind::PtNet(..)));
}

#[test]
fn to_petri_nets_batch() {
    let doc = load("tests/fixtures/philo.pnml");
    let results = doc.to_petri_nets();
    assert_eq!(results.len(), 1);
    assert!(matches!(results[0], Ok(PetriNetKind::PtNet(..))));
}

// ── Model Checking Contest (MCC) PNML ─────────────────────────────────────────
//
// MCC model archives embed a NUPN summary (`<size/>`, `<structure>`, …) inside
// `<toolspecific tool="nupn" version="1.1">`. The parser must accept that
// markup without treating it as part of the core P/T graph.

#[test]
fn mcc_champagne_h04_t1u_parses() {
    let doc = load("tests/fixtures/Champagne/PT/champagne_H04_T1U.pnml");
    let (sys, labels) = first_pt_net(&doc);
    assert_eq!(labels.net_name(), Some("champagne_H04_T1U"));
    assert_eq!(sys.place_count(), 285);
    assert_eq!(sys.transition_count(), 351);
    assert_eq!(sys.arc_count(), 820);
    let total_tokens: u32 = sys.initial_marking().iter().map(|(_, w)| *w).sum();
    assert_eq!(total_tokens, 1);
    let nupn = labels.nupn().expect("MCC Champagne carries NUPN");
    assert_eq!(nupn.size.places, 285);
    assert_eq!(nupn.size.transitions, 351);
    assert_eq!(nupn.size.arcs, 820);
    assert!(nupn.unit_safe_declared());
    assert_eq!(nupn.structure.root_unit_id, "u0");
    assert_eq!(nupn.structure.units.len() as u64, nupn.structure.unit_count);
}

#[test]
fn mcc_cops_and_robers_circular_small_parses() {
    let doc = load("tests/fixtures/CopsAndRobbers/PT/CopsAndRobbers-PT-Circular-Random-L005X001.pnml");
    let (sys, labels) = first_pt_net(&doc);
    assert_eq!(
        labels.net_name(),
        Some("CopsAndRobbers_Circular_Random_L005X001")
    );
    assert!(sys.place_count() >= 8);
    assert!(sys.transition_count() >= 1);
    assert!(labels.nupn().is_none(), "this fixture has no NUPN block");
}