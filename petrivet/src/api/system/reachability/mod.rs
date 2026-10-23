use crate::net::p_invariant::PInvariant;
use crate::net::siphon_trap::{Siphon, Trap};
use crate::net::{Place, Transition};
use crate::prelude::Marking;
use crate::system::parikh_vector::ParikhVector;
use ahash::HashSet;

#[expect(clippy::module_inception)]
mod reachability;
mod coverability;

pub use reachability::ReachabilityResult;
pub use coverability::CoverabilityResult;

/// Which procedures a reachability or coverability question may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachabilityEffort {
    /// Only efficient (polynomial-time) procedures: the answer is `None` if none applies.
    Efficient,
    /// Use any available procedure, falling back to the (potentially expensive) CEGAR analysis.
    Any,
}

/// A lemma is a "truth" about a Petri net.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lemma {
    /// A P-Invariant (weighted sum of places) that must hold in all reachable markings.
    PInvariant(PInvariant),
    /// A Trap that is marked in the initial marking, and therefore must
    /// remain marked in all reachable markings.
    InitiallyMarkedTrap(Trap),
    /// A Siphon that is unmarked in the initial marking, and therefore must
    /// remain unmarked in all reachable markings.
    InitiallyUnmarkedSiphon(Siphon),
    /// A component of the marking equation which any reachable marking must satisfy.
    MarkingEquation {
        /// A place in the net.
        place: Place,
        /// The number of tokens in `place` in the initial marking.
        initial_marking: u32,
        /// The set of transitions that can change the marking of `place`,
        /// along with the net effect on `place` when they fire.
        net_effects: HashSet<(Transition, i16)>,
    },
    /// If `feeder` ever fires, `trap` must become (and stay) marked.
    TrapBecomesMarked {
        /// The transition that feeds tokens into the trap.
        feeder: Transition,
        /// The trap that must become marked if `feeder` fires.
        trap: Trap,
    },
    /// If `transition` fires, some producer of `place` must fire *before* it (or, if `place` has
    /// no producer at all, `transition` can never fire).
    TransitionOrdering {
        /// This transition can only fire if some producer of `place` has fired before it.
        transition: Transition,
        /// The place which is not sufficiently marked for `transition` in the initial marking.
        place: Place,
        /// The set of transitions that produce tokens into `place`.
        /// At least one of these must fire before `transition` can fire.
        feeders: HashSet<Transition>,
    },
    /// A Wimmel & Wolf (2011) increment constraint, diagnosing a dead end reached while trying
    /// to realize a candidate Parikh vector as a real firing sequence.
    Increment {
        component_places: HashSet<Place>,
        component_transitions: HashSet<Transition>,
        firing_sequence: Vec<Transition>,
    },
}

/// A `SpuriousSolutionEliminatedEvent` is emitted by the CEGAR analysis engine whenever a
/// lemma is derived to eliminate spurious solutions from the search space.
#[derive(Debug, Clone)]
pub struct SpuriousSolutionEliminatedEvent {
    /// The marking which triggered the derivation of a lemma to eliminate spurious solutions
    /// from the search space, if any.
    pub spurious_marking: Option<Marking<u32>>,
    /// The Parikh vector which triggered the derivation of a lemma to eliminate spurious solutions
    /// from the search space, if any.
    pub spurious_parikh_vector: Option<ParikhVector<u32>>,
    /// The lemma which was derived to eliminate spurious solutions from the search space.
    pub lemma: Lemma,
}