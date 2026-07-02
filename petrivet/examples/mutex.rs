//! Demonstrates a basic mutual exclusion algorithm.
//! The Petri net consists of two processes that request access to a critical section,
//! ensuring that only one process can be in the critical section at a time.
//! The mutual exclusion is enforced by a shared mutex place.

use petrivet::prelude::{NetBuilder, PetriNet};

fn main() {
    let mut b = NetBuilder::new();

    // process 1
    let [idle1, wait1, crit1] = b.add_places();
    let [req1, enter1, exit1] = b.add_transitions();
    b.add_arcs((idle1, req1, wait1, enter1, crit1, exit1, idle1));

    // process 2
    let [idle2, wait2, crit2] = b.add_places();
    let [req2, enter2, exit2] = b.add_transitions();
    b.add_arcs((idle2, req2, wait2, enter2, crit2, exit2, idle2));

    // synchronization: a shared mutex place that ensures mutual exclusion
    let mutex = b.add_place();
    b.add_arc((mutex, enter1));
    b.add_arc((exit1, mutex));
    b.add_arc((mutex, enter2));
    b.add_arc((exit2, mutex));

    let net = b.build().expect("connected non-degenerate net");

    // initial state: both processes idle, mutex available
    let initial_marking = [(idle1, 1), (idle2, 1), (mutex, 1)];
    // dangerous situation: both processes in critical section at the same time
    let dangerous = [(crit1, 1), (crit2, 1)];

    assert!(
        !PetriNet::new(&net, initial_marking).is_coverable(dangerous),
        "Mutual exclusion failed: both processes can be in critical section simultaneously."
    );
    println!("Mutual exclusion verified: both processes cannot be in critical section simultaneously.");
}
