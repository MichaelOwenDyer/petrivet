//! Reachability analysis for Petri net systems.
//!
//! This module provides reachability analysis with specialization for structural subclasses.
//! Reachability determines whether a target marking can be reached from the initial marking.
//!
//! # Complexity by Structural Class
//!
//! | Net Class          | Reachability Decision | Complexity                     |
//! |--------------------|----------------------|--------------------------------|
//! | Circuit            | Trivial              | O(|S|)                         |
//! | S-net (live)       | Trivial              | O(|S|)                         |
//! | T-net (live)       | Easy                 | Linear equation solving        |
//! | Free-choice (live) | Polynomial           | O(poly) - decidable!           |
//! | General            | Ackermann-complete   | Non-elementary complexity      |
//!
//! # Key Theorems
//!
//! ## Circuit
//! M is reachable from M₀ iff M(S) = M₀(S).
//!
//! ## S-net (live)
//! M is reachable from M₀ iff M(S) = M₀(S).
//!
//! ## T-net (live)
//! M is reachable from M₀ iff M₀ ∼ M (same linear invariants).
//!
//! ## Free-choice (live, bounded, cyclic)
//! M is reachable from M₀ iff:
//! 1. ∃X ∈ ℕ^|T| such that M = M₀ + N·X
//! 2. (N_U, M_U) has no unmarked traps, where U = {t | X(t) = 0}

use crate::analysis::System;
use crate::behavior::Marking;
use crate::structure::class::{Circuit, FreeChoiceNet, SNet, TNet};
use crate::structure::{Net, Transition};

/// Result of a reachability query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachabilityResult {
    /// The target marking is reachable via the given firing sequence.
    Reachable(Box<[Transition]>),
    /// The target marking is definitely not reachable.
    Unreachable,
    /// Reachability could not be determined.
    Unknown,
}

impl ReachabilityResult {
    /// Returns true if the target is known to be reachable.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        matches!(self, ReachabilityResult::Reachable(_))
    }

    /// Returns the firing sequence if reachable.
    #[must_use]
    pub fn firing_sequence(&self) -> Option<&[Transition]> {
        match self {
            ReachabilityResult::Reachable(seq) => Some(seq),
            _ => None,
        }
    }
}

/// Trait for reachability analysis on Petri net systems.
///
/// This trait provides methods for checking if a target marking is reachable
/// from the initial marking, and for finding firing sequences.
pub trait ReachabilityAnalysis {
    /// Checks if the target marking is reachable from the initial marking.
    ///
    /// Returns `Reachable(sequence)` with a witness firing sequence if reachable,
    /// `Unreachable` if definitely not reachable, or `Unknown` if undetermined.
    fn is_reachable(&self, target: &Marking) -> ReachabilityResult;

    /// Checks if the target marking is coverable from the initial marking.
    ///
    /// A marking M is coverable if there exists a reachable marking M' such that M' ≥ M.
    /// Coverability is decidable (EXPSPACE-complete) even when reachability is not.
    fn is_coverable(&self, target: &Marking) -> ReachabilityResult;
}

// =============================================================================
// General Net Implementation (Fallback)
// =============================================================================

impl ReachabilityAnalysis for System<Net> {
    fn is_reachable(&self, _target: &Marking) -> ReachabilityResult {
        // General reachability is Ackermann-complete (non-elementary complexity)
        // Requires Leroux's algorithm or similar advanced techniques
        ReachabilityResult::Unknown
    }

    fn is_coverable(&self, _target: &Marking) -> ReachabilityResult {
        // Coverability is decidable (EXPSPACE-complete)
        // Uses the Karp-Miller algorithm
        ReachabilityResult::Unknown
    }
}

// =============================================================================
// Circuit Implementation
// =============================================================================

impl ReachabilityAnalysis for System<Circuit> {
    /// M is reachable from M₀ in a circuit iff M(S) = M₀(S).
    ///
    /// This is O(|S|) - just check if token counts match.
    fn is_reachable(&self, target: &Marking) -> ReachabilityResult {
        let initial_sum: i32 = self.initial_marking().iter().map(|t| t.0).sum();
        let target_sum: i32 = target.iter().map(|t| t.0).sum();

        if initial_sum == target_sum {
            // Tokens match, so reachable. Finding the actual sequence is more work.
            // For a circuit, we can always find a sequence if the sums match.
            ReachabilityResult::Reachable(Box::new([]))  // TODO: compute actual sequence
        } else {
            ReachabilityResult::Unreachable
        }
    }

    fn is_coverable(&self, target: &Marking) -> ReachabilityResult {
        // In a circuit, coverability is more restrictive due to token conservation
        let initial_sum: i32 = self.initial_marking().iter().map(|t| t.0).sum();
        let target_sum: i32 = target.iter().map(|t| t.0).sum();

        if target_sum <= initial_sum {
            // We can cover any marking with fewer or equal total tokens
            // (since tokens can flow to any place in a circuit)
            ReachabilityResult::Reachable(Box::new([]))
        } else {
            ReachabilityResult::Unreachable
        }
    }
}

// =============================================================================
// S-net Implementation
// =============================================================================

impl ReachabilityAnalysis for System<SNet> {
    /// Let (N, M₀) be a live S-system. M is reachable from M₀ iff M(S) = M₀(S).
    ///
    /// This is O(|S|) - just check if total token counts match.
    fn is_reachable(&self, target: &Marking) -> ReachabilityResult {
        let initial_sum: i32 = self.initial_marking().iter().map(|t| t.0).sum();
        let target_sum: i32 = target.iter().map(|t| t.0).sum();

        if initial_sum == target_sum {
            // Check if the net is strongly connected (required for liveness)
            let net = self.net.as_ref();
            if net.is_strongly_connected() && initial_sum > 0 {
                ReachabilityResult::Reachable(Box::new([]))  // TODO: compute actual sequence
            } else {
                // Net is not live, can't use the simple theorem
                ReachabilityResult::Unknown
            }
        } else {
            ReachabilityResult::Unreachable
        }
    }

    fn is_coverable(&self, target: &Marking) -> ReachabilityResult {
        let initial_sum: i32 = self.initial_marking().iter().map(|t| t.0).sum();
        let target_sum: i32 = target.iter().map(|t| t.0).sum();

        if target_sum <= initial_sum {
            ReachabilityResult::Reachable(Box::new([]))
        } else {
            ReachabilityResult::Unreachable
        }
    }
}

// =============================================================================
// T-net Implementation
// =============================================================================

impl ReachabilityAnalysis for System<TNet> {
    /// Let (N, M₀) be a live T-system. M is reachable from M₀ iff M₀ ∼ M.
    ///
    /// This means we can decide reachability by solving a system of linear equations!
    /// Specifically, M is reachable iff there exists X ∈ ℕ^|T| such that M = M₀ + N·X.
    fn is_reachable(&self, _target: &Marking) -> ReachabilityResult {
        // TODO: Solve the marking equation M = M₀ + N·X over integers
        // This requires checking if the target marking is in the same equivalence class
        ReachabilityResult::Unknown
    }

    fn is_coverable(&self, _target: &Marking) -> ReachabilityResult {
        // For T-nets, coverability can also use structural analysis
        ReachabilityResult::Unknown
    }
}

// =============================================================================
// Free-Choice Net Implementation
// =============================================================================

impl ReachabilityAnalysis for System<FreeChoiceNet> {
    /// For a live, bounded, cyclic free-choice system (N, M₀):
    /// M is reachable iff:
    /// 1. ∃X ∈ ℕ^|T| such that M = M₀ + N·X
    /// 2. (N_U, M_U) has no unmarked traps, where U = {t | X(t) = 0}
    ///
    /// This is decidable in POLYNOMIAL TIME!
    fn is_reachable(&self, _target: &Marking) -> ReachabilityResult {
        // TODO: Implement the polynomial-time algorithm
        // 1. Solve the marking equation to find candidate X vectors
        // 2. For each solution, check the trap condition
        ReachabilityResult::Unknown
    }

    fn is_coverable(&self, _target: &Marking) -> ReachabilityResult {
        // Free-choice coverability can use similar techniques
        ReachabilityResult::Unknown
    }
}

// =============================================================================
// Firing Sequence Length Bounds
// =============================================================================

/// Trait for computing upper bounds on firing sequence lengths.
///
/// These bounds are useful for search algorithms and completeness guarantees.
pub trait FiringSequenceBounds {
    /// Returns an upper bound on the length of the shortest firing sequence
    /// to reach any reachable marking.
    ///
    /// This is useful for bounded model checking: if we haven't found a path
    /// within this bound, we know it doesn't exist.
    fn max_sequence_length(&self) -> Option<usize>;
}

impl FiringSequenceBounds for System<FreeChoiceNet> {
    /// Shortest sequence theorem:
    /// Let (N, M₀) be a b-bounded free-choice system and let M be a reachable marking.
    /// Then there is a firing sequence M₀ →^σ M such that |σ| ≤ bn(n+1)(n+2)/6,
    /// where n = |T| is the number of transitions.
    fn max_sequence_length(&self) -> Option<usize> {
        // TODO: Compute bound from boundedness analysis
        // We need to know b (the bound) first
        None
    }
}

impl FiringSequenceBounds for System<TNet> {
    /// For a b-bounded T-system:
    /// Any reachable marking can be reached in at most b·n(n-1)/2 steps.
    fn max_sequence_length(&self) -> Option<usize> {
        // TODO: Compute bound from boundedness analysis
        None
    }
}

// =============================================================================
// Blanket Implementation for Reference Types
// =============================================================================

impl<'a, N> ReachabilityAnalysis for &'a System<N>
where
    System<N>: ReachabilityAnalysis,
{
    fn is_reachable(&self, target: &Marking) -> ReachabilityResult {
        (*self).is_reachable(target)
    }

    fn is_coverable(&self, target: &Marking) -> ReachabilityResult {
        (*self).is_coverable(target)
    }
}
