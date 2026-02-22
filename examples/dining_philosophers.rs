//! Dining philosophers modeled as a Petri net.
//!
//! Five philosophers sit around a table with five forks. Each philosopher
//! alternates between thinking and eating. To eat, a philosopher must pick
//! up both the fork to their left and the fork to their right. Since forks
//! are shared between adjacent philosophers, this creates contention.
//!
//! ```text
//!              fork0
//!         P0 -------- P1
//!        /                \
//!   fork4                  fork1
//!      /                    \
//!     P4                    P2
//!       \                  /
//!        fork3 ---- fork2
//!              P3
//! ```
//!
//! TODO: Explain this model in more detail.
//!
//! Run: `cargo run --example dining_philosophers`

use petrivet::net::builder::NetBuilder;
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

    let mut sys = System::new(net, initial);

    println!("--- Simulation ---\n");

    // Show that the firing sequence take_left_0, take_left_1, ..., take_left_(N-1) is possible (all philosophers pick up their left fork), but then no philosopher can eat (deadlock).

    for (i, &take_left) in take_left.iter().enumerate() {
        sys.try_fire(take_left).ok();
        println!("Philosopher {i} takes left fork");
    }

    println!("Marking after all philosophers take left fork: {}", sys.marking());
    if sys.is_deadlocked() {
        println!("All philosophers have taken their left fork, but no one can eat! DEADLOCK");
    } else {
        println!("Unexpectedly, the system is not deadlocked!");
    }

    // TODO: Analyze the net further

    println!("\n=== Done ===");
}
