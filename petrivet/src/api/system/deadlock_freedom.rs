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
            Deadlocks::InitialDeadlock(deadlock) => {
                deadlock.take()
            }
            Deadlocks::Explorer(reachability_explorer) => {
                reachability_explorer
                    .core
                    .search(|m| reachability_explorer.core.state_space.net.is_deadlock(m))
                    .map(|m| reachability_explorer.mapping.encode(m.clone()))
            }
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure for deadlock-freedom
    /// for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    ///
    /// # Abstain rather than fabricate a negative
    ///
    /// A `Some(true)` here is certificate-backed: liveness implies
    /// deadlock-freedom, and the Commoner–Hack criterion (`Ok`) is a sufficient
    /// structural condition for it. We deliberately do **not** return a structural
    /// `Some(false)`: a net that is *not* live is not thereby deadlock-*ful* —
    /// non-liveness only means some transition can die, whereas a deadlock requires
    /// a reachable marking enabling *no* transition. Concluding "has a deadlock"
    /// from "not live" would be a fabricated negative with no witness. When the
    /// structural path cannot certify deadlock-freedom we abstain (`None`) and let
    /// [`deadlocks`](Self::deadlocks) decide by exploration.
    #[must_use]
    pub fn is_efficiently_deadlock_free(&self) -> Option<bool> {
        match self.is_efficiently_live() {
            Some(true) => Some(true), // liveness implies deadlock-freedom
            // No `Some(false)` arm: non-liveness does not certify a reachable
            // deadlock, so abstain instead of fabricating a negative.
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
            self
                .dense_net
                .is_deadlock(&self.marking)
                .then(|| self.mapping.encode(self.marking.clone()))
                .map_or_else(
                    || Deadlocks::Explorer(self.explore_reachability(ExplorationOrder::BreadthFirst)),
                    |deadlock| Deadlocks::InitialDeadlock(Some(deadlock))
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
        assert!(
            !dead.is_deadlock_free(),
            "an m₀ deadlock must be detected"
        );
        assert!(
            dead.deadlocks().next().is_some(),
            "deadlocks() must yield the initial deadlock marking"
        );
        // Control: the SAME cycle marked is live, hence deadlock-free; m₀ is not a
        // deadlock and the fix does not over-report.
        let live = net.with_initial_marking([(p0, 1)]);
        assert!(live.is_deadlock_free(), "a live marked cycle is deadlock-free");
        assert!(
            live.deadlocks().next().is_none(),
            "a live marked cycle has no deadlock"
        );
    }

    /// Regression: the structural deadlock-freedom path must never return
    /// `Some(false)`. Non-liveness (a transition can die) does not certify a
    /// reachable total deadlock, so a structural "has a deadlock" would be a
    /// fabricated negative. The former code returned `Some(false)` for non-live
    /// free-choice nets; it now abstains (`None`) and lets `deadlocks()` decide.
    #[test]
    fn efficient_deadlock_free_never_fabricates_false() {
        use crate::builder::NetBuilder;

        // A free choice at p0 (•t0 = •t1 = {p0}) where the t1 branch drains a token
        // into the sink p2 — the net can reach a deadlock. The structural path must
        // still not claim `Some(false)`; the honest answer comes from exploration.
        let mut fc = NetBuilder::new();
        let [p0, p1, p2] = fc.add_places();
        let [t0, t1] = fc.add_transitions();
        fc.add_arc((p0, t0));
        fc.add_arc((p0, t1)); // free choice
        fc.add_arc((t0, p1));
        fc.add_arc((p1, t0)); // t0 cycles through p1
        fc.add_arc((t1, p2)); // t1 drains into the sink p2
        let fc_net = fc.build().expect("valid net");
        let fc_sys = fc_net.with_initial_marking([(p0, 1)]);
        assert_ne!(
            fc_sys.is_efficiently_deadlock_free(),
            Some(false),
            "the structural path must never fabricate Some(false)"
        );

        // An unmarked (dead) cycle is genuinely NOT deadlock-free, but the
        // structural path must still abstain rather than return a fabricated false.
        let (dead, _d0, _dt0, _d1, _dt1) = crate::api::system::tests::two_place_cycle();
        let dead_sys = dead.with_initial_marking([]);
        assert_ne!(
            dead_sys.is_efficiently_deadlock_free(),
            Some(false),
            "even a genuinely deadlocked net gets no structural false verdict"
        );
    }
}