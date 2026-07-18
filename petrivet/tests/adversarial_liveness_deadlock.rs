//! Adversarial soundness tests for the liveness and deadlock-freedom analyzers,
//! cross-checked against an INDEPENDENT brute-force state-space oracle that shares
//! no code with the library (only the primitive `try_fire` / `is_enabled`
//! simulation calls). The cardinal sin these tests hunt is an unsound verdict: a
//! `true` from `is_live()` on a non-live net, or from `is_deadlock_free()` on a
//! net that can reach a total deadlock.
//!
//! # Provenance / adaptation notes
//!
//! Adapted from a preserved "Workflow 2" verification file written against a
//! `petrivet::model` registry/driver layer (`Verdict` / `Query` / `Budget` /
//! `decide`) that does NOT exist on this branch. The driver-vs-cascade ("S2")
//! scaffolding has been removed; the assertions now target the real public
//! analysis surface (`is_live`, `is_deadlock_free`, `deadlocks`).
//!
//! The historical FINDING the original documented -- a false `deadlock_free` when
//! the INITIAL marking is itself a total deadlock -- has since been FIXED (see
//! `Deadlocks::initial_deadlock` in `api/system/deadlock_freedom.rs`, which tests
//! the seed marking explicitly). The corresponding test below therefore pins the
//! SOUND verdict (`!is_deadlock_free()`), not the old buggy `true`.

use petrivet::prelude::{Marking, Net, NetBuilder, NetClass, PetriNet, Place, Transition};
use std::collections::HashMap;

// ---------- independent brute-force oracle ----------
//
// Shares NO analysis code with the library (only primitive try_fire/is_enabled).
// BFS over reachable markings INCLUDING the initial marking; computes deadlock
// presence and per-transition L4 liveness via backward closure.

fn marking_key(m: &Marking<u32>, places: &[Place]) -> Vec<u32> {
    places.iter().map(|&p| m.get(p)).collect()
}

struct Oracle {
    transitions: Vec<Transition>,
    reachable: Vec<Vec<u32>>,
    /// (transition-list-index, next-state-index) per state.
    succ: Vec<Vec<(usize, usize)>>,
    complete: bool,
}

// Every net exercised here is either tiny-and-bounded (state space < 100) or
// deliberately unbounded (caught by `complete == false`). 50k is far above the
// former and reached quickly for the latter.
const CAP: usize = 50_000;

fn explore(net: &Net, m0: &Marking<u32>) -> Oracle {
    let places: Vec<Place> = net.places().collect();
    let transitions: Vec<Transition> = net.transitions().collect();
    let mut reachable: Vec<Vec<u32>> = Vec::new();
    let mut index: HashMap<Vec<u32>, usize> = HashMap::new();
    let mut markings: Vec<Marking<u32>> = Vec::new();
    let mut succ: Vec<Vec<(usize, usize)>> = Vec::new();

    let k0 = marking_key(m0, &places);
    index.insert(k0.clone(), 0);
    reachable.push(k0);
    markings.push(m0.clone());
    succ.push(Vec::new());

    let mut frontier = vec![0usize];
    let mut complete = true;
    while let Some(i) = frontier.pop() {
        if reachable.len() > CAP {
            complete = false;
            break;
        }
        let m = markings[i].clone();
        let mut local = Vec::new();
        for (ti, &t) in transitions.iter().enumerate() {
            let mut sys = PetriNet::new(net, m.clone());
            if sys.try_fire(t).is_ok() {
                let nm = sys.marking();
                let key = marking_key(&nm, &places);
                let j = if let Some(&j) = index.get(&key) {
                    j
                } else {
                    let j = reachable.len();
                    index.insert(key.clone(), j);
                    reachable.push(key);
                    markings.push(nm);
                    succ.push(Vec::new());
                    frontier.push(j);
                    j
                };
                local.push((ti, j));
            }
        }
        succ[i] = local;
    }

    Oracle { transitions, reachable, succ, complete }
}

impl Oracle {
    /// Is some reachable marking (INCLUDING m0) a total deadlock?
    fn has_deadlock(&self) -> bool {
        self.succ.iter().any(std::vec::Vec::is_empty)
    }

    /// Textbook L4-liveness: for every transition and every reachable marking M,
    /// some marking reachable from M enables the transition.
    fn is_live(&self) -> bool {
        let n = self.reachable.len();
        let mut rev: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, s) in self.succ.iter().enumerate() {
            for &(_, j) in s {
                rev[j].push(i);
            }
        }
        for ti in 0..self.transitions.len() {
            let mut eventually = vec![false; n];
            let mut stack: Vec<usize> = Vec::new();
            for (i, s) in self.succ.iter().enumerate() {
                if s.iter().any(|&(t, _)| t == ti) {
                    eventually[i] = true;
                    stack.push(i);
                }
            }
            while let Some(i) = stack.pop() {
                for &p in &rev[i] {
                    if !eventually[p] {
                        eventually[p] = true;
                        stack.push(p);
                    }
                }
            }
            if !eventually.iter().all(|&b| b) {
                return false;
            }
        }
        true
    }
}

// ---------- builders ----------

fn live_cycle() -> (Net, Place, Place, Transition, Transition) {
    let mut b = NetBuilder::new();
    let [p0, p1] = b.add_places();
    let [t0, t1] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p1));
    b.add_arc((p1, t1));
    b.add_arc((t1, p0));
    let net = b.build().expect("valid cycle");
    (net, p0, p1, t0, t1)
}

/// A family of small, CONNECTED, bounded candidate nets used to scan for the
/// BLOCKER-2 profile (general, CHC-Ok, deadlock-free, not live) and a second
/// general shape.
fn candidates() -> Vec<(&'static str, Net, Marking<u32>)> {
    let mut out: Vec<(&'static str, Net, Marking<u32>)> = Vec::new();

    // "blocker2_non_fc": general (non-free-choice), bounded, deadlock-free, the
    // Commoner-Hack universal HOLDS, yet NOT live.
    //   t0:{p1,p3}->{p2}, t1:{p2,p3}->{p0,p1}, t2:{p2}->{p0,p1}, t3:{p0,p1}->{p1,p3}
    //   marked {p0,p1,p3}. (t1,t2 share input p2 with different presets -> not FC.)
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
        out.push(("blocker2_non_fc", net, m0));
    }

    // "ab_cycle_plus_asym_t": a<->b live cycle plus tc:{b,c}->{c} (asymmetric).
    {
        let mut b = NetBuilder::new();
        let [pa, pb, pc] = b.add_places();
        let [ta, tb, tc] = b.add_transitions();
        b.add_arc((pa, ta));
        b.add_arc((ta, pb));
        b.add_arc((pb, tb));
        b.add_arc((tb, pa));
        b.add_arc((pb, tc));
        b.add_arc((pc, tc));
        b.add_arc((tc, pc));
        let net = b.build().expect("valid net");
        let m0: Marking<u32> = [(pa, 1), (pc, 1)].into();
        out.push(("ab_cycle_plus_asym_t", net, m0));
    }

    out
}

// ============================================================================
//  The FIXED m0-deadlock case: when the INITIAL marking is itself a total
//  deadlock, the system is NOT deadlock-free. This pins the fix (the explorer's
//  seed marking is now tested), for both an empty and a marked m0-deadlock.
// ============================================================================

#[test]
fn initial_marking_deadlock_is_not_deadlock_free() {
    // (a) The unmarked two-place cycle: at m0 = [] neither transition is enabled,
    // so m0 is itself a reachable total deadlock.
    let (net, _p0, _p1, t0, t1) = live_cycle();
    let m0: Marking<u32> = [].into();
    let sys = PetriNet::new(&net, m0.clone());
    assert!(!sys.is_enabled(t0) && !sys.is_enabled(t1), "m0 enables no transition");

    let o = explore(&net, &m0);
    assert!(o.complete, "bounded");
    assert!(o.has_deadlock(), "the initial marking is a total deadlock");
    assert_eq!(o.reachable.len(), 1, "nothing fires, so only m0 is reachable");

    // SOUND verdict (post-fix): m0 is a reachable deadlock -> NOT deadlock-free.
    assert!(
        !sys.is_deadlock_free(),
        "an m0-deadlock must be detected -- the seed marking is reachable"
    );
    let witness = sys.deadlocks().next().expect("deadlocks() must yield the m0 deadlock");
    let wsys = PetriNet::new(&net, witness);
    assert!(
        net.transitions().all(|t| !wsys.is_enabled(t)),
        "the yielded deadlock witness enables no transition"
    );
    // Liveness on the same dead net is correctly false.
    assert!(!sys.is_live(), "the unmarked cycle is dead, hence not live");

    // (b) A MARKED net whose m0 still enables no transition: t0 needs BOTH p0 and
    // p1; mark only p0 -> t0 disabled -> m0 is a deadlock. Confirms the fix is
    // about the initial marking being a deadlock, not specifically the empty one.
    let mut b = NetBuilder::new();
    let [p0, p1, p2] = b.add_places();
    let [t0m] = b.add_transitions();
    b.add_arc((p0, t0m));
    b.add_arc((p1, t0m));
    b.add_arc((t0m, p2));
    let net2 = b.build().expect("valid net");
    let m0b: Marking<u32> = [(p0, 1)].into(); // p1 empty -> t0 disabled -> deadlock
    let sys2 = PetriNet::new(&net2, m0b.clone());
    assert!(!sys2.is_enabled(t0m), "t0 disabled (p1 empty) -> m0 is a deadlock");
    let o2 = explore(&net2, &m0b);
    assert!(o2.complete && o2.has_deadlock(), "m0 enables no transition -> a reachable deadlock");
    assert!(
        !sys2.is_deadlock_free(),
        "a marked m0-deadlock must also be detected"
    );
}

// ============================================================================
//  BLOCKER-2: a GENERAL, bounded, deadlock-free net for which the Commoner-Hack
//  criterion HOLDS yet the net is NOT live. `is_live()` must NOT return true;
//  `is_deadlock_free()` must return true and the oracle must confirm both.
// ============================================================================

#[test]
fn blocker2_general_chc_holds_deadlock_free_but_not_live() {
    let mut found = false;
    for (name, net, m0) in candidates() {
        let sys = PetriNet::new(&net, m0.clone());
        let o = explore(&net, &m0);
        if !o.complete {
            continue;
        }
        let chc_ok = sys.commoner_hack_criterion().is_ok();
        let oracle_live = o.is_live();
        let oracle_df = !o.has_deadlock();
        let is_fc = net.class().is_free_choice();

        if !is_fc && chc_ok && oracle_df && !oracle_live {
            found = true;

            // CARDINAL: liveness must NOT be reported on a non-live net.
            assert!(
                !sys.is_live(),
                "[{name}] CARDINAL SIN: is_live() on a non-live general CHC-Ok net"
            );
            // Deadlock-freedom holds here and is the TRUE answer.
            assert!(sys.is_deadlock_free(), "[{name}] the net is deadlock-free");
            assert!(oracle_df, "[{name}] oracle confirms no reachable deadlock");
        }
    }
    assert!(
        found,
        "the suite must contain a bounded, deadlock-free, CHC-holding, NON-free-choice, \
         NON-live net (the BLOCKER-2 shape)"
    );
}

// ============================================================================
//  A genuinely deadlocking net (a reachable deadlock reached by FIRING, not m0):
//  `is_deadlock_free()` must be false and `deadlocks()` must yield a genuine
//  total-deadlock witness. CHC must be Err (CHC is sufficient for DF).
// ============================================================================

#[test]
fn deadlocking_net_not_deadlock_free_with_genuine_witness() {
    // Free choice at p0: t0 cycles p0<->p1 OR t1 drains p0 into a sink p2 (no
    // outgoing transition). Firing t1 reaches the total deadlock {p2:1}. m0 is
    // NOT a deadlock, so this exercises the successor path.
    let mut b = NetBuilder::new();
    let [p0, p1, p2] = b.add_places();
    let [t0, t1] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p1));
    b.add_arc((p1, t0));
    b.add_arc((p0, t1));
    b.add_arc((t1, p2)); // p2 is a sink
    let net = b.build().expect("valid net");
    let m0: Marking<u32> = [(p0, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    let o = explore(&net, &m0);
    assert!(o.complete);
    assert!(o.has_deadlock(), "firing t1 reaches a total deadlock");

    assert!(
        sys.commoner_hack_criterion().is_err(),
        "a deadlocking net cannot pass CHC (CHC is sufficient for DF)"
    );

    assert!(!sys.is_deadlock_free(), "CARDINAL: a deadlocking net is not deadlock-free");
    let witness = sys.deadlocks().next().expect("deadlocks() carries a witness marking");
    let wsys = PetriNet::new(&net, witness);
    assert!(
        net.transitions().all(|t| !wsys.is_enabled(t)),
        "the deadlock witness must be a genuine total deadlock"
    );
    assert!(!sys.is_live(), "a deadlocking net is not live");
}

// ============================================================================
//  A live free-choice net (Esparza Fig 5.3): `is_live()` true via CHC, and both
//  the independent oracle and `is_deadlock_free()` confirm it.
// ============================================================================

#[test]
fn live_free_choice_net_is_live_and_deadlock_free() {
    let mut b = NetBuilder::new();
    let [s1, s2, s3, s4, s5, s6, s7, s8] = b.add_places();
    let [t1, t2, t3, t4, t5, t6, t7] = b.add_transitions();
    b.add_arc((s1, t1)); b.add_arc((s2, t1));
    b.add_arc((s1, t2)); b.add_arc((s2, t2));
    b.add_arc((t1, s3)); b.add_arc((t1, s4));
    b.add_arc((t2, s5)); b.add_arc((t2, s6));
    b.add_arc((s3, t3)); b.add_arc((t3, s7));
    b.add_arc((s4, t4)); b.add_arc((t4, s8));
    b.add_arc((s5, t5)); b.add_arc((t5, s7));
    b.add_arc((s6, t6)); b.add_arc((t6, s8));
    b.add_arc((s7, t7)); b.add_arc((s8, t7));
    b.add_arc((t7, s1)); b.add_arc((t7, s2));
    let net = b.build().expect("valid fc");
    assert_eq!(net.class(), NetClass::FreeChoice);
    let m0: Marking<u32> = [(s1, 1), (s2, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    assert!(sys.commoner_hack_criterion().is_ok(), "CHC holds");

    let o = explore(&net, &m0);
    assert!(o.complete, "bounded");
    assert!(o.is_live(), "independent oracle: the net is live");
    assert!(!o.has_deadlock(), "no deadlock");

    assert!(sys.is_live(), "the library must agree the FC net is live");
    assert!(sys.is_deadlock_free(), "live => deadlock-free");
}

// ============================================================================
//  UNBOUNDED net with a reachable deadlock: the analyzers must TERMINATE and
//  return sound verdicts. `try_build_reachability_graph()` errs (unbounded);
//  firing t1 permanently disables the pump, so the net can deadlock and is not
//  live. Guards against non-termination and against a false `deadlock_free`.
// ============================================================================

#[test]
fn unbounded_net_with_reachable_deadlock_terminates_soundly() {
    // t0: {p0} -> {p0, p1}  (self-loop keeps t0 enabled; p1 pumps -> unbounded)
    // t1: {p0, p2} -> {p2}  (consumes p0 without returning it -> disables the pump)
    let mut b = NetBuilder::new();
    let [p0, p1, p2] = b.add_places();
    let [t0, t1] = b.add_transitions();
    b.add_arc((p0, t0));
    b.add_arc((t0, p0));
    b.add_arc((t0, p1));
    b.add_arc((p0, t1));
    b.add_arc((p2, t1));
    b.add_arc((t1, p2));
    let net = b.build().expect("valid net");
    let m0: Marking<u32> = [(p0, 1), (p2, 1)].into();
    let sys = PetriNet::new(&net, m0.clone());

    // Unbounded: no exact reachability graph (p1 grows without bound).
    assert!(sys.is_efficiently_live().is_none(), "general net: no efficient liveness path");
    assert!(sys.try_build_reachability_graph().is_err(), "must be unbounded");

    // Sound verdicts, reached without exhausting the (infinite) state space:
    // firing t1 disables both transitions, so the net can deadlock and is not live.
    assert!(!sys.is_deadlock_free(), "firing t1 reaches a deadlock -> not deadlock-free");
    let witness = sys.deadlocks().next().expect("deadlocks() finds the reachable deadlock");
    let wsys = PetriNet::new(&net, witness);
    assert!(net.transitions().all(|t| !wsys.is_enabled(t)), "genuine deadlock witness");
    assert!(!sys.is_live(), "the net is not live");
}

// ============================================================================
//  The brute-force soundness sweep: over a family of small bounded nets, both
//  `is_live()` and `is_deadlock_free()` must EXACTLY match the independent
//  oracle. Any divergence is a finding. Every deadlock refutation must carry a
//  genuine total-deadlock witness.
// ============================================================================

#[test]
fn brute_force_soundness_sweep_matches_independent_oracle() {
    let mut nets: Vec<(&'static str, Net, Marking<u32>)> = Vec::new();

    {
        let (net, p0, _p1, _t0, _t1) = live_cycle();
        nets.push(("live_circuit", net, [(p0, 1)].into()));
    }
    {
        // live marked graph (every circuit marked)
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0)); b.add_arc((p0, t1));
        b.add_arc((t1, p1)); b.add_arc((p1, t0));
        b.add_arc((t0, p2)); b.add_arc((p2, t1));
        let net = b.build().expect("mg");
        nets.push(("live_marked_graph", net, [(p1, 1), (p2, 1)].into()));
    }
    {
        // mutex (live and bounded)
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();
        b.add_arcs((idle1, t_req1, wait1, t_enter1, crit1, t_exit1, idle1));
        b.add_arcs((idle2, t_req2, wait2, t_enter2, crit2, t_exit2, idle2));
        b.add_arcs((mutex, t_enter1, mutex));
        b.add_arcs((mutex, t_enter2, mutex));
        let net = b.build().expect("mutex");
        nets.push(("mutex", net, [(idle1, 1), (idle2, 1), (mutex, 1)].into()));
    }
    {
        // deadlocking free-choice net (deadlock is a SUCCESSOR, not m0)
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1)); b.add_arc((p1, t0));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        let net = b.build().expect("dl");
        nets.push(("deadlocking_fc", net, [(p0, 1)].into()));
    }
    for (name, net, m0) in candidates() {
        nets.push((name, net, m0));
    }

    for (name, net, m0) in &nets {
        let o = explore(net, m0);
        assert!(o.complete, "[{name}] fixture must be bounded for an exact oracle");

        // Skip m0-deadlock nets (pinned separately); this sweep guards the rest.
        let sys = PetriNet::new(net, m0.clone());
        let m0_is_deadlock = net.transitions().all(|t| !sys.is_enabled(t));
        if m0_is_deadlock {
            continue;
        }

        let oracle_live = o.is_live();
        let oracle_df = !o.has_deadlock();

        assert_eq!(
            sys.is_live(),
            oracle_live,
            "[{name}] is_live() disagrees with the independent oracle (class {:?})",
            net.class(),
        );
        assert_eq!(
            sys.is_deadlock_free(),
            oracle_df,
            "[{name}] is_deadlock_free() disagrees with the independent oracle (class {:?})",
            net.class(),
        );

        if !sys.is_deadlock_free() {
            let witness = sys.deadlocks().next().expect("[{name}] refutation carries a witness");
            let wsys = PetriNet::new(net, witness);
            assert!(
                net.transitions().all(|t| !wsys.is_enabled(t)),
                "[{name}] deadlock witness must enable no transition"
            );
        }
    }
}
