use crate::core::cegar::refinements::explore::IncrementRefinement;
use crate::core::cegar::refinements::initially_marked_trap::InitiallyMarkedTrapRefinement;
use crate::core::cegar::refinements::p_invariant::PInvariantRefinement;
use crate::core::cegar::refinements::trap_becomes_marked::TrapBecomesMarkedRefinement;

pub mod initially_marked_trap;
pub mod transition_ordering;
pub mod p_invariant;
pub mod trap_becomes_marked;
pub mod marking_equation;
pub mod explore;

/// Describes an entire CEGAR refinement *rule firing*: one step of the search found a spurious
/// candidate, diagnosed why, and encoded a reason into the solver. Used to narrate the search as
/// it runs (see [`crate::core::cegar::observe::CegarEvent`]).
///
/// Contrast with [`IdxLemma`], which describes one individually-tracked SMT assertion - a single
/// rule firing can assert several lemmas (e.g. `TrapBecomesMarkedRefinement::encode_into` asserts
/// one `TrapBecomesMarked` lemma per feeder of the trap it found; `IncrementRefinement::encode_into`
/// asserts one `Increment` lemma per bottleneck component it finds).
#[derive(Debug, Clone)]
pub enum IdxRefinement {
    /// A P-Invariant and its value in the initial marking, which must
    /// remain constant in all reachable markings.
    PInvariant(PInvariantRefinement),
    /// A trap that is marked in the initial marking, and therefore must
    /// remain marked in all reachable markings.
    InitiallyMarkedTrap(InitiallyMarkedTrapRefinement),
    /// Enforces that a trap within the subnet formed by the transitions in the
    /// candidate Parikh vector must be marked in the candidate marking, since
    /// firing those transitions would necessarily mark the trap if it were unmarked.
    TrapBecomesMarked(TrapBecomesMarkedRefinement),
    /// A Wimmel & Wolf (2011) increment constraint, diagnosing a dead end reached while trying
    /// to realize a candidate Parikh vector as a real firing sequence.
    Increment(IncrementRefinement),
}
