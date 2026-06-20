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
pub struct Deadlocks<'a>(Option<ReachabilityExplorer<'a>>);

impl Iterator for Deadlocks<'_> {
    type Item = Deadlock;

    fn next(&mut self) -> Option<Deadlock> {
        self.0.as_mut().and_then(|explorer| {
            explorer.core
                .search(|m| explorer.core.state_space.net.is_deadlock(m))
                .map(|m| explorer.mapping.marking(m.clone()))
        })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure for deadlock-freedom
    /// for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    ///
    /// # A2-family soundness note
    ///
    /// A `Some(true)` here is backed by a real theorem: liveness implies
    /// deadlock-freedom, and the Commoner-Hack criterion (`Ok`) is a sufficient
    /// structural condition for deadlock-freedom.
    ///
    /// We deliberately do **not** return a structural `Some(false)`. A net that
    /// is *not* live is not thereby deadlock-*ful*: non-liveness only means some
    /// transition can die, whereas a deadlock requires a reachable marking
    /// enabling *no* transition. Concluding "has a deadlock" from "not live"
    /// would be a fabricated negative with no witness (the same A2 sin as the
    /// `is_covered_by_s_components` stub). When the structural path cannot
    /// certify deadlock-freedom we abstain (`None`) and let `deadlocks()` decide
    /// by exploration. A certifying structural *deadlock-free* verdict for the
    /// general case (an exhibited siphon-with-marked-trap cover) is backlog B11
    /// (M9). Rationale: foundational-design §F5 (the derived-decider finding),
    /// BACKLOG A2/B11.
    #[must_use]
    pub fn is_efficiently_deadlock_free(&self) -> Option<bool> {
        match self.is_efficiently_live() {
            Some(true) => Some(true), // liveness implies deadlock-freedom
            // NOTE: no `Some(false)` arm — non-liveness does not certify a
            // reachable deadlock; abstain instead of fabricating a negative.
            _ => self.commoner_hack_criterion().ok().map(|_| true),
        }
    }

    /// Returns an iterator over all reachable deadlock markings in the system.
    #[must_use]
    pub fn deadlocks(&self) -> Deadlocks<'_> {
        if self.is_efficiently_deadlock_free() == Some(true) {
            Deadlocks(None)
        } else {
            Deadlocks(Some(self.explore_reachability(ExplorationOrder::BreadthFirst)))
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
    use crate::builder::NetBuilder;
    use crate::prelude::PetriNet;

    /// A2-family regression (the derived fabricated-negative). The structural
    /// deadlock-freedom path must never return `Some(false)`: non-liveness (a
    /// transition can die) does NOT certify a reachable total deadlock, so
    /// concluding "has a deadlock" structurally would be a fabricated negative
    /// with no witness. The old code returned `Some(false)` for non-live
    /// free-choice nets (inheriting it from `is_efficiently_live`); the fix
    /// abstains (`None`) and lets `deadlocks()` decide by exploration. A
    /// certifying structural deadlock-*free* verdict is backlog B11 (M9).
    ///
    /// The invariant is total: across S-net, T-net, free-choice, and general
    /// nets — live, non-live, and deadlocked — `is_efficiently_deadlock_free`
    /// returns only `Some(true)` (certificate-backed) or `None` (abstain).
    #[test]
    fn deadlock_free_efficient_path_never_fabricates_false() {
        // A net with a free choice at p0 (•t0 = •t1 = {p0}) where the t1 branch
        // drains a token into the sink p2 — the net can reach a deadlock. The
        // structural deadlock-freedom path must still NOT claim `Some(false)`;
        // the honest answer comes from `deadlocks()` exploration.
        let mut fc = NetBuilder::new();
        let [p0, p1, p2] = fc.add_places();
        let [t0, t1] = fc.add_transitions();
        fc.add_arc((p0, t0)); // •t0 = {p0}
        fc.add_arc((p0, t1)); // •t1 = {p0}  → free choice
        fc.add_arc((t0, p1));
        fc.add_arc((p1, t0)); // t0 cycles through p1
        fc.add_arc((t1, p2)); // t1 drains a token into the sink p2
        let fc_net = fc.build().expect("valid net");
        let fc_sys = PetriNet::new(fc_net, [(p0, 1)]);
        assert_ne!(
            fc_sys.is_efficiently_deadlock_free(),
            Some(false),
            "A2: the structural deadlock-freedom path must never fabricate `Some(false)`"
        );

        // A live cycle: deadlock-free, structurally decided as Some(true).
        let (cyc, c0, _t0, _c1, _t1) = crate::api::system::tests::two_place_cycle();
        let cyc_sys = PetriNet::new(cyc, [(c0, 1)]);
        assert_eq!(cyc_sys.is_efficiently_deadlock_free(), Some(true));

        // An unmarked (dead) cycle: NOT deadlock-free, but the structural path
        // must abstain (`None`) rather than return a fabricated `Some(false)`.
        let (dead, _d0, _dt0, _d1, _dt1) = crate::api::system::tests::two_place_cycle();
        let dead_sys = PetriNet::new(dead, []);
        assert_ne!(
            dead_sys.is_efficiently_deadlock_free(),
            Some(false),
            "A2: even a genuinely deadlocked net is not given a structural false verdict"
        );
    }
}