use crate::class::NetClass;
use crate::marking::Marking;
use crate::net::Net;
use crate::prelude::PetriNet;
use crate::state_space::{ExplorationOrder, ReachabilityExplorer};

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
    /// The initial marking iff it is itself a deadlock — yielded first, exactly once.
    InitialDeadlock(Option<Deadlock>),
    /// The system is not certified deadlock-free: explore the reachable markings for deadlocks.
    Explorer(ReachabilityExplorer<'a>),
}

impl Iterator for Deadlocks<'_> {
    type Item = Deadlock;

    fn next(&mut self) -> Option<Deadlock> {
        match self {
            Deadlocks::DeadlockFree => None,
            Deadlocks::InitialDeadlock(deadlock) => deadlock.take(),
            Deadlocks::Explorer(reachability_explorer) => reachability_explorer
                .core
                .search(|m| reachability_explorer.core.state_space.net.is_deadlock(m))
                .map(|m| reachability_explorer.mapping.encode(m.clone())),
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure for deadlock-freedom
    /// for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_deadlock_free(&self) -> Option<bool> {
        match self.is_efficiently_live() {
            Some(true) => Some(true), // liveness implies deadlock-freedom
            Some(false) if self.class() == NetClass::FreeChoice => Some(false), // same condition, no need to check it again
            _ => self.commoner_hack_criterion().ok().map(|_| true),
        }
    }

    /// Returns an iterator over all reachable deadlock markings in the system.
    #[must_use]
    pub fn deadlocks(&self) -> Deadlocks<'_> {
        if self.is_efficiently_deadlock_free() == Some(true) {
            // Certified deadlock-free: no reachable marking — including m₀ — is a
            // deadlock.
            Deadlocks::DeadlockFree
        } else {
            // Yield the initial marking once if it is a deadlock.
            // Otherwise, explore the reachable markings for deadlocks.
            self.dense_net
                .is_deadlock(&self.marking)
                .then(|| self.mapping.encode(self.marking.clone()))
                .map_or_else(
                    || {
                        Deadlocks::Explorer(
                            self.explore_reachability(ExplorationOrder::BreadthFirst),
                        )
                    },
                    |deadlock| Deadlocks::InitialDeadlock(Some(deadlock)),
                )
        }
    }

    /// Returns true if the system is deadlock-free.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.deadlocks().next().is_none()
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
        let (net, p0, _t0, _p1, _t1) = crate::api::system::tests::two_place_cycle();
        // EMPTY initial marking: t0 needs p0, t1 needs p1, both empty — no
        // transition is enabled, so m₀ is itself a reachable total deadlock.
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
