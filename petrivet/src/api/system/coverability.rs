use crate::core::cegar::CegarProperty;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;
pub use crate::system::lemma::Lemma;
use crate::core::cegar::observe::IdxCegarEvent;
use crate::system::observe::CegarEvent;
use std::sync::{Arc, mpsc};

#[derive(Debug, Clone)]
pub enum CoverabilityResult {
    /// The target marking is coverable from M₀.
    Coverable {
        /// A reachable marking which covers the target marking.
        marking: Marking<u32>,
        /// A transition firing sequence from `M₀` to `covering_marking`.
        firing_sequence: Vec<Transition>,
    },
    /// The target marking is not coverable from M₀.
    Uncoverable {
        contradiction: Vec<Lemma>,
    },
}

impl CoverabilityResult {
    /// Whether the target is coverable.
    #[must_use]
    pub const fn is_coverable(&self) -> bool {
        matches!(self, Self::Coverable { .. })
    }

    /// Whether the target is not coverable.
    #[must_use]
    pub const fn is_uncoverable(&self) -> bool {
        matches!(self, Self::Uncoverable { .. })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns whether `target` is coverable from the initial marking.
    pub fn is_coverable(&self, target: impl Into<Marking<u32>>) -> bool {
        self.analyze_coverability(target.into(), None).is_coverable()
    }

    /// Analyzes whether the given target marking is coverable in this Petri net.
    ///
    /// Searches for a reachable marking is `M` such that `M(p) >= target(p)` for every place `p`.
    /// If such a marking exists, returns `CoverabilityResult::Coverable` with the reachable marking
    /// and a firing sequence from the initial marking to it.
    /// If no such marking exists, returns `CoverabilityResult::Uncoverable` with a contradiction
    /// that proves the target marking is not coverable.
    #[must_use]
    pub fn analyze_coverability(
        &self,
        target: impl Into<Marking<u32>>,
        observer: Option<mpsc::Sender<CegarEvent>>,
    ) -> CoverabilityResult {
        let m0 = &self.marking;
        let target = &self.mapping.decode(target.into());

        if m0 >= target {
            return CoverabilityResult::Coverable {
                firing_sequence: Vec::new(),
                marking: self.mapping.encode(m0.clone()),
            };
        }
        let observer_fn = observer.map(|observer| {
            let mapping = Arc::clone(&self.mapping);
            Box::new(move |event: IdxCegarEvent| {
                let _ = observer.send(mapping.cegar_event(event));
            }) as Box<dyn Fn(IdxCegarEvent) + Send>
        });
        let cegar_result = self.dense_net.cegar_decide(
            m0,
            target,
            CegarProperty::Reachability,
            observer_fn
        );
        self.mapping.coverability_result(cegar_result)
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::marking::Marking;
    use crate::prelude::PetriNet;
    use crate::system::coverability::{CoverabilityResult, Lemma};
    use std::sync::mpsc;

    #[test]
    fn coverability_initial_marking_covers() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        let res = sys.analyze_coverability([(p0, 1)], None);
        assert!(res.is_coverable());
        match res {
            CoverabilityResult::Coverable {
                marking,
                firing_sequence,
            } => {
                assert_eq!(marking, Marking::from([(p0, 1)]));
                assert_eq!(firing_sequence.len(), 0);
            }
            _ => panic!("expected InitialMarking proof"),
        }
    }

    /// The same net as `core::cegar::tests::once_only`, reachable only through the public API:
    /// t1 can only fire once, since re-marking s1 requires t2, which is gated by a resource (s7)
    /// that only becomes available after firing away s5/s6 - by which point x is already marked
    /// and t1/t2 can no longer add to it. Uncoverable, but only provably so after CEGAR works
    /// through a few rounds of spurious candidates, so it's a reasonable smoke test for the
    /// observer: every event it reports should agree with the final contradiction.
    #[test]
    fn observer_sees_every_lemma_in_the_final_contradiction() {
        use crate::system::lemma::Lemma;

        let mut b = NetBuilder::new();
        let [s1, s2, s3, s4, s5, s6, s7, x] = b.add_places();
        let [t1, t2, t3, t4, t5, t6] = b.add_transitions();
        b.add_arcs((s1, t1, s2, t2, s1));
        b.add_arcs((s7, t5, s5, t3, s3, t2));
        b.add_arcs((s7, t6, s6, t4, s4, t2));
        b.add_arc((t5, s4));
        b.add_arc((t6, s3));
        b.add_arc((t2, s7));
        b.add_arc((t1, x));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(s1, 1), (s5, 1), (s6, 1)]);

        let (sender, receiver) = mpsc::channel();
        let res = sys.analyze_coverability([(s2, 1), (s5, 1), (s6, 1), (x, 2)], Some(sender));

        assert!(res.is_uncoverable());
        match res {
            CoverabilityResult::Uncoverable { contradiction } => {
                // Every lemma CEGAR actually derived to rule out a spurious candidate must have
                // been reported to the observer along the way (it can't appear from nowhere at
                // the very end). `MarkingEquation` lemmas are the one exception: they're baseline
                // facts asserted upfront for every place, not something derived in response to a
                // spurious candidate, so they're never routed through the observer.
                for lemma in &contradiction {
                    if matches!(lemma, Lemma::MarkingEquation { .. }) {
                        continue;
                    }
                    assert!(
                        receiver.try_iter().any(|event| &event.lemma == lemma),
                        "contradiction lemma {lemma:?} was never reported to the observer"
                    );
                }
            }
            CoverabilityResult::Coverable { .. } => unreachable!("checked above"),
        }
    }

    #[test]
    fn free_choice_uncoverable() {
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
        let CoverabilityResult::Uncoverable { contradiction } = dbg!(pn.analyze_coverability(dangerous, None)) else {
            panic!("expected the target marking to be uncoverable from the initial marking, but it was coverable.");
        };
        assert_eq!(contradiction.len(), 1);
        assert!(contradiction.contains(&Lemma::InitiallyMarkedTrap([s1, s2, s3, s6, s7, s8].into_iter().collect())));
    }

    #[test]
    fn causal_ordering_contradiction() {
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
        let coverability = dbg!(pn.analyze_coverability(target, None));

        let CoverabilityResult::Uncoverable { contradiction: lemmas } = coverability else {
            panic!("expected the target marking to be uncoverable from the initial marking, but it was coverable.");
        };
        assert_eq!(lemmas.len(), 3, "expected 3 contradictory lemmas, found {}", lemmas.len());
        assert!(lemmas.contains(&Lemma::MarkingEquation {
            place: s3,
            initial_marking: 0,
            net_effects: std::iter::once((t2, 1)).collect(),
        }));
        assert!(lemmas.contains(&Lemma::CausalOrdering {
            transition: t1,
            place: s1,
            feeders: std::iter::once(t2).collect(),
        }));
        assert!(lemmas.contains(&Lemma::CausalOrdering {
            transition: t2,
            place: s2,
            feeders: std::iter::once(t1).collect(),
        }));
    }

    #[test]
    fn only_once() {
        let mut b = NetBuilder::new();

        let [s1, s2, s3, s4, s5, s6, s7, count] = dbg!(b.add_places());
        let [t_once, t2, t3, t4, t5, t6] = dbg!(b.add_transitions());

        b.add_arcs((s1, t_once, s2, t2, s1));
        b.add_arcs((s7, t5, s5, t3, s3, t2));
        b.add_arcs((s7, t6, s6, t4, s4, t2));
        b.add_arc((t5, s4));
        b.add_arc((t6, s3));
        b.add_arc((t2, s7));
        b.add_arc((t_once, count));

        let net = b.build().expect("connected and non-degenerate net");
        println!("Structural class: {}", net.class());

        let initial_marking = dbg!([(s1, 1), (s5, 1), (s6, 1)]);
        let target = dbg!([(s2, 1), (s5, 1), (s6, 1), (count, 2)]);

        let pn = PetriNet::new(&net, initial_marking);
        let coverability = dbg!(pn.analyze_coverability(target, None));

        let CoverabilityResult::Uncoverable { contradiction: lemmas } = coverability else {
            panic!("expected the target marking to be uncoverable from the initial marking, but it was coverable.");
        };
        assert_eq!(lemmas.len(), 9, "expected 9 contradictory lemmas, found {}", lemmas.len());
        assert!(lemmas.contains(&Lemma::MarkingEquation {
            place: s2,
            initial_marking: 0,
            net_effects: [(t2, -1), (t_once, 1)].into_iter().collect(),
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
            place: count,
            initial_marking: 0,
            net_effects: [(t_once, 1)].into_iter().collect(),
        }));
        assert!(lemmas.contains(&Lemma::CausalOrdering {
            transition: t2,
            place: s4,
            feeders: [t4, t5].into_iter().collect(),
        }));
        assert!(lemmas.contains(&Lemma::CausalOrdering {
            transition: t2,
            place: s3,
            feeders: [t3, t6].into_iter().collect(),
        }));
        assert!(lemmas.contains(&Lemma::CausalOrdering {
            transition: t5,
            place: s7,
            feeders: [t2].into_iter().collect(),
        }));
        assert!(lemmas.contains(&Lemma::CausalOrdering {
            transition: t6,
            place: s7,
            feeders: [t2].into_iter().collect(),
        }));
    }
}
