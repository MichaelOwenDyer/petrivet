//! M2 · C1/C2/C5/C7 — the checker invariants, exercised from *outside* the crate.
//!
//! This integration test is the **external-consumer** view of the M2 deliverable.
//! A downstream crate cannot mint a [`Decided`](petrivet::model) candidate (its
//! constructors are `pub(crate)` — that is the M1 firewall: only in-crate deciders
//! propose candidates, and the full `accept` gate path is tested *inside* the
//! crate). What a consumer *can* do, and what this file proves, is:
//!
//! - **C1, directly:** build a [`Certificate`](petrivet::model::Certificate) and
//!   call its public `check` against a net it owns.
//! - **C2 (checking as a test invariant):** a valid witness `check`s `true`, and
//!   the verdict it witnesses agrees with ground truth (the public `is_*`
//!   surface).
//! - **Original-net independence (the load-bearing M2 property):** a witness from
//!   a *different generator* for the same `(net, query, verdict)` validates
//!   identically, and a witness for a *different net* or *different query* is
//!   rejected — the checker is provenance-blind and re-establishes the property
//!   against the original net.
//! - **C5/C7 surfaces are public:** the trusted-base ledger (`f`) and the
//!   checkable-frontier map are readable by a consumer (the thesis renders them).

use ahash::{HashMap, HashSet};
use petrivet::model::{
    frontier_table, trusted_base, CheckComplexity, Certificate, FiringSequenceCert,
    ParikhVectorCert, Query, SiphonTrapClaim, SiphonTrapCoverCert, TokenConservationCert,
};
use petrivet::prelude::{Marking, Net, NetBuilder, Place, Transition};

/// A two-place cycle `p0 -> t0 -> p1 -> t1 -> p0` (a state machine / circuit).
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

/// A marked graph `t0: {pa} -> {p0, p2}`, `t1: {p0, p2} -> {pa}`.
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

// ---------------------------------------------------------------------------
// C2 — a valid witness checks true, and agrees with ground truth.
// ---------------------------------------------------------------------------

/// A firing-sequence witness `check`s `true` for the marking it reaches, and the
/// library's own analysis agrees the target is reachable.
#[test]
fn firing_sequence_checks_and_agrees_with_ground_truth() {
    let (net, p0, t0, p1, _t1) = two_place_cycle();
    let m0: Marking<u32> = [(p0, 1)].into();
    let target: Marking<u32> = [(p1, 1)].into();
    let cert = FiringSequenceCert { sequence: Box::new([t0]) };

    assert!(cert.check(&net, &m0, &Query::Reachable(target.clone())));

    // Ground truth via the public surface.
    let sys = net.with_initial_marking([(p0, 1)]);
    assert!(sys.is_reachable(&target), "ground truth: target is reachable");
}

/// A token-conservation witness `check`s `true` for an unreachable target on a
/// state machine, and the library agrees the target is unreachable.
#[test]
fn token_conservation_checks_and_agrees_with_ground_truth() {
    let (net, p0, _t0, _p1, _t1) = two_place_cycle();
    let m0: Marking<u32> = [(p0, 1)].into();
    let target: Marking<u32> = [(p0, 2)].into(); // sum 2 ≠ 1 in a one-token cycle

    assert!(TokenConservationCert.check(&net, &m0, &Query::Reachable(target.clone())));

    let sys = net.with_initial_marking([(p0, 1)]);
    assert!(!sys.is_reachable(&target), "ground truth: target is unreachable");
}

/// A siphon/trap cover `check`s `true` for a live net, and the library agrees the
/// net is live.
#[test]
fn siphon_trap_cover_checks_and_agrees_with_ground_truth() {
    let (net, p0, _t0, p1, _t1) = two_place_cycle();
    let m0: Marking<u32> = [(p0, 1)].into();
    let mut set: HashSet<Place> = HashSet::default();
    set.insert(p0);
    set.insert(p1);
    let cert = SiphonTrapCoverCert {
        claim: SiphonTrapClaim::Live,
        cover: Box::new([(set.clone(), set)]),
    };

    assert!(cert.check(&net, &m0, &Query::Trivial));

    let sys = net.with_initial_marking([(p0, 1)]);
    assert!(sys.is_live(), "ground truth: the marked cycle is live");
}

// ---------------------------------------------------------------------------
// Original-net independence: a different generator's witness validates
// identically.
// ---------------------------------------------------------------------------

/// A firing-sequence witness and a Parikh-vector witness — from two independent
/// "generators" — validate the SAME `(net, query, verdict)` identically. The
/// checker is provenance-blind: only validity against the original net matters.
#[test]
fn independent_witnesses_for_the_same_verdict_validate_identically() {
    let (net, places, [t0, _t1]) = marked_graph();
    let [pa, p0, p2] = places;
    let m0: Marking<u32> = [(pa, 1)].into();
    let target: Marking<u32> = [(p0, 1), (p2, 1)].into();
    let query = Query::Reachable(target.clone());

    // Generator A: a firing-sequence witness.
    let cert_seq = FiringSequenceCert { sequence: Box::new([t0]) };
    // Generator B: a Parikh vector — a structurally different witness.
    let mut counts: HashMap<Transition, u32> = HashMap::default();
    counts.insert(t0, 1);
    let cert_parikh = ParikhVectorCert { firing_counts: counts };

    assert!(cert_seq.check(&net, &m0, &query), "firing-sequence witness validates");
    assert!(cert_parikh.check(&net, &m0, &query), "Parikh witness validates identically");

    let sys = net.with_initial_marking([(pa, 1)]);
    assert!(sys.is_reachable(&target));
}

// ---------------------------------------------------------------------------
// The firewall at the boundary: wrong net / wrong query is rejected.
// ---------------------------------------------------------------------------

/// A witness valid for net A is rejected when checked against a *different* net B
/// — the checker re-establishes the property against the net it is handed.
#[test]
fn a_witness_is_rejected_against_a_different_net() {
    let (net_a, places_a, [t0_a, _t1_a]) = marked_graph();
    let [pa_a, p0_a, p2_a] = places_a;
    let m0_a: Marking<u32> = [(pa_a, 1)].into();
    let query = Query::Reachable([(p0_a, 1), (p2_a, 1)].into());

    let mut counts: HashMap<Transition, u32> = HashMap::default();
    counts.insert(t0_a, 1);
    let cert = ParikhVectorCert { firing_counts: counts };

    assert!(cert.check(&net_a, &m0_a, &query), "valid against net A");

    // Net B: a plain cycle. The same witness, same query, checked against B —
    // the displacement does not balance and the class differs, so it is rejected.
    let (net_b, p0_b, _t0_b, _p1_b, _t1_b) = two_place_cycle();
    let m0_b: Marking<u32> = [(p0_b, 1)].into();
    assert!(
        !cert.check(&net_b, &m0_b, &query),
        "a witness for net A must not validate against net B"
    );
}

/// A firing-sequence witness valid for one query is rejected against a different
/// query — the target is part of what is checked.
#[test]
fn a_witness_is_rejected_against_a_different_query() {
    let (net, p0, t0, p1, _t1) = two_place_cycle();
    let m0: Marking<u32> = [(p0, 1)].into();
    let cert = FiringSequenceCert { sequence: Box::new([t0]) };

    // [t0] reaches [(p1,1)] — accepted for that query.
    assert!(cert.check(&net, &m0, &Query::Reachable([(p1, 1)].into())));
    // Rejected for the (different) target [(p0,1)] the token left.
    assert!(!cert.check(&net, &m0, &Query::Reachable([(p0, 1)].into())));
    // Rejected for a query-free Trivial query (it answers reachability).
    assert!(!cert.check(&net, &m0, &Query::Trivial));
}

/// An unmarked siphon/trap cover (the counterexample case) is rejected — it must
/// not be accepted as a liveness certificate on the generator's word.
#[test]
fn an_unmarked_siphon_trap_cover_is_rejected() {
    let (net, p0, _t0, p1, _t1) = two_place_cycle();
    let m0: Marking<u32> = [].into(); // unmarked: the trap holds no token
    let mut set: HashSet<Place> = HashSet::default();
    set.insert(p0);
    set.insert(p1);
    let cert = SiphonTrapCoverCert {
        claim: SiphonTrapClaim::Live,
        cover: Box::new([(set.clone(), set)]),
    };

    assert!(!cert.check(&net, &m0, &Query::Trivial), "an unmarked trap certifies nothing");

    // And the library agrees the unmarked net is not live.
    let sys = net.with_initial_marking([]);
    assert!(!sys.is_live());
}

// ---------------------------------------------------------------------------
// C5 / C7 — the ledger and the frontier map are public and coherent.
// ---------------------------------------------------------------------------

/// The trusted-base ledger (C5/A6) is public, reports `f`, and exposes the
/// bare-boolean trusted base S3 holds non-increasing.
#[test]
fn trusted_base_ledger_is_public_and_reports_f() {
    let ledger = trusted_base();
    assert!(!ledger.deciders.is_empty());
    let f = ledger.certifying_fraction();
    assert!((0.0..=1.0).contains(&f));
    assert!(ledger.trusted_base_size() >= 1, "the trusted base is non-empty at M2");
    assert!(ledger.certifying_count() >= 1, "some deciders are certifying at M2");
}

/// The checkable-frontier map (C7) is public and coherent: no checker is "built"
/// on a wall cell, and the wall is explicitly mapped.
#[test]
fn frontier_map_is_public_and_coherent() {
    let table = frontier_table();
    assert!(!table.is_empty());
    assert!(
        table
            .iter()
            .all(|e| !(e.checker_built && e.check == CheckComplexity::Wall)),
        "no checker is built on a wall cell"
    );
    assert!(
        table.iter().any(|e| e.check == CheckComplexity::Wall),
        "the wall is explicitly mapped (general liveness; ILP-only infeasibility)"
    );
}
