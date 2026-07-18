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

/// A certificate that the system is deadlock-free via the Commoner/Hack
/// criterion: every minimal siphon of the net, each paired with a trap it
/// contains that is marked at the initial marking. Each pair is independently
/// checkable evidence; together they witness that no reachable marking can empty
/// a siphon, so the system cannot reach a deadlock.
pub type DeadlockFreedomCertificate = super::chc::CommonerHackCriterion;

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

    /// Returns the Commoner/Hack **deadlock-freedom witness** that
    /// [`is_efficiently_deadlock_free`](Self::is_efficiently_deadlock_free) computes
    /// and then discards (that method yields only a bare boolean). A caller can
    /// inspect it — or independently re-check each pair — to see *why* the system is
    /// deadlock-free.
    ///
    /// - `Some(pairs)` with pairs non-empty — every minimal siphon is paired with a
    ///   trap that is marked at M₀. This is a sound proof: every siphon contains a
    ///   minimal siphon whose marked trap stays marked, so no reachable marking can
    ///   leave a siphon unmarked, and an unmarked siphon is exactly what a deadlock
    ///   requires.
    /// - `Some(empty)` — the criterion holds *vacuously* because the net has no
    ///   proper siphon (e.g. it has a source transition, which — having no input
    ///   places — is always enabled). Such a net is genuinely deadlock-free; there
    ///   is simply no siphon-trap argument to exhibit.
    /// - `None` — a siphon with no marked trap exists (a counterexample), so
    ///   deadlock-freedom cannot be established via this criterion.
    #[must_use]
    pub fn deadlock_freedom_certificate(&self) -> Option<DeadlockFreedomCertificate> {
        // Surface the witness faithfully: `Ok(_)` (possibly empty) when the criterion
        // holds, `None` on a counterexample siphon.
        self.commoner_hack_criterion().ok()
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::class::NetClass;
    use crate::prelude::PetriNet;

    #[test]
    fn certificate_surfaces_the_discarded_chc_witness_for_a_general_net() {
        // A general (non-free-choice) mutual-exclusion net: `enter1` and `enter2`
        // both consume the shared `mutex`, so •enter1 = {idle1, mutex} and
        // •enter2 = {idle2, mutex} overlap without either containing the other.
        // This is exactly the general-net arm where `is_efficiently_deadlock_free`
        // computes the Commoner/Hack witness and throws it away.
        let mut b = NetBuilder::new();
        let [idle1, crit1, idle2, crit2, mutex] = b.add_places();
        let [enter1, exit1, enter2, exit2] = b.add_transitions();
        b.add_arc((idle1, enter1));
        b.add_arc((mutex, enter1));
        b.add_arc((enter1, crit1));
        b.add_arc((crit1, exit1));
        b.add_arc((exit1, idle1));
        b.add_arc((exit1, mutex));
        b.add_arc((idle2, enter2));
        b.add_arc((mutex, enter2));
        b.add_arc((enter2, crit2));
        b.add_arc((crit2, exit2));
        b.add_arc((exit2, idle2));
        b.add_arc((exit2, mutex));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(idle1, 1), (idle2, 1), (mutex, 1)]);

        assert_ne!(
            sys.class(),
            NetClass::FreeChoice,
            "fixture must exercise the general-net arm"
        );

        let cert = sys
            .deadlock_freedom_certificate()
            .expect("the marked mutex net satisfies the Commoner/Hack criterion");
        assert!(!cert.is_empty());
        for pair in &cert {
            assert!(!pair.siphon.is_empty());
            // The Ok arm pairs every siphon with a trap that is marked at M₀.
            assert!(!pair.trap.is_empty(), "each siphon must be paired with a marked trap");
            assert!(
                pair.trap.iter().any(|&p| sys.marking().get(p) > 0),
                "the paired trap must hold a token at the initial marking"
            );
        }
        // A valid CHC certificate is sufficient for deadlock-freedom.
        assert!(sys.is_deadlock_free());
    }

    #[test]
    fn abstains_when_a_counterexample_siphon_exists() {
        // Unmarked two-place cycle (classified `Circuit`). The whole place set
        // {p0, p1} is a siphon, and its maximal trap {p0, p1} is unmarked at M₀, so
        // the Commoner/Hack criterion yields a COUNTEREXAMPLE (`Err`) and the
        // accessor returns `None` — deadlock-freedom cannot be certified this way
        // (indeed M₀ is itself a deadlock). Correspondingly,
        // `is_efficiently_deadlock_free` returns `None` here (a sound abstention),
        // falling back to reachability exploration.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, []);

        assert!(sys.deadlock_freedom_certificate().is_none());
        assert_eq!(sys.is_efficiently_deadlock_free(), None);
    }
}