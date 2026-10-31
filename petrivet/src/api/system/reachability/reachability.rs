use crate::core::cegar::observe::ToCegarCallbackFn;
use crate::core::cegar::{CegarProblem, CegarProperty, cegar_decide};
use crate::core::net::siphon_trap::maximal_trap_in;
use crate::core::solver::DefaultSolver;
use crate::net::class::NetClass;
use crate::net::{Net, Transition};
use crate::system::PetriNet;
use crate::system::marking::Marking;
use crate::system::reachability::Lemma;
use crate::system::reachability::ReachabilityEffort;
use crate::system::reachability::RefinementEvent;
use crate::timeout::Timeout;
use bon::bon;
use std::sync::{Arc, mpsc};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reachability {
    /// The target marking is reachable from M₀.
    Reachable {
        firing_sequence: Vec<Transition>,
    },
    /// The target marking is definitely not reachable from M₀.
    Unreachable {
        contradiction: Vec<Lemma>,
    },
}

impl Reachability {
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

#[bon]
impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether `target` is reachable from the initial marking.
    ///
    /// Tries efficient procedures first.
    /// If `effort` is [`Efficient`](ReachabilityEffort::Efficient) and none applies, the answer is `None`;
    /// otherwise the question goes to [`reachability`](Self::reachability).
    /// If `timeout` is specified, the analysis gives up after that duration and the answer is `None`.
    /// Use [`reachability`](Self::reachability) for a witness or a refutation.
    #[must_use]
    #[builder]
    pub fn is_reachable(
        &self,
        #[builder(into)]
        target: Marking<u32>,
        effort: Option<ReachabilityEffort>,
        #[builder(name = timeout, with = |timeout: impl Timeout| timeout.into_instant())]
        deadline: Option<Instant>,
    ) -> Option<bool> {
        if let Some(answer) = self.is_efficiently_reachable(&target) {
            return Some(answer);
        }
        if effort == Some(ReachabilityEffort::Efficient) {
            return None;
        }
        self.reachability()
            .target(target)
            .maybe_timeout(deadline)
            .call()
            .map(|result| result.is_reachable())
    }

    /// If there is an efficient (polynomial-time) procedure to determine
    /// whether the given target marking is reachable in this Petri net,
    /// returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    fn is_efficiently_reachable(&self, target: &Marking<u32>) -> Option<bool> {
        let target = self.mapping.idx_marking(target.clone());
        match self.class() {
            NetClass::Circuit | NetClass::StateMachine if self.is_strongly_connected() => {
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

    /// Analyzes reachability of a target marking.
    /// Analyzes reachability of a target marking, like [`reachability`](Self::reachability),
    /// but additionally invokes `observer` with a [`RefinementEvent`] every time the search rules out a
    /// spurious candidate solution along the way. Useful for progress reporting on searches that
    /// take a while - the returned [`Reachability`] is unaffected by what the observer does.
    #[must_use]
    #[builder]
    pub fn reachability(
        &self,
        #[builder(into)]
        target: Marking<u32>,
        observer: Option<mpsc::Sender<RefinementEvent>>,
        #[builder(name = timeout, with = |timeout: impl Timeout| timeout.into_instant())]
        deadline: Option<Instant>,
    ) -> Option<Reachability> {
        let m0 = &self.marking;
        let target = &self.mapping.idx_marking(target);

        if m0 == target {
            return Some(Reachability::Reachable {
                firing_sequence: Vec::new(),
            });
        }

        let problem = CegarProblem {
            net: &self.dense_net,
            m0,
            target,
            property: CegarProperty::Reachable,
        };
        // Wrap the provided observer in a type-erased closure
        // that maps the internal `IdxCegarEvent` to the public `CegarEvent`.
        let callback_fn = observer.map(|sender| {
            sender.to_cegar_callback_fn(Arc::clone(&self.mapping))
        });
        let cegar_result = cegar_decide::<DefaultSolver>(problem, callback_fn, deadline);
        self.mapping.reachability_result(cegar_result)
    }
}

#[cfg(test)]
mod tests {
    use crate::net::builder::NetBuilder;
    use crate::net::class::NetClass;
    use crate::system::PetriNet;

    /// A deadline that has already passed makes the SMT solver give up, which must surface as
    /// `Inconclusive` (not a panic), while a generous deadline still yields the definite answer.
    #[test]
    fn reachability_within_gives_up_at_the_deadline() {
        use crate::system::reachability::Reachability;
        use std::time::Duration;
        let (net, p0, _t0, p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.reachability().target([(p1, 1)]).timeout(Duration::ZERO).call().is_none());
        assert!(matches!(
            sys.reachability().target([(p1, 1)]).timeout(Duration::from_secs(60)).call(),
            Some(Reachability::Reachable { .. })
        ));
    }

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, p0, _t0, p1, _t1) = crate::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.is_reachable().target([(p1, 1)]).call(), Some(true));
        assert_eq!(sys.is_reachable().target([(p0, 1)]).call(), Some(true));
        assert_eq!(sys.is_reachable().target([(p0, 2)]).call(), Some(false));
        assert_eq!(sys.is_reachable().target([]).call(), Some(false));
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
        assert_eq!(sys.is_reachable().target([(p2, 1)]).call(), Some(true));
        assert_eq!(sys.is_reachable().target([(p0, 1), (p1, 1)]).call(), Some(true));
        assert_eq!(sys.is_reachable().target([(p1, 1)]).call(), Some(false));
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
        assert_eq!(sys.is_reachable().target([(p0, 1)]).call(), Some(true));
        assert_eq!(sys.is_reachable().target([(p1, 1)]).call(), Some(true));
    }

    /// A net with no efficient reachability procedure: the answer needs the CEGAR analysis.
    fn general_net() -> (PetriNet, crate::net::Place, crate::net::Place) {
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
        (PetriNet::new(net, [(p0, 1)]), p0, p1)
    }

    #[test]
    fn efficient_effort_declines_where_no_efficient_procedure_applies() {
        use crate::system::reachability::ReachabilityEffort;
        let (sys, _p0, p1) = general_net();
        assert_eq!(sys.is_reachable().target([(p1, 1)]).effort(ReachabilityEffort::Efficient).call(), None);
        assert_eq!(sys.is_reachable().target([(p1, 1)]).effort(ReachabilityEffort::Any).call(), Some(true));
        assert_eq!(sys.is_reachable().target([(p1, 1)]).call(), Some(true));

        // a strongly connected state machine is decided by comparing token sums, even with `Efficient`
        let (net, p0, _t0, p1, _t1) = crate::system::tests::two_place_cycle();
        let cycle = net.with_initial_marking([(p0, 1)]);
        assert_eq!(cycle.is_reachable().target([(p1, 1)]).effort(ReachabilityEffort::Efficient).call(), Some(true));
        assert_eq!(cycle.is_reachable().target([(p0, 2)]).effort(ReachabilityEffort::Efficient).call(), Some(false));
    }

    #[test]
    fn is_reachable_gives_up_at_the_timeout() {
        use std::time::Duration;
        let (sys, _p0, p1) = general_net();
        assert_eq!(sys.is_reachable().target([(p1, 1)]).timeout(Duration::ZERO).call(), None);
        assert_eq!(sys.is_reachable().target([(p1, 1)]).timeout(Duration::from_secs(60)).call(), Some(true));
    }
}
