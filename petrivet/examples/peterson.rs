use petrivet::prelude::{NetBuilder, PetriNet};

/// Demonstrates [Peterson's mutual exclusion algorithm](https://en.wikipedia.org/wiki/Peterson%27s_algorithm).
///
/// ```
/// var m1,m2 : {false,true}, init false
/// var hold : {1,2} , init 1
/// parallel
///     process 1
///         loop
///             p1: m1 := true
///             p2: hold := 1
///             p3: await(¬m2 ∨ hold=2)
///                 (critical section)
///             p4: m1 := false
///         end loop
///     end process 1
///     process 2
///         loop
///             q1: m2 := true
///             q2: hold := 2
///             q3: await(¬m1 ∨ hold=1)
///                 (critical section)
///             q4: m2 := false
///         end loop
///     end process 2
/// end parallel
/// ```
fn main() {
    let mut b = NetBuilder::new();

    // m1: {false,true}
    let [m1f, m1t] = b.add_places();
    // m2: {false,true}
    let [m2f, m2t] = b.add_places();
    // hold: {1,2}
    let [hold1, hold2] = b.add_places();

    // process 1
    let [p1, p2, p3, p4] = b.add_places();
    let [u1, u2, u3, u4, u5, u6] = b.add_transitions();
    // p1: m1 := true
    b.add_arcs((p1, u1, p2));
    b.add_arcs((m1f, u1, m1t));

    // p2: hold := 1 (two alternatives)
    b.add_arcs((p2, u2, p3));
    b.add_arcs((hold2, u2, hold1)); // hold is 2; set to 1
    b.add_arcs((p2, u3, p3));
    b.add_arcs((hold1, u3, hold1)); // hold is 1; keep it 1

    // p3: await(¬m2 ∨ hold=2) (two alternatives)
    b.add_arcs((p3, u4, p4));
    b.add_arcs((m2f, u4, m2f)); // m2 is false, proceed
    b.add_arcs((p3, u5, p4));
    b.add_arcs((hold2, u5, hold2)); // hold is 2, proceed

    // p4: m1 := false
    b.add_arcs((p4, u6, p1));
    b.add_arcs((m1t, u6, m1f));

    // process 2
    let [q1, q2, q3, q4] = b.add_places();
    let [v1, v2, v3, v4, v5, v6] = b.add_transitions();

    // q1: m2 := true
    b.add_arcs((q1, v1, q2));
    b.add_arcs((m2f, v1, m2t));

    // q2: hold := 2 (two alternatives)
    b.add_arcs((q2, v2, q3));
    b.add_arcs((hold1, v2, hold2)); // hold is 1; set to 2
    b.add_arcs((q2, v3, q3));
    b.add_arcs((hold2, v3, hold2)); // hold is 2; keep it 2

    // q3: await(¬m1 ∨ hold=1) (two alternatives)
    b.add_arcs((q3, v4, q4));
    b.add_arcs((m1f, v4, m1f)); // m1 is false, proceed
    b.add_arcs((q3, v5, q4));
    b.add_arcs((hold1, v5, hold1)); // hold is 1, proceed

    // q4: m2 := false
    b.add_arcs((q4, v6, q1));
    b.add_arcs((m2t, v6, m2f));

    let net = b.build().expect("connected and non-degenerate net");
    println!("Structural class: {}", net.class());

    // initial state: m1 = false, m2 = false, hold = 1, both processes idle
    let initial_marking = [(m1f, 1), (m2f, 1), (hold1, 1), (p1, 1), (q1, 1)];
    // dangerous situation: both processes in critical section at the same time
    let dangerous = [(p4, 1), (q4, 1)];

    assert!(
        !PetriNet::new(&net, initial_marking).is_coverable(dangerous),
        "Peterson's mutual exclusion algorithm failed: both processes can be in critical section simultaneously."
    );
    println!("Peterson's mutual exclusion algorithm verified: both processes cannot be in critical section simultaneously.");
}
