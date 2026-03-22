pub mod explorer;
pub mod coverability;
pub mod reachability;

use crate::net::Transition;
use crate::{Net, System, TransitionKey};
pub use coverability::*;
pub use explorer::ExplorationOrder;
pub use reachability::*;
use std::fmt;
use std::marker::PhantomData;

impl<N: AsRef<Net>> System<N> {
    /// Whether a transition is enabled under the current marking.
    ///
    /// A transition t is enabled if every input place p in its preset has
    /// at least one token.
    #[must_use]
    pub fn is_enabled(&self, t: TransitionKey) -> bool {
        let net = self.net.as_ref();
        let dt = net.dense_transition(t);
        net.dense_input_places(dt).iter().all(|&p| self.marking[p] >= 1)
    }

    /// Dense-index firing for internal use by the state-space explorer.
    pub(crate) fn is_enabled_dense(&self, t: Transition) -> bool {
        let net = self.net.as_ref();
        net.dense_input_places(t).iter().all(|&p| self.marking[p] >= 1)
    }

    /// Returns the set of currently enabled transitions.
    ///
    /// This is a read-only query. To fire one of these, use [`try_fire`](Self::try_fire)
    /// or [`choose_and_fire`](Self::choose_and_fire).
    #[must_use]
    pub fn enabled_transitions(&self) -> Box<[TransitionKey]> {
        let net = self.net.as_ref();
        net.transition_keys().filter(|&t| self.is_enabled(t)).collect()
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub fn is_deadlocked(&self) -> bool {
        let net = self.net.as_ref();
        net.transitions().all(|t| !self.is_enabled_dense(t))
    }

    /// Check-and-fire a specific transition.
    ///
    /// Returns `Ok(())` if the transition was enabled and has been fired.
    /// # Errors
    /// Returns `Err(NotEnabled)` if it was not enabled.
    pub fn try_fire(&mut self, t: TransitionKey) -> Result<(), NotEnabled> {
        if self.is_enabled(t) {
            self.fire_unchecked(t);
            Ok(())
        } else {
            Err(NotEnabled(t))
        }
    }

    /// Fire any single enabled transition.
    ///
    /// Returns the transition that was fired, or `None` if no transition is
    /// enabled (deadlock).
    pub fn fire_any(&mut self) -> Option<TransitionKey> {
        let net = self.net.as_ref();
        let t = net.transition_keys().find(|&t| self.is_enabled(t))?;
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
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// let mut sys = System::new(net, [1, 0]);
    ///
    /// // Pick the first enabled transition
    /// let fired = sys.choose_and_fire(|enabled| enabled.first());
    /// assert_eq!(fired, Some(t0));
    ///
    /// // Pick a specific transition (t1 is now enabled since marking is [0,1])
    /// let fired = sys.choose_and_fire(|enabled| {
    ///     enabled.iter().find(|et| *et == t1)
    /// });
    /// assert_eq!(fired, Some(t1));
    /// ```
    pub fn choose_and_fire<F>(&mut self, choose: F) -> Option<TransitionKey>
    where
        F: for<'a> FnOnce(EnabledSet<'a>) -> Option<EnabledTransition<'a>>,
    {
        let enabled = self.enabled_transitions();
        let set = EnabledSet(enabled, PhantomData);
        let chosen = choose(set)?.0;
        self.fire_unchecked(chosen);
        Some(chosen)
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled. Underflow will
    /// panic in debug mode and wrap in release mode.
    pub fn fire_unchecked(&mut self, t: TransitionKey) {
        let net = self.net.as_ref();
        let dt = net.dense_transition(t);
        for &p in net.dense_input_places(dt) {
            self.marking[p] -= 1;
        }
        for &p in net.dense_output_places(dt) {
            self.marking[p] += 1;
        }
    }
}

/// Proof that a transition was found enabled in the current marking.
///
/// Cannot be constructed outside this module (private fields), cannot be
/// copied or cloned, and cannot escape the [`choose_and_fire`](System::choose_and_fire)
/// closure (higher-ranked lifetime bound).
pub struct EnabledTransition<'a>(TransitionKey, PhantomData<&'a ()>);

impl std::ops::Deref for EnabledTransition<'_> {
    type Target = TransitionKey;
    fn deref(&self) -> &TransitionKey {
        &self.0
    }
}

impl PartialEq<TransitionKey> for EnabledTransition<'_> {
    fn eq(&self, other: &TransitionKey) -> bool {
        self.0 == *other
    }
}

impl PartialEq<EnabledTransition<'_>> for TransitionKey {
    fn eq(&self, other: &EnabledTransition<'_>) -> bool {
        *self == other.0
    }
}

impl fmt::Debug for EnabledTransition<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnabledTransition({:?})", self.0)
    }
}

/// The set of transitions enabled in a specific marking.
///
/// Only exists inside the [`choose_and_fire`](System::choose_and_fire) closure.
pub struct EnabledSet<'a>(Box<[TransitionKey]>, PhantomData<&'a ()>);

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
pub struct NotEnabled(TransitionKey);

impl fmt::Display for NotEnabled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotEnabled(t) => write!(f, "transition {t} is not enabled"),
        }
    }
}

impl std::error::Error for NotEnabled {}