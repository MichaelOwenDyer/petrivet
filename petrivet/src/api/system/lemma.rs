use ahash::HashSet;
use crate::net::{Place, Transition};
use crate::net::invariant::PInvariant;
use crate::net::siphon_trap::Trap;

/// A set of lemmas that are jointly unsatisfiable, proving that the target marking is not coverable.
/// The set is not guaranteed to be minimal or irreducible, but each lemma is independently verifiable.
#[derive(Debug, Clone)]
pub struct Contradiction {
    pub lemmas: Vec<Lemma>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lemma {
    /// A P-Invariant (weighted sum of places) that the initial marking satisfies,
    /// but the SMT solver's proposed marking did not.
    PInvariant(PInvariant),
    /// A Trap that is marked in the initial marking, and therefore must
    /// remain marked in all reachable markings.
    InitiallyMarkedTrap(Trap),
    /// An equation: the marking of `place` is determined by its initial marking and the firings
    /// of its neighboring transitions, per the incidence matrix:
    MarkingEquation {
        place: Place,
        initial_marking: u32,
        net_effects: HashSet<(Transition, i16)>,
    },
    /// If `feeder` ever fires, `trap` must become (and stay) marked.
    TrapBecomesMarked {
        feeder: Transition,
        trap: Trap,
    },
    /// If `transition` fires, some producer of `place` must fire *before* it (or, if `place` has
    /// no producer at all, `transition` can never fire).
    CausalOrdering {
        transition: Transition,
        place: Place,
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