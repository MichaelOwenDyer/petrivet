//! A Petri net system: net + marking, with simulation and behavioral analysis.
//!
//! `System<N>` pairs a net structure with a mutable marking, providing methods
//! to simulate (check enablement, fire transitions) and analyze behavior
//! (boundedness, liveness, deadness).
//!
//! # Quick start
//!
//! ```
//! use petrivet::builder::NetBuilder;
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
//! # use petrivet::builder::NetBuilder;
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

pub mod boundedness;
pub mod coverability;
pub mod reachability;
pub mod deadlock_freedom;
pub mod liveness;
pub mod chc;

use crate::core::marking::IdxMarking;
use crate::core::state_space::ExplorationOrder;
use crate::prelude::{Marking, Net, Place, Transition};
use crate::state_space::{CoverabilityExplorer, CoverabilityGraph};
use crate::state_space::{ReachabilityExplorer, ReachabilityGraph};
use std::fmt;
use std::ops::Deref;

/// A Petri net system `(N, M₀)` consists of a [`Net`] `N` and an initial [`Marking`] `M₀`.
///
/// ```no_run
/// use petrivet::prelude::PetriNet;
/// let pn = PetriNet::new(&net, [(p0, 1), (p2, 5)]);
/// ```
///
/// You may simulate the behavior of the system, mutating its marking,
/// by firing [`Transitions`](Transition).
///
/// ```no_run
/// pn.try_fire(t0).ok_or(|| "not enabled!")?;
/// ```
///
/// If you want to fire any enabled transition without caring which one,
/// use [`fire_any()`](Self::fire_any).
///
/// ```no_run
/// pn.fire_any().ok_or("deadlock!")?;
/// ```
///
/// To reset the system back to the marking it was initialized with,
/// use [`reset()`](Self::reset).
#[derive(Debug, Clone)]
pub struct PetriNet<N = Net> {
    /// The [`Net`] structure, which is immutable and can be shared across multiple
    /// Petri nets depending on the choice of `N` (e.g. `&Net`, `Arc<Net>`, ...).
    pub net: N,
    /// The mutable current marking of the system, which changes as transitions are fired.
    ///
    /// The current state of this marking is used as the starting point, or "initial marking",
    /// for all analysis procedures.
    // todo: make private
    pub(crate) marking: IdxMarking<u32>,
    /// The marking which the Petri net was initialized with. This will never change.
    ///
    /// This is purposefully not called the *initial* marking because the other marking
    /// field serves as the de-facto initial marking for analysis questions, even if it
    /// was previously mutated through simulation.
    reset_marking: IdxMarking<u32>,
}

impl<N: AsRef<Net>> Deref for PetriNet<N> {
    type Target = Net;

    fn deref(&self) -> &Self::Target {
        self.net.as_ref()
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Creates a new Petri net from a net and initial marking.
    #[must_use]
    pub fn new(net: N, initial_marking: impl Into<Marking<u32>>) -> Self {
        let initial_marking = net.as_ref().mapping.decode(initial_marking.into());
        let reset_marking = initial_marking.clone();
        Self { net, marking: initial_marking, reset_marking }
    }

    /// Consumes the system and returns (`net`, `initial_marking`, `current_marking`).
    #[must_use]
    pub fn into_parts(self) -> (N, Marking<u32>, Marking<u32>) {
        let PetriNet { net, marking, reset_marking: reset } = self;
        let marking = net.as_ref().mapping.encode(marking);
        let reset = net.as_ref().mapping.encode(reset);
        (net, marking, reset)
    }

    /// Returns the current marking of the system.
    #[must_use]
    pub fn marking(&self) -> Marking<u32> {
        let current_marking = self.marking.clone();
        self.mapping.encode(current_marking)
    }

    /// Returns the token count at a place identified by its [`Place`].
    /// Returns 0 for places which do not exist in the net.
    #[must_use]
    pub fn tokens_in(&self, p: Place) -> u32 {
        self.mapping
            .place_idx(p)
            .map_or(0, |p_idx| self.marking[p_idx])
    }

    /// Resets the current marking to the initial marking.
    /// Returns the marking before the reset.
    pub fn reset(&mut self) -> Marking<u32> {
        let previous = std::mem::replace(
            &mut self.marking,
            self.reset_marking.clone()
        );
        self.mapping.encode(previous)
    }

    /// Whether a transition is enabled under the current marking.
    ///
    /// A transition t is enabled if every input place p in its preset has
    /// at least one token.
    #[must_use]
    pub fn is_enabled(&self, t: Transition) -> bool {
        self.mapping
            .transition_idx(t)
            .is_some_and(|t_idx| self.dense_net.is_enabled_in(t_idx, &self.marking))
    }

    /// Returns the set of currently enabled transitions.
    ///
    /// This is a read-only query. To fire one of these, use [`try_fire`](Self::try_fire)
    /// or [`choose_and_fire`](Self::choose_and_fire).
    pub fn enabled_transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.dense_net
            .transition_indices()
            .filter(|&t_idx| self.dense_net.is_enabled_in(t_idx, &self.marking))
            .map(|idx| self.mapping.transition(idx))
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub fn is_deadlocked(&self) -> bool {
        self.enabled_transitions().next().is_none()
    }

    /// Check-and-fire a specific transition `t`.
    ///
    /// Returns `Ok(t)` if the transition was enabled and has been fired.
    ///
    /// # Errors
    /// Returns `Err(NotEnabled(t))` if it was not enabled (i.e. if any input place had zero tokens),
    /// or if the transition does not exist in the net.
    pub fn try_fire(&mut self, t: Transition) -> Result<Transition, NotEnabled> {
        self.mapping
            .transition_idx(t)
            .ok_or(NotEnabled(t))
            .and_then(|t_idx| {
                if self.dense_net.is_enabled_in(t_idx, &self.marking) {
                    self.fire_unchecked(t);
                    Ok(t)
                } else {
                    Err(NotEnabled(t))
                }
            })
    }

    /// Fire any single enabled transition.
    ///
    /// Returns the transition that was fired, or `None` if no transition is
    /// enabled (deadlock).
    pub fn fire_any(&mut self) -> Option<Transition> {
        let t = self.transitions().find(|&t| self.is_enabled(t))?;
        self.fire_unchecked(t);
        Some(t)
    }

    /// Fires each transition of `sequence` in order, mutating the marking.
    ///
    /// This is the natural way to replay a firing-sequence witness, such as the
    /// one returned by reachability analysis, against a system.
    ///
    /// # Errors
    ///
    /// Returns `Err(NotEnabled(t))` at the first transition that is not enabled
    /// when its turn comes (or does not exist in the net). On error the marking is
    /// left at the last successful firing; transitions after the failure are not
    /// attempted.
    pub fn fire_sequence<I>(&mut self, sequence: I) -> Result<(), NotEnabled>
    where
        I: IntoIterator<Item = Transition>,
    {
        for t in sequence {
            self.try_fire(t)?;
        }
        Ok(())
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled. Underflow will
    /// panic in debug mode and wrap in release mode.
    ///
    /// # Panics
    ///
    /// Panics if the transition is not enabled (i.e. if any input place has zero tokens)
    /// or if any token count would overflow `u32::MAX`.
    pub fn fire_unchecked(&mut self, t: Transition) {
        if let Some(t_idx) = self.mapping.transition_idx(t) {
            for &p_idx in &self.net.as_ref().dense_net.preset_t[t_idx] {
                self.marking[p_idx] = self.marking[p_idx]
                    .checked_sub(1)
                    .expect("fire_unchecked: token underflow");
            }
            for &p_idx in &self.net.as_ref().dense_net.postset_t[t_idx] {
                self.marking[p_idx] = self.marking[p_idx]
                    .checked_add(1)
                    .expect("fire_unchecked: token overflow");
            }
        }
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

// state space convenience methods
impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns a [`CoverabilityExplorer`] for this system
    /// initialized with the given [`ExplorationOrder`].
    pub fn explore_coverability(&self, order: ExplorationOrder) -> CoverabilityExplorer<'_> {
        CoverabilityExplorer::new(self, order)
    }

    /// Returns the complete coverability graph for this system.
    ///
    /// Warning! This may be a HUGE structure!
    pub fn build_coverability_graph(&self) -> CoverabilityGraph<'_> {
        self.explore_coverability(ExplorationOrder::BreadthFirst).build_graph()
    }

    /// Attempt to construct a [`ReachabilityGraph`] of this [`PetriNet`],
    /// returning either itself if the system is bounded or a partially-explored
    /// [`CoverabilityExplorer`] if it is unbounded.
    ///
    /// Not knowing whether we will encounter unboundedness, this method first
    /// constructs a Karp-Miller coverability tree which introduces ω as soon
    /// as unbounded growth is detected. This comes at the cost of an additional
    /// check per explored marking (omega acceleration). If we finish exploring
    /// the coverability tree without ever introducing ω, we have in fact explored
    /// the full reachability graph and can return it directly. Otherwise, we return
    /// the [`CoverabilityExplorer`] in its current state, which contains the explored
    /// portion of the coverability graph up to the first ω, and can be further explored.
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
    pub fn try_build_reachability_graph(&self) -> Result<ReachabilityGraph<'_>, CoverabilityExplorer<'_>> {
        self.explore_coverability(ExplorationOrder::BreadthFirst).try_build_reachability_graph()
    }

    /// Returns a [`ReachabilityExplorer`] for this system
    /// initialized with the given [`ExplorationOrder`].
    pub fn explore_reachability(&self, order: ExplorationOrder) -> ReachabilityExplorer<'_> {
        ReachabilityExplorer::new(self, order)
    }

    /// Returns the complete reachability graph for this system.
    ///
    /// WARNING! For unbounded nets, this will not terminate!
    pub fn build_reachability_graph(&self) -> ReachabilityGraph<'_> {
        self.explore_reachability(ExplorationOrder::BreadthFirst).build_graph()
    }
}

#[cfg(feature = "pnml")]
mod pnml {
    use crate::pnml::convert::PnmlConversionError;
    use crate::pnml::PnmlDocument;
    use crate::prelude::{Net, PetriNet};
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

#[cfg(test)]
mod tests {
    use super::NotEnabled;
    use crate::prelude::{Marking, Net, NetBuilder, Place, Transition};

    /// Builds a simple two-place cycle: p0 -> t0 -> p1 -> t1 -> p0
    pub fn two_place_cycle() -> (Net, Place, Transition, Place, Transition) {
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
        assert_eq!(sys.marking(), [(p0, 1)].into());
        assert!(sys.is_enabled(t0));
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.marking(), [(p1, 1)].into());
    }

    #[test]
    fn try_fire_not_enabled() {
        let (net, p0, _t0, _p1, t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.try_fire(t1).is_err());
    }

    #[test]
    fn fire_sequence_replays_and_reports_first_block() {
        let (net, p0, t0, p1, t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([(p0, 1)]);
        // t0 then t1 returns to the start.
        sys.fire_sequence([t0, t1]).expect("both enabled in turn");
        assert_eq!(sys.marking(), [(p0, 1)].into());

        // t1 is not enabled at the start; the sequence stops there and the marking
        // is left untouched (nothing fired).
        let err = sys.fire_sequence([t1, t0]).expect_err("t1 not enabled first");
        assert_eq!(err, NotEnabled(t1));
        assert_eq!(sys.marking(), [(p0, 1)].into());

        // A partial sequence leaves the marking at the last successful firing.
        let err = sys.fire_sequence([t0, t0]).expect_err("t0 not enabled twice");
        assert_eq!(err, NotEnabled(t0));
        assert_eq!(sys.marking(), [(p1, 1)].into());
    }

    #[test]
    fn fire_any_deadlock() {
        let (net, _p0, _t0, _p1, _t1) = two_place_cycle();
        let mut sys = net.with_initial_marking([]);
        assert!(sys.is_deadlocked());
        assert!(sys.fire_any().is_none());
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
        let parts = sys.into_parts();
        assert_eq!(parts.1, Marking::from([(p1, 1)]));
        assert_eq!(parts.2, Marking::from([(p0, 1)]));
    }
}
