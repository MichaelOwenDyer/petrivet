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

use petrivet::net::builder::NetBuilder;
use petrivet::system::System;

fn main() {
    println!("=== Mutual Exclusion Protocol ===\n");

    // --- Build the net ---

    let mut b = NetBuilder::new();

    // Places: process 1 states, process 2 states, shared mutex
    let [idle1, wait1, crit1] = b.add_places();
    let [idle2, wait2, crit2] = b.add_places();
    let mutex = b.add_place();

    // Transitions: request, enter critical, exit critical (per process)
    let [t_req1, t_enter1, t_exit1] = b.add_transitions();
    let [t_req2, t_enter2, t_exit2] = b.add_transitions();

    // Process 1: idle1 → t_req1 → wait1 → t_enter1 → crit1 → t_exit1 → idle1
    b.add_arc((idle1, t_req1));
    b.add_arc((t_req1, wait1));
    b.add_arc((wait1, t_enter1));
    b.add_arc((t_enter1, crit1));
    b.add_arc((crit1, t_exit1));
    b.add_arc((t_exit1, idle1));

    // Process 2: idle2 → t_req2 → wait2 → t_enter2 → crit2 → t_exit2 → idle2
    b.add_arc((idle2, t_req2));
    b.add_arc((t_req2, wait2));
    b.add_arc((wait2, t_enter2));
    b.add_arc((t_enter2, crit2));
    b.add_arc((crit2, t_exit2));
    b.add_arc((t_exit2, idle2));

    // Mutex: consumed by enter, produced by exit
    b.add_arc((mutex, t_enter1));
    b.add_arc((t_exit1, mutex));
    b.add_arc((mutex, t_enter2));
    b.add_arc((t_exit2, mutex));

    let net = b.build().expect("valid net");
    println!("Structural class: {}", net.class());

    // --- Initial marking: both processes idle, mutex available ---
    // Places: idle1, wait1, crit1, idle2, wait2, crit2, mutex
    let mut sys = System::new(net, [1, 0, 0, 1, 0, 0, 1]);

    let names = ["req1", "enter1", "exit1", "req2", "enter2", "exit2"];
    let place_names = ["idle1", "wait1", "crit1", "idle2", "wait2", "crit2", "mutex"];

    println!();
    print_state(&sys, &place_names);

    // --- Simulate 12 steps, always picking the first enabled transition ---
    for step in 1..=12 {
        if let Some(t) = sys.fire_any() {
            println!("Step {step:>2}: fire {:<8} → {}", names[t.index()], sys.marking());
        } else {
            println!("Step {step:>2}: DEADLOCK");
            break;
        }

        // Safety check: both processes must never be in critical section at once
        assert!(
            sys.marking()[crit1] == 0 || sys.marking()[crit2] == 0,
            "mutual exclusion violated!"
        );
    }

    println!();
    print_state(&sys, &place_names);

    // --- Demonstrate choose_and_fire with priority ---
    println!("\n--- Priority simulation: process 2 has priority ---\n");
    sys.reset();
    print_state(&sys, &place_names);

    for step in 1..=12 {
        // Prefer process 2 transitions (indices 3, 4, 5) over process 1
        let priority = [t_req2, t_enter2, t_exit2, t_req1, t_enter1, t_exit1];

        let fired = sys.choose_and_fire(|enabled| {
            priority.iter()
                .find_map(|&t| enabled.iter().find(|et| *et == t))
        });

        if let Some(t) = fired {
            println!("Step {step:>2}: fire {:<8} → {}", names[t.index()], sys.marking());
        } else {
            println!("Step {step:>2}: DEADLOCK");
            break;
        }
    }

    // --- Demonstrate try_fire ---
    println!("\n--- Manual firing with try_fire ---\n");
    sys.reset();

    println!("Trying to enter critical section without requesting first...");
    match sys.try_fire(t_enter1) {
        Ok(()) => println!("  Entered (unexpected!)"),
        Err(e) => println!("  Blocked: {e}"),
    }

    println!("Requesting access for process 1...");
    sys.try_fire(t_req1).expect("should succeed");
    println!("  Marking: {}", sys.marking());

    println!("Entering critical section...");
    sys.try_fire(t_enter1).expect("should succeed");
    println!("  Marking: {}", sys.marking());

    println!("Process 2 requests and tries to enter...");
    sys.try_fire(t_req2).expect("should succeed");
    match sys.try_fire(t_enter2) {
        Ok(()) => println!("  Entered (mutex violation!)"),
        Err(e) => println!("  Blocked: {e} (mutex held by process 1)"),
    }

    println!("Process 1 exits critical section...");
    sys.try_fire(t_exit1).expect("should succeed");
    println!("  Marking: {}", sys.marking());

    println!("Now process 2 can enter...");
    sys.try_fire(t_enter2).expect("should succeed");
    println!("  Marking: {}", sys.marking());

    println!("\n=== Done ===");
}

fn print_state<N: AsRef<petrivet::net::Net>>(sys: &System<N>, names: &[&str]) {
    print!("State: ");
    for (i, &name) in names.iter().enumerate() {
        let tokens = sys.marking()[petrivet::net::Place::from_index(i)];
        if tokens > 0 {
            print!("{name}={tokens} ");
        }
    }
    println!();
}
