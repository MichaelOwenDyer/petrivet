//! Structural and behavioral analysis of Petri nets.
//!
//! This module provides:
//! - **Structural analysis**: S-invariants, T-invariants, siphons, traps
//! - **Semi-decision procedures**: marking equation (LP/ILP) for reachability
//!   and boundedness
//!
//! High-level behavioral queries (e.g. `is_bounded`, `is_live`) are exposed
//! directly on [`System`](crate::system::System), which internally dispatches
//! to the best available algorithm based on net class.

pub mod structural;
pub mod semi_decision;
pub mod math;
