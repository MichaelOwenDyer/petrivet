//! The three-valued [`Verdict`] and its [`Abstention`] reasons.
//!
//! A `Verdict<P, N>` is the result type every analysis converges on: a checked
//! positive proof `P`, a checked negative refutation `N`, or an honest
//! `Inconclusive` carrying *why* the procedure abstained. The third value is
//! **type-distinct** from `Refuted`, so "cannot yet decide" can never be
//! silently read as "provably false" — the defect this type exists to make
//! unrepresentable (`liveness.rs`'s all-L0 fallback on unbounded nets, where
//! "unknown" and "provably dead" were the same value).
//!
//! The only public path to `Proven`/`Refuted` is
//! [`accept`](crate::model::accept): a verdict in either decided state can only
//! have come from a passing [`Certificate::check`](crate::model::Certificate),
//! because the decided variants carry a [`Proof`]/[`Refutation`] wrapper whose
//! field is private — the wrapper is mintable only inside `model`, and the sole
//! producer of the private [`Decided`](crate::model::Decided) token that builds
//! it lives behind that gate.
//!
//! # The firewall is a type invariant, not a grep
//!
//! `Verdict::Proven`/`Refuted` cannot be name-constructed from outside this
//! module, because their payloads ([`Proof<P>`]/[`Refutation<N>`]) have a
//! *private* field. This closes the public-variant hole that a bare
//! `pub enum Verdict { Proven(P), .. }` would leave open (an external consumer
//! could write `Verdict::Proven(payload)` with no certificate). The
//! `compile_fail` doctest below is the executable proof of the M1 `[PROP]`
//! "no public path yields `Proven`/`Refuted` without a passing `check`" gate:
//!
//! ```compile_fail
//! // A downstream crate cannot mint a decided verdict: `Proof` has a private
//! // field, so `Verdict::Proven(_)` does not name-construct outside `model`.
//! use petrivet::model::Verdict;
//! let _forged: Verdict<&str, &str> = Verdict::Proven("I never passed a check");
//! ```
//!
//! ```compile_fail
//! use petrivet::model::Verdict;
//! // Same for the negative pole: `Refutation`'s field is private.
//! let _forged: Verdict<&str, &str> = Verdict::Refuted("forged refutation");
//! ```
//!
//! ```compile_fail
//! // Nor does naming the wrapper help: `Proof`'s field is `pub(super)`, so the
//! // tuple constructor is invisible outside `crate::model`.
//! use petrivet::model::{Verdict, Proof};
//! let _forged: Verdict<&str, &str> = Verdict::Proven(Proof("forged via wrapper"));
//! ```
//!
//! By contrast `Inconclusive` is freely constructible — abstention is honest and
//! carries no proof obligation — via [`Verdict::inconclusive`]:
//!
//! ```
//! use petrivet::model::{Verdict, Abstention};
//! let v: Verdict<&str, &str> = Verdict::inconclusive(Abstention::Unbounded);
//! assert!(v.is_inconclusive());
//! ```

/// A checked positive proof, the payload of [`Verdict::Proven`].
///
/// The inner field is **private**: a `Proof<P>` can be constructed only inside
/// `crate::model`, by [`Decided::into_verdict`](crate::model::Decided) after the
/// certificate has passed. This is what makes `Verdict::Proven(_)`
/// un-name-constructible from any other module — the firewall as a type
/// invariant rather than a review convention.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Proof<P>(pub(super) P);

/// A checked negative refutation, the payload of [`Verdict::Refuted`].
///
/// See [`Proof`]: the private field is the firewall. A refutation, like a proof,
/// is mintable only behind [`accept`](crate::model::accept).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Refutation<N>(pub(super) N);

impl<P> Proof<P> {
    /// Borrow the witnessed proof payload.
    #[must_use]
    pub const fn witness(&self) -> &P {
        &self.0
    }

    /// Consume the wrapper, yielding the proof payload.
    #[must_use]
    pub fn into_witness(self) -> P {
        self.0
    }
}

impl<N> Refutation<N> {
    /// Borrow the witnessed refutation payload.
    #[must_use]
    pub const fn witness(&self) -> &N {
        &self.0
    }

    /// Consume the wrapper, yielding the refutation payload.
    #[must_use]
    pub fn into_witness(self) -> N {
        self.0
    }
}

/// A three-valued verdict for a property query.
///
/// - `Proven(Proof<P>)` — the property holds, witnessed by a checked proof `P`.
/// - `Refuted(Refutation<N>)` — the property fails, witnessed by a checked
///   refutation `N`.
/// - `Inconclusive(Abstention)` — no available procedure decided the query,
///   tagged with the reason it abstained.
///
/// `Proven`/`Refuted` are constructible *only* through
/// [`accept`](crate::model::accept), which gates on the certificate's `check`
/// against the original net: their [`Proof`]/[`Refutation`] payloads have a
/// private field that no module but `model` can fill. `Inconclusive` is freely
/// constructible (see [`Verdict::inconclusive`]): abstention is honest and
/// carries no proof obligation.
///
/// Note `Verdict` derives `Serialize` (emitting a result is harmless) but **not**
/// `Deserialize`: a derived `Deserialize` would generate a hidden constructor of
/// `Proven`/`Refuted` from arbitrary input, re-opening the firewall the private
/// payloads close. A deserialize-then-re-check path (deserialize a *certificate*
/// payload, then funnel it back through `accept`) is C6/M2 work, not M1.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[must_use = "a verdict carries a proof obligation; inspect or propagate it"]
pub enum Verdict<P, N> {
    /// The property holds, witnessed by a checked proof.
    Proven(Proof<P>),
    /// The property fails, witnessed by a checked refutation.
    Refuted(Refutation<N>),
    /// No available procedure decided the query; tagged with the reason.
    Inconclusive(Abstention),
}

/// Why a procedure returned [`Verdict::Inconclusive`].
///
/// The reason is type-carried (not a unit) so the `f_struct` harness (G4a) can
/// attribute *why* the structural tier abstained when it computes its
/// denominators — a unit `Inconclusive` would make that lossy and would let the
/// L0 defect re-enter as "everything is inconclusive."
///
/// `#[non_exhaustive]`: the cancellation milestone (D6) adds `BudgetExhausted`
/// without a breaking change; downstream `match`es must already carry a wildcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Abstention {
    /// The net is unbounded, so the finite procedure the query needs does not
    /// terminate (e.g. a reachability-graph liveness analysis on an unbounded
    /// system). This is the honest replacement for the all-L0 fallback.
    Unbounded,
    /// No efficient (polynomial) procedure is known for this net class and
    /// query, and no exhaustive fallback was attempted or it did not decide.
    NoEfficientProcedure,
    /// A candidate witness was produced but its certificate did not `check`
    /// against the original net, so it was rejected. The honest landing place
    /// for a generator bug or a witness built against the wrong query.
    NotApplicable,
}

impl<P, N> Verdict<P, N> {
    /// Construct an abstaining verdict. This is the **only** public constructor
    /// of any `Verdict` variant — abstention is honest and carries no proof
    /// obligation, so it needs no certificate. `Proven`/`Refuted` have no public
    /// constructor: they are reachable solely through
    /// [`accept`](crate::model::accept).
    pub const fn inconclusive(reason: Abstention) -> Self {
        Self::Inconclusive(reason)
    }

    /// Whether this verdict is a checked positive proof.
    #[must_use]
    pub const fn is_proven(&self) -> bool {
        matches!(self, Self::Proven(_))
    }

    /// Whether this verdict is a checked negative refutation.
    #[must_use]
    pub const fn is_refuted(&self) -> bool {
        matches!(self, Self::Refuted(_))
    }

    /// Whether this verdict abstained.
    #[must_use]
    pub const fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive(_))
    }

    /// The proof payload, if this verdict is `Proven`.
    #[must_use]
    pub const fn proof(&self) -> Option<&P> {
        if let Self::Proven(p) = self {
            Some(p.witness())
        } else {
            None
        }
    }

    /// The refutation payload, if this verdict is `Refuted`.
    #[must_use]
    pub const fn refutation(&self) -> Option<&N> {
        if let Self::Refuted(n) = self {
            Some(n.witness())
        } else {
            None
        }
    }

    /// The abstention reason, if this verdict is `Inconclusive`.
    #[must_use]
    pub const fn abstention(&self) -> Option<Abstention> {
        if let Self::Inconclusive(reason) = self {
            Some(*reason)
        } else {
            None
        }
    }

    // Deliberately NO `From<Verdict> for bool` and NO `unwrap_or(false)`:
    // collapsing the three values to a boolean is exactly the L0 defect this
    // type retires. Callers that need a boolean must decide what `Inconclusive`
    // means for their query at the call site, in the open.
}

#[cfg(test)]
mod tests {
    use super::{Abstention, Proof, Refutation, Verdict};

    // These tests construct `Proof`/`Refutation` directly via the `pub(super)`
    // field. That is legitimate: they live *inside* `crate::model`, the one
    // place permitted to fill that field. The firewall claim is about modules
    // *outside* `model` — proven by the `compile_fail` doctests on this module —
    // not about in-crate code, which `accept` already gates at runtime.

    /// `Inconclusive` is type-distinct from `Refuted`: the verdict that an
    /// unbounded net's liveness analysis returns is structurally unreadable as
    /// a refutation (the L0 defect made unrepresentable). This is the
    /// type-level half of the M1 "Inconclusive, not L0" gate; the wiring of
    /// `analyze_liveness` onto this type is M2.
    #[test]
    fn inconclusive_is_type_distinct_from_refuted() {
        let v: Verdict<&str, &str> = Verdict::inconclusive(Abstention::Unbounded);
        assert!(v.is_inconclusive());
        assert!(!v.is_refuted());
        assert!(!v.is_proven());
        // The reason is carried, not erased to a bare "unknown".
        assert_eq!(v.abstention(), Some(Abstention::Unbounded));
        assert_eq!(v.refutation(), None);
        assert_eq!(v.proof(), None);
    }

    #[test]
    fn accessors_match_variants() {
        let proven: Verdict<u8, u8> = Verdict::Proven(Proof(7));
        assert_eq!(proven.proof(), Some(&7));
        assert_eq!(proven.refutation(), None);
        assert_eq!(proven.abstention(), None);

        let refuted: Verdict<u8, u8> = Verdict::Refuted(Refutation(9));
        assert_eq!(refuted.refutation(), Some(&9));
        assert_eq!(refuted.proof(), None);
    }

    /// The `Proof`/`Refutation` wrappers expose their witness without leaking
    /// the constructor: `witness`/`into_witness` read the payload, but the
    /// tuple field stays `pub(super)`.
    #[test]
    fn wrappers_expose_witness() {
        let p = Proof("proof");
        assert_eq!(p.witness(), &"proof");
        assert_eq!(p.into_witness(), "proof");
        let r = Refutation("refutation");
        assert_eq!(r.witness(), &"refutation");
        assert_eq!(r.into_witness(), "refutation");
    }

    /// The result types **serialize** under the `serde` feature — emitting a
    /// verdict (proof, refutation, or abstention) as JSON is harmless. This is
    /// the emit half of the M1 `[LINT]` round-trip gate.
    ///
    /// `Verdict` deliberately does **not** derive `Deserialize`: a derived
    /// `Deserialize` would synthesize a hidden constructor of `Proven`/`Refuted`
    /// from arbitrary input (`serde_json::from_str::<Verdict<_,_>>("{\"Proven\":…}")`),
    /// re-opening the exact firewall the private `Proof`/`Refutation` fields
    /// close — an unchecked verdict trusted on its face. A deserialize-then-
    /// re-check path (deserialize a *certificate* payload, re-run `accept`)
    /// belongs to C6/M2, not M1. So the decided variants are write-only across
    /// serde; only `Abstention` (no proof obligation) round-trips in full.
    #[cfg(feature = "serde")]
    #[test]
    fn decided_verdicts_serialize_but_do_not_deserialize() {
        let cases: [Verdict<u32, String>; 3] = [
            Verdict::Proven(Proof(42)),
            Verdict::Refuted(Refutation("violated".to_string())),
            Verdict::inconclusive(Abstention::NoEfficientProcedure),
        ];
        for original in &cases {
            // Serialization succeeds for every variant.
            let json = serde_json::to_string(original).expect("serialize");
            assert!(!json.is_empty());
        }
    }

    /// Every `Abstention` reason round-trips: abstention carries no proof
    /// obligation, so deserializing one forges nothing.
    #[cfg(feature = "serde")]
    #[test]
    fn abstention_round_trips_through_serde() {
        for reason in [
            Abstention::Unbounded,
            Abstention::NoEfficientProcedure,
            Abstention::NotApplicable,
        ] {
            let json = serde_json::to_string(&reason).expect("serialize");
            let restored: Abstention = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(reason, restored);
        }
    }
}
