//! Adversarial soundness tests for the reachability / coverability / boundedness
//! analyzers. Each net is small enough that the true answer is computed by hand
//! from Petri-net theory (and, for the bounded ones, is exact by inspection).
//! The cardinal sin these hunt is a false *positive* safety verdict: a `true`
//! from `is_reachable` on an unreachable target, or from `is_bounded` on an
//! unbounded net.
//!
//! # Provenance / adaptation notes
//!
//! Adapted from a preserved "Workflow 2" verification file written against a
//! `petrivet::model` registry/driver layer (`Verdict` / `Query` / `Budget` /
//! `decide`) that does NOT exist on this branch. The driver-vs-cascade ("S2")
//! comparisons collapse to the underlying public API once the driver is removed,
//! so the vacuous S2 sweeps were dropped; what remains are the adversarial
//! soundness probes against `is_reachable`, `is_coverable`, and `is_bounded`
//! that are NOT already covered by the in-crate unit tests (token-sum overflow,
//! a CHC-satisfying-but-unbounded asymmetric-choice net, a non-SC marked graph
//! with a source, equal-sum-but-unreachable targets on a non-SC state machine).

use petrivet::prelude::{Marking, Net, NetBuilder, NetClass, PetriNet, Place};

// --------------------------------------------------------------------------
// Fixtures
// --------------------------------------------------------------------------

/// Strongly-connected two-place circuit: `p0 -> t0 -> p1 -> t1 -> p0`.
fn sc_state_machine() -> (Net, Place, Place) {
    let mut b = NetBuilder::new();
    let [p0, p1] = b.add_places();
    let [t0, t1] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p1));
    b.add_arc((p1, t1));
    b.add_arc((t1, p0));
    (b.build().expect("valid SC cycle"), p0, p1)
}

/// A NON-strongly-connected state machine: a forward chain plus a free-choice
/// fork to a sink, so equal-token-sum targets exist that are nonetheless
/// unreachable. `p0 -> t0 -> p1 -> t1 -> p2`, plus `p0 -> t2 -> p3`.
fn non_sc_state_machine() -> (Net, Place, Place, Place, Place) {
    let mut b = NetBuilder::new();
    let [p0, p1, p2, p3] = b.add_places();
    let [t0, t1, t2] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p1));
    b.add_arc((p1, t1));
    b.add_arc((t1, p2));
    b.add_arc((p0, t2));
    b.add_arc((t2, p3));
    (b.build().expect("valid non-SC SM"), p0, p1, p2, p3)
}

/// Unbounded producer: `t0: p0 -> p0, p1` (p0 conserved at 1, p1 pumps).
fn unbounded_producer() -> (Net, Place, Place) {
    let mut b = NetBuilder::new();
    let [p0, p1] = b.add_places();
    let [t0] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p0));
    b.add_arc((t0, p1));
    (b.build().expect("valid producer"), p0, p1)
}

// --------------------------------------------------------------------------
// REACHABILITY
// --------------------------------------------------------------------------

/// On a NON-strongly-connected state machine, an equal-token-sum target that is
/// genuinely UNREACHABLE must not be reported reachable (the equal-sum theorem
/// requires strong connectivity). Token conservation still refutes a
/// different-sum target, and a target reachable within the forward component is
/// still decided reachable.
#[test]
fn reach_non_sc_state_machine_equal_sum_unreachable_not_false_positive() {
    let (net, p0, _p1, p2, _p3) = non_sc_state_machine();
    // Seed in the MIDDLE of the forward chain: from {p1:1} the token can only
    // move forward to p2; it can never return to the source place p0.
    let m0: Marking<u32> = [(_p1, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());
    assert!(!sys.is_strongly_connected(), "fixture must be NON-strongly-connected");

    // {p0:1}: SAME token sum (1), but p0 is a source with no inflow => UNREACHABLE.
    let unreachable_equal_sum: Marking<u32> = [(p0, 1)].into();
    assert!(
        !sys.is_reachable(&unreachable_equal_sum),
        "FALSE POSITIVE: an equal-sum but unreachable target on a non-SC state machine \
         must not be reported reachable"
    );

    // {p2:1}: reachable by firing t1.
    let reachable: Marking<u32> = [(p2, 1)].into();
    assert!(sys.is_reachable(&reachable), "{{p2:1}} is reachable by firing t1");

    // {p0:2}: different token sum (2 != 1) => refuted by conservation.
    let different_sum: Marking<u32> = [(p0, 2)].into();
    assert!(!sys.is_reachable(&different_sum), "different token sum => unreachable");
}

/// A strongly-connected state machine: every equal-token-sum target is reachable
/// (Theorem 5.1.5). The 3-place SC cycle with one token reaches each of the three
/// single-place markings.
#[test]
fn reach_sc_state_machine_all_equal_sum_targets_reachable() {
    let mut b = NetBuilder::new();
    let [p0, p1, p2] = b.add_places();
    let [t0, t1, t2] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p1));
    b.add_arc((p1, t1));
    b.add_arc((t1, p2));
    b.add_arc((p2, t2));
    b.add_arc((t2, p0));
    let net = b.build().expect("valid SC 3-cycle");
    let m0: Marking<u32> = [(p0, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());
    assert!(sys.is_strongly_connected());
    for target in [
        Into::<Marking<u32>>::into([(p0, 1)]),
        [(p1, 1)].into(),
        [(p2, 1)].into(),
    ] {
        assert!(sys.is_reachable(&target), "every equal-sum target is reachable in an SC SM");
    }
}

/// Hand-truth table on a strongly-connected marked graph (1-safe).
#[test]
fn reach_marked_graph_hand_truth() {
    let mut b = NetBuilder::new();
    let [pa, p0, p2] = b.add_places();
    let [t0, t1] = b.add_transitions();
    // t0: {pa} -> {p0, p2}; t1: {p0, p2} -> {pa}
    b.add_arc((pa, t0));
    b.add_arc((t0, p0));
    b.add_arc((t0, p2));
    b.add_arc((p0, t1));
    b.add_arc((p2, t1));
    b.add_arc((t1, pa));
    let net = b.build().expect("valid MG");
    let m0: Marking<u32> = [(pa, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    for (target, expect) in [
        (Into::<Marking<u32>>::into([(p0, 1), (p2, 1)]), true), // fire t0
        ([(pa, 1)].into(), true),                               // identity
        ([(p0, 1)].into(), false),                              // firing t0 also marks p2
        ([(p0, 2), (p2, 2)].into(), false),                     // 1-safe MG, sum too big
    ] {
        let target: Marking<u32> = target;
        assert_eq!(
            sys.is_reachable(&target),
            expect,
            "is_reachable disagrees with hand-truth for {target:?}",
        );
    }
}

/// Reachability on an UNBOUNDED net must TERMINATE and must not report a
/// genuinely unreachable target as reachable. `p0` is conserved at 1 by the
/// self-loop, so `{p0:2}` is unreachable no matter how far `p1` pumps.
#[test]
fn reach_unbounded_net_conserved_place_no_false_positive() {
    let (net, p0, _p1) = unbounded_producer();
    let m0: Marking<u32> = [(p0, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    let conserved_violation: Marking<u32> = [(p0, 2)].into();
    assert!(
        !sys.is_reachable(&conserved_violation),
        "p0 is conserved at 1; {{p0:2}} must not be reported reachable (and the query \
         must terminate on an unbounded net)"
    );
}

/// OVERFLOW: `is_efficiently_reachable` (Circuit arm, `reachability.rs:102`)
/// decides reachability via `self.marking.sum() == target.total_tokens()`. Both
/// sums are computed in `u32` (`IdxMarking::sum` / `Marking::total_tokens`, which
/// call `Iterator::sum::<u32>()`). A marking whose *total* token count exceeds
/// `u32::MAX` — even though every place count individually fits in `u32` —
/// overflows that sum. The sound answer: a target whose true total differs is
/// unreachable (a circuit conserves tokens); one whose true total equals the
/// initial is reachable.
///
/// EXPOSES A REAL LIBRARY BUG (ignored, not fixed — out of scope): with
/// `m0 = {p0: 3e9, p1: 2e9}` (true total 5e9 > u32::MAX) this call OVERFLOWS.
/// In a debug build `is_reachable` PANICS ("attempt to add with overflow" at
/// `core/marking.rs:49`); in a release build the sum WRAPS to 705_032_704, so a
/// target with that literal total compares equal and is minted a false
/// `reachable` verdict — an unsound false positive. The same u32-sum pattern
/// recurs in `analyze_reachability` (reachability.rs:146-147). A fix would sum
/// token counts in a wider type (u64/exact). When fixed, un-ignore: the sound
/// assertions below should then pass.
#[test]
#[ignore = "exposes a real bug: is_efficiently_reachable/analyze_reachability sum token \
counts in u32 (marking.rs sum/total_tokens); a marking whose total exceeds u32::MAX \
overflows — panics in debug, wraps to a false 'reachable' in release"]
fn reach_token_sum_overflow_no_false_positive() {
    let (net, p0, p1) = sc_state_machine();
    // initial true total 5e9 (individually valid u32; the SUM overflows u32).
    let m0: Marking<u32> = [(p0, 3_000_000_000_u32), (p1, 2_000_000_000_u32)].into();
    let sys = PetriNet::new(&net, m0.clone());

    // target true total 705_032_704 (the u32-wrap of 5e9): DIFFERENT true total,
    // hence unreachable in a token-conserving circuit.
    let wrapped_equal: Marking<u32> = [(p0, 705_032_704_u32)].into();
    assert!(
        !sys.is_reachable(&wrapped_equal),
        "a u32 token-sum wrap must not mint a reachable verdict on a conserving circuit"
    );

    // a genuinely equal-true-total target IS reachable (capability preserved).
    let equal_total: Marking<u32> = [(p0, 2_000_000_000_u32), (p1, 3_000_000_000_u32)].into();
    assert!(
        sys.is_reachable(&equal_total),
        "an equal-true-total target is reachable in a circuit"
    );
}

// --------------------------------------------------------------------------
// COVERABILITY
// --------------------------------------------------------------------------

/// Trivial cover, an uncoverable target on a conservative cycle, and an
/// omega-coverable target on an unbounded producer.
#[test]
fn cover_trivial_uncoverable_and_omega() {
    // (a) Two-place conservative cycle, one token.
    let (net, p0, p1) = sc_state_machine();
    let m0: Marking<u32> = [(p0, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    assert!(sys.is_coverable(Into::<Marking<u32>>::into([(p0, 1)])), "m0 trivially covers {{p0:1}}");
    assert!(
        !sys.is_coverable(Into::<Marking<u32>>::into([(p0, 1), (p1, 1)])),
        "sum 2 > 1 on a conservative cycle => uncoverable"
    );

    // (b) Unbounded producer: {p1:5} is coverable by pumping t0.
    let (pump, q0, q1) = unbounded_producer();
    let pm0: Marking<u32> = [(q0, 1)].into();
    let psys = PetriNet::new(&pump, pm0.clone());
    assert!(
        psys.is_coverable(Into::<Marking<u32>>::into([(q1, 5)])),
        "an omega-coverable target must be coverable"
    );
    let _ = q0;
}

// --------------------------------------------------------------------------
// BOUNDEDNESS
// --------------------------------------------------------------------------

/// THE CARDINAL-SIN BOUNDARY: an asymmetric-choice net that satisfies the
/// Commoner-Hack criterion (hence deadlock-free) but is genuinely UNBOUNDED. CHC
/// is sufficient for *liveness* on AC nets, NOT for boundedness. A `true` from
/// `is_bounded` here would be a false safety verdict.
#[test]
fn bound_ac_chc_net_unbounded_is_not_falsely_bounded() {
    let mut b = NetBuilder::new();
    let [pa, pb, punb] = b.add_places();
    let [t_pump, t1] = b.add_transitions();
    // t_pump: {pa} -> {pa, punb}  (pa conserved, punb pumped without bound)
    b.add_arc((pa, t_pump));
    b.add_arc((t_pump, pa));
    b.add_arc((t_pump, punb));
    // t1: {pa, pb} -> {pa, pb}  (makes place-postsets incomparable => true AC)
    b.add_arc((pa, t1));
    b.add_arc((pb, t1));
    b.add_arc((t1, pa));
    b.add_arc((t1, pb));
    let net = b.build().expect("valid AC net");
    assert_eq!(net.class(), NetClass::AsymmetricChoice, "must be a true AC net");
    let m0: Marking<u32> = [(pa, 1), (pb, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    assert!(sys.commoner_hack_criterion().is_ok(), "CHC holds on this net");
    assert!(
        sys.is_coverable(Into::<Marking<u32>>::into([(punb, 1000)])),
        "punb pumps without bound"
    );
    assert!(
        !sys.is_bounded(),
        "CARDINAL SIN: a CHC-satisfying-but-unbounded AC net must not be reported bounded"
    );
}

/// A NON-strongly-connected marked graph with a source transition is genuinely
/// unbounded; the structural class fact (marked graph) must NOT be mistaken for
/// a boundedness proof.
#[test]
fn bound_non_sc_marked_graph_unbounded() {
    // t0 (source) -> p0 -> t1 -> p1 -> t2 (sink). t0 always enabled => p0 pumps.
    let mut b = NetBuilder::new();
    let [p0, p1] = b.add_places();
    let [t0, t1, t2] = b.add_transitions();
    b.add_arc((t0, p0));
    b.add_arc((p0, t1));
    b.add_arc((t1, p1));
    b.add_arc((p1, t2));
    let net = b.build().expect("valid MG");
    assert_eq!(net.class(), NetClass::MarkedGraph);
    let m0: Marking<u32> = [(p0, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());
    assert!(!sys.is_strongly_connected());
    assert!(
        !sys.is_bounded(),
        "FALSE-BOUNDED: a non-SC marked graph with a source transition is unbounded"
    );
}
