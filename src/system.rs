//! A Petri net system: net + marking, with simulation support.
//!
//! `System<N>` pairs a net structure with a mutable marking, providing methods
//! to check enablement and fire transitions. The type parameter `N` defaults to
//! [`Net`] for simplicity, but accepts any type implementing `AsRef<Net>`.
//!
//! # Firing API
//!
//! Three patterns for firing transitions:
//!
//! ```ignore
//! // 1. I know which transition — just try it
//! system.try_fire(t0)?;
//!
//! // 2. I need to choose from the enabled set — zero redundant checks
//! system.choose_and_fire(|enabled| enabled.first());
//!
//! // 3. Fire anything, I don't care which
//! system.fire_any();
//! ```

use crate::marking::Marking;
use crate::net::{Net, Transition};
use std::fmt;
use std::marker::PhantomData;

/// A Petri net system: a net N paired with a mutable marking.
///
/// The initial marking is stored for reference and [`reset`](System::reset).
/// The current marking evolves as transitions fire.
#[derive(Debug, Clone)]
pub struct System<N = Net> {
    net: N,
    initial_marking: Marking,
    marking: Marking,
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
    pub fn reset(&mut self) {
        self.marking = self.initial_marking.clone();
    }

    /// Consumes the system and returns (`net`, `initial_marking`, `current_marking`).
    #[must_use]
    pub fn into_parts(self) -> (N, Marking, Marking) {
        (self.net, self.initial_marking, self.marking)
    }

    /// Whether a transition is enabled under the current marking.
    ///
    /// A transition t is enabled if every input place p in its preset has
    /// at least one token.
    #[must_use]
    pub fn is_enabled(&self, t: Transition) -> bool {
        let net = self.net.as_ref();
        net.preset_t(t).iter().all(|&p| self.marking[p] >= 1)
    }

    /// Returns the set of currently enabled transitions.
    ///
    /// This is a read-only query. To fire one of these, use [`try_fire`](Self::try_fire)
    /// or [`choose_and_fire`](Self::choose_and_fire).
    #[must_use]
    pub fn enabled_transitions(&self) -> Vec<Transition> {
        let net = self.net.as_ref();
        net.transitions().filter(|&t| self.is_enabled(t)).collect()
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub fn is_deadlocked(&self) -> bool {
        let net = self.net.as_ref();
        net.transitions().all(|t| !self.is_enabled(t))
    }

    /// Check-and-fire a specific transition.
    ///
    /// Returns `Ok(())` if the transition was enabled and has been fired.
    /// Returns `Err(FireError::NotEnabled)` if it was not enabled.
    pub fn try_fire(&mut self, t: Transition) -> Result<(), FireError> {
        if self.is_enabled(t) {
            self.fire_unchecked(t);
            Ok(())
        } else {
            Err(FireError::NotEnabled(t))
        }
    }

    /// Fire any single enabled transition.
    ///
    /// Returns the transition that was fired, or `None` if no transition is
    /// enabled (deadlock).
    pub fn fire_any(&mut self) -> Option<Transition> {
        let net = self.net.as_ref();
        let t = net.transitions().find(|&t| self.is_enabled(t))?;
        self.fire_unchecked(t);
        Some(t)
    }

    /// Compute the enabled set, let the caller choose one, and fire it.
    ///
    /// The closure receives an [`EnabledSet`] and returns an
    /// [`EnabledTransition`] proof token for the chosen transition. The token
    /// cannot be fabricated (private fields), duplicated (not Copy/Clone), or
    /// stashed outside the closure (higher-ranked lifetime). This makes the
    /// subsequent fire infallible with zero redundant enablement checks.
    ///
    /// Returns the fired transition, or `None` if the closure chose not to fire
    /// (or no transitions were enabled).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Pick the first enabled transition
    /// system.choose_and_fire(|enabled| enabled.first());
    ///
    /// // Pick a specific transition if it's enabled
    /// system.choose_and_fire(|enabled| {
    ///     enabled.iter().find(|et| *et == t0)
    /// });
    /// ```
    pub fn choose_and_fire<F>(&mut self, choose: F) -> Option<Transition>
    where
        F: for<'a> FnOnce(EnabledSet<'a>) -> Option<EnabledTransition<'a>>,
    {
        let enabled = self.enabled_transitions();
        let set = EnabledSet(enabled, PhantomData);
        let chosen = choose(set)?;
        let t = chosen.0;
        self.fire_unchecked(t);
        Some(t)
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled. Underflow will
    /// panic in debug mode and wrap in release mode.
    fn fire_unchecked(&mut self, t: Transition) {
        let net = self.net.as_ref();
        for &p in net.preset_t(t) {
            self.marking[p] -= 1;
        }
        for &p in net.postset_t(t) {
            self.marking[p] += 1;
        }
    }
}

/// Proof that a transition was found enabled in the current marking.
///
/// Cannot be constructed outside this module (private fields), cannot be
/// copied or cloned, and cannot escape the [`choose_and_fire`](System::choose_and_fire)
/// closure (higher-ranked lifetime bound).
pub struct EnabledTransition<'a>(Transition, PhantomData<&'a ()>);

impl std::ops::Deref for EnabledTransition<'_> {
    type Target = Transition;
    fn deref(&self) -> &Transition {
        &self.0
    }
}

impl PartialEq<Transition> for EnabledTransition<'_> {
    fn eq(&self, other: &Transition) -> bool {
        self.0 == *other
    }
}

impl PartialEq<EnabledTransition<'_>> for Transition {
    fn eq(&self, other: &EnabledTransition<'_>) -> bool {
        *self == other.0
    }
}

impl fmt::Debug for EnabledTransition<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnabledTransition({})", self.0)
    }
}

/// The set of transitions enabled in a specific marking.
///
/// Only exists inside the [`choose_and_fire`](System::choose_and_fire) closure.
pub struct EnabledSet<'a>(Vec<Transition>, PhantomData<&'a ()>);

impl<'a> EnabledSet<'a> {
    /// Returns the first enabled transition, if any.
    #[must_use]
    pub fn first(&self) -> Option<EnabledTransition<'a>> {
        self.0.first().map(|&t| EnabledTransition(t, PhantomData))
    }

    /// Returns the enabled transition at the given index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<EnabledTransition<'a>> {
        self.0.get(index).map(|&t| EnabledTransition(t, PhantomData))
    }

    /// Iterator over enabled transitions as proof tokens.
    pub fn iter(&self) -> impl Iterator<Item = EnabledTransition<'a>> + '_ {
        self.0.iter().map(|&t| EnabledTransition(t, PhantomData))
    }

    /// Number of enabled transitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no transitions are enabled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for EnabledSet<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EnabledSet").field(&self.0).finish()
    }
}

/// Error returned when attempting to fire a transition that is not enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FireError {
    /// The transition is not enabled under the current marking.
    NotEnabled(Transition),
}

impl fmt::Display for FireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FireError::NotEnabled(t) => write!(f, "transition {t} is not enabled"),
        }
    }
}

impl std::error::Error for FireError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    use crate::net::class::ClassifiedNet;

    /// Shorthand for creating a `Marking<u32>` in tests.
    fn m(val: impl Into<Marking>) -> Marking { val.into() }

    /// Builds a simple two-place cycle: p0 -> t0 -> p1 -> t1 -> p0
    fn two_place_cycle() -> (ClassifiedNet, Transition, Transition) {
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
}
