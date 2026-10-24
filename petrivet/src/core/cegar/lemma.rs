use crate::core::cegar::refinements::p_invariant::IdxPInvariant;
use crate::core::net::idx_set::{PlaceIdxSet, TransitionIdxSet};
use crate::core::net::siphon_trap::{IdxSiphon, IdxTrap};
use crate::core::net::{PlaceIdx, TransitionIdx};

/// A constraint that was part of the SMT problem when the solver proved it unsatisfiable.
///
/// Every variant except `Increment` is a fact about the net which holds in all reachable markings,
/// and states a claim precise enough that a reader who knows the problem can verify it directly
/// from net structure, without any dependency on this library's SMT encoding or search.
/// `Increment` is not such a fact: it is a search heuristic that steers the solver towards a
/// realizable firing count vector, and it may exclude realizable solutions. It records the dead end
/// it was derived from (a replayable firing sequence), but no proof of unreachability may rest on it.
/// A refutation whose core contains an `Increment` is therefore never reported as such.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdxLemma {
    /// An invariant: every reachable marking must satisfy `w·M = value`.
    PInvariant(IdxPInvariant),
    /// An invariant: the sum of tokens in the trap is always positive, because it was positive
    /// in m0 and can never be emptied.
    InitiallyMarkedTrap(IdxTrap),
    /// An invariant: the sum of tokens in the siphon is always zero, because it was zero
    /// in m0 and can never be filled.
    InitiallyUnmarkedSiphon(IdxSiphon),
    /// An implication: if `feeder` ever fires, `trap` must become (and stay) marked.
    TrapBecomesMarked {
        /// A trap of the net, initially unmarked.
        trap: IdxTrap,
        /// A transition that produces into the trap. If it fires, the trap must become marked.
        feeder: TransitionIdx,
    },
    /// An equation: the marking of `place` is determined by its initial marking and the firings
    /// of its neighboring transitions, per the incidence matrix:
    /// (`m(p) = m0(p) + Σ_t N(place,t)·X(t)`).
    MarkingEquation {
        /// The place for which the balance equation holds.
        place: PlaceIdx,
        /// The initial marking of the place.
        initial_marking: u32,
        /// The net effect of the neighboring transitions on the place.
        net_effects: Vec<(TransitionIdx, i16)>,
    },
    /// If `transition` fires, some producer of `place` must fire *before* it
    /// (or, if `place` has no producer at all, `transition` can never fire).
    /// This is only asserted when a *cycle* is detected, to prevent the SMT solver from proposing
    /// spurious "ouroboros" Parikh vectors which borrow non-existent tokens.
    TransitionOrdering {
        /// The transition which must wait to fire until `place` has been produced into.
        t_idx: TransitionIdx,
        /// The place which must be produced into before `transition` can fire.
        p_idx: PlaceIdx,
        /// The transitions which produce into `place`. At least one of these must fire before
        /// `transition` can fire. If this is empty, `transition` can never fire at all.
        feeders: Vec<TransitionIdx>,
    },
    /// A Wimmel & Wolf (2011) increment constraint: `component_places` and
    /// `component_transitions` form a source strongly-connected-component of the bottleneck
    /// graph induced by a failed attempt to realize a Parikh vector, so they can only receive
    /// further tokens from transitions outside the component.
    ///
    /// This is a search heuristic and not a fact about the net: it asks the solver for a vector that
    /// produces more tokens into the component than the failed one did, which is only justified
    /// above the minimal solution of the system and for the firing order of one dead end.
    /// It can exclude realizable solutions, so a core containing it is no proof of unreachability.
    /// `firing_sequence` is the replayable dead end it was derived from, for diagnosis only
    /// (see `crate::core::cegar::refinements::explore`).
    Increment {
        component_places: PlaceIdxSet,
        component_transitions: TransitionIdxSet,
        firing_sequence: Vec<TransitionIdx>,
    },
}