use petrivet::prelude::{NetBuilder, PetriNet};
use petrivet::system::coverability::Coverability;

fn main() {
    let mut b = NetBuilder::new();

    let [s1, s2, s3, s4, s5, s6, s7, s8] = dbg!(b.add_places());
    let [t1, t2, t3, t4, t5, t6, t7] = dbg!(b.add_transitions());

    b.add_arcs((s1, t1, s3, t3, s7, t7, s1));
    b.add_arcs((s2, t1, s4, t4, s8, t7));
    b.add_arcs((s1, t2, s5, t5, s7, t7, s2));
    b.add_arcs((s2, t2, s6, t6, s8, t7));

    let net = b.build().expect("connected and non-degenerate net");
    println!("Structural class: {}", net.class());

    // initial state: m1 = false, m2 = false, hold = 1, both processes idle
    let initial_marking = dbg!([(s3, 1), (s6, 1)]);
    // dangerous situation: both processes in critical section at the same time
    let dangerous = dbg!([(s4, 1), (s5, 1)]);

    let pn = PetriNet::new(&net, initial_marking);
    let Coverability::Uncoverable { contradiction } = dbg!(pn.analyze_coverability(dangerous)) else {
        panic!("expected the target marking to be uncoverable from the initial marking, but it was coverable.");
    };
}
