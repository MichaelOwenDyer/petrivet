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
use petrivet::net::PlaceKey;
use petrivet::state_space::ExplorationOrder;
use petrivet::state_space::ReachabilityGraph;
use petrivet::system::System;

const N: usize = 4;

/// Build a marking vector where the given place keys each receive one token.
/// Uses `net.places()` to determine dense ordering without accessing pub(crate) types.
fn marking_with_tokens(place_order: &[PlaceKey], marked: &[PlaceKey]) -> Vec<u32> {
    place_order
        .iter()
        .map(|pk| if marked.contains(pk) { 1 } else { 0 })
        .collect()
}

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

    // Initial marking: all philosophers thinking, all forks on table.
    // Build from PlaceKey order exposed by net.place_keys() (dense index order).
    let place_order: Vec<PlaceKey> = net.place_keys().collect();
    let mut marked_places: Vec<PlaceKey> = Vec::new();
    for i in 0..N {
        marked_places.push(thinking[i]);
        marked_places.push(forks[i]);
    }
    let initial = marking_with_tokens(&place_order, &marked_places);

    let mut sys = System::new(&net, initial.clone());

    println!("--- Simulation ---\n");

    // Show that the firing sequence take_left_0, take_left_1, ..., take_left_(N-1) is possible
    // (all philosophers pick up their left fork), but then no philosopher can eat (deadlock).

    for (i, &take_left_k) in take_left.iter().enumerate() {
        sys.try_fire(take_left_k).ok();
        println!("Philosopher {i} takes left fork");
    }

    println!("Marking after all take left fork: {}", sys.current_marking());
    if sys.is_deadlocked() {
        println!("All philosophers have taken their left fork, but no one can eat! DEADLOCK\n");
    } else {
        println!("Unexpectedly, the system is not deadlocked!\n");
    }

    println!("--- State Space Analysis ---\n");

    let sys = System::new(&net, initial.clone());
    let rg = ReachabilityGraph::build(&sys, ExplorationOrder::BreadthFirst);
    println!("Reachable states: {}", rg.state_count());
    println!("Edges: {}", rg.edge_count());
    println!("Deadlock-free: {}", rg.is_deadlock_free());

    println!("Deadlock states:");
    for (i, dl) in rg.deadlocks().enumerate() {
        println!("  {}: {}", i + 1, dl);
    }

    // TODO: path_to() returns Box<[Transition]> where Transition is pub(crate).
    // Cannot use from outside the crate. Need a public path_to variant that returns
    // TransitionKey, or a public method on Net to convert Transition → TransitionKey.

    println!("\n--- Liveness ---\n");

    // TODO: liveness_levels() returns TransitionMap<LivenessLevel> where TransitionMap
    // is pub(crate). Cannot bind or iterate it from outside the crate. Need a public
    // return type (e.g., Vec<(TransitionKey, LivenessLevel)> or a key-indexed map).
    println!("System live: {}", rg.is_live());

    println!("\n--- Structural Analysis ---\n");

    let inv = structural::compute_invariants(&net);
    println!("S-invariants: {} basis vectors", inv.s_invariants.len());
    println!(
        "Conservative (covered by S-invariants): {}",
        inv.is_covered_by_s_invariants(net.place_count() as usize)
    );
    println!("T-invariants: {} basis vectors", inv.t_invariants.len());
    println!(
        "Structurally bounded: {}",
        net.is_structurally_bounded()
    );

    // TODO: minimal_siphons() returns Box<[HashSet<Place>]> where Place is pub(crate).
    // Cannot use from outside the crate. Need a public variant returning PlaceKey sets.

    let m0 = Marking::from(initial);
    let chc = structural::commoner_hack_criterion(&net, &m0);
    println!("Every siphon contains a marked trap: {}", chc.is_satisfied());
    if !chc.is_satisfied() {
        println!("→ Commoner criterion violated: system is not live (confirmed).");
    }

    println!("\n=== Done ===");
}
