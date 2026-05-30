//! Analyzing a manufacturing workflow with Petri net techniques.
//!
//! This example models a simple printed circuit board (PCB) assembly line:
//!
//! ```text
//!                     ┌──────────────┐
//!                     │              ▼
//!   raw ─→ [solder] ─→ soldered ─→ [inspect] ─┬─→ passed ─→ [ship] ─→ done
//!     ▲                                        │
//!     │                                        └─→ failed ─→ [rework] ─┘
//!     │                                                         │
//!     └─────────────────────────────────────────────────────────┘
//! ```
//!
//! - Boards arrive at `raw`, get soldered, then inspected.
//! - Inspection either passes (board ships) or fails (board gets reworked
//!   and returns to `raw` for another attempt).
//! - The solder station has limited capacity: a `station` place acts as a
//!   semaphore so only one board is soldered at a time.
//!
//! We analyze this workflow for:
//! 1. **Structural class**: What kind of Petri net is this?
//! 2. **Boundedness**: Can tokens accumulate without limit?
//! 3. **Liveness**: Can every operation eventually happen?
//! 4. **Deadlock-freedom**: Is the system guaranteed to make progress?
//! 5. **Invariants**: What conservation laws hold?
//! 6. **Siphons and traps**: Structural causes of potential deadlocks?
//! 7. **Reachability**: Can a specific state be reached?
//! 8. **Marking equation**: Quick necessary-condition check for reachability
//!
//! These techniques apply broadly to any system with concurrent, discrete
//! events: network protocols, business process engines, hardware pipelines,
//! robotic workcells, biological regulatory networks, and more.
//!
//! Run: `cargo run --example workflow_analysis`

use petrivet::prelude::{NetBuilder, PetriNet};
use petrivet::state_space::ExplorationOrder;

fn main() {
    println!("=== PCB Assembly Line Analysis ===\n");

    let mut b = NetBuilder::new();

    let [
        raw,
        station,
        soldered,
        passed,
        failed,
        done,
    ] = b.add_places();

    let transitions @ [
        solder,
        inspect_pass,
        inspect_fail,
        ship,
        rework,
    ] = b.add_transitions();

    // Solder: raw + station → soldered + station
    b.add_arcs((raw, solder, soldered));
    b.add_arcs((station, solder, station));

    // Inspect: soldered → passed  OR  soldered → failed
    b.add_arcs((soldered, inspect_pass, passed));
    b.add_arcs((soldered, inspect_fail, failed));

    // Ship: passed → done
    b.add_arcs((passed, ship, done));

    // Rework: failed → raw (retry)
    b.add_arcs((failed, rework, raw));

    let net = b.build().expect("valid net");

    println!("Net: {} places, {} transitions", net.place_count(), net.transition_count());
    println!("Structural class: {}", net.class());

    println!("\n--- Structural Analysis ---\n");

    println!(
        "\nStructurally bounded (bounded under all markings): {}",
        net.is_structurally_bounded()
    );

    println!("\n--- Behavioral Analysis (3 boards, 1 station) ---\n");

    // 3 raw boards, 1 station slot, everything else empty
    let sys = PetriNet::new(&net, [(raw, 3), (station, 1)]);
    let boundedness = sys.analyze_boundedness();
    let liveness = sys.analyze_liveness();

    println!("Bounded: {:?}", boundedness.system_bound());
    println!("Live (every transition always eventually firable): {}", liveness.global_level().is_live());

    for t in &transitions {
        println!("Transition {:?} liveness: {}", t, liveness.level(*t));
    }

    println!("\n--- Reachability Analysis ---\n");

    // Can all 3 boards reach "done"?
    let result = sys.analyze_reachability([(station, 1), (done, 3)].into());
    println!(
        "All 3 boards done? {}",
        if result.is_reachable() { "reachable" }
        else if result.is_unreachable() { "definitely unreachable" }
        else { "inconclusive" }
    );

    // Can we magically get 4 boards done from 3?
    let impossible = [(station, 1), (done, 4)].into();
    let result2 = sys.analyze_reachability(impossible);
    println!(
        "4 boards done from 3? {}",
        if result2.is_reachable() { "reachable" }
        else if result2.is_unreachable() { "definitely unreachable" }
        else { "inconclusive" }
    );

    println!("\n--- Coverability Graph ---\n");

    let cg = sys.build_coverability_graph();
    println!("Markings: {}, Edges: {}", cg.marking_count(), cg.transition_count());
    println!("Bounded: {}", cg.is_bounded());

    let threshold = [(station, 1.into()), (done, 3.into())].into();
    println!(
        "All-done marking coverable: {}",
        cg.cover(threshold).map_or_else(
            || "no".to_string(),
            |cover| format!("yes: {cover:?}")
        )
    );

    println!("\n--- Reachability Graph ---\n");

    let rg = sys.build_reachability_graph();
    println!("States: {}, Edges: {}", rg.marking_count(), rg.transition_count());
    println!("Deadlock-free: {}", rg.is_deadlock_free());

    if !rg.is_deadlock_free() {
        println!("Deadlocks found:");
        for dl in rg.deadlocks() {
            println!("  {dl:?}");
        }
    }

    println!("\nLiveness (from RG): {}", rg.transition_liveness().is_live());

    println!("\n--- Incremental Exploration ---\n");

    let mut explorer = sys.explore_reachability(ExplorationOrder::BreadthFirst);
    println!("Starting incremental exploration...");

    let mut new_states = 0;
    for step in explorer.explore_iter().take(20) {
        if step.is_new {
            new_states += 1;
        }
    }
    println!(
        "After 20 steps: {} states discovered ({} new), fully explored: {}",
        explorer.marking_count(),
        new_states,
        explorer.is_fully_explored()
    );

    // Continue exploring until done
    let rg = explorer.build_graph();
    println!(
        "After full exploration: {} states, {} edges",
        rg.marking_count(),
        rg.transition_count()
    );

    println!("Promoted to ReachabilityGraph: live: {}", rg.transition_liveness().is_live());
}
