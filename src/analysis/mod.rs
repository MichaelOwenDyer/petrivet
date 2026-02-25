//! Structural and behavioral analysis of Petri nets.
//!
//! This module provides low-level analysis primitives:
//!
//! - [`structural`] — S-invariants, T-invariants, siphons, traps, and
//!   Commoner's theorem. These depend only on net topology and can prove
//!   properties like conservativeness and liveness without state space
//!   exploration.
//!
//! - [`semi_decision`] — LP/ILP formulations of the marking equation for
//!   reachability filtering and structural boundedness. These run in
//!   polynomial time and serve as fast necessary-condition checks.
//!
//! - [`math`] — Integer linear algebra (Bareiss null space) used internally
//!   by invariant computation.
//!
//! Most users should start with the high-level behavioral queries on
//! [`System`](crate::system::System) (e.g. `is_bounded`, `is_live`,
//! `liveness_levels`), which internally dispatch to the best available
//! algorithm based on net class. Use this module directly when you need
//! access to invariant vectors, siphon/trap sets, or marking equation
//! results for custom analysis.

pub mod structural;
pub mod semi_decision;
pub mod math;
