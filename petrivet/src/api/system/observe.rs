use crate::api::system::parikh_vector::ParikhVector;
use crate::system::lemma::Lemma;
use crate::system::marking::Marking;

/// A `CegarEvent` is emitted by the CEGAR-based analysis whenever a spurious candidate
/// is found and a lemma is derived to eliminate it.
#[derive(Debug, Clone)]
pub struct CegarEvent {
    /// The spurious marking the solver has proposed at this point in the search.
    pub spurious_marking: Marking<u32>,
    /// The spurious Parikh vector the solver has proposed at this point in the search.
    pub spurious_parikh_vector: Option<ParikhVector<u32>>,
    /// The lemma derived to eliminate this spurious candidate.
    pub lemma: Lemma,
}
