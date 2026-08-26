use crate::core::cegar::lemma::IdxLemma;
use crate::core::marking::IdxMarking;
use crate::core::parikh::IdxParikhVector;

/// A type-erased sink for [`IdxCegarEvent`]s.
pub type CegarObserverFn = Box<dyn Fn(IdxCegarEvent) + Send>;

#[derive(Debug, Clone)]
pub struct IdxCegarEvent {
    /// The marking which the SMT solver thought was a possible solution,
    /// but was actually spurious.
    pub spurious_marking: Option<IdxMarking<u32>>,
    /// The Parikh vector which the SMT solver thought was a possible solution,
    /// but was actually spurious. This is only present if transition variables
    /// have been added to the SMT problem.
    pub spurious_parikh_vector: Option<IdxParikhVector<u32>>,
    /// The lemma which was generated to eliminate this spurious solution.
    pub lemma: IdxLemma,
}
