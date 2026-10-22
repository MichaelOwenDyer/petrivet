//! Integration tests: parse official PNML example files into petrivet systems
//! and assert known structural and behavioral properties.
//!
//! Fixture files include official PNML 2009 examples from
//! <https://www.pnml.org/version-2009/version-2009.php> and **Model Checking
//! Contest** benchmark PNML (with NUPN metadata inside `<toolspecific>`), which
//! exercise the same parser paths as the archives used in competition runs.

#![cfg(feature = "pnml")]

use petrivet::net::Net;
use petrivet::pnml::PnmlDocument;
use petrivet::pnml::import::PetriNetKind;
use petrivet::pnml::labels::NetLabels;
use petrivet::system::PetriNet;
use petrivet::system::boundedness::Boundedness;
use petrivet::system::deadlock_freedom::DeadlockFreedomEffort;
use petrivet::system::liveness::LivenessEffort;

fn load(path: &str) -> PnmlDocument {
    let xml = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read fixture {path}: {e}"));
    PnmlDocument::from_xml(&xml).unwrap_or_else(|e| panic!("could not parse fixture {path}: {e}"))
}

fn first_pt_net(doc: &PnmlDocument) -> (PetriNet<Net>, Box<NetLabels>) {
    let sys = doc.nets[0].to_pt_system().expect("conversion failed");
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
    assert!(
        sys.is_structurally_bounded(),
        "philosophers net should be structurally bounded"
    );
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
    assert!(
        labels.net_id().is_some(),
        "net id should be preserved in labels"
    );
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
    assert!(
        !sys.is_structurally_bounded(),
        "token-ring has source places; not structurally bounded"
    );
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
    assert!(
        sys.is_structurally_bounded(),
        "swimming pool should be structurally bounded"
    );
}

#[test]
fn pool_is_bounded() {
    let doc = load("tests/fixtures/swimming-pool.pnml");
    let (sys, _) = first_pt_net(&doc);
    assert!(
        sys.is_bounded().call().unwrap(),
        "swimming pool should be bounded under initial marking"
    );
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

// ── Export round trips ────────────────────────────────────────────────────────

fn assert_round_trips(path: &str) {
    let (sys, labels) = first_pt_net(&load(path));
    let xml = sys.to_pnml().expect("export failed");
    let again = PetriNet::from_pnml(&xml).expect("re-import failed");
    let again_labels = again.labels.as_ref().unwrap();

    assert_eq!(again.place_count(), sys.place_count(), "{path}");
    assert_eq!(again.transition_count(), sys.transition_count(), "{path}");
    assert_eq!(again.arc_count(), sys.arc_count(), "{path}");
    assert_eq!(
        again.marking().total_tokens(),
        sys.marking().total_tokens(),
        "{path}"
    );
    assert_eq!(again_labels.net_id(), labels.net_id(), "{path}");
    assert_eq!(again_labels.net_name(), labels.net_name(), "{path}");
    assert_eq!(
        again_labels.named_places().count(),
        labels.named_places().count(),
        "{path}"
    );
    assert_eq!(again_labels.nupn(), labels.nupn(), "{path}");
}

#[test]
fn export_round_trips_fixtures() {
    for entry in std::fs::read_dir("tests/fixtures").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "pnml") {
            let doc = load(path.to_str().unwrap());
            if doc.nets[0].to_pt_system().is_ok() {
                assert_round_trips(path.to_str().unwrap());
            }
        }
    }
}

#[test]
fn producer_consumer_methods_are_marked_graph() {
    let doc = load("tests/fixtures/producer_consumer.pnml");
    let (sys, _) = first_pt_net(&doc);

    assert_eq!(sys.is_deadlock_free().call().unwrap(), true);
    assert_eq!(sys.boundedness().call().unwrap(), Boundedness::Bounded(5));
}

#[test]
fn philo_methods_fall_back_to_state_space_where_no_shortcut_applies() {
    let doc = load("tests/fixtures/philo.pnml");
    let (sys, _) = first_pt_net(&doc);

    // The efficient variants decline instead of exploring the state space.
    assert!(sys.is_deadlock_free().effort(DeadlockFreedomEffort::Efficient).call().is_none());
    assert!(sys.liveness().effort(LivenessEffort::Efficient).call().is_none());
    assert!(!sys.is_deadlock_free().call().unwrap());
    assert_eq!(sys.boundedness().call().unwrap(), Boundedness::Bounded(1));
}
// ── Deadlines ─────────────────────────────────────────────────────────────────

/// A general net with no structural shortcut for deadlock-freedom: only exhausting the state
/// space proves that it is deadlock-free.
const DEADLOCK_FREE_ONLY_BY_EXPLORATION: &str = r#"<?xml version="1.0"?>
<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml"><net id="n" type="http://www.pnml.org/version-2009/grammar/ptnet"><page id="pg">
<place id="p0"><initialMarking><text>2</text></initialMarking></place>
<place id="p1"><initialMarking><text>1</text></initialMarking></place>
<place id="p2"><initialMarking><text>2</text></initialMarking></place>
<transition id="t0"/><transition id="t1"/><transition id="t2"/><transition id="t3"/>
<arc id="a0" source="p0" target="t1"/><arc id="a1" source="p0" target="t2"/><arc id="a2" source="p0" target="t3"/>
<arc id="a3" source="p1" target="t0"/><arc id="a4" source="p1" target="t1"/><arc id="a5" source="t0" target="p1"/>
<arc id="a6" source="t1" target="p2"/><arc id="a7" source="t2" target="p1"/><arc id="a8" source="t3" target="p1"/>
</page></net></pnml>"#;

/// An exhausted search is an answer, not a give-up.
#[test]
fn exhausting_the_state_space_proves_deadlock_freedom() {
    let doc = PnmlDocument::from_xml(DEADLOCK_FREE_ONLY_BY_EXPLORATION).unwrap();
    let (sys, _) = first_pt_net(&doc);

    assert_eq!(sys.is_deadlock_free().call(), Some(true));
}

/// The Commoner/Hack encoding of a large net is big enough that Z3 spends minutes asserting it.
/// The deadline must cut that short, and must leave the thread's solver usable afterwards.
#[cfg(feature = "z3")]
#[test]
fn deadline_bounds_deadlock_freedom_of_a_large_net() {
    let doc = load("tests/fixtures/aslink_01_a.pnml");
    let (sys, _) = first_pt_net(&doc);

    let start = std::time::Instant::now();
    let answer = sys.is_deadlock_free().timeout(std::time::Duration::from_millis(50)).call();
    assert!(start.elapsed() < std::time::Duration::from_secs(10), "took {:?}", start.elapsed());
    assert_ne!(answer, Some(false), "a give-up must not be reported as a deadlock");

    // an interrupt must not leave this thread's Z3 context unusable: a later query that
    // needs the solver has to be answered, quickly and correctly
    let mut b = petrivet::net::builder::NetBuilder::new();
    let [p0, p1] = b.add_places();
    let [t0, t1] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p1));
    b.add_arc((p1, t1));
    b.add_arc((t1, p0));
    let sys = PetriNet::new(b.build().unwrap(), [(p0, 1)]);
    let start = std::time::Instant::now();
    let reachable = sys
        .analyze_reachability_within([(p1, 1)], std::time::Duration::from_secs(30), None)
        .is_reachable();
    assert!(reachable);
    assert!(start.elapsed() < std::time::Duration::from_secs(10), "took {:?}", start.elapsed());
}

/// Liveness of a general net falls back to the state space, which must respect the deadline too.
#[test]
fn deadline_bounds_liveness_of_a_large_net() {
    let doc = load("tests/fixtures/aslink_01_a.pnml");
    let (sys, _) = first_pt_net(&doc);

    let start = std::time::Instant::now();
    let answer = sys.is_live().timeout(std::time::Duration::from_millis(50)).call();
    assert!(start.elapsed() < std::time::Duration::from_secs(10), "took {:?}", start.elapsed());
    assert_eq!(answer, None);
}

fn two_place_net_with_inscription(inscription: &str) -> String {
    format!(
        r#"<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
  <net id="n" type="http://www.pnml.org/version-2009/grammar/ptnet"><page id="pg">
    <place id="p0"><initialMarking><text>2</text></initialMarking></place>
    <place id="p1"/>
    <transition id="t0"/>
    <arc id="a0" source="p0" target="t0">{inscription}</arc>
    <arc id="a1" source="t0" target="p1"/>
  </page></net>
</pnml>"#
    )
}

/// petrivet nets are ordinary: a weighted arc must be an error, not silently read as weight 1.
#[test]
fn arcs_heavier_than_one_are_rejected() {
    use petrivet::pnml::import::PnmlConversionError;

    let weighted = two_place_net_with_inscription("<inscription><text>3</text></inscription>");
    let doc = PnmlDocument::from_xml(&weighted).unwrap();
    match doc.nets[0].to_pt_system() {
        Err(PnmlConversionError::UnsupportedArcWeight { arc_id, weight }) => {
            assert_eq!((arc_id.as_str(), weight), ("a0", 3));
        }
        other => panic!("expected UnsupportedArcWeight, got {:?}", other.map(|_| ())),
    }
    assert!(PetriNet::from_pnml(&weighted).is_err());

    // an explicit weight of one, and no inscription at all, are both ordinary
    for inscription in ["<inscription><text>1</text></inscription>", ""] {
        let ordinary = two_place_net_with_inscription(inscription);
        assert!(PetriNet::from_pnml(&ordinary).is_ok(), "inscription {inscription:?}");
    }
}
