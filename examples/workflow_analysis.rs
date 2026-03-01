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
//! 1. **Structural class** — What kind of Petri net is this?
//! 2. **Boundedness** — Can tokens accumulate without limit?
//! 3. **Liveness** — Can every operation eventually happen?
//! 4. **Deadlock-freedom** — Is the system guaranteed to make progress?
//! 5. **Invariants** — What conservation laws hold?
//! 6. **Siphons and traps** — Structural causes of potential deadlocks?
//! 7. **Reachability** — Can a specific state be reached?
//! 8. **Marking equation** — Quick necessary-condition check for reachability
//!
//! These techniques apply broadly to any system with concurrent, discrete
//! events: network protocols, business process engines, hardware pipelines,
//! robotic workcells, biological regulatory networks, and more.
//!
//! Run: `cargo run --example workflow_analysis`

use petrivet::analysis::semi_decision;
use petrivet::analysis::structural;
use petrivet::coverability::CoverabilityGraph;
use petrivet::explorer::ExplorationOrder;
use petrivet::marking::Marking;
use petrivet::net::builder::NetBuilder;
use petrivet::reachability::{ReachabilityExplorer, ReachabilityGraph};
use petrivet::system::System;

fn main() {
    println!("=== PCB Assembly Line Analysis ===\n");

    let mut b = NetBuilder::new();

    let raw = b.add_place();
    let station = b.add_place(); // solder station capacity
    let soldered = b.add_place();
    let passed = b.add_place();
    let failed = b.add_place();
    let done = b.add_place();

    let t_solder = b.add_transition();
    let t_inspect = b.add_transition();
    let t_ship = b.add_transition();
    let t_rework = b.add_transition();

    // Solder: raw + station → soldered + station
    b.add_arc((raw, t_solder));
    b.add_arc((station, t_solder));
    b.add_arc((t_solder, soldered));
    b.add_arc((t_solder, station));

    // Inspect: soldered → passed  OR  soldered → failed
    b.add_arc((soldered, t_inspect));
    b.add_arc((t_inspect, passed));

    // Ship: passed → done
    b.add_arc((passed, t_ship));
    b.add_arc((t_ship, done));

    // Rework: failed → raw (retry)
    b.add_arc((failed, t_rework));
    b.add_arc((t_rework, raw));

    // Make it a choice: inspection can also fail
    // We need a second inspect transition for the failure branch
    let t_inspect_fail = b.add_transition();
    b.add_arc((soldered, t_inspect_fail));
    b.add_arc((t_inspect_fail, failed));

    let net = b.build().expect("valid net");
    let place_names = ["raw", "station", "soldered", "passed", "failed", "done"];
    let trans_names = ["solder", "inspect_pass", "ship", "rework", "inspect_fail"];

    println!("Net: {} places, {} transitions", net.n_places(), net.n_transitions());
    println!("Structural class: {}", net.class());

    println!("\n--- Structural Analysis ---\n");

    let inv = structural::compute_invariants(&net);
    println!(
        "S-invariants (place conservation laws): {} basis vectors",
        inv.s_invariants.len()
    );
    for (i, s) in inv.s_invariants.iter().enumerate() {
        let terms: Vec<String> = s
            .iter()
            .enumerate()
            .filter(|&(_, &v)| v != 0)
            .map(|(j, v)| format!("{}·{}", v, place_names[j]))
            .collect();
        println!("  y{i} = [{}]", terms.join(" + "));
    }
    println!(
        "  Covered by S-invariants (conservative): {}",
        inv.is_covered_by_s_invariants(net.n_places())
    );

    println!(
        "\nT-invariants (reproducible firing sequences): {} basis vectors",
        inv.t_invariants.len()
    );
    for (i, t) in inv.t_invariants.iter().enumerate() {
        let terms: Vec<String> = t
            .iter()
            .enumerate()
            .filter(|&(_, &v)| v != 0)
            .map(|(j, v)| format!("{}·{}", v, trans_names[j]))
            .collect();
        println!("  x{i} = [{}]", terms.join(" + "));
    }

    println!(
        "\nStructurally bounded (bounded under all markings): {}",
        net.is_structurally_bounded()
    );

    let siphons = structural::minimal_siphons(&net);
    let traps = structural::minimal_traps(&net);
    println!("\nMinimal siphons: {}", siphons.len());
    for s in &siphons {
        let names: Vec<&str> = s.iter().map(|p| place_names[p.index()]).collect();
        println!("  {{{}}}", names.join(", "));
    }
    println!("Minimal traps: {}", traps.len());
    for t in &traps {
        let names: Vec<&str> = t.iter().map(|p| place_names[p.index()]).collect();
        println!("  {{{}}}", names.join(", "));
    }

    println!("\n--- Behavioral Analysis (3 boards, 1 station) ---\n");

    // 3 raw boards, 1 station slot, everything else empty
    let sys = System::new(net.clone(), [3u32, 1, 0, 0, 0, 0]);

    println!("Bounded: {}", sys.is_bounded());
    println!("Quasi-live (every transition can fire): {}", sys.is_quasi_live());
    println!("Live (every transition always eventually firable): {}", sys.is_live());

    for (i, name) in trans_names.iter().enumerate() {
        let t = petrivet::net::Transition::from_index(i);
        println!(
            "  {} dead? {}",
            name,
            sys.is_dead(t)
        );
    }

    if let Some(levels) = sys.liveness_levels() {
        println!("\nPer-transition liveness levels:");
        for (i, level) in levels.iter().enumerate() {
            println!("  {}: {:?}", trans_names[i], level);
        }
    }

    println!("\n--- Semi-Decision Procedures ---\n");

    let initial = Marking::from([3u32, 1, 0, 0, 0, 0]);

    // Can all 3 boards reach "done"?
    let target_all_done = Marking::from([0u32, 1, 0, 0, 0, 3]);
    let me = semi_decision::find_marking_equation_rational_solution(&net, &initial, &target_all_done);
    println!(
        "All 3 boards done? LP says: {}",
        if me.is_feasible() { "possibly reachable" } else { "definitely unreachable" }
    );

    let me_ilp = semi_decision::find_marking_equation_integer_solution(&net, &initial, &target_all_done);
    println!(
        "All 3 boards done? ILP says: {}",
        if me_ilp.is_feasible() { "possibly reachable (integer solution exists)" } else { "definitely unreachable" }
    );

    // Can we magically get 4 boards done from 3?
    let impossible = Marking::from([0u32, 1, 0, 0, 0, 4]);
    let me2 = semi_decision::find_marking_equation_rational_solution(&net, &initial, &impossible);
    println!(
        "4 boards done from 3? LP says: {}",
        if me2.is_feasible() { "possibly reachable" } else { "definitely unreachable" }
    );

    println!("\n--- Coverability Graph ---\n");

    let cg = CoverabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
    println!("States: {}, Edges: {}", cg.state_count(), cg.edge_count());
    println!("Bounded: {}", cg.is_bounded());

    let threshold = Marking::from([0u32, 1, 0, 0, 0, 3]);
    println!("All-done marking coverable: {}", cg.is_coverable(&threshold));

    println!("\n--- Reachability Graph ---\n");

    let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
    println!("States: {}, Edges: {}", rg.state_count(), rg.edge_count());
    println!("Deadlock-free: {}", rg.is_deadlock_free());

    if !rg.is_deadlock_free() {
        println!("Deadlocks found:");
        for dl in rg.deadlocks() {
            let desc: Vec<String> = dl
                .iter()
                .enumerate()
                .filter(|&(_, &v)| v > 0)
                .map(|(i, v)| format!("{}={}", place_names[i], v))
                .collect();
            println!("  {{{}}}", desc.join(", "));
        }
    }

    let target = Marking::from([0u32, 1, 0, 0, 0, 3]);
    if let Some(path) = rg.path_to(&target) {
        println!(
            "\nShortest path to all-done ({} steps):",
            path.len()
        );
        for (i, t) in path.iter().enumerate() {
            println!("  {}: {}", i + 1, trans_names[t.index()]);
        }
    }

    println!("\nLiveness (from RG): {}", rg.is_live());
    let levels = rg.liveness_levels();
    for (i, level) in levels.iter().enumerate() {
        println!("  {}: {:?}", trans_names[i], level);
    }

    println!("\n--- Incremental Exploration ---\n");

    let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
    println!("Starting incremental exploration...");

    let mut new_states = 0;
    for step in explorer.iter().take(20) {
        if step.is_new {
            new_states += 1;
        }
    }
    println!(
        "After 20 steps: {} states discovered ({} new), fully explored: {}",
        explorer.state_count(),
        new_states,
        explorer.is_fully_explored()
    );

    // Continue exploring until done
    explorer.explore_all();
    println!(
        "After full exploration: {} states, {} edges",
        explorer.state_count(),
        explorer.edge_count()
    );

    // Promote to ReachabilityGraph for analysis
    let rg2 = ReachabilityGraph::try_from(explorer).expect("fully explored");
    println!("Promoted to ReachabilityGraph — live: {}", rg2.is_live());

    println!("\n--- Commoner's Theorem (Free-Choice Liveness) ---\n");

    let m0 = Marking::from([3u32, 1, 0, 0, 0, 0]);
    if net.is_free_choice() {
        let live = structural::every_siphon_contains_marked_trap(&net, &m0, &siphons);
        println!(
            "Net is free-choice. Commoner criterion: every siphon contains a marked trap? {live}",
        );
        println!("Therefore the system is {}.", if live { "live" } else { "NOT live" });
    } else {
        println!("Net is not free-choice; Commoner's theorem does not apply.");
        println!("Falling back to state-space liveness check: {}", sys.is_live());
    }

    println!("\n=== Analysis Complete ===");
}
