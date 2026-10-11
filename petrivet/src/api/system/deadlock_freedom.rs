use crate::core::net::idx_set::PlaceIdxSet;
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
            NetClass::StateMachine => Some(
                self.marking.sum() > 0 && self.some_token_can_never_reach_a_sink()
            ),
            // FIXME: Counterexample: a live circuit marks an initially unmarked circuit.
            NetClass::MarkedGraph => Some(!self.has_unmarked_circuit()),
            _ => None,
        }
    }

    /// Returns an iterator over all reachable deadlock markings in the system.
    #[must_use]
    pub fn deadlocks(&self) -> Deadlocks<'_> {
        if self.is_efficiently_deadlock_free() == Some(true)
            || self.commoner_hack_criterion().is_ok() {
            Deadlocks::DeadlockFree
        } else if self.dense_net.is_deadlock(&self.marking) {
            Deadlocks::InitialDeadlock(Some(self.mapping.marking(self.marking.clone())))
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
    /// Returns true if some marked place can never reach a place with an empty postset.
    fn some_token_can_never_reach_a_sink(&self) -> bool {
        let mut sinks = PlaceIdxSet::none_of(self.dense_net.place_count());
        for p_idx in self.dense_net.place_indices() {
            if self.dense_net.postset_p[p_idx].is_empty() {
                sinks.add(p_idx);
            }
        }
        let places_that_can_reach_a_sink = self.dense_net.pre_star(sinks);
        !self.marking.support().is_subset(&places_that_can_reach_a_sink)
    }
}

#[cfg(test)]
mod tests {
    /// Soundness regression. When the INITIAL marking is itself a total deadlock,
    /// the system is **not** deadlock-free. The state-space explorer's `search`
    /// evaluates the deadlock predicate only on newly-discovered *successor*
    /// markings, so the (reachable) seed marking was never tested: `deadlocks()`
    /// returned nothing and `is_deadlock_free()` fabricated a `true`.
    /// `deadlocks()` now tests the seed first, exactly once.
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
}
