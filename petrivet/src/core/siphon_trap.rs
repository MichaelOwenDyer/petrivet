use crate::core::net::idx_set::PlaceIdxSet;

/// A siphon is a set of places D such that •D ⊆ D•.
///
/// In other words, every transition that produces to D also consumes from D.
/// This is significant because it means once a siphon is unmarked,
/// it can never be marked again (all transitions which could mark it are dead).
pub type IdxSiphon = PlaceIdxSet;

/// A trap is a set of places Q such that Q• ⊆ •Q.
///
/// In other words, every transition that consumes from Q also produces to Q.
/// This is significant because it means once a trap is marked, it can never be unmarked again.
pub type IdxTrap = PlaceIdxSet;