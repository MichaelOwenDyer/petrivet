//! Boundedness analysis for Petri net systems.
//!
//! This module provides boundedness analysis with specialization for structural subclasses.
//! Boundedness is a behavioral property that describes whether the number of tokens
//! in any place can grow without limit.
//!
//! # Definitions
//!
//! - A place `p` is **k-bounded** if for all reachable markings M, M(p) ≤ k.
//! - A system is **k-bounded** if all places are k-bounded.
//! - A system is **bounded** if it is k-bounded for some k.
//! - A place is **safe** if it is 1-bounded (at most one token).
//! - A system is **safe** if all places are safe.
//!
//! # Complexity by Structural Class
//!
//! | Net Class       | Boundedness Decision | Complexity                  |
//! |-----------------|----------------------|-----------------------------|
//! | Circuit         | Trivial              | O(|S|)                      |
//! | S-net           | Trivial              | O(|S|)                      |
//! | T-net           | Polynomial           | Check strong connectivity   |
//! | Free-choice     | Polynomial           | Heck's theorem (if live)    |
//! | General         | EXPSPACE-complete    | Coverability graph          |

use crate::analysis::System;
use crate::behavior::{Omega, Tokens};
use crate::structure::class::{Circuit, FreeChoiceNet, SNet, TNet};
use crate::structure::{Net, Place};

/// Result of a boundedness analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundednessResult {
    /// The system/place is bounded with the given bound.
    Bounded(u32),
    /// The system/place is unbounded.
    Unbounded,
    /// Boundedness could not be determined.
    Unknown,
}

impl BoundednessResult {
    /// Returns true if the result indicates boundedness.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        matches!(self, BoundednessResult::Bounded(_))
    }

    /// Returns the bound if known, or None otherwise.
    #[must_use]
    pub fn bound(&self) -> Option<u32> {
        match self {
            BoundednessResult::Bounded(k) => Some(*k),
            _ => None,
        }
    }
}

/// Trait for boundedness analysis on Petri net systems.
///
/// This trait provides methods for checking boundedness of individual places
/// or the entire system. Implementations are specialized for different structural
/// classes to take advantage of theoretical shortcuts.
pub trait BoundednessAnalysis {
    /// Checks if the entire system is bounded.
    ///
    /// Returns `Bounded(k)` if the system is k-bounded, `Unbounded` if any
    /// place can grow without limit, or `Unknown` if analysis is incomplete.
    fn boundedness(&self) -> BoundednessResult;

    /// Checks if the system is safe (1-bounded).
    fn is_safe(&self) -> bool {
        matches!(self.boundedness(), BoundednessResult::Bounded(k) if k <= 1)
    }

    /// Checks boundedness of a specific place.
    fn place_boundedness(&self, place: Place) -> BoundednessResult;
}

// =============================================================================
// General Net Implementation (Fallback)
// =============================================================================

impl BoundednessAnalysis for System<Net> {
    fn boundedness(&self) -> BoundednessResult {
        // General case: requires coverability graph exploration
        // A place is unbounded iff it has ω in the coverability graph
        // This is EXPSPACE-complete
        BoundednessResult::Unknown
    }

    fn place_boundedness(&self, _place: Place) -> BoundednessResult {
        // Check coverability graph for ω in this place
        BoundednessResult::Unknown
    }
}

// =============================================================================
// Circuit Implementation
// =============================================================================

impl BoundednessAnalysis for System<Circuit> {
    /// A circuit (N, M₀) is k-bounded iff M₀(S) ≤ k.
    ///
    /// The total number of tokens is conserved in a circuit, so the
    /// bound is simply the sum of initial tokens.
    fn boundedness(&self) -> BoundednessResult {
        let total_tokens: i32 = self.initial_marking().iter().map(|t| t.0).sum();
        if total_tokens >= 0 {
            BoundednessResult::Bounded(total_tokens as u32)
        } else {
            BoundednessResult::Bounded(0)
        }
    }

    fn place_boundedness(&self, _place: Place) -> BoundednessResult {
        // In a circuit, any place can potentially hold all tokens
        self.boundedness()
    }
}

// =============================================================================
// S-net Implementation
// =============================================================================

impl BoundednessAnalysis for System<SNet> {
    /// A live S-system (N, M₀) is k-bounded iff M₀(S) ≤ k.
    ///
    /// The total number of tokens is conserved in an S-net.
    fn boundedness(&self) -> BoundednessResult {
        let total_tokens: i32 = self.initial_marking().iter().map(|t| t.0).sum();
        if total_tokens >= 0 {
            BoundednessResult::Bounded(total_tokens as u32)
        } else {
            BoundednessResult::Bounded(0)
        }
    }

    fn place_boundedness(&self, _place: Place) -> BoundednessResult {
        // In an S-net, tokens are conserved, so any place is bounded by total tokens
        self.boundedness()
    }
}

// =============================================================================
// T-net Implementation
// =============================================================================

impl BoundednessAnalysis for System<TNet> {
    /// A live T-system (N, M₀) is bounded iff N is strongly connected.
    ///
    /// More specifically:
    /// - A place s is bounded iff it belongs to some circuit
    /// - max{M(s) | M reachable} = min{M₀(γ) | γ contains s}
    fn boundedness(&self) -> BoundednessResult {
        let net = self.net.as_ref();
        if net.is_strongly_connected() {
            // If strongly connected, need to find minimum tokens in any circuit
            // that contains each place. For now, return Unknown as this requires
            // circuit enumeration.
            BoundednessResult::Unknown
        } else {
            // If not strongly connected, some places may be unbounded
            BoundednessResult::Unknown
        }
    }

    fn place_boundedness(&self, _place: Place) -> BoundednessResult {
        // A place is bounded iff it belongs to some circuit
        // Its bound is the minimum tokens in any circuit containing it
        BoundednessResult::Unknown
    }
}

// =============================================================================
// Free-Choice Net Implementation
// =============================================================================

impl BoundednessAnalysis for System<FreeChoiceNet> {
    /// Heck's Boundedness Theorem:
    /// Let (N, M₀) be a live free-choice system.
    /// Then (N, M₀) is bounded iff every place of N belongs to an S-component.
    fn boundedness(&self) -> BoundednessResult {
        // TODO: Implement S-component analysis
        // 1. Find all S-components
        // 2. Check if every place belongs to at least one
        BoundednessResult::Unknown
    }

    fn place_boundedness(&self, _place: Place) -> BoundednessResult {
        // For a live, bounded free-choice system:
        // max{M(s) | M reachable} = min{M₀(S') | S' is an S-component containing s}
        BoundednessResult::Unknown
    }
}

// =============================================================================
// Blanket Implementation for Reference Types
// =============================================================================

impl<N> BoundednessAnalysis for &System<N>
where
    System<N>: BoundednessAnalysis,
{
    fn boundedness(&self) -> BoundednessResult {
        (*self).boundedness()
    }

    fn place_boundedness(&self, place: Place) -> BoundednessResult {
        (*self).place_boundedness(place)
    }
}
