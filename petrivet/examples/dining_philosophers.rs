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

use petrivet::api::builder::NetBuilder;
use petrivet::api::state_space::ReachabilityGraph;
use petrivet::marking::Marking;
use petrivet::PetriNet;

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

    let initial: Marking<u32> = thinking.into_iter()
        .chain(forks)
        .map(|p| (p, 1))
        .collect();

    let mut sys = PetriNet::new(&net, initial.clone());

    println!("--- Simulation ---\n");

    // Show that the firing sequence take_left_0, take_left_1, ..., take_left_(N-1) is possible
    // (all philosophers pick up their left fork), but then no philosopher can eat (deadlock).

    for (i, &take_left_k) in take_left.iter().enumerate() {
        sys.try_fire(take_left_k).ok();
        println!("Philosopher {i} takes left fork");
    }

    println!("Marking after all take left fork: {:?}", sys.current_marking());
    if sys.is_deadlocked() {
        println!("All philosophers have taken their left fork, but no one can eat! DEADLOCK\n");
    } else {
        println!("Unexpectedly, the system is not deadlocked!\n");
    }

    println!("--- State Space Analysis ---\n");


    let sys = PetriNet::new(&net, initial.clone());
    let rg = ReachabilityGraph::build(&sys);
    println!("Reachable states: {}", rg.marking_count());
    println!("Edges: {}", rg.transition_count());
    println!("Deadlock-free: {}", rg.is_deadlock_free());

    println!("Deadlock states:");
    for (i, dl) in rg.deadlocks().enumerate() {
        println!("  {}: {:?}", i + 1, dl);
    }
}
