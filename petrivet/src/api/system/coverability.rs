use crate::core::cegar::{CegarProperty, CegarResult};
use crate::core::mapping::DenseMapping;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;
pub use crate::system::lemma::Lemma;
use crate::system::observe::CegarEvent;
use std::sync::{Arc, mpsc};
use crate::core::cegar::observe::IdxCegarEvent;

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

/// Shared by [`analyze_coverability`](PetriNet::analyze_coverability) and
/// [`analyze_coverability_with_observer`](PetriNet::analyze_coverability_with_observer).
fn translate(mapping: &DenseMapping, result: CegarResult) -> CoverabilityResult {
    match result {
        CegarResult::Satisfiable { marking, firing_sequence } => CoverabilityResult::Coverable {
            firing_sequence: mapping.firing_sequence(firing_sequence),
            marking: mapping.encode(marking),
        },
        CegarResult::Unsatisfiable { contradiction } => CoverabilityResult::Uncoverable {
            contradiction: contradiction.into_iter().map(|lemma| mapping.lemma(lemma)).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::marking::Marking;
    use crate::prelude::PetriNet;
    use crate::system::coverability::CoverabilityResult;
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
}
