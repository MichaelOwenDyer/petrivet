//! Mutual exclusion protocol modeled as a Petri net.
//!
//! Two processes compete for a shared resource (mutex). Each process
//! cycles through: idle → waiting → critical → idle. Only one process
//! can hold the mutex at a time.
//!
//! ```text
//!     ┌─────────────────────────────────────────────────────┐
//!     ▼                                                     │
//!   idle1 ─→ [req1] ─→ wait1 ───┐                           │
//!                               ├─→ [enter1] ─→ crit1 ─→ [exit1]
//!                               ▲                           │
//!                               │                           │
//!                             mutex ◄───────────────────────┤
//!                               │                           │
//!                               ▼                           │
//!                               ├─→ [enter2] ─→ crit2 ─→ [exit2]
//!   idle2 ─→ [req2] ─→ wait2 ───┘                           │
//!     ▲                                                     │
//!     └─────────────────────────────────────────────────────┘
//! ```
//!
//! `mutex` is a single shared place. Each `[enter]` transition consumes
//! a token from both its `wait` place and `mutex`. Each `[exit]` transition
//! produces a token into both its `idle` place and `mutex`. Since there is
//! only one mutex token, only one process can be in its critical section
//! at a time.
//!
//! Run: `cargo run --example mutex`

use petrivet::prelude::{Net, PetriNet, NetBuilder};
use petrivet::pnml::labels::NetLabels;

fn main() {
    println!("=== Mutual Exclusion Protocol ===\n");

    let mut b = NetBuilder::new();

    // Places: process 1 states, process 2 states, shared mutex
    let [idle1, wait1, crit1] = b.add_places();
    let [idle2, wait2, crit2] = b.add_places();
    let mutex = b.add_place();

    // Transitions: request, enter critical, exit critical (per process)
    let [t_req1, t_enter1, t_exit1] = b.add_transitions();
    let [t_req2, t_enter2, t_exit2] = b.add_transitions();

    b.add_arcs((idle1, t_req1, wait1, t_enter1, crit1, t_exit1, idle1));
    b.add_arcs((idle2, t_req2, wait2, t_enter2, crit2, t_exit2, idle2));

    b.add_arcs((mutex, t_enter1, mutex));
    b.add_arcs((mutex, t_enter2, mutex));

    let net = b.build().expect("valid net");
    println!("Structural class: {}", net.class());

    let mut labels = NetLabels::new();
    labels.set_transition_name(t_req1, "req1");
    labels.set_transition_name(t_enter1, "enter1");
    labels.set_transition_name(t_exit1, "exit1");
    labels.set_transition_name(t_req2, "req2");
    labels.set_transition_name(t_enter2, "enter2");
    labels.set_transition_name(t_exit2, "exit2");

    labels.set_place_name(idle1, "idle1");
    labels.set_place_name(wait1, "wait1");
    labels.set_place_name(crit1, "crit1");
    labels.set_place_name(idle2, "idle2");
    labels.set_place_name(wait2, "wait2");
    labels.set_place_name(crit2, "crit2");
    labels.set_place_name(mutex, "mutex");

    // Initial marking: both processes idle, mutex available
    // Places: idle1, wait1, crit1, idle2, wait2, crit2, mutex
    let mut sys = PetriNet::new(&net, [(idle1, 1), (idle2, 1), (mutex, 1)]);
    // todo: fix
    // sys.labels = Some(Box::new(labels));

    println!();
    print_state(&sys, &net);

    // Simulate 12 steps, always picking the first enabled transition
    for step in 1..=12 {
        if let Some(t) = sys.fire_any() {
            let name = sys.labels.as_ref().unwrap().transition_name(t).unwrap_or("unnamed transition");
            println!("Step {step:>2}: fire {name:<8} → {:?}", sys.marking());
        } else {
            println!("Step {step:>2}: DEADLOCK");
            break;
        }

        // Safety check: both processes must never be in critical section at once
        assert!(
            sys.current_tokens(crit1) == 0 || sys.current_tokens(crit2) == 0,
            "mutual exclusion violated!"
        );
    }

    println!();
    print_state(&sys, &net);

    println!("\n--- Priority simulation: process 2 has priority ---\n");
    let mut sys = PetriNet::new(&net, [(idle1, 1), (idle2, 1), (mutex, 1)]);
    print_state(&sys, &net);

    for step in 1..=12 {
        // Prefer process 2 transitions (indices 3, 4, 5) over process 1
        let priority = [t_req2, t_enter2, t_exit2, t_req1, t_enter1, t_exit1];

        let fired = sys
            .enabled_transitions()
            .min_by_key(|t| priority.iter().position(|&p| p == *t).unwrap_or(usize::MAX))
            .and_then(|t| sys.try_fire(t).ok());

        if let Some(t) = fired {
            let name = net.labels.as_ref().unwrap().transition_name(t).unwrap_or("unnamed transition");
            println!("Step {step:>2}: fire {name:<8} → {:?}", sys.marking());
        } else {
            println!("Step {step:>2}: DEADLOCK");
            break;
        }
    }

    println!("\n--- Manual firing with try_fire ---\n");
    let mut sys = PetriNet::new(&net, [(idle1, 1), (idle2, 1), (mutex, 1)]);

    println!("Trying to enter critical section without requesting first...");
    match sys.try_fire(t_enter1) {
        Ok(_) => println!("  Entered (unexpected!)"),
        Err(e) => println!("  Blocked: {e}"),
    }

    println!("Requesting access for process 1...");
    sys.try_fire(t_req1).expect("should succeed");
    println!("  Marking: {:?}", sys.marking());

    println!("Entering critical section...");
    sys.try_fire(t_enter1).expect("should succeed");
    println!("  Marking: {:?}", sys.marking());

    println!("Process 2 requests and tries to enter...");
    sys.try_fire(t_req2).expect("should succeed");
    match sys.try_fire(t_enter2) {
        Ok(_) => println!("  Entered (mutex violation!)"),
        Err(e) => println!("  Blocked: {e} (mutex held by process 1)"),
    }

    println!("Process 1 exits critical section...");
    sys.try_fire(t_exit1).expect("should succeed");
    println!("  Marking: {:?}", sys.marking());

    println!("Now process 2 can enter...");
    sys.try_fire(t_enter2).expect("should succeed");
    println!("  Marking: {:?}", sys.marking());

    println!("\n=== Done ===");
}

fn print_state(sys: &PetriNet<impl AsRef<Net>>, net: &Net) {
    print!("State: ");
    let labels = net.labels.as_ref().unwrap();
    for p in net.places() {
        let tokens = sys.current_tokens(p);
        if tokens > 0 {
            let name = labels.place_name(p).unwrap_or("unnamed place");
            print!("{name}={tokens} ");
        }
    }
    println!();
}
