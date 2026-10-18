use crate::net::Net;
use crate::net::class::NetClass;
use crate::state_space::{ExplorationOrder, ReachabilityGraphExplorer};
use crate::system::PetriNet;
use crate::system::marking::Marking;

/// A **deadlock** is a [`Marking`] which enables no [`Transition`](crate::net::Transition).
///
/// **Deadlock-freedom** is a property of a system (N, M₀)
/// which holds when no reachable marking is a deadlock.
///
/// Deadlock-freedom is a desirable property in many systems,
/// as it guarantees that the system can always make progress
/// and will never get stuck in a state where no actions are possible.
pub type Deadlock = Marking<u32>;

/// An incremental iterator over all reachable deadlock markings in a system.
pub enum Deadlocks<'a> {
    /// The system is certified deadlock-free: no reachable marking is a deadlock.
    DeadlockFree,
    /// The initial marking iff it is itself a deadlock - yielded first, exactly once.
    InitialDeadlock(Option<Deadlock>),
    /// The system is not certified deadlock-free: explore the reachable markings for deadlocks.
    Explorer(ReachabilityGraphExplorer<'a>),
}

impl Iterator for Deadlocks<'_> {
    type Item = Deadlock;

    fn next(&mut self) -> Option<Deadlock> {
        match self {
            Deadlocks::DeadlockFree => None,
            Deadlocks::InitialDeadlock(deadlock) => deadlock.take(),
            Deadlocks::Explorer(reachability_explorer) => reachability_explorer
                .core
                .find(|m| reachability_explorer.core.state_space.net.is_deadlock(m))
                .map(|m| reachability_explorer.mapping.marking(m.clone())),
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure for deadlock-freedom
    /// for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_deadlock_free(&self) -> Option<bool> {
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() > 0),
            _ if self.has_source_transition() => Some(true),
            NetClass::StateMachine => Some(self.is_state_machine_deadlock_free()),
            NetClass::MarkedGraph => Some(self.is_marked_graph_deadlock_free()),
            _ => None,
        }
    }

    /// Returns an iterator over all reachable deadlock markings in the system.
    #[must_use]
    pub fn deadlocks(&self) -> Deadlocks<'_> {
        if self.dense_net.is_deadlock(&self.marking) {
            Deadlocks::InitialDeadlock(Some(self.mapping.marking(self.marking.clone())))
        } else if self.is_efficiently_deadlock_free() == Some(true)
            || self.commoner_hack_criterion().is_ok() {
            Deadlocks::DeadlockFree
        } else {
            Deadlocks::Explorer(self.explore_reachability_graph(ExplorationOrder::BreadthFirst))
        }
    }

    /// Returns true if the system is deadlock-free.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.deadlocks().next().is_none()
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
    /// A deadlocked system is not deadlock-free, and the initial marking is yielded as a deadlock.
    #[test]
    fn initial_marking_deadlock_is_detected() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        let dead = net.with_initial_marking([]);
        assert!(!dead.is_deadlock_free(), "an m₀ deadlock must be detected");
        assert!(
            dead.deadlocks().next().is_some(),
            "deadlocks() must yield the initial deadlock marking"
        );
        // Control: the SAME cycle marked is live, hence deadlock-free; m₀ is not a
        // deadlock and the fix does not over-report.
        let live = net.with_initial_marking([(p0, 1)]);
        assert!(
            live.is_deadlock_free(),
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
        assert_eq!(sys.is_efficiently_deadlock_free(), Some(true));
        assert!(sys.is_deadlock_free());
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
        assert_eq!(sys.is_efficiently_deadlock_free(), Some(false));
        assert!(!sys.is_deadlock_free());
    }
}
