//! A Petri net system: net + marking, with simulation and behavioral analysis.
//!
//! `System<N>` pairs a net structure with a mutable marking, providing methods
//! to simulate (check enablement, fire transitions) and analyze behavior
//! (boundedness, liveness, deadness).
//!
//! # Quick start
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::System;
//!
//! // Build a simple producer-consumer net
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arc((idle, start));
//! b.add_arc((start, busy));
//! b.add_arc((busy, finish));
//! b.add_arc((finish, idle));
//! let net = b.build().expect("valid net");
//!
//! let mut sys = System::new(net, [1, 0]);
//!
//! // Simulation
//! assert!(sys.is_enabled(start));
//! sys.try_fire(start).unwrap();
//! assert_eq!(sys.marking().iter().collect::<Vec<_>>(), vec![&0, &1]);
//!
//! // Behavioral analysis
//! sys.reset();
//! assert!(sys.is_bounded());
//! assert!(sys.is_live());
//! ```
//!
//! # Firing patterns
//!
//! Three patterns for firing transitions:
//!
//! ```
//! # use petrivet::net::builder::NetBuilder;
//! # use petrivet::system::System;
//! # let mut b = NetBuilder::new();
//! # let [p0, p1] = b.add_places();
//! # let [t0, t1] = b.add_transitions();
//! # b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! # b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! # let net = b.build().unwrap();
//! # let mut sys = System::new(net, [1, 0]);
//! // 1. I know which transition - just try it
//! sys.try_fire(t0).unwrap();
//!
//! // 2. I need to choose from the enabled set - zero redundant checks
//! sys.choose_and_fire(|enabled| enabled.first());
//!
//! // 3. Fire anything, I don't care which
//! sys.fire_any();
//! ```

use crate::marking::Marking;
use crate::net::{Net, Transition};

/// A Petri net system: a net N paired with a mutable marking.
///
/// `N` can be any type that provides access to a [`Net`] reference via [`AsRef<Net>`]:
/// `Net` (owned), `&Net` (borrowed), `Rc<Net>`, `Arc<Net>`, etc.
/// This lets callers choose the ownership strategy that fits their use case.
///
/// The initial marking is stored for reference and [`reset`](System::reset).
/// The current marking evolves as transitions fire.
#[derive(Debug, Clone)]
pub struct System<N: AsRef<Net>> {
    pub(crate) net: N,
    pub(crate) initial_marking: Marking,
    pub(crate) marking: Marking,
}

impl<N: AsRef<Net>> System<N> {
    /// Creates a new system from a net and initial marking.
    ///
    /// Accepts anything that converts to `Marking`.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if the marking length doesn't match the number of
    /// places in the net.
    #[must_use]
    pub fn new(net: N, initial_marking: impl Into<Marking>) -> Self {
        let initial_marking = initial_marking.into();
        debug_assert_eq!(
            initial_marking.len(),
            net.as_ref().n_places(),
            "marking length ({}) must equal number of places ({})",
            initial_marking.len(),
            net.as_ref().n_places(),
        );
        let marking = initial_marking.clone();
        Self { net, initial_marking, marking }
    }

    /// Returns a reference to the underlying net.
    #[must_use]
    pub fn net(&self) -> &N {
        &self.net
    }

    /// Returns the initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        &self.initial_marking
    }

    /// Returns the current marking.
    #[must_use]
    pub fn marking(&self) -> &Marking {
        &self.marking
    }

    /// Resets the current marking to the initial marking.
    /// Returns the old marking before the reset.
    pub fn reset(&mut self) -> Marking {
        std::mem::replace(&mut self.marking, self.initial_marking.clone())
    }

    /// Consumes the system and returns (`net`, `initial_marking`, `current_marking`).
    #[must_use]
    pub fn into_parts(self) -> (N, Marking, Marking) {
        (self.net, self.initial_marking, self.marking)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    /// Shorthand for creating a `Marking<u32>` in tests.
    fn m(val: impl Into<Marking>) -> Marking { val.into() }

    /// Builds a simple two-place cycle: p0 -> t0 -> p1 -> t1 -> p0
    fn two_place_cycle() -> (Net, Transition, Transition) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().expect("valid net");
        (net, t0, t1)
    }

    #[test]
    fn basic_firing() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        assert_eq!(sys.marking(), m([1, 0]));
        assert!(sys.is_enabled(t0));
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.marking(), m([0, 1]));
    }

    #[test]
    fn try_fire_not_enabled() {
        let (net, _t0, t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        assert!(sys.try_fire(t1).is_err());
    }

    #[test]
    fn fire_any_deadlock() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [0, 0]);
        assert!(sys.is_deadlocked());
        assert!(sys.fire_any().is_none());
    }

    #[test]
    fn fire_any_success() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        assert!(!sys.is_deadlocked());
        let fired = sys.fire_any();
        assert!(fired.is_some());
        assert_eq!(sys.marking(), m([0, 1]));
    }

    #[test]
    fn choose_and_fire_first() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        let fired = sys.choose_and_fire(|enabled| enabled.first());
        assert_eq!(fired, Some(t0));
        assert_eq!(sys.marking(), m([0, 1]));
    }

    #[test]
    fn choose_and_fire_specific() {
        let (net, _t0, t1) = two_place_cycle();
        let mut sys = System::new(net, [0, 1]);
        let fired = sys.choose_and_fire(|enabled| {
            enabled.iter().find(|et| *et == t1)
        });
        assert_eq!(fired, Some(t1));
        assert_eq!(sys.marking(), m([1, 0]));
    }

    #[test]
    fn choose_and_fire_none_enabled() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [0, 0]);
        let fired = sys.choose_and_fire(|enabled| enabled.first());
        assert_eq!(fired, None);
    }

    #[test]
    fn choose_and_fire_user_declines() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        let fired = sys.choose_and_fire(|_enabled| None);
        assert_eq!(fired, None);
        assert_eq!(sys.marking(), m([1, 0]));
    }

    #[test]
    fn reset() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.marking(), m([0, 1]));
        sys.reset();
        assert_eq!(sys.marking(), m([1, 0]));
    }

    #[test]
    fn enabled_transitions_query() {
        let (net, t0, t1) = two_place_cycle();
        let sys = System::new(net, [1, 1]);
        let enabled = sys.enabled_transitions();
        assert!(enabled.contains(&t0));
        assert!(enabled.contains(&t1));
    }

    #[test]
    fn into_parts() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        sys.try_fire(t0).unwrap();
        let (_net, initial, current) = sys.into_parts();
        assert_eq!(initial, m([1, 0]));
        assert_eq!(current, m([0, 1]));
    }

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, _, _) = two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_bounded());
    }

    #[test]
    fn cycle_is_live() {
        let (net, _, _) = two_place_cycle();
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_live());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let (net, _, _) = two_place_cycle();
        let sys = System::new(net, [0, 0]);
        assert!(!sys.is_live());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, t0, t1) = two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = System::new(net, [0, 0]);
        let liveness = sys.analyze_liveness();
        assert!(liveness.transition_level(t0).is_dead());
        assert!(liveness.transition_level(t1).is_dead());
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, t0, t1) = two_place_cycle();
        let sys = System::new(net, [1, 0]);
        let liveness = sys.analyze_liveness();
        assert!(!liveness.transition_level(t0).is_dead());
        assert!(!liveness.transition_level(t1).is_dead());
    }

    #[test]
    fn unbounded_not_structurally_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        assert!(!net.is_structurally_bounded());
        let sys = System::new(net, [1, 0]);
        assert!(!sys.is_bounded());
    }

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, _, _) = two_place_cycle();
        assert!(net.is_s_net());
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_reachable(&m([0, 1])));
        assert!(sys.is_reachable(&m([1, 0])));
        assert!(!sys.is_reachable(&m([2, 0])));
        assert!(!sys.is_reachable(&m([0, 0])));
    }

    #[test]
    fn t_net_reachability_dispatches() {
        // T-net: {p0, p1} → t0 → p2 → t1 → {p0, p1}
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((p1, t0)); b.add_arc((t0, p2));
        b.add_arc((p2, t1)); b.add_arc((t1, p0)); b.add_arc((t1, p1));
        let net = b.build().unwrap();
        assert!(net.is_t_net());
        let sys = System::new(net, [1, 1, 0]);
        assert!(sys.is_reachable(&m([0, 0, 1])));
        assert!(sys.is_reachable(&m([1, 1, 0])));
        assert!(!sys.is_reachable(&m([1, 0, 0])));
    }

    #[test]
    fn general_net_reachability_fallback() {
        // Free-choice net (not S-net, not T-net): falls back to exploration
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        b.add_arc((p1, t2)); b.add_arc((t2, p0));
        b.add_arc((p2, t2)); b.add_arc((t2, p0));
        let net = b.build().unwrap();
        assert!(!net.is_s_net());
        assert!(!net.is_t_net());
        let sys = System::new(net, [1, 0, 0]);
        assert!(sys.is_reachable(&m([0, 1, 0])));
        assert!(sys.is_reachable(&m([1, 0, 0])));
    }

    #[test]
    fn mutex_is_live_and_bounded() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().expect("valid net");
        let sys = System::new(net, [1u32, 0, 0, 1, 0, 0, 1]);
        assert!(sys.is_bounded());
        assert!(sys.is_live());
    }
}
