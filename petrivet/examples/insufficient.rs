use petrivet::prelude::{NetBuilder, PetriNet};
use petrivet::system::coverability::{Coverability, Lemma};

fn main() {
    let mut b = NetBuilder::new();

    let [s1, s2, s3] = dbg!(b.add_places());
    let [t1, t2] = dbg!(b.add_transitions());

    b.add_arcs((s1, t1, s2, t2, s1));
    b.add_arc((t2, s3));

    let net = b.build().expect("connected and non-degenerate net");
    println!("Structural class: {}", net.class());

    let m0 = [];
    let target = [(s3, 1)];

    let pn = PetriNet::new(&net, m0);
    let coverability = dbg!(pn.analyze_coverability(target));

    let Coverability::Uncoverable { contradiction: lemmas } = coverability else {
        panic!("expected the target marking to be uncoverable from the initial marking, but it was coverable.");
    };
    assert_eq!(lemmas.len(), 3, "expected 3 contradictory lemmas, found {}", lemmas.len());
    assert!(lemmas.contains(&Lemma::MarkingEquation {
        place: s3,
        initial_marking: 0,
        net_effects: [(t2, 1)].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::CausalOrdering {
        transition: t1,
        place: s1,
        feeders: [t2].into_iter().collect(),
    }));
    assert!(lemmas.contains(&Lemma::CausalOrdering {
        transition: t2,
        place: s2,
        feeders: [t1].into_iter().collect(),
    }));
}

