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
pub struct Deadlocks<'a>(Option<ReachabilityExplorer<'a>>);

impl Iterator for Deadlocks<'_> {
    type Item = Deadlock;

    fn next(&mut self) -> Option<Deadlock> {
        self.0.as_mut().and_then(|explorer| {
            explorer.core
                .search(|m| explorer.core.state_space.net.is_deadlock(m))
                .map(|m| explorer.mapping.encode(m.clone()))
        })
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
            _ => self.commoner_hack_criterion().ok().map(|_| true)
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