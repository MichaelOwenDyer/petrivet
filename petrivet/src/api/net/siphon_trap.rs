use crate::net::Place;
use ahash::HashSet;

/// A siphon is a set of places D such that •D ⊆ D•.
///
/// In other words, every transition that produces into D also consumes from D.
/// This is significant because it means once a siphon becomes unmarked,
/// it can never become marked again.
pub type Siphon = HashSet<Place>;

/// A trap is a set of places Q such that Q• ⊆ •Q.
///
/// In other words, every transition that consumes from Q also produces to Q.
/// This is significant because it means once a trap becomes marked,
/// it can never be unmarked again.
pub type Trap = HashSet<Place>;
