use petrivet::builder::NetBuilder;
use petrivet::prelude::PetriNet;
use petrivet::system::coverability::{CoverabilityResult, Lemma};

fn main() {
    let mut b = NetBuilder::new();

    let [s1, s2, s3, s4, s5, s6, s7, count_t1] = dbg!(b.add_places());
    let [t1, t2, t3, t4, t5, t6] = dbg!(b.add_transitions());

    b.add_arcs((s1, t1, s2, t2, s1));
    b.add_arcs((s7, t5, s5, t3, s3, t2));
    b.add_arcs((s7, t6, s6, t4, s4, t2));
    b.add_arc((t5, s4));
    b.add_arc((t6, s3));
    b.add_arc((t2, s7));
    b.add_arc((t1, count_t1));

    let net = b.build().expect("connected and non-degenerate net");
    println!("Structural class: {}", net.class());

    let initial_marking = dbg!([(s1, 1), (s5, 1), (s6, 1)]);
    let target = dbg!([(s2, 1), (s5, 1), (s6, 1), (count_t1, 2)]);

    let pn = PetriNet::new(&net, initial_marking);
    let coverability = dbg!(pn.analyze_coverability(target, None));

    let CoverabilityResult::Uncoverable { contradiction: lemmas } = coverability else {
        panic!("expected the target marking to be uncoverable from the initial marking, but it was coverable.");
    };
    assert_eq!(lemmas.len(), 9, "expected 9 contradictory lemmas, found {}", lemmas.len());
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s1,
        initial_marking: 1,
        net_effects: [(t1, -1), (t2, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s3,
        initial_marking: 0,
        net_effects: [(t2, -1), (t3, 1), (t6, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s4,
        initial_marking: 0,
        net_effects: [(t2, -1), (t4, 1), (t5, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s5,
        initial_marking: 1,
        net_effects: [(t3, -1), (t5, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s6,
        initial_marking: 1,
        net_effects: [(t4, -1), (t6, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s7,
        initial_marking: 0,
        net_effects: [(t2, 1), (t5, -1), (t6, -1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: count_t1,
        initial_marking: 0,
        net_effects: [(t1, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::TrapBecomesMarked {
        feeder: t3,
        trap: [s3, s4, s7].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::TrapBecomesMarked {
        feeder: t4,
        trap: [s3, s4, s7].into_iter().collect(),
    }));
}