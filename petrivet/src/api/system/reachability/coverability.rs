use crate::core::cegar::observe::ToCegarCallbackFn;
use crate::core::cegar::{CegarProblem, CegarQuestion, cegar_decide};
use crate::core::solver::DefaultSolver;
use crate::net::class::NetClass;
use crate::net::{Net, Transition};
use crate::system::PetriNet;
use crate::system::marking::Marking;
use crate::system::reachability::Lemma;
use crate::system::reachability::ReachabilityEffort;
use crate::system::reachability::SpuriousSolutionEliminatedEvent;
use bon::bon;
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
    Inconclusive,
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

#[bon]
impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns Some(true) if the target marking is coverable from the initial marking,
    /// Some(false) if it is not coverable, and None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_coverable(&self, target: &Marking<u32>) -> Option<bool> {
        let target = self.mapping.idx_marking(target.clone());
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() >= target.sum()),
            NetClass::StateMachine if self.is_strongly_connected() => {
                Some(self.marking.sum() >= target.sum())
            }
            _ => None,
        }
    }

    /// Returns whether `target` is coverable from the initial marking.
    ///
    /// Tries the efficient procedures first ([`is_efficiently_coverable`](Self::is_efficiently_coverable)).
    /// If `effort` is [`Efficient`](ReachabilityEffort::Efficient) and none applies, the answer is `None`;
    /// otherwise the question goes to [`analyze_coverability`](Self::analyze_coverability).
    /// If `timeout` is specified, the analysis gives up after that duration and the answer is `None`.
    /// Use [`analyze_coverability`](Self::analyze_coverability) for a witness or a refutation.
    #[must_use]
    #[builder]
    pub fn is_coverable(
        &self,
        #[builder(into)] target: Marking<u32>,
        effort: Option<ReachabilityEffort>,
        timeout: Option<std::time::Duration>,
    ) -> Option<bool> {
        if let Some(answer) = self.is_efficiently_coverable(&target) {
            return Some(answer);
        }
        if effort == Some(ReachabilityEffort::Efficient) {
            return None;
        }
        let deadline = timeout.and_then(|t| std::time::Instant::now().checked_add(t));
        match self.analyze_coverability_until(target, None, deadline) {
            CoverabilityResult::Coverable { .. } => Some(true),
            CoverabilityResult::Uncoverable { .. } => Some(false),
            CoverabilityResult::Inconclusive => None,
        }
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
        observer: Option<mpsc::Sender<SpuriousSolutionEliminatedEvent>>,
    ) -> CoverabilityResult {
        self.analyze_coverability_until(target, observer, None)
    }

    /// Like [`analyze_coverability`](Self::analyze_coverability), but gives up after `timeout`
    /// and returns [`CoverabilityResult::Inconclusive`] if the search has not finished by then.
    #[must_use]
    pub fn analyze_coverability_within(
        &self,
        target: impl Into<Marking<u32>>,
        timeout: std::time::Duration,
        observer: Option<mpsc::Sender<SpuriousSolutionEliminatedEvent>>,
    ) -> CoverabilityResult {
        let deadline = std::time::Instant::now().checked_add(timeout);
        self.analyze_coverability_until(target, observer, deadline)
    }

    fn analyze_coverability_until(
        &self,
        target: impl Into<Marking<u32>>,
        observer: Option<mpsc::Sender<SpuriousSolutionEliminatedEvent>>,
        deadline: Option<std::time::Instant>,
    ) -> CoverabilityResult {
        let m0 = &self.marking;
        let target = &self.mapping.idx_marking(target.into());

        if m0 >= target {
            return CoverabilityResult::Coverable {
                firing_sequence: Vec::new(),
                marking: self.mapping.marking(m0.clone()),
            };
        }

        let problem = CegarProblem {
            net: &self.dense_net,
            m0,
            target,
            question: CegarQuestion::Coverable,
            deadline,
        };
        let observer_fn = observer.map(|sender| {
            sender.to_cegar_callback_fn(Arc::clone(&self.mapping))
        });
        let cegar_result = cegar_decide::<DefaultSolver>(problem, observer_fn);
        self.mapping.coverability_result(cegar_result)
    }
}

#[cfg(test)]
mod tests {
    use crate::net::builder::NetBuilder;
    use crate::system::PetriNet;
    use crate::system::marking::Marking;
    use crate::system::reachability::CoverabilityResult;
    use std::sync::mpsc;

    /// A deadline that has already passed makes the SMT solver give up, which must surface as
    /// `Inconclusive` (not a panic), while a generous deadline still yields the definite answer.
    #[test]
    fn coverability_within_gives_up_at_the_deadline() {
        use std::time::Duration;
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        assert!(matches!(
            sys.analyze_coverability_within([(p1, 1)], Duration::ZERO, None),
            CoverabilityResult::Inconclusive
        ));
        assert!(sys
            .analyze_coverability_within([(p1, 1)], Duration::from_secs(60), None)
            .is_coverable());
    }

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
        use crate::system::reachability::Lemma;

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
            CoverabilityResult::Coverable { .. } | CoverabilityResult::Inconclusive => unreachable!("checked above"),
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
        assert_eq!(contradiction.len(), 2, "expected 2 contradictory lemmas, found {}", contradiction.len());
    }

    #[test]
    fn transition_ordering_contradiction() {
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
        assert_eq!(lemmas.len(), 1, "expected 1 contradictory lemma, found {}", lemmas.len());
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
    }

    #[test]
    fn coverability_builder_efficient_effort_timeout_and_default() {
        use crate::net::class::NetClass;
        use crate::system::reachability::ReachabilityEffort;
        use std::time::Duration;

        // a strongly connected state machine is decided by comparing token sums, even with `Efficient`
        let (net, p0, _t0, p1, _t1) = crate::system::tests::two_place_cycle();
        let cycle = net.with_initial_marking([(p0, 1)]);
        assert_eq!(cycle.is_coverable().target([(p1, 1)]).effort(ReachabilityEffort::Efficient).call(), Some(true));
        assert_eq!(cycle.is_coverable().target([(p0, 2)]).effort(ReachabilityEffort::Efficient).call(), Some(false));

        // a general net has no efficient procedure
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arcs((p0, t0, p1));
        b.add_arcs((p0, t1, p2));
        b.add_arcs((p1, t2, p0));
        b.add_arcs((p2, t2, p0));
        b.add_arc((p1, t1));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::General);
        let sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.is_coverable().target([(p1, 1)]).effort(ReachabilityEffort::Efficient).call(), None);
        assert_eq!(sys.is_coverable().target([(p1, 1)]).call(), Some(true));
        assert_eq!(sys.is_coverable().target([(p1, 1)]).timeout(Duration::ZERO).call(), None);
        assert_eq!(sys.is_coverable().target([(p1, 1)]).timeout(Duration::from_secs(60)).call(), Some(true));
    }
}
