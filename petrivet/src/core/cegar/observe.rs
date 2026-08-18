use crate::api::parikh_vector::ParikhVector;
use crate::core::cegar::lemma::IdxLemma;
use crate::marking::Marking;

#[derive(Debug, Clone)]
pub struct CegarEvent {
    /// The marking which the SMT solver thought was a possible solution,
    /// but was actually spurious.
    pub spurious_marking: Option<Marking<u32>>,
    /// The Parikh vector which the SMT solver thought was a possible solution,
    /// but was actually spurious. This is only present if transition variables
    /// have been added to the SMT problem.
    pub spurious_parikh_vector: Option<ParikhVector<u32>>,
    /// The lemma which was generated to eliminate this spurious solution.
    pub lemma: IdxLemma,
}
