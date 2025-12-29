//! Analysis framework for Petri nets with specialization for structural subclasses.
//!
//! This module provides a trait-based architecture for defining analysis methods that can be
//! specialized for different structural subclasses of Petri nets. The key design principles are:
//!
//! 1. **Separation of concerns**: Structural analysis (on Net) vs behavioral analysis (on System)
//! 2. **Fallback semantics**: Specialized implementations override general ones automatically
//! 3. **Type-level guarantees**: Decidability and complexity are encoded in the type system
//!
//! # Architecture
//!
//! The architecture uses a layered trait system:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         Analysis Traits                                  │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
//! │  │  Liveness   │  │ Boundedness │  │Reachability │  │  Coverability│     │
//! │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘     │
//! │         │                │                │                │            │
//! │         ▼                ▼                ▼                ▼            │
//! │  ┌─────────────────────────────────────────────────────────────────┐    │
//! │  │                    System<N> (N, M₀)                            │    │
//! │  │  Delegates to specialized impls based on N's structural class   │    │
//! │  └─────────────────────────────────────────────────────────────────┘    │
//! │                                    │                                    │
//! │         ┌──────────────────────────┼──────────────────────────┐         │
//! │         ▼                          ▼                          ▼         │
//! │  ┌─────────────┐           ┌─────────────┐           ┌─────────────┐    │
//! │  │   Circuit   │           │    SNet     │           │    TNet     │    │
//! │  │  Impl for   │           │  Impl for   │           │  Impl for   │    │
//! │  │  Circuit    │           │    SNet     │           │    TNet     │    │
//! │  └──────┬──────┘           └──────┬──────┘           └──────┬──────┘    │
//! │         │                         │                         │           │
//! │         └─────────────────────────┼─────────────────────────┘           │
//! │                                   ▼                                     │
//! │                          ┌─────────────┐                                │
//! │                          │FreeChoiceNet│                                │
//! │                          └──────┬──────┘                                │
//! │                                 │                                       │
//! │                                 ▼                                       │
//! │                          ┌─────────────┐                                │
//! │                          │     Net     │                                │
//! │                          │  (fallback) │                                │
//! │                          └─────────────┘                                │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use petrivet::analysis::{System, LivenessAnalysis, BoundednessAnalysis};
//!
//! // For a general net, we get the general (potentially expensive) algorithms
//! let system = System::new(net, m0);
//! let is_live = system.is_live();  // Uses general algorithm
//!
//! // For an S-net, we get the specialized O(1) algorithm
//! let s_net: SNet = net.try_into()?;
//! let system = System::new(s_net, m0);
//! let is_live = system.is_live();  // Uses SNet-specific theorem!
//! ```
//!
//! # Adding New Analysis Methods
//!
//! To add a new analysis method:
//!
//! 1. Define the trait with the analysis signature
//! 2. Implement it for `System<Net>` (the fallback)
//! 3. Implement specialized versions for the structural subclasses where shortcuts exist
//!
//! The blanket implementations will automatically delegate to the most specific implementation.

pub mod liveness;
pub mod boundedness;
pub mod reachability;
pub mod structural;

use crate::behavior::Marking;
use crate::structure::Net;
use std::ops::Deref;

/// A Petri net system: a net N combined with an initial marking M₀.
///
/// This is the primary type for behavioral analysis. The type parameter `N` represents
/// the structural class of the net, enabling specialized analysis implementations.
///
/// # Type Parameter
///
/// - `N`: The net type, which can be `Net`, `FreeChoiceNet`, `SNet`, `TNet`, or `Circuit`.
///        This enables compile-time specialization of analysis methods.
///
/// # Examples
///
/// ```ignore
/// // Create a system with a general net
/// let system: System<Net> = System::new(net, initial_marking);
///
/// // Create a system with a known S-net (enables specialized analysis)
/// let s_net: SNet = net.try_into()?;
/// let system: System<SNet> = System::new(s_net, initial_marking);
/// ```
#[derive(Debug, Clone)]
pub struct System<N> {
    net: N,
    initial_marking: Marking,
}

impl<N> System<N> {
    /// Creates a new system from a net and initial marking.
    #[must_use]
    pub fn new<M: Into<Marking>>(net: N, initial_marking: M) -> Self {
        let initial_marking = initial_marking.into();
        Self { net, initial_marking }
    }

    /// Returns a reference to the underlying net.
    #[must_use]
    pub fn net(&self) -> &N {
        &self.net
    }

    /// Returns a reference to the initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        &self.initial_marking
    }

    /// Consumes the system and returns its components.
    #[must_use]
    pub fn into_parts(self) -> (N, Marking) {
        (self.net, self.initial_marking)
    }
}

// Implement Deref for System to allow easy access to net methods
impl<N> Deref for System<N> {
    type Target = N;

    fn deref(&self) -> &Self::Target {
        &self.net
    }
}

// =============================================================================
// Unified Analysis Interface
// =============================================================================

use crate::structure::class::{Circuit, FreeChoiceNet, SNet, StructureClass, TNet};

/// A type-erased system that dispatches to the most efficient implementation.
///
/// This enum allows you to work with systems of any structural class through
/// a unified interface, automatically benefiting from specialized implementations.
///
/// # Example
///
/// ```ignore
/// let net = builder.build()?;  // Returns StructureClass
/// let system = ClassifiedSystem::new(net, initial_marking);
///
/// // Automatically uses the best available algorithm
/// let is_live = system.is_live();
/// ```
#[derive(Debug, Clone)]
pub enum ClassifiedSystem {
    Circuit(System<Circuit>),
    SNet(System<SNet>),
    TNet(System<TNet>),
    FreeChoice(System<FreeChoiceNet>),
    Unrestricted(System<Net>),
}

impl ClassifiedSystem {
    /// Creates a classified system from a structure class and initial marking.
    #[must_use]
    pub fn new<M: Into<Marking>>(class: StructureClass, initial_marking: M) -> Self {
        let initial_marking = initial_marking.into();
        match class {
            StructureClass::Circuit(net) => ClassifiedSystem::Circuit(System::new(net, initial_marking)),
            StructureClass::SNet(net) => ClassifiedSystem::SNet(System::new(net, initial_marking)),
            StructureClass::TNet(net) => ClassifiedSystem::TNet(System::new(net, initial_marking)),
            StructureClass::FreeChoiceNet(net) => ClassifiedSystem::FreeChoice(System::new(net, initial_marking)),
            StructureClass::Unrestricted(net) => ClassifiedSystem::Unrestricted(System::new(net, initial_marking)),
        }
    }

    /// Returns a reference to the underlying net (as the general Net type).
    #[must_use]
    pub fn net(&self) -> &Net {
        match self {
            ClassifiedSystem::Circuit(s) => s.net.as_ref(),
            ClassifiedSystem::SNet(s) => s.net.as_ref(),
            ClassifiedSystem::TNet(s) => s.net.as_ref(),
            ClassifiedSystem::FreeChoice(s) => s.net.as_ref(),
            ClassifiedSystem::Unrestricted(s) => &s.net,
        }
    }

    /// Returns a reference to the initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        match self {
            ClassifiedSystem::Circuit(s) => &s.initial_marking,
            ClassifiedSystem::SNet(s) => &s.initial_marking,
            ClassifiedSystem::TNet(s) => &s.initial_marking,
            ClassifiedSystem::FreeChoice(s) => &s.initial_marking,
            ClassifiedSystem::Unrestricted(s) => &s.initial_marking,
        }
    }

    /// Returns the structural class name for debugging/logging.
    #[must_use]
    pub fn class_name(&self) -> &'static str {
        match self {
            ClassifiedSystem::Circuit(_) => "Circuit",
            ClassifiedSystem::SNet(_) => "S-net",
            ClassifiedSystem::TNet(_) => "T-net",
            ClassifiedSystem::FreeChoice(_) => "Free-choice",
            ClassifiedSystem::Unrestricted(_) => "Unrestricted",
        }
    }
}

// Implement the analysis traits for ClassifiedSystem by dispatching
use crate::analysis::liveness::{LivenessAnalysis, LivenessResult};
use crate::analysis::boundedness::{BoundednessAnalysis, BoundednessResult};
use crate::analysis::reachability::{ReachabilityAnalysis, ReachabilityResult};
use crate::structure::Transition;

impl LivenessAnalysis for ClassifiedSystem {
    fn is_live(&self) -> LivenessResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.is_live(),
            ClassifiedSystem::SNet(s) => s.is_live(),
            ClassifiedSystem::TNet(s) => s.is_live(),
            ClassifiedSystem::FreeChoice(s) => s.is_live(),
            ClassifiedSystem::Unrestricted(s) => s.is_live(),
        }
    }

    fn is_transition_live(&self, transition: Transition) -> LivenessResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.is_transition_live(transition),
            ClassifiedSystem::SNet(s) => s.is_transition_live(transition),
            ClassifiedSystem::TNet(s) => s.is_transition_live(transition),
            ClassifiedSystem::FreeChoice(s) => s.is_transition_live(transition),
            ClassifiedSystem::Unrestricted(s) => s.is_transition_live(transition),
        }
    }

    fn is_deadlock_free(&self) -> LivenessResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.is_deadlock_free(),
            ClassifiedSystem::SNet(s) => s.is_deadlock_free(),
            ClassifiedSystem::TNet(s) => s.is_deadlock_free(),
            ClassifiedSystem::FreeChoice(s) => s.is_deadlock_free(),
            ClassifiedSystem::Unrestricted(s) => s.is_deadlock_free(),
        }
    }
}

impl BoundednessAnalysis for ClassifiedSystem {
    fn boundedness(&self) -> BoundednessResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.boundedness(),
            ClassifiedSystem::SNet(s) => s.boundedness(),
            ClassifiedSystem::TNet(s) => s.boundedness(),
            ClassifiedSystem::FreeChoice(s) => s.boundedness(),
            ClassifiedSystem::Unrestricted(s) => s.boundedness(),
        }
    }

    fn place_boundedness(&self, place: crate::structure::Place) -> BoundednessResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.place_boundedness(place),
            ClassifiedSystem::SNet(s) => s.place_boundedness(place),
            ClassifiedSystem::TNet(s) => s.place_boundedness(place),
            ClassifiedSystem::FreeChoice(s) => s.place_boundedness(place),
            ClassifiedSystem::Unrestricted(s) => s.place_boundedness(place),
        }
    }
}

impl ReachabilityAnalysis for ClassifiedSystem {
    fn is_reachable(&self, target: &Marking) -> ReachabilityResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.is_reachable(target),
            ClassifiedSystem::SNet(s) => s.is_reachable(target),
            ClassifiedSystem::TNet(s) => s.is_reachable(target),
            ClassifiedSystem::FreeChoice(s) => s.is_reachable(target),
            ClassifiedSystem::Unrestricted(s) => s.is_reachable(target),
        }
    }

    fn is_coverable(&self, target: &Marking) -> ReachabilityResult {
        match self {
            ClassifiedSystem::Circuit(s) => s.is_coverable(target),
            ClassifiedSystem::SNet(s) => s.is_coverable(target),
            ClassifiedSystem::TNet(s) => s.is_coverable(target),
            ClassifiedSystem::FreeChoice(s) => s.is_coverable(target),
            ClassifiedSystem::Unrestricted(s) => s.is_coverable(target),
        }
    }
}
