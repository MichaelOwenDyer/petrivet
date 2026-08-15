use crate::core::cegar::refinements::IdxRefinement;
use crate::core::marking::IdxMarking;
use crate::core::parikh::IdxParikhVector;

#[derive(Debug, Clone)]
pub struct CegarEvent {
    /// The marking which the SMT solver thought was a possible solution,
    /// but was actually spurious.
    pub spurious_marking: IdxMarking<u32>,
    /// The Parikh vector which the SMT solver thought was a possible solution,
    /// but was actually spurious. This is only present if transition variables
    /// have been added to the SMT problem.
    pub spurious_parikh_vector: Option<IdxParikhVector<u32>>,
    /// The refinement which was generated to eliminate this spurious solution.
    pub refinement: IdxRefinement,
}
