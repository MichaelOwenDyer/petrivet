//! M3 · D1 — the **decider registry**: deciders, a policy, and a driver.
//!
//! Today's analysis cascade is a hand-coded `match self.class()` per property,
//! scattered across `api/system/*`. M3 makes that schedule a *value*: each
//! decision procedure becomes a [`Decide`]r with declared [`Polarity`],
//! [`CostClass`], and class admissibility; a [`Policy`] orders the admissible
//! deciders; a [`Driver`] runs them and returns the first decided verdict, gating
//! every acceptance through [`accept`](crate::model::accept) (the trust boundary).
//!
//! The default policy reproduces today's cascade **exactly** (the S2 contract):
//! the per-property decider lists are registered in cascade order, and
//! [`DefaultPolicy`] preserves that order among the admissible deciders — it does
//! **not** re-sort by [`CostClass`] (the cascade order is hand-tuned, not a pure
//! cost ranking; `cost_class` is metadata for telemetry and the future learned
//! policy, the `SATzilla` sequel). Reproduction is proven by the `[REGRESS]` gate
//! (`mcc-tests`) and the per-property `[PROP]` order-invariance tests.
//!
//! # The seam
//!
//! A future structural generator (Epic B: the S/T-component deciders B3–B6, the
//! class-agnostic deciders B10/B11) slots in by adding one `Box<dyn Decide<P,N>>`
//! at its cascade position in the relevant per-property list and pairing it with a
//! certificate — **no change to [`Decide`], [`Policy`], or [`Driver`]**. The B2
//! cluster keystone is such a future input; B2 itself registers no decider.
//!
//! # The certificate routing
//!
//! Every decider mints `Proven`/`Refuted` *only* via [`accept`](crate::model::accept),
//! never by any other route — uniform with the firewall (C4 verify-on-return).
//! Where the cascade already emits a re-checkable witness (a firing sequence, a
//! token-conservation mismatch, a siphon/trap cover, an exact sub-invariant), the
//! decider routes through the matching C1 certificate, which genuinely re-validates
//! it. Where the cascade emits only a bare structural verdict (a trusted-base
//! decider — the ledger's bare-boolean rows), the decider routes through
//! [`BareBooleanCert`]: a polarity-carrying placeholder whose `check` performs no
//! independent re-derivation (there is no compact witness yet), so the verdict is
//! still gated by the A6 polarity-coherence half of `accept` while remaining
//! honestly *uncertified* (the ledger continues to count it bare-boolean; `f` is
//! not inflated). Replacing a `BareBooleanCert` with a real certificate is exactly
//! how a B-epic decider flips a ledger row from bare-boolean to certifying.

use crate::class::NetClass;
use crate::marking::Marking;
use crate::model::certificate::Certificate;
use crate::model::frontier::Polarity;
use crate::model::query::Query;
use crate::model::verdict::{Abstention, Verdict};
use crate::net::Net;

pub mod reachability;

pub use reachability::reachability_driver;

/// The asymptotic cost tier of a decider, used as telemetry and as the ordering
/// key for *future* cost-aware policies (the learned `SATzilla` sequel, Epic D).
///
/// Ordinal: `Constant < NearLinear < Polynomial < StateSpace`. The
/// [`DefaultPolicy`] deliberately does **not** order by this — today's cascade is
/// hand-ordered and not a pure cost ranking — but a decider declares it so the
/// inventory and any later policy can reason about cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CostClass {
    /// `O(1)` on precomputed/cached fields: a class tag, strong-connectivity, a
    /// token-sum comparison, marking identity.
    Constant,
    /// `O(|P| + |T|)` graph walks: a single firing-sequence realization, an
    /// unmarked-circuit scan, a union-find pass.
    NearLinear,
    /// Exact ℚ linear algebra (the marking-equation guard, an exact sub-invariant)
    /// or siphon/trap (Commoner–Hack) enumeration — polynomial *to check*, though
    /// some generators are worst-case exponential.
    Polynomial,
    /// Karp–Miller coverability / reachability-graph construction. May be
    /// exponential and is the only tier that can fail to terminate on an unbounded
    /// net (where it abstains at the ω-frontier).
    StateSpace,
}

/// A run budget threaded into [`Decide::run`] — the seam for cooperative
/// cancellation and bounded per-decider measurement (Epic D6).
///
/// Today's cascade has no budget (it runs each procedure to completion or to its
/// own ω/abstain point), so [`Budget::unbounded`] is the configuration that
/// reproduces it exactly. The cheap structural deciders ignore the budget; only
/// the [`StateSpace`](CostClass::StateSpace) tier would consult it once
/// cancellation lands. A finite budget is a *new* capability that may legitimately
/// turn a decided verdict into `Inconclusive`, so it must never be the default in
/// the `[REGRESS]` gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Maximum states a state-space decider may expand; `None` is unbounded
    /// (reproduces today's cascade).
    pub max_states: Option<usize>,
}

impl Budget {
    /// The unbounded budget — the configuration under which the [`DefaultPolicy`]
    /// driver reproduces today's cascade exactly.
    #[must_use]
    pub const fn unbounded() -> Self {
        Self { max_states: None }
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// One certifying decision procedure for a single property `P` (positive proof) /
/// `N` (negative refutation).
///
/// A `Decide`r is *admissible* for some [`NetClass`]es, has a fixed [`Polarity`]
/// and [`CostClass`], and on [`run`](Decide::run) either proposes a candidate
/// verdict — **gated through [`accept`](crate::model::accept)** — or abstains
/// (`Inconclusive`).
///
/// # Invariant (the trust boundary)
///
/// An implementor MUST mint every `Proven`/`Refuted` by calling
/// [`accept`](crate::model::accept) with a [`Decided`](crate::model::Decided)
/// candidate and a [`Certificate`]. It must NEVER construct a decided verdict by
/// any other route. A decider with no compact re-checkable witness yet routes
/// through [`BareBooleanCert`] so the boundary stays uniform.
pub trait Decide<P, N> {
    /// A stable identity for the ledger / telemetry.
    fn name(&self) -> &'static str;

    /// The pole this decider can conclude. `Exact` = it can decide both poles
    /// (e.g. a state-space search). This is decider-level metadata; the per-`run`
    /// candidate's pole and its certificate's pole are what `accept` checks for
    /// coherence.
    fn polarity(&self) -> Polarity;

    /// The decider's cost tier (telemetry / future cost-aware policy).
    fn cost_class(&self) -> CostClass;

    /// Whether this decider may be tried on a net of the given class. MUST use the
    /// subsumption predicates (`NetClass::is_state_machine()` etc.) wherever the
    /// cascade did — a `Circuit` is admissible to the state-machine and
    /// marked-graph arms both.
    fn admissible(&self, class: NetClass) -> bool;

    /// Run against the net, its **initial** marking `m0`, and the query. Returns a
    /// decided `Verdict` ONLY via [`accept`](crate::model::accept); otherwise
    /// `Verdict::inconclusive(..)`.
    fn run(&self, net: &Net, m0: &Marking<u32>, query: &Query, budget: Budget) -> Verdict<P, N>;
}

/// Orders the admissible deciders for a query.
///
/// The order is the S2 contract: for a net admissible to several deciders it
/// determines which proof object is minted first (the accepted *pole* is
/// order-invariant for sound deciders — the `[PROP]` gate — but the witness need
/// not be).
pub trait Policy<P, N> {
    /// Return the admissible deciders, in the order they must be tried.
    fn order<'d>(
        &self,
        class: NetClass,
        deciders: &'d [Box<dyn Decide<P, N>>],
    ) -> Vec<&'d dyn Decide<P, N>>;
}

/// The cascade-faithful default policy: keeps the admissible deciders in
/// registration order.
///
/// Registration order *is* cascade order; this policy does **not** re-sort by
/// [`CostClass`]. Reproducing the hand-ordered cascade exactly is the S2 contract.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPolicy;

impl<P, N> Policy<P, N> for DefaultPolicy {
    fn order<'d>(
        &self,
        class: NetClass,
        deciders: &'d [Box<dyn Decide<P, N>>],
    ) -> Vec<&'d dyn Decide<P, N>> {
        deciders
            .iter()
            .map(AsRef::as_ref)
            .filter(|d| d.admissible(class))
            .collect()
    }
}

/// Runs a property's deciders in policy order, returning the first decided verdict.
///
/// Returns the first `Proven`/`Refuted`, else `Inconclusive`. The driver never
/// constructs a verdict itself — each decider mints only via
/// [`accept`](crate::model::accept); the driver only selects, orders, and
/// short-circuits on the first decided result.
pub struct Driver<P, N, Pol: Policy<P, N>> {
    deciders: Vec<Box<dyn Decide<P, N>>>,
    policy: Pol,
}

impl<P, N, Pol: Policy<P, N>> Driver<P, N, Pol> {
    /// Build a driver from a per-property decider list (in cascade order) and a
    /// policy.
    #[must_use]
    pub fn new(deciders: Vec<Box<dyn Decide<P, N>>>, policy: Pol) -> Self {
        Self { deciders, policy }
    }

    /// The deciders, in registration (cascade) order — for inspection and the
    /// order-invariance `[PROP]` test.
    #[must_use]
    pub fn deciders(&self) -> &[Box<dyn Decide<P, N>>] {
        &self.deciders
    }

    /// Decide the query: order the admissible deciders by the policy, run each, and
    /// return the first `Proven`/`Refuted`. If every admissible decider abstains,
    /// return `Inconclusive(NoEfficientProcedure)`.
    pub fn decide(
        &self,
        net: &Net,
        m0: &Marking<u32>,
        query: &Query,
        budget: Budget,
    ) -> Verdict<P, N> {
        for decider in self.policy.order(net.class(), &self.deciders) {
            let verdict = decider.run(net, m0, query, budget);
            match verdict {
                Verdict::Proven(_) | Verdict::Refuted(_) => return verdict,
                Verdict::Inconclusive(_) => {}
            }
        }
        Verdict::inconclusive(Abstention::NoEfficientProcedure)
    }
}

/// A trusted-base placeholder certificate: it declares a [`Polarity`] but performs
/// **no independent re-derivation** of its property.
///
/// It exists so a *bare-boolean* decider (one whose verdict has no compact
/// re-checkable witness yet — the ledger's trusted-base rows) can route uniformly
/// through [`accept`](crate::model::accept): the A6 polarity-coherence half of the
/// gate still applies, but `check` returns `true` unconditionally because there is
/// nothing independent to check — soundness rests on the decider's own
/// correctness, which is exactly what "in the trusted base" means.
///
/// This does **not** inflate the certifying fraction `f`: the
/// [`ledger`](crate::model::trusted_base) classifies such a decider `BareBoolean`
/// regardless of this routing (the kind tracks whether a witness is *re-derived*,
/// not whether `accept` is called). Replacing this with a real [`Certificate`] —
/// the Epic-B work that lands a witness — is what flips a ledger row to certifying.
#[derive(Debug, Clone, Copy)]
pub struct BareBooleanCert {
    polarity: Polarity,
}

impl BareBooleanCert {
    /// A positive (prove-yes) trusted-base placeholder.
    #[must_use]
    pub const fn prove_yes() -> Self {
        Self { polarity: Polarity::ProveYes }
    }

    /// A negative (prove-no) trusted-base placeholder.
    #[must_use]
    pub const fn prove_no() -> Self {
        Self { polarity: Polarity::ProveNo }
    }
}

impl Certificate for BareBooleanCert {
    /// No independent re-derivation: the decider is in the trusted base. Returns
    /// `true` so `accept` promotes the candidate (subject to the polarity gate);
    /// soundness rests on the decider, not on a re-checked witness.
    fn check(&self, _net: &Net, _m0: &Marking<u32>, _query: &Query) -> bool {
        true
    }

    fn polarity(&self) -> Polarity {
        self.polarity
    }
}
