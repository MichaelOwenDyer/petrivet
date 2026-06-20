//! The [`Certificate`] trait: a witness that re-establishes its property
//! against the *original* net.
//!
//! This is the load-bearing interface of the whole trust architecture. A
//! certificate's `check` re-establishes the claimed property against the
//! original `(net, m0, query)`, assuming nothing about which decider or
//! reduction produced the witness and sharing no code with the generators
//! beyond primitive net access. That single discipline is what keeps the
//! trusted base constant under reduction-lifting and makes the format
//! tool-agnostic (a certificate from a different procedure checks identically).
//!
//! The trait is intentionally **not sealed**: the per-property checkers (M2),
//! lifted certificates (Epic F), and the structural generators (Epic B) must
//! all implement it. The set that must be closed is *who may accept a verdict*
//! — enforced by [`accept`](crate::model::accept) being the sole door to
//! `Proven`/`Refuted` — not *who may describe a witness*.

use crate::marking::Marking;
use crate::net::Net;
use crate::model::frontier::Polarity;
use crate::model::query::Query;

/// A checkable witness for a property verdict.
///
/// `check` returns `true` iff the witness re-establishes its property against
/// the **original** `(net, m0, query)`. It must use only primitive net access
/// (`fire`/`is_enabled`, exact dot products) and must not depend on the
/// generator that produced the witness — that independence is what makes the
/// checker, and not the generator, the trusted code.
///
/// `m0` is passed separately from `net` because the certificate is checked
/// against the system's *initial* marking `M₀`, not against any mutated current
/// marking a generator may have left behind.
pub trait Certificate {
    /// Re-establish the claimed property against the original net.
    ///
    /// Returns `true` iff the witness is valid for `(net, m0, query)`.
    #[must_use]
    fn check(&self, net: &Net, m0: &Marking<u32>, query: &Query) -> bool;

    /// Which pole this witness establishes when [`check`](Certificate::check)
    /// passes: [`Polarity::ProveYes`] for a positive proof (the property *holds* —
    /// a reachability firing sequence, a structural-boundedness sub-invariant, a
    /// Commoner–Hack liveness cover) or [`Polarity::ProveNo`] for a refutation (the
    /// property *fails* — a token-conservation witness for unreachability).
    ///
    /// A single witness establishes exactly one pole, so an implementor returns
    /// `ProveYes`/`ProveNo`, never [`Polarity::Exact`] — `Exact` is a *decider*'s
    /// capability (it possesses both kinds of witness), not a certificate's.
    ///
    /// [`accept`](crate::model::accept) requires this to agree with the candidate
    /// verdict's pole (the A6 polarity-coherence gate): a passing check on a
    /// wrong-signed candidate collapses to `Inconclusive` rather than minting a
    /// wrong-signed verdict. There is deliberately **no default** — a new
    /// certificate must declare its pole, so no silent default can mis-sign a verdict.
    #[must_use]
    fn polarity(&self) -> Polarity;
}
