use crate::core::cegar::refinements::p_invariant::IdxPInvariant;
use crate::core::net::idx_set::{PlaceIdxSet, TransitionIdxSet};
use crate::core::net::{PlaceIdx, TransitionIdx};
use crate::core::siphon_trap::IdxTrap;

/// A single, independently-verifiable fact about the net that was used as a hypothesis in
/// deriving unsatisfiability of a reachability or coverability problem.
///
/// Each variant states a claim precise enough that a reader who knows the problem can verify
/// it directly from net structure, without any dependency on this library's SMT encoding or search.
/// Where a fact isn't checkable from static structure alone (`Increment`, which depends on a 
/// specific attempted simulation), the variant carries enough data - a replayable firing sequence
/// - to make the check mechanical anyway.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdxLemma {
    /// An invariant: every reachable marking must satisfy `w·M = value`.
    PInvariant(IdxPInvariant),
    /// An invariant: the sum of tokens in the trap is always positive, because it was positive
    /// in m0 and can never be emptied.
    InitiallyMarkedTrap(IdxTrap),
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
    /// Verify: replay `firing_sequence` from `m0` to confirm it's valid and to recompute the
    /// dead-end marking, then recompute the bottleneck graph and token estimate per the
    /// documented algorithm (see `crate::core::cegar::refinements::explore`).
    Increment {
        component_places: PlaceIdxSet,
        component_transitions: TransitionIdxSet,
        firing_sequence: Vec<TransitionIdx>,
    },
}