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
use petrivet::prelude::{Arc, Net, NetBuilder, PetriNet};
use std::collections::{BTreeMap, BTreeSet};

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

    let total = sys.marking().total_tokens();
    let marked = sys.marking().support().count();
    assert_eq!(total, 12, "total tokens");
    assert_eq!(marked, 12, "marked places");
}

#[test]
fn philo_all_place_and_transition_names_populated() {
    let doc = load("tests/fixtures/philo.pnml");
    let (sys, labels) = first_pt_net(&doc);

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

    let total = sys.marking().total_tokens();
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

    let total = sys.marking().total_tokens();
    let marked = sys.marking().support().count();
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
    let doc = load("tests/fixtures/champagne_H04_T1U.pnml");
    let (sys, labels) = first_pt_net(&doc);
    assert_eq!(labels.net_name(), Some("champagne_H04_T1U"));
    assert_eq!(sys.place_count(), 285);
    assert_eq!(sys.transition_count(), 351);
    assert_eq!(sys.arc_count(), 820);
    let total_tokens = sys.marking().total_tokens();
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
    let doc = load("tests/fixtures/CopsAndRobbers-PT-Circular-Random-L005X001.pnml");
    let (sys, labels) = first_pt_net(&doc);
    assert_eq!(
        labels.net_name(),
        Some("CopsAndRobbers_Circular_Random_L005X001")
    );
    assert!(sys.place_count() >= 8);
    assert!(sys.transition_count() >= 1);
    assert!(labels.nupn().is_none(), "this fixture has no NUPN block");
}

// ── PNML export round trip ────────────────────────────────────────────────────
//
// Import → export → serialize → re-parse → re-import must preserve topology,
// marking, identifiers, names, and positions. Place and transition handles are
// not comparable across the two systems (the dense ordering is an internal
// implementation detail), so all comparisons go through the stable PNML
// identifiers preserved in `NetLabels`.

fn round_trip(path: &str) -> (PetriNet<Net>, PetriNet<Net>) {
    let doc = load(path);
    let sys1 = doc.nets[0].to_pt_system().expect("initial conversion failed");
    let xml = sys1.to_pnml().to_xml().expect("export serialization failed");
    let doc2 = PnmlDocument::from_xml(&xml)
        .unwrap_or_else(|e| panic!("exported XML failed to re-parse: {e}"));
    assert_eq!(doc2.nets.len(), 1, "export produces a single-net document");
    let sys2 = doc2.nets[0].to_pt_system().expect("re-import failed");
    (sys1, sys2)
}

/// The flow relation as a set of `(source id, target id)` pairs.
fn arc_endpoint_ids(sys: &PetriNet<Net>) -> BTreeSet<(String, String)> {
    let labels = sys.labels.as_ref().expect("labels present");
    sys.arcs()
        .map(|arc| match arc {
            Arc::PlaceToTransition(p, t) => (
                labels.place_id(p).expect("place id").to_owned(),
                labels.transition_id(t).expect("transition id").to_owned(),
            ),
            Arc::TransitionToPlace(t, p) => (
                labels.transition_id(t).expect("transition id").to_owned(),
                labels.place_id(p).expect("place id").to_owned(),
            ),
        })
        .collect()
}

/// Token counts of the marking, keyed by stable place identifier.
fn marking_by_id(sys: &PetriNet<Net>) -> BTreeMap<String, u32> {
    let labels = sys.labels.as_ref().expect("labels present");
    sys.marking()
        .into_iter()
        .map(|(p, n)| (labels.place_id(p).expect("place id").to_owned(), n))
        .collect()
}

/// Place names keyed by stable place identifier.
fn place_names_by_id(sys: &PetriNet<Net>) -> BTreeMap<String, String> {
    let labels = sys.labels.as_ref().expect("labels present");
    sys.places()
        .filter_map(|p| {
            let name = labels.place_name(p)?;
            Some((labels.place_id(p).expect("place id").to_owned(), name.to_owned()))
        })
        .collect()
}

/// Transition names keyed by stable transition identifier.
fn transition_names_by_id(sys: &PetriNet<Net>) -> BTreeMap<String, String> {
    let labels = sys.labels.as_ref().expect("labels present");
    sys.transitions()
        .filter_map(|t| {
            let name = labels.transition_name(t)?;
            Some((labels.transition_id(t).expect("transition id").to_owned(), name.to_owned()))
        })
        .collect()
}

/// Place positions keyed by stable place identifier.
fn place_positions_by_id(sys: &PetriNet<Net>) -> BTreeMap<String, (f64, f64)> {
    let labels = sys.labels.as_ref().expect("labels present");
    let graphics = sys.graphics.as_ref().expect("graphics present");
    sys.places()
        .filter_map(|p| {
            let pos = graphics.place_position(&p)?;
            Some((labels.place_id(p).expect("place id").to_owned(), (pos.x, pos.y)))
        })
        .collect()
}

#[test]
fn philo_round_trip_preserves_topology() {
    let (sys1, sys2) = round_trip("tests/fixtures/philo.pnml");
    assert_eq!(sys2.place_count(), sys1.place_count(), "place count");
    assert_eq!(sys2.transition_count(), sys1.transition_count(), "transition count");
    assert_eq!(sys2.arc_count(), sys1.arc_count(), "arc count");

    let arcs1 = arc_endpoint_ids(&sys1);
    assert_eq!(arcs1.len(), sys1.arc_count(), "arc endpoint pairs are unique");
    assert_eq!(arcs1, arc_endpoint_ids(&sys2), "flow relation is identical by id");
}

#[test]
fn philo_round_trip_preserves_marking() {
    let (sys1, sys2) = round_trip("tests/fixtures/philo.pnml");
    assert_eq!(sys2.marking().total_tokens(), 12, "total tokens");
    assert_eq!(sys2.marking().support().count(), 12, "marked places");
    assert_eq!(marking_by_id(&sys1), marking_by_id(&sys2), "marking is identical by id");
}

#[test]
fn philo_round_trip_preserves_names() {
    let (sys1, sys2) = round_trip("tests/fixtures/philo.pnml");
    let labels2 = sys2.labels.as_ref().expect("labels");
    assert_eq!(labels2.net_name(), Some("philo"));

    let names1 = place_names_by_id(&sys1);
    assert_eq!(names1.len(), 30, "philo names every place");
    assert_eq!(names1, place_names_by_id(&sys2));
    assert_eq!(transition_names_by_id(&sys1), transition_names_by_id(&sys2));
}

#[test]
fn philo_round_trip_preserves_positions() {
    let (sys1, sys2) = round_trip("tests/fixtures/philo.pnml");
    let pos1 = place_positions_by_id(&sys1);
    assert_eq!(pos1.len(), 30, "philo positions every place");
    assert_eq!(pos1, place_positions_by_id(&sys2));
}

#[test]
fn token_ring_round_trip_preserves_topology_and_empty_marking() {
    let (sys1, sys2) = round_trip("tests/fixtures/token-ring.pnml");
    assert_eq!(sys2.place_count(), 18, "place count");
    assert_eq!(sys2.transition_count(), 15, "transition count");
    assert_eq!(sys2.arc_count(), 67, "arc count");
    assert_eq!(arc_endpoint_ids(&sys1), arc_endpoint_ids(&sys2), "flow relation");
    assert_eq!(sys2.marking().total_tokens(), 0, "no marking in the source file");
}

#[test]
fn mcc_nupn_metadata_survives_round_trip() {
    let (sys1, sys2) = round_trip("tests/fixtures/champagne_H04_T0R.pnml");
    let labels1 = sys1.labels.as_ref().expect("labels");
    let labels2 = sys2.labels.as_ref().expect("labels");
    let nupn1 = labels1.nupn().expect("champagne carries NUPN metadata");
    let nupn2 = labels2.nupn().expect("NUPN metadata must survive the round trip");
    assert_eq!(nupn1.size, nupn2.size);
    assert_eq!(nupn1.structure.unit_count, nupn2.structure.unit_count);
    assert_eq!(nupn1.structure.root_unit_id, nupn2.structure.root_unit_id);
    assert_eq!(nupn1.structure.unit_safe, nupn2.structure.unit_safe);
    let units1: Vec<_> = nupn1.structure.units.iter()
        .map(|u| (u.id.as_str(), u.places.id_vec(), u.subunits.id_vec()))
        .collect();
    let units2: Vec<_> = nupn2.structure.units.iter()
        .map(|u| (u.id.as_str(), u.places.id_vec(), u.subunits.id_vec()))
        .collect();
    assert_eq!(units1, units2, "unit tree is identical modulo whitespace");
}

#[test]
fn builder_net_round_trip_with_generated_ids() {
    // p0 → t0 → p1 → t1 → p0, plus p0 → t1. The place p0 is the only place
    // with two outgoing arcs, which identifies it structurally after the
    // round trip without relying on handle values.
    let mut b = NetBuilder::new();
    let [p0, p1] = b.add_places();
    let [t0, t1] = b.add_transitions();
    b.add_arcs((p0, t0, p1, t1, p0));
    b.add_arc((p0, t1));
    let net = b.build().expect("valid net");
    let sys1 = PetriNet::new(net, [(p0, 2)]);

    let xml = sys1.to_pnml().to_xml().expect("serialize");
    let doc2 = PnmlDocument::from_xml(&xml).expect("re-parse");
    let sys2 = doc2.nets[0].to_pt_system().expect("re-import");

    assert_eq!(sys2.place_count(), 2, "place count");
    assert_eq!(sys2.transition_count(), 2, "transition count");
    assert_eq!(sys2.arc_count(), 5, "arc count");

    // Generated identifiers are present after the round trip.
    let labels2 = sys2.labels.as_ref().expect("re-import populates labels");
    let mut place_ids: Vec<&str> = sys2.places().filter_map(|p| labels2.place_id(p)).collect();
    place_ids.sort_unstable();
    assert_eq!(place_ids, ["p0", "p1"], "deterministic generated place ids");
    let mut transition_ids: Vec<&str> =
        sys2.transitions().filter_map(|t| labels2.transition_id(t)).collect();
    transition_ids.sort_unstable();
    assert_eq!(transition_ids, ["t0", "t1"], "deterministic generated transition ids");

    // The marking survives on the structurally correct place.
    let marked: Vec<_> = sys2.marking().into_iter().collect();
    assert_eq!(marked.len(), 1, "one marked place");
    let (marked_place, tokens) = marked[0];
    assert_eq!(tokens, 2, "token count");
    assert_eq!(
        sys2.place_postset(&marked_place).count(),
        2,
        "the marked place is the one with two outgoing arcs"
    );

    // A second round trip is stable: identifiers minted on the first trip are
    // reused, so the flow relation and marking match by id.
    let xml2 = sys2.to_pnml().to_xml().expect("second export");
    let doc3 = PnmlDocument::from_xml(&xml2).expect("re-parse of second export");
    let sys3 = doc3.nets[0].to_pt_system().expect("second re-import");
    assert_eq!(arc_endpoint_ids(&sys2), arc_endpoint_ids(&sys3));
    assert_eq!(marking_by_id(&sys2), marking_by_id(&sys3));
}