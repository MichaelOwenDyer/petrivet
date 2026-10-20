use crate::core::cegar::observe::ToCegarCallbackFn;
use crate::core::cegar::{CegarProblem, CegarQuestion, cegar_decide};
use crate::core::net::siphon_trap::maximal_trap_in;
use crate::core::solver::DefaultSolver;
use crate::net::class::NetClass;
use crate::net::{Net, Transition};
use crate::system::PetriNet;
use crate::system::marking::Marking;
use crate::system::reachability::Lemma;
use crate::system::reachability::SpuriousSolutionEliminatedEvent;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum ReachabilityResult {
    /// The target marking is reachable from M₀.
    Reachable {
        firing_sequence: Vec<Transition>,
    },
    /// The target marking is definitely not reachable from M₀.
    Unreachable {
        contradiction: Vec<Lemma>,
    },
    /// The reachability of the target marking is inconclusive.
    Inconclusive,
}

impl ReachabilityResult {
    /// Whether the target is definitely reachable.
    #[must_use]
    pub const fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable { .. })
    }

    /// Whether the target is definitely unreachable.
    #[must_use]
    pub const fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable { .. })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure to determine
    /// whether the given target marking is reachable in this Petri net,
    /// returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_reachable(&self, target: &Marking<u32>) -> Option<bool> {
        let target = self.mapping.idx_marking(target.clone());
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() == target.sum()),
            NetClass::StateMachine if self.is_strongly_connected() => {
                Some(self.marking.sum() == target.sum())
            }
            // If the maximal unmarked trap in the target is marked in m0, then the target is unreachable.
            _ if maximal_trap_in(&self.dense_net, target.support().complement())
                .into_iter()
                .any(|p_idx| self.marking[p_idx] > 0) => Some(false),
            // NetClass::MarkedGraph if self.is_live() => Some(self.marking() ~ target) // todo: ~ relation
            // NetClass::FreeChoice if self.is_live() && self.is_bounded() => None, // todo: requires ILP + trap check
            _ => None,
        }
    }

    /// Whether `target` is reachable from the initial marking.
    ///
    /// Delegates to [`analyze_reachability`](Self::analyze_reachability).
    /// Returns `false` for inconclusive results.
    #[must_use]
    pub fn is_reachable(&self, target: impl Into<Marking<u32>>) -> Option<bool> {
        let target = target.into();
        self.is_efficiently_reachable(&target).or_else(|| {
            match self.analyze_reachability(target, None) {
                ReachabilityResult::Reachable { .. } => Some(true),
                ReachabilityResult::Unreachable { .. } => Some(false),
                ReachabilityResult::Inconclusive => None,
            }
        })
    }

    /// Analyzes reachability of a target marking.
    /// Analyzes reachability of a target marking, like [`analyze_reachability`](Self::analyze_reachability),
    /// but additionally invokes `observer` with a [`SpuriousSolutionEliminatedEvent`] every time the search rules out a
    /// spurious candidate solution along the way. Useful for progress reporting on searches that
    /// take a while - the returned [`ReachabilityResult`] is unaffected by what the observer does.
    #[must_use]
    pub fn analyze_reachability(
        &self,
        target: impl Into<Marking<u32>>,
        observer: Option<mpsc::Sender<SpuriousSolutionEliminatedEvent>>,
    ) -> ReachabilityResult {
        self.analyze_reachability_until(target, observer, None)
    }

    /// Like [`analyze_reachability`](Self::analyze_reachability), but gives up after `timeout`
    /// and returns [`ReachabilityResult::Inconclusive`] if the search has not finished by then.
    #[must_use]
    pub fn analyze_reachability_within(
        &self,
        target: impl Into<Marking<u32>>,
        timeout: Duration,
        observer: Option<mpsc::Sender<SpuriousSolutionEliminatedEvent>>,
    ) -> ReachabilityResult {
        self.analyze_reachability_until(target, observer, Instant::now().checked_add(timeout))
    }

    fn analyze_reachability_until(
        &self,
        target: impl Into<Marking<u32>>,
        observer: Option<mpsc::Sender<SpuriousSolutionEliminatedEvent>>,
        deadline: Option<Instant>,
    ) -> ReachabilityResult {
        let m0 = &self.marking;
        let target = &self.mapping.idx_marking(target.into());

        if m0 == target {
            return ReachabilityResult::Reachable {
                firing_sequence: Vec::new(),
            };
        }

        let problem = CegarProblem {
            net: &self.dense_net,
            m0,
            target,
            question: CegarQuestion::Reachable,
            deadline,
        };
        // Wrap the provided observer in a type-erased closure
        // that maps the internal `IdxCegarEvent` to the public `CegarEvent`.
        let callback_fn = observer.map(|sender| {
            sender.to_cegar_callback_fn(Arc::clone(&self.mapping))
        });
        let cegar_result = cegar_decide::<DefaultSolver>(problem, callback_fn);
        self.mapping.reachability_result(cegar_result)
    }
}

#[cfg(test)]
mod tests {
    use crate::net::builder::NetBuilder;
    use crate::net::class::NetClass;

    /// A deadline that has already passed makes the SMT solver give up, which must surface as
    /// `Inconclusive` (not a panic), while a generous deadline still yields the definite answer.
    #[test]
    fn reachability_within_gives_up_at_the_deadline() {
        use crate::system::reachability::ReachabilityResult;
        use std::time::Duration;
        let (net, p0, _t0, p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(matches!(
            sys.analyze_reachability_within([(p1, 1)], Duration::ZERO, None),
            ReachabilityResult::Inconclusive
        ));
        assert!(sys
            .analyze_reachability_within([(p1, 1)], Duration::from_secs(60), None)
            .is_reachable());
    }

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, p0, _t0, p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.is_reachable([(p1, 1)]), Some(true));
        assert_eq!(sys.is_reachable([(p0, 1)]), Some(true));
        assert_eq!(sys.is_reachable([(p0, 2)]), Some(false));
        assert_eq!(sys.is_reachable([]), Some(false));
    }

    #[test]
    fn t_net_reachability_dispatches() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p1, t0));
        b.add_arcs((t0, p2, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p1));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        assert_eq!(sys.is_reachable([(p2, 1)]), Some(true));
        assert_eq!(sys.is_reachable([(p0, 1), (p1, 1)]), Some(true));
        assert_eq!(sys.is_reachable([(p1, 1)]), Some(false));
    }

    #[test]
    fn general_net_reachability_fallback() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arcs((p0, t0, p1));
        b.add_arcs((p0, t1, p2));
        b.add_arcs((p1, t2, p0));
        b.add_arcs((p2, t2, p0));
        b.add_arc((p1, t1)); // extra arc to make it a general net
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::General);
        let sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.is_reachable([(p0, 1)]), Some(true));
        assert_eq!(sys.is_reachable([(p1, 1)]), Some(true));
    }
}
