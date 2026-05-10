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
//! use petrivet::net::system::PetriNet;
//!
//! // Build a simple producer-consumer net
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arcs((idle, start, busy, finish, idle));
//! let net = b.build().expect("valid net");
//!
//! let mut sys = PetriNet::new(&net, [(idle, 1)].into());
//!
//! // Simulation
//! assert!(sys.is_enabled(start));
//! sys.fire_unchecked(start);
//! assert_eq!(sys.current_marking(), [(busy, 1)].into());
//!
//! // Behavioral analysis
//! let sys = PetriNet::new(&net, [(idle, 1)].into());
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
//! # use petrivet::net::system::PetriNet;
//! # let mut b = NetBuilder::new();
//! # let [p0, p1] = b.add_places();
//! # let [t0, t1] = b.add_transitions();
//! # b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! # b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! # let net = b.build().unwrap();
//! # let mut sys = PetriNet::new(net, [1, 0]);
//! // 1. I know which transition - just try it
//! sys.try_fire(t0).unwrap();
//!
//! // 2. I need to choose from the enabled set - zero redundant checks
//! sys.choose_and_fire(|enabled| enabled.first());
//!
//! // 3. Fire anything, I don't care which
//! sys.fire_any();
//! ```

use crate::net::idx::TransitionIdx;
use crate::net::marking::IdxMarking;
use crate::net::Net;
use crate::{CoverabilityExplorer, CoverabilityGraph, ExplorationOrder, Marking, Place, ReachabilityExplorer, ReachabilityGraph, Transition};
use std::fmt;
use std::marker::PhantomData;

/// Internal representation of a Petri net system with dense indexing for efficient state-space exploration.
#[derive(Debug, Clone)]
pub(crate) struct DensePetriNet<N: AsRef<Net>> {
    pub(crate) net: N,
    pub(crate) initial_marking: IdxMarking<u32>,
    pub(crate) current_marking: IdxMarking<u32>,
}

impl<N: AsRef<Net>> DensePetriNet<N> {
    pub(crate) fn into_parts(self) -> (N, IdxMarking<u32>, IdxMarking<u32>) {
        (self.net, self.initial_marking, self.current_marking)
    }

    pub(crate) fn net(&self) -> &Net {
        self.net.as_ref()
    }

    /// Resets the current marking to the initial marking.
    /// Returns the marking before the reset.
    pub(crate) fn reset(&mut self) -> IdxMarking {
        std::mem::replace(
            &mut self.current_marking,
            self.initial_marking.clone()
        )
    }

    /// Dense-index firing for internal use by the state-space explorer.
    pub(crate) fn is_enabled(&self, t: TransitionIdx) -> bool {
        self.net().core.is_enabled_in(t, &self.current_marking)
    }

    /// Returns the set of currently enabled transitions.
    pub(crate) fn enabled_transitions(&self) -> impl Iterator<Item = TransitionIdx> {
        self.net().core
            .transition_indices()
            .filter(|&t| self.is_enabled(t))
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub(crate) fn is_deadlocked(&self) -> bool {
        self.enabled_transitions().next().is_none()
    }

    /// Check-and-fire a specific transition.
    pub fn try_fire(&mut self, t: TransitionIdx) -> Result<(), ()> {
        if self.is_enabled(t) {
            self.fire_unchecked(t);
            Ok(())
        } else {
            Err(())
        }
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled.
    /// Token underflow will panic in debug mode and wrap in release mode.
    pub(crate) fn fire_unchecked(&mut self, t_idx: TransitionIdx) {
        for &p in &self.net.as_ref().core.preset_t[t_idx] {
            self.current_marking[p] -= 1;
        }
        for &p in &self.net.as_ref().core.postset_t[t_idx] {
            self.current_marking[p] += 1;
        }
    }
}

/// A Petri net system `(N, M₀)`,
/// where `N` is the structure of the net and `M₀` is the initial marking.
///
/// Additionally, the system tracks the current marking, which can be mutated by firing transitions:
/// see [`try_fire`](Self::try_fire), [`choose_and_fire`](Self::choose_and_fire), and [`fire_any`](Self::fire_any).
/// The current marking can be reset to the initial marking via [`reset`](Self::reset).
///
/// **Note:** Analysis methods will use the **current** marking as the starting point.
#[derive(Debug, Clone)]
pub struct PetriNet<N: AsRef<Net> = Net> {
    pub(crate) core: DensePetriNet<N>,
}

#[cfg(feature = "pnml")]
mod pnml {
    use crate::pnml::convert::PnmlConversionError;
    use crate::pnml::PnmlDocument;
    use crate::{Net, PetriNet};
    use std::error::Error;
    use std::fmt;
    use std::fmt::{Display, Formatter};

    #[derive(Debug, Clone)]
    pub enum FromPnmlError {
        Syntax(quick_xml::DeError),
        Empty,
        Conversion(PnmlConversionError),
    }

    impl Display for FromPnmlError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                FromPnmlError::Syntax(e) => write!(f, "PNML syntax error: {e}"),
                FromPnmlError::Empty => write!(f, "PNML document contains no nets"),
                FromPnmlError::Conversion(e) => write!(f, "PNML conversion error: {e}"),
            }
        }
    }

    impl Error for FromPnmlError {}

    impl PetriNet<Net> {
        /// Parses the first Petri Net (including initial marking) out of a PNML document.
        /// Accepts the PNML content as a string slice.
        ///
        /// # Errors
        ///
        /// Returns an error if the XML failed to parse, if there were no nets in the file,
        /// or if the first petri net in the file is not a PT net, as specified by `net_type`.
        pub fn from_pnml(pnml: &str) -> Result<Self, FromPnmlError> {
            PnmlDocument::from_xml(pnml).map_err(FromPnmlError::Syntax)
                .and_then(|doc| doc.nets.into_iter().next().ok_or(FromPnmlError::Empty))
                .and_then(|net| net.to_pt_system().map_err(FromPnmlError::Conversion))
        }
    }

    impl Net {
        /// Parses the first Net (not including initial marking) out of a PNML document.
        /// Accepts the PNML content as a string slice.
        ///
        /// # Errors
        ///
        /// Returns an error if the XML failed to parse, if there were no nets in the file,
        /// or if the first net in the file is not a PT net, as specified by `net_type`.
        ///
        /// TODO: Allow parsing just the net structure out of any type of PNML
        pub fn from_pnml(pnml: &str) -> Result<Self, FromPnmlError> {
            PetriNet::from_pnml(pnml).map(|system| system.into_parts().0)
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Creates a new Petri net from a net and initial marking.
    #[must_use]
    pub fn new(net: N, initial_marking: impl Into<Marking>) -> Self {
        let initial_marking = initial_marking.into();
        let initial_marking = net.as_ref().to_idx_marking(initial_marking);
        let current_marking = initial_marking.clone();
        Self { core: DensePetriNet { net, initial_marking, current_marking } }
    }

    /// Returns a reference to the underlying net.
    pub fn net(&self) -> &Net {
        self.core.net.as_ref()
    }

    /// Returns the current marking of the system.
    #[must_use]
    pub fn current_marking(&self) -> Marking {
        let current_marking = self.core.current_marking.clone();
        self.core.net().to_marking(current_marking)
    }

    /// Returns the initial marking of the system.
    pub fn initial_marking(&self) -> Marking {
        let initial_marking = self.core.initial_marking.clone();
        self.core.net().to_marking(initial_marking)
    }

    /// Resets the current marking to the initial marking.
    /// Returns the marking before the reset.
    pub fn reset(&mut self) -> Marking {
        let previous = self.core.reset();
        self.core.net().to_marking(previous)
    }

    /// Returns the token count at a place identified by its [`Place`].
    /// Returns 0 for places which do not exist in the net.
    #[must_use]
    pub fn current_tokens(&self, p: Place) -> u32 {
        self.core.net()
            .place_index(p)
            .map_or(0, |&p_idx| self.core.current_marking[p_idx])
    }

    /// Consumes the system and returns (`net`, `initial_marking`, `current_marking`).
    #[must_use]
    pub fn into_parts(self) -> (N, Marking, Marking) {
        let (net, initial_marking, current_marking) = self.core.into_parts();
        let initial_marking = net.as_ref().to_marking(initial_marking);
        let current_marking = net.as_ref().to_marking(current_marking);
        (net, initial_marking, current_marking)
    }
    
    /// Returns true if the underlying net is a [circuit](crate::net::class::NetClass::Circuit).
    pub fn is_circuit(&self) -> bool {
        self.core.net().is_circuit()
    }

    /// Returns true if the underlying net is a [state machine](crate::net::class::NetClass::StateMachine).
    pub fn is_state_machine(&self) -> bool {
        self.core.net().is_state_machine()
    }
    
    /// Returns true if the underlying net is a [marked graph](crate::net::class::NetClass::MarkedGraph).
    pub fn is_marked_graph(&self) -> bool {
        self.core.net().is_marked_graph()
    }
    
    /// Returns true if the underlying net is a [free-choice net](crate::net::class::NetClass::FreeChoice).
    pub fn is_free_choice_system(&self) -> bool {
        self.core.net().is_free_choice_net()
    }
    
    /// Returns true if the underlying net is an [asymmetric-choice net](crate::net::class::NetClass::AsymmetricChoice).
    pub fn is_asymmetric_choice_system(&self) -> bool {
        self.core.net().is_asymmetric_choice_net()
    }

    /// Returns a reachability explorer for this system, using the specified exploration order.
    pub fn explore_reachability(&self, order: ExplorationOrder) -> ReachabilityExplorer<'_> {
        ReachabilityExplorer::new(self, order)
    }

    /// Returns a coverability explorer for this system, using the specified exploration order.
    pub fn explore_coverability(&self, order: ExplorationOrder) -> CoverabilityExplorer<'_> {
        CoverabilityExplorer::new(self, order)
    }

    /// Returns the complete coverability graph for this system.
    pub fn build_coverability_graph(&self) -> CoverabilityGraph<'_> {
        CoverabilityGraph::new(self)
    }

    pub fn build_reachability_graph(&self) -> ReachabilityGraph<'_> {
        ReachabilityGraph::build(self)
    }

    /// Drive Karp–Miller exploration, returning a reachability graph if the
    /// system is bounded, or a partially-explored coverability explorer as
    /// soon as the first ω is introduced.
    ///
    /// This is the right entry point when you want the speed of exploring
    /// the reachability graph directly but cannot rule out unboundedness
    /// upfront. For unbounded nets you avoid the cost of completing the
    /// full coverability graph; for bounded nets the cost is identical to
    /// `build_reachability_graph` (no ω is ever introduced, no extra work).
    ///
    /// # Errors
    /// Returns `Err(partial_explorer)` as soon as any explored marking
    /// contains ω. The frontier is preserved, so callers may resume.
    #[allow(clippy::result_large_err)]
    pub fn build_reachability_or_coverability(&self) -> Result<ReachabilityGraph<'_>, CoverabilityExplorer<'_>> {
        CoverabilityExplorer::new(self, ExplorationOrder::BreadthFirst)
            .build_reachability_or_coverability()
    }

    /// Whether some reachable marking puts more than one token in any place
    /// (a FALSE witness for the MCC `OneSafe` examination), short-circuiting
    /// on the first witness.
    ///
    /// **Termination is the caller's responsibility.** On unbounded nets
    /// this does not terminate; rule out unboundedness first via
    /// [`Net::is_structurally_bounded`] or
    /// [`Self::build_reachability_or_coverability`].
    ///
    /// [`Net::is_structurally_bounded`]: crate::Net::is_structurally_bounded
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .any_marking_satisfies(|m| m.iter().any(|&t| t > 1))
    }

    /// Returns true if state space enumeration encounters any deadlock marking.
    pub fn has_reachable_deadlock_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .any_marking_satisfies(|m| self.net().core.is_deadlock(m))
    }

    /// True iff structural analysis alone proves the net 1-safe under the
    /// initial marking. Uses [`find_positive_place_subvariant`] to derive
    /// per-place upper bounds in polynomial time; if every place is bounded
    /// by 1, the answer is TRUE without any state-space exploration.
    /// Returns `false` when the bound is loose or the LP is infeasible —
    /// it is a one-sided check, not a decision procedure.
    ///
    /// [`find_positive_place_subvariant`]: crate::analysis::semi_decision::find_positive_place_subvariant
    pub fn is_structurally_one_safe(&self) -> bool {
        use crate::analysis::semi_decision::find_positive_place_subvariant;
        let net = self.core.net();
        let Some(weights) = find_positive_place_subvariant(&net.core) else {
            return false;
        };
        let weighted_sum: f64 = weights.iter()
            .zip(self.core.initial_marking.iter())
            .map(|(&w, &m)| w * f64::from(m))
            .sum();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        weights.iter().all(|&w| (weighted_sum / w).floor() as u32 <= 1)
    }

    /// Whether a transition is enabled under the current marking.
    ///
    /// A transition t is enabled if every input place p in its preset has
    /// at least one token.
    #[must_use]
    pub fn is_enabled(&self, t: Transition) -> bool {
        self.net().transition_index(t).is_some_and(|&idx| self.core.is_enabled(idx))
    }

    /// Returns the set of currently enabled transitions.
    ///
    /// This is a read-only query. To fire one of these, use [`try_fire`](Self::try_fire)
    /// or [`choose_and_fire`](Self::choose_and_fire).
    pub fn enabled_transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.core.enabled_transitions().map(|idx| self.net().ordered_transitions[idx])
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub fn is_deadlocked(&self) -> bool {
        self.core.is_deadlocked()
    }

    /// Check-and-fire a specific transition.
    ///
    /// Returns `Ok(())` if the transition was enabled and has been fired.
    /// # Errors
    /// Returns `Err(NotEnabled)` if it was not enabled.
    pub fn try_fire(&mut self, t: Transition) -> Result<(), NotEnabled> {
        self.core.net.as_ref()
            .transition_index(t)
            .copied()
            .ok_or(())
            .and_then(|t_idx| self.core.try_fire(t_idx))
            .map_err(|()| NotEnabled(t))
    }

    /// Fire any single enabled transition.
    ///
    /// Returns the transition that was fired, or `None` if no transition is
    /// enabled (deadlock).
    pub fn fire_any(&mut self) -> Option<Transition> {
        let net = self.net();
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
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::net::system::PetriNet;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// let mut sys = PetriNet::new(net, [1, 0]);
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
    pub fn choose_and_fire<F>(&mut self, choose: F) -> Option<Transition>
    where
        F: for<'a> FnOnce(EnabledSet<'a>) -> Option<EnabledTransition<'a>>,
    {
        let enabled = self.enabled_transitions().collect();
        let set = EnabledSet(enabled, PhantomData);
        let chosen = choose(set)?.0;
        self.fire_unchecked(chosen);
        Some(chosen)
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled. Underflow will
    /// panic in debug mode and wrap in release mode.
    pub fn fire_unchecked(&mut self, t: Transition) {
        if let Some(&idx) = self.net().transition_index(t) {
            for &p in &self.core.net.as_ref().core.preset_t[idx] {
                self.core.current_marking[p] -= 1;
            }
            for &p in &self.core.net.as_ref().core.postset_t[idx] {
                self.core.current_marking[p] += 1;
            }
        }
    }
}

/// Proof that a transition was found enabled in the current marking.
///
/// Cannot be constructed outside this module (private fields), cannot be
/// copied or cloned, and cannot escape the [`choose_and_fire`](PetriNet::choose_and_fire)
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
        write!(f, "EnabledTransition({:?})", self.0)
    }
}

/// The set of transitions enabled in a specific marking.
///
/// Only exists inside the [`choose_and_fire`](PetriNet::choose_and_fire) closure.
pub struct EnabledSet<'a>(Box<[Transition]>, PhantomData<&'a ()>);

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
pub struct NotEnabled(Transition);

impl fmt::Display for NotEnabled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotEnabled(_) => write!(f, "transition is not enabled"),
        }
    }
}

impl std::error::Error for NotEnabled {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::builder::NetBuilder;
    use crate::net::Transition;

    /// Builds a simple two-place cycle: p0 -> t0 -> p1 -> t1 -> p0
    fn two_place_cycle() -> (Net, Place, Transition, Place, Transition) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().expect("valid net");
        (net, p0, t0, p1, t1)
    }

    #[test]
    fn basic_firing() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        assert_eq!(sys.current_marking(), [(p0, 1)].into());
        assert!(sys.is_enabled(t0));
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.current_marking(), [(p1, 1)].into());
    }

    #[test]
    fn try_fire_not_enabled() {
        let (net, p0, _t0, _p1, t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.try_fire(t1).is_err());
    }

    #[test]
    fn fire_any_deadlock() {
        let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([]);
        assert!(sys.is_deadlocked());
        assert!(sys.fire_any().is_none());
    }

    #[test]
    fn choose_and_fire_first() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        let fired = sys.choose_and_fire(|enabled| enabled.first());
        assert_eq!(fired, Some(t0));
        assert_eq!(sys.current_marking(), [(p1, 1)].into());
    }

    #[test]
    fn choose_and_fire_specific() {
        let (net, p0, _t0, p1, t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p1, 1)]);
        let fired = sys.choose_and_fire(|enabled| {
            enabled.iter().find(|et| *et == t1)
        });
        assert_eq!(fired, Some(t1));
        assert_eq!(sys.current_marking(), [(p0, 1)].into());
    }

    #[test]
    fn choose_and_fire_none_enabled() {
        let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([]);
        let fired = sys.choose_and_fire(|enabled| enabled.first());
        assert_eq!(fired, None);
    }

    #[test]
    fn choose_and_fire_user_declines() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        let fired = sys.choose_and_fire(|_enabled| None);
        assert_eq!(fired, None);
        assert_eq!(sys.current_marking(), [(p0, 1)].into());
    }

    #[test]
    fn enabled_transitions_query() {
        let (net, p0, t0, p1, t1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        let enabled = sys.enabled_transitions().collect::<Box<_>>();
        assert!(enabled.contains(&t0));
        assert!(enabled.contains(&t1));
    }

    #[test]
    fn into_parts() {
        let (net, p0, t0, p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        sys.try_fire(t0).unwrap();
        let (_, _, current) = sys.into_parts();
        assert_eq!(current.as_ref(), &[(p1, 1)]);
    }

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_bounded());
    }

    #[test]
    fn cycle_is_live() {
        let (net, p0, _t0, _p1, _t1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_live());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
        let sys = net.with_initial_marking([]);
        assert!(!sys.is_live());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, _p0, t0, _p1, t1) = two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = net.with_initial_marking([]);
        let liveness = sys.analyze_liveness();
        assert!(liveness.transition_level(t0).is_some_and(|l| l.is_dead()));
        assert!(liveness.transition_level(t1).is_some_and(|l| l.is_dead()));
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, p0, t0, _p1, t1) = two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        let liveness = sys.analyze_liveness();
        assert!(liveness.transition_level(t0).is_some_and(|l| !l.is_dead()));
        assert!(liveness.transition_level(t1).is_some_and(|l| !l.is_dead()));
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
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(!sys.is_bounded());
    }

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, p0, _t0, p1, _t1)= two_place_cycle();
        assert!(net.is_state_machine());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_reachable([(p1, 1)].into()));
        assert!(sys.is_reachable([(p0, 1)].into()));
        assert!(!sys.is_reachable([(p0, 2)].into()));
        assert!(!sys.is_reachable([].into()));
    }

    #[test]
    fn t_net_reachability_dispatches() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p1, t0));
        b.add_arcs((t0, p2, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p1));
        let net = b.build().unwrap();
        assert!(net.is_marked_graph());
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        assert!(sys.is_reachable([(p2, 1)].into()));
        assert!(sys.is_reachable([(p0, 1), (p1, 1)].into()));
        assert!(!sys.is_reachable([(p1, 1)].into()));
    }

    #[test]
    fn general_net_reachability_fallback() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arcs((p0, t0, p1));
        b.add_arcs((p0, t1, p2));
        b.add_arcs((p1, t2, p0));
        b.add_arcs((p2, t2, p0));
        let net = b.build().unwrap();
        assert!(!net.is_state_machine());
        assert!(!net.is_marked_graph());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_reachable([(p0, 1)].into()));
        assert!(sys.is_reachable([(p1, 1)].into()));
    }

    #[test]
    fn mutex_is_live_and_bounded() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arcs((idle1, t_req1, wait1));
        b.add_arcs((wait1, t_enter1, crit1));
        b.add_arcs((crit1, t_exit1, idle1));
        b.add_arcs((idle2, t_req2, wait2));
        b.add_arcs((wait2, t_enter2, crit2));
        b.add_arcs((crit2, t_exit2, idle2));
        b.add_arcs((mutex, t_enter1, mutex));
        b.add_arcs((mutex, t_enter2, mutex));

        let net = b.build().expect("valid net");
        let sys = PetriNet::new(net, [(idle1, 1), (idle2, 1), (mutex, 1)]);
        assert!(sys.is_bounded());
        assert!(sys.is_live());
    }
}
