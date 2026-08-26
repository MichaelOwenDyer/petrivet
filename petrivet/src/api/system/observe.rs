//! Progress events emitted while a CEGAR-based analysis
//! ([`analyze_coverability_with_observer`](crate::system::PetriNet::analyze_coverability_with_observer),
//! [`analyze_reachability_with_observer`](crate::system::PetriNet::analyze_reachability_with_observer))
//! is running.

use crate::marking::Marking;
use crate::parikh_vector::ParikhVector;
use crate::system::lemma::Lemma;

/// The SMT solver proposed a candidate solution that turned out to be spurious, and CEGAR derived
/// `lemma` to rule it (and every other candidate it implicates) out. Purely informational: the
/// search runs to completion regardless of what an observer does with these events, so this is
/// meant for progress reporting and diagnostics, not for controlling the search.
#[derive(Debug, Clone)]
pub struct CegarEvent {
    /// The spurious marking the solver has proposed at this point in the search.
    pub spurious_marking: Option<Marking<u32>>,
    /// The spurious Parikh vector the solver has proposed at this point in the search.
    pub spurious_parikh_vector: Option<ParikhVector<u32>>,
    /// The lemma derived to eliminate this spurious candidate.
    pub lemma: Lemma,
}
