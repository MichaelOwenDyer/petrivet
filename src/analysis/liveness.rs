//! Liveness analysis for Petri net systems.
//!
//! This module provides liveness analysis with specialization for structural subclasses.
//! Liveness is a behavioral property that describes whether transitions can continue
//! to fire during system execution.
//!
//! # Liveness Levels
//!
//! - **L0 (Dead)**: The transition never fires.
//! - **L1 (Potentially fireable)**: The transition can fire at least once.
//! - **L2 (Quasi-live)**: For any reachable marking, the transition can fire again.
//! - **L3 (Weakly live)**: The transition can fire infinitely often in some execution.
//! - **L4 (Live/Strictly live)**: From any reachable marking, the transition can eventually fire.
//!
//! A system is called "live" if all its transitions are L4-live.
//!
//! # Complexity by Structural Class
//!
//! | Net Class       | Liveness Decision | Complexity                |
//! |-----------------|-------------------|---------------------------|
//! | Circuit         | Trivial           | O(|S|)                    |
//! | S-net           | Easy              | O(|S|) if connected       |
//! | T-net           | Polynomial        | Check all circuits        |
//! | Free-choice     | Polynomial        | Commoner's theorem        |
//! | General         | EXPSPACE-complete | Coverability + more       |

use crate::analysis::System;
use crate::behavior::Tokens;
use crate::structure::class::{Circuit, FreeChoiceNet, SNet, TNet};
use crate::structure::{Net, Transition};

/// Result of a liveness check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessResult {
    /// The system/transition is live.
    Live,
    /// The system/transition is not live.
    NotLive,
    /// Liveness could not be determined (analysis incomplete or undecidable).
    Unknown,
}

impl From<bool> for LivenessResult {
    fn from(b: bool) -> Self {
        if b { LivenessResult::Live } else { LivenessResult::NotLive }
    }
}

/// Trait for liveness analysis on Petri net systems.
///
/// This trait provides methods for checking liveness of individual transitions
/// or the entire system. Implementations are specialized for different structural
/// classes to take advantage of theoretical shortcuts.
///
/// # Default Implementation
///
/// The default implementation uses general algorithms (coverability graph exploration)
/// which are correct but potentially expensive. Structural subclasses override these
/// methods with polynomial-time or constant-time algorithms where applicable.
pub trait LivenessAnalysis {
    /// Checks if the entire system is live (all transitions are L4-live).
    ///
    /// A system is live if from every reachable marking, every transition can
    /// eventually fire (possibly after firing other transitions).
    fn is_live(&self) -> LivenessResult;

    /// Checks if a specific transition is L4-live.
    fn is_transition_live(&self, transition: Transition) -> LivenessResult;

    /// Checks if the system is deadlock-free.
    ///
    /// A system is deadlock-free if from every reachable marking, at least one
    /// transition is enabled.
    fn is_deadlock_free(&self) -> LivenessResult;
}

// =============================================================================
// General Net Implementation (Fallback)
// =============================================================================

impl LivenessAnalysis for System<Net> {
    fn is_live(&self) -> LivenessResult {
        // General case: requires full state space exploration
        // This is EXPSPACE-complete in general
        // TODO: Implement using coverability graph and SCC analysis
        LivenessResult::Unknown
    }

    fn is_transition_live(&self, _transition: Transition) -> LivenessResult {
        // General case: check if transition appears in all bottom SCCs
        // of the coverability graph
        LivenessResult::Unknown
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        // General case: explore reachability graph looking for deadlocks
        LivenessResult::Unknown
    }
}

// =============================================================================
// Circuit Implementation (Most Specialized)
// =============================================================================

impl AsRef<Net> for Circuit {
    fn as_ref(&self) -> &Net {
        &self.0
    }
}

impl LivenessAnalysis for System<Circuit> {
    /// A circuit (N, M₀) is live iff M₀(S) > 0.
    ///
    /// This is O(|S|) - we just check if there are any tokens at all.
    fn is_live(&self) -> LivenessResult {
        let has_tokens = !self.initial_marking().is_zero();
        has_tokens.into()
    }

    fn is_transition_live(&self, _transition: Transition) -> LivenessResult {
        // In a circuit, all transitions have the same liveness status
        self.is_live()
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        // A circuit is deadlock-free iff it's live
        self.is_live()
    }
}

// =============================================================================
// S-net Implementation
// =============================================================================

impl AsRef<Net> for SNet {
    fn as_ref(&self) -> &Net {
        &self.0
    }
}

impl LivenessAnalysis for System<SNet> {
    /// An S-system (N, M₀) is live iff N is strongly connected and M₀(S) > 0.
    ///
    /// This is O(|S| + |T| + |F|) - just check connectivity and token presence.
    fn is_live(&self) -> LivenessResult {
        let net = self.net.as_ref();
        let is_strongly_connected = net.is_strongly_connected();
        let has_tokens = !self.initial_marking().is_zero();
        (is_strongly_connected && has_tokens).into()
    }

    fn is_transition_live(&self, _transition: Transition) -> LivenessResult {
        // In an S-net, all transitions have the same liveness status
        // (either all live or all dead)
        self.is_live()
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        // An S-system is deadlock-free iff it has tokens
        // (but may not be live if not strongly connected)
        let has_tokens = !self.initial_marking().is_zero();
        has_tokens.into()
    }
}

// =============================================================================
// T-net Implementation
// =============================================================================

impl AsRef<Net> for TNet {
    fn as_ref(&self) -> &Net {
        &self.0
    }
}

impl LivenessAnalysis for System<TNet> {
    /// A T-system (N, M₀) is live iff M₀(γ) > 0 for every circuit γ of N.
    ///
    /// Intuitively, a T-system is live iff every circuit contains at least one token.
    fn is_live(&self) -> LivenessResult {
        // TODO: Find all circuits and check each has at least one token
        // This requires circuit enumeration which can be expensive but is polynomial
        // for the structural check
        LivenessResult::Unknown
    }

    fn is_transition_live(&self, _transition: Transition) -> LivenessResult {
        // In a T-net, liveness of a transition depends on the circuits it's part of
        LivenessResult::Unknown
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        // For a strongly connected T-net, deadlock-freedom is equivalent to liveness
        let net = self.as_ref();
        if net.is_strongly_connected() {
            self.is_live()
        } else {
            LivenessResult::Unknown
        }
    }
}

// =============================================================================
// Free-Choice Net Implementation
// =============================================================================

impl AsRef<Net> for FreeChoiceNet {
    fn as_ref(&self) -> &Net {
        &self.0
    }
}

impl LivenessAnalysis for System<FreeChoiceNet> {
    /// Commoner's Liveness Theorem:
    /// A free-choice net (N, M₀) is live iff every siphon of N
    /// contains a trap marked at M₀.
    ///
    /// This is decidable in polynomial time!
    fn is_live(&self) -> LivenessResult {
        // TODO: Implement siphon/trap analysis
        // 1. Find all minimal siphons
        // 2. For each siphon, check if it contains a marked trap
        LivenessResult::Unknown
    }

    fn is_transition_live(&self, _transition: Transition) -> LivenessResult {
        // For free-choice nets, either all transitions are live or
        // specific analysis is needed
        LivenessResult::Unknown
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        // Free-choice deadlock-freedom can also use structural analysis
        LivenessResult::Unknown
    }
}

// =============================================================================
// Blanket Implementation for Reference Types
// =============================================================================

impl<'a, N> LivenessAnalysis for &'a System<N>
where
    System<N>: LivenessAnalysis,
{
    fn is_live(&self) -> LivenessResult {
        (*self).is_live()
    }

    fn is_transition_live(&self, transition: Transition) -> LivenessResult {
        (*self).is_transition_live(transition)
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        (*self).is_deadlock_free()
    }
}
