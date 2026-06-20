//! The [`Query`] a certificate is checked against.
//!
//! A certificate is meaningless without the query it answers: a firing-sequence
//! witness claims to reach *a particular target*; a separating P-invariant must
//! be tested against *a particular marking difference*. So
//! [`Certificate::check`](crate::model::Certificate::check) takes the query as a
//! required argument, and the query is **owned** — it is part of the
//! offline-checkable certificate and the training label, not a borrow.
//!
//! Query-free properties (liveness, boundedness, deadlock-freedom) pass the
//! unit variant [`Query::Trivial`] rather than a separate unit type.

use crate::marking::Marking;

/// The property question a certificate answers, checked against the original net.
///
/// `#[non_exhaustive]`: new query kinds (e.g. a place/transition-scoped query)
/// can be added without a breaking change; downstream `match`es carry a
/// wildcard.
///
/// `Query` does **not** derive serde in M1. It carries [`Marking<u32>`], and
/// `Marking`/`Place`/`UniqueSortedSlice` do not yet derive serde — their
/// `Deserialize` must re-establish the sorted-unique, no-zero-token canonical
/// invariants through a custom impl, which is C6 (the interchange format)
/// breadth, not M1. The contract is *serde-ready by design* (owned, borrow-free,
/// `'static`); wiring it through the marking stack is deferred so M1 does not
/// silently widen its scope. See `foundational-design.md` §F3″ decision 5.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Query {
    /// A query-free property (liveness, boundedness, deadlock-freedom). The
    /// unit query; a certificate for such a property ignores its content.
    Trivial,
    /// Is the given target marking reachable from `M₀`?
    Reachable(Marking<u32>),
    /// Is the given target marking coverable from `M₀`?
    Coverable(Marking<u32>),
}
