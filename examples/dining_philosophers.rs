//! Dining philosophers modeled as a Petri net.
//!
//! Five philosophers sit around a table with five forks. Each philosopher
//! alternates between thinking and eating. To eat, a philosopher must pick
//! up both the fork to their left and the fork to their right. Since forks
//! are shared between adjacent philosophers, this creates contention.
//!
//! Each philosopher `i` has three places (`thinking_i`, `holding_fork_i`, `eating_i`)
//! and three transitions (`take_left_i`, `take_right_i`, `put_down_forks_i`):
//!
//! - `take_left_i`: consumes from `thinking_i` and `fork_i`, produces to `holding_fork_i`
//! - `take_right_i`: consumes from `holding_fork_i` and `fork_{(i+1)%N}`, produces to `eating_i`
//! - `put_down_forks_i`: consumes from `eating_i`, produces to `thinking_i`, `fork_i`, and `fork_{(i+1)%N}`
//!
//! This model is known to deadlock: if all philosophers simultaneously pick
//! up their left fork, nobody can pick up their right fork. We verify this
//! with state space analysis and structural analysis techniques.
//!
//! Run: `cargo run --example dining_philosophers`

use petrivet::analysis::structural;
use petrivet::marking::Marking;
use petrivet::net::builder::NetBuilder;
use petrivet::state_space::ExplorationOrder;
use petrivet::state_space::ReachabilityGraph;
use petrivet::system::System;

const N: usize = 4;

fn main() {
    println!("=== Dining Philosophers ({N} philosophers) ===\n");

    let mut net = NetBuilder::new();

    let forks = net.add_places::<N>();
    let thinking = net.add_places::<N>();
    let holding_fork = net.add_places::<N>();
    let eating = net.add_places::<N>();

    let take_left = net.add_transitions::<N>();
    let take_right = net.add_transitions::<N>();
    let put_down_forks = net.add_transitions::<N>();

    for i in 0..N {
        let thinking = thinking[i];
        let holding_fork = holding_fork[i];
        let eating = eating[i];
        let take_left = take_left[i];
        let take_right = take_right[i];
        let put_down_forks = put_down_forks[i];
        let left_fork = forks[i];
        let right_fork = forks[(i + 1) % N];

        net.add_arc((thinking, take_left));
        net.add_arc((left_fork, take_left));
        net.add_arc((take_left, holding_fork));
        net.add_arc((holding_fork, take_right));
        net.add_arc((right_fork, take_right));
        net.add_arc((take_right, eating));
        net.add_arc((eating, put_down_forks));
        net.add_arc((put_down_forks, thinking));
        net.add_arc((put_down_forks, left_fork));
        net.add_arc((put_down_forks, right_fork));
    }

    let net = net.build().expect("valid net");
    println!("Structural class: {}\n", net.class());

    // Initial marking: all philosophers thinking, all forks on table
    let mut initial = vec![0u32; 4 * N];
    for i in 0..N {
        initial[thinking[i].index()] = 1; // thinking
        initial[forks[i].index()] = 1;  // fork available
    }

    let mut sys = System::new(net.clone(), initial.clone());

    println!("--- Simulation ---\n");

    // Show that the firing sequence take_left_0, take_left_1, ..., take_left_(N-1) is possible (all philosophers pick up their left fork), but then no philosopher can eat (deadlock).

    for (i, &take_left) in take_left.iter().enumerate() {
        sys.try_fire(take_left).ok();
        println!("Philosopher {i} takes left fork");
    }

    println!("Marking after all take left fork: {}", sys.marking());
    if sys.is_deadlocked() {
        println!("All philosophers have taken their left fork, but no one can eat! DEADLOCK\n");
    } else {
        println!("Unexpectedly, the system is not deadlocked!\n");
    }

    println!("--- State Space Analysis ---\n");

    sys.reset();
    let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
    println!("Reachable states: {}", rg.state_count());
    println!("Edges: {}", rg.edge_count());
    println!("Deadlock-free: {}", rg.is_deadlock_free());

    let deadlocks = rg.deadlocks();
    println!("Deadlock states: {}", deadlocks.len());
    for (i, dl) in deadlocks.iter().enumerate() {
        println!("  deadlock {}: {}", i + 1, dl);
    }

    // Show the shortest path to the deadlock
    if let Some(dl) = deadlocks.first() {
        let dl_marking: Marking = (*dl).clone();
        if let Some(path) = rg.path_to(&dl_marking) {
            println!(
                "\nShortest path to deadlock ({} steps):",
                path.len()
            );
            for (step, t) in path.iter().enumerate() {
                let kind = t.index() / N;
                let phil = t.index() % N;
                let name = match kind {
                    0 => "take_left",
                    1 => "take_right",
                    2 => "put_down",
                    _ => "?",
                };
                println!("  {}: philosopher {} {}", step + 1, phil, name);
            }
        }
    }

    println!("\n--- Liveness ---\n");

    let levels = rg.liveness_levels();
    for (i, level) in levels.iter().enumerate() {
        let kind = i / N;
        let phil = i % N;
        let name = match kind {
            0 => "take_left",
            1 => "take_right",
            2 => "put_down",
            _ => "?",
        };
        println!("  philosopher {} {}: {:?}", phil, name, level);
    }
    println!("\nSystem live: {}", rg.is_live());

    println!("\n--- Structural Analysis ---\n");

    let inv = structural::compute_invariants(&net);
    println!("S-invariants: {} basis vectors", inv.s_invariants.len());
    println!(
        "Conservative (covered by S-invariants): {}",
        inv.is_covered_by_s_invariants(net.n_places())
    );
    println!("T-invariants: {} basis vectors", inv.t_invariants.len());
    println!(
        "Structurally bounded: {}",
        net.is_structurally_bounded()
    );

    let siphons = structural::minimal_siphons(&net);
    println!("\nMinimal siphons: {}", siphons.len());

    let m0 = Marking::from(initial.clone());
    let chc = structural::every_siphon_contains_marked_trap(&net, &m0, &siphons);
    println!("Every siphon contains a marked trap: {}", chc);
    if !chc {
        println!("→ Commoner criterion violated: system is not live (confirmed).");
    }

    println!("\n=== Done ===");
}
