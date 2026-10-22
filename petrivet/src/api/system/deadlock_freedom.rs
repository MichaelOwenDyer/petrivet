use crate::net::Net;
use crate::net::class::NetClass;
use crate::state_space::{ExplorationOrder, ReachabilityGraphExplorer};
use crate::system::PetriNet;
use crate::system::marking::Marking;
use bon::bon;

/// A **deadlock** is a [`Marking`] which enables no [`Transition`](crate::net::Transition).
///
/// **Deadlock-freedom** is a property of a system (N, M₀)
/// which holds when no reachable marking is a deadlock.
///
/// Deadlock-freedom is a desirable property in many systems,
/// as it guarantees that the system can always make progress
/// and will never get stuck in a state where no actions are possible.
pub type Deadlock = Marking<u32>;

/// The procedure that decided a deadlock-freedom question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeadlockFreedomMethod {
    /// The initial marking is itself a deadlock.
    InitialMarking,
    /// A circuit is deadlock-free iff it carries a token.
    Circuit,
    /// A net with a source transition (no input places) can always fire that transition.
    SourceTransition,
    /// Analysis of the token flow in a state machine.
    StateMachine,
    /// Analysis of the unmarked circuits of a marked graph.
    MarkedGraph,
    /// The Commoner/Hack criterion: every proper siphon contains a marked trap.
    CommonerHack,
    /// The reachability graph was explored for deadlock markings.
    ReachabilityGraph,
}

/// An incremental iterator over all reachable deadlock markings in a system.
pub struct Deadlocks<'a>(DeadlocksInner<'a>);

impl Deadlocks<'_> {
    /// Sets a timeout for the deadlock search. If the timeout is reached, the iterator will stop yielding deadlocks.
    pub fn with_timeout(self, timeout: Option<std::time::Duration>) -> Self {
        self.with_deadline(timeout.and_then(|t| std::time::Instant::now().checked_add(t)))
    }

    pub fn with_deadline(mut self, deadline: Option<std::time::Instant>) -> Self {
        self.0 = match self.0 {
            DeadlocksInner::OneOrNone(deadlock) => DeadlocksInner::OneOrNone(deadlock),
            DeadlocksInner::Explorer { explorer, .. } => DeadlocksInner::Explorer {
                explorer,
                deadline,
            },
        };
        self
    }
}

enum DeadlocksInner<'a> {
    /// The initial marking iff it is itself a deadlock - yielded first, exactly once.
    OneOrNone(Option<Deadlock>),
    /// The system is not certified deadlock-free: explore the reachable markings for deadlocks.
    Explorer {
        explorer: ReachabilityGraphExplorer<'a>,
        deadline: Option<std::time::Instant>,
    },
}

impl Iterator for Deadlocks<'_> {
    type Item = Deadlock;

    fn next(&mut self) -> Option<Deadlock> {
        match &mut self.0 {
            DeadlocksInner::OneOrNone(deadlock) => deadlock.take(),
            DeadlocksInner::Explorer { explorer, deadline } => explorer
                .core
                .find(|m| explorer.core.state_space.net.is_deadlock(m), *deadline)
                .map(|m| explorer.mapping.marking(m.clone())),
        }
    }
}

pub enum DeadlockFreedomEffort {
    /// Use only efficient (polynomial-time) checks for deadlock-freedom.
    Efficient,
    /// Explore the reachability graph for deadlocks if necessary.
    Full,
}

#[bon]
impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns an iterator over all reachable deadlock markings in the system.
    #[must_use]
    pub fn deadlocks(&self) -> Deadlocks<'_> {
        if self.dense_net.is_deadlock(&self.marking) {
            Deadlocks(DeadlocksInner::OneOrNone(Some(self.mapping.marking(self.marking.clone()))))
        } else if self.commoner_hack_criterion() == Some(Ok(())) {
            Deadlocks(DeadlocksInner::OneOrNone(None))
        } else {
            Deadlocks(DeadlocksInner::Explorer {
                explorer: self.explore_reachability_graph(ExplorationOrder::BreadthFirst),
                deadline: None,
            })
        }
    }

    /// Returns true if the system is deadlock-free.
    #[must_use]
    #[builder]
    pub fn is_deadlock_free(
        &self,
        effort: Option<DeadlockFreedomEffort>,
        timeout: Option<std::time::Duration>,
    ) -> Option<bool> {
        let effort = effort.unwrap_or(DeadlockFreedomEffort::Full);
        let deadline = timeout.and_then(|t| std::time::Instant::now().checked_add(t));
        self.with_deadlock_cache(|cache| {
            match self.class() {
                _ if let Some(answer) = cache.is_deadlock_free() => Some(answer),
                NetClass::Circuit => {
                    let deadlock_free = self.marking.sum() > 0;
                    if !deadlock_free {
                        cache.add_deadlock(&self.marking);
                    }
                    cache.mark_complete();
                    Some(deadlock_free)
                },
                _ if self.has_source_transition() => {
                    cache.mark_deadlock_free();
                    Some(true)
                },
                NetClass::StateMachine => {
                    let deadlock_free = self.is_state_machine_deadlock_free();
                    if deadlock_free {
                        cache.mark_deadlock_free();
                    }
                    Some(deadlock_free)
                },
                NetClass::MarkedGraph => {
                    let deadlock_free = self.is_marked_graph_deadlock_free();
                    if deadlock_free {
                        cache.mark_deadlock_free();
                    }
                    Some(deadlock_free)
                },
                _ if self.dense_net.is_deadlock(&self.marking) => {
                    cache.add_deadlock(&self.marking);
                    cache.mark_complete();
                    Some(false)
                },
                _ if matches!(effort, DeadlockFreedomEffort::Efficient) => None,
                _ if self.commoner_hack_criterion_until(deadline) == Some(Ok(())) => {
                    cache.mark_deadlock_free();
                    Some(true)
                },
                _ if deadline.is_some_and(|d| std::time::Instant::now() >= d) => None,
                _ => {
                    let mut explorer = self.explore_reachability_graph(ExplorationOrder::BreadthFirst);
                    let net = explorer.core.state_space.net;
                    let deadlock = explorer.core.find(|m| net.is_deadlock(m), deadline).cloned();
                    match deadlock {
                        Some(deadlock) => {
                            cache.add_deadlock(&deadlock);
                            Some(false)
                        },
                        None if !explorer.is_fully_explored() => None,
                        None => {
                            cache.mark_deadlock_free();
                            Some(true)
                        },
                    }
                }
            }
        })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Called only for state machines.
    /// Returns true if the state machine is deadlock-free,
    /// i.e. some token can never reach a sink place.
    fn is_state_machine_deadlock_free(&self) -> bool {
        self.dense_net.some_token_can_never_reach_a_sink(&self.marking)
    }

    /// Called only for marked graphs.
    /// Returns true if the marked graph is deadlock-free,
    /// i.e. some transition is not on or downstream of an unmarked circuit.
    fn is_marked_graph_deadlock_free(&self) -> bool {
        self.dense_net.some_transition_is_not_downstream_of_an_unmarked_circuit(&self.marking)
    }
}

#[cfg(test)]
mod tests {
    /// The reported method names the procedure that actually decided the question.
    #[test]
    fn analyze_reports_the_deciding_method() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        assert!(net.with_initial_marking([(p0, 1)]).is_deadlock_free().call().unwrap());
        assert!(!net.with_initial_marking([]).is_deadlock_free().call().unwrap());
    }

    /// A deadlocked system is not deadlock-free, and the initial marking is yielded as a deadlock.
    #[test]
    fn initial_marking_deadlock_is_detected() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        let dead = net.with_initial_marking([]);
        assert!(!dead.is_deadlock_free().call().unwrap(), "an m₀ deadlock must be detected");
        assert!(
            dead.deadlocks().next().is_some(),
            "deadlocks() must yield the initial deadlock marking"
        );
        let live = net.with_initial_marking([(p0, 1)]);
        assert!(
            live.is_deadlock_free().call().unwrap(),
            "a live marked cycle is deadlock-free"
        );
        assert!(
            live.deadlocks().next().is_none(),
            "a live marked cycle has no deadlock"
        );
    }

    /// A live circuit feeds a transition of an unmarked circuit.
    /// The unmarked circuit stays dead forever, but the live circuit keeps firing,
    /// so the system is deadlock-free even though it contains an unmarked circuit.
    #[test]
    fn live_circuit_next_to_unmarked_circuit_is_deadlock_free() {
        use crate::net::builder::NetBuilder;
        use crate::net::class::NetClass;
        use crate::system::PetriNet;

        let mut b = NetBuilder::new();
        let [p1, p2] = b.add_places();
        let [t1, t2] = b.add_transitions();
        b.add_arcs((t1, p1, t2, p2, t1));
        let [q1, q2] = b.add_places();
        let [u1, u2] = b.add_transitions();
        b.add_arcs((u1, q1, u2, q2, u1));
        let link = b.add_place();
        b.add_arcs((t1, link, u1));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, [(p1, 1)]);
        assert!(sys.has_unmarked_circuit());
        assert!(sys.is_deadlock_free().call().unwrap());
    }

    /// If every transition is on or downstream of an unmarked circuit, all transitions
    /// fire only finitely often and a deadlock is reachable.
    #[test]
    fn marked_graph_with_every_transition_downstream_of_unmarked_circuit_deadlocks() {
        use crate::net::builder::NetBuilder;
        use crate::net::class::NetClass;
        use crate::system::PetriNet;

        let mut b = NetBuilder::new();
        let [p0, p1, p_link, p2, p3] = b.add_places();
        let [t0, t1, t2, t3] = b.add_transitions();
        b.add_arcs((t0, p0, t1, p1, t0)); // unmarked circuit
        b.add_arcs((t1, p_link, t2));
        b.add_arcs((t2, p2, t3, p3, t2)); // marked circuit, but fed by a dead transition
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);

        let sys = PetriNet::new(net, [(p2, 1)]);
        assert!(!sys.is_deadlock_free().call().unwrap());
    }
}
