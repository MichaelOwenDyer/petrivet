//! The verdict/certificate contract — the single API surface for every result.
//!
//! This module is the consolidation point the rest of the program is written
//! against (`literature.rs` and `petrivet-wasm` both reach for
//! `petrivet::model::*`). It defines the three abstractions of the trust
//! architecture and re-exports the per-property result and proof shapes from
//! where they live in `api/system/*`.
//!
//! # The three abstractions
//!
//! - [`Verdict<P, N>`] — a three-valued result with a type-distinct
//!   [`Inconclusive`](Verdict::Inconclusive) carrying its [`Abstention`] reason.
//!   The decided variants carry [`Proof`]/[`Refutation`] wrappers whose field is
//!   private to this module, so `Verdict::Proven`/`Refuted` cannot be
//!   name-constructed from outside — the firewall is a type invariant, proven by
//!   the `compile_fail` doctests on the verdict module.
//! - [`Certificate`] — a witness whose `check` re-establishes its property
//!   against the *original* net (the trust boundary; not sealed).
//! - [`accept`] — the only public constructor of `Proven`/`Refuted`, gating on
//!   the certificate via the private [`Decided`] token; it is the sole site that
//!   fills the private `Proof`/`Refutation` field.
//!
//! # Why a re-export hub, not a relocation
//!
//! The per-property result types (`ReachabilityResult`, `CoverabilityResult`,
//! `BoundednessAnalysis`, `LivenessAnalysis`, …) stay with their analyses and
//! are *re-exported* here, rather than moved into one file. A prior god-module
//! that gathered five shapes into one place was a maintenance and cycle sink and
//! was deleted; relocation is a wide edit for no contract benefit. The contract
//! is the consolidated *surface*, not a consolidated *file*.
//!
//! # Scope (M1)
//!
//! M1 lands the contract types and makes `petrivet::model::*` resolve. The
//! per-certificate `check` *bodies*, the decider registry / polarity, the
//! certificate interchange format, and wiring `accept` into the `analyze_*`
//! return paths are later milestones (M2/M3). See
//! `docs/foundations/foundational-design.md` §F3″ for the decision record.

mod accept;
mod certificate;
mod checkers;
#[cfg(feature = "serde")]
mod format;
mod frontier;
mod ledger;
mod query;
mod registry;
mod verdict;

pub use accept::{accept, Decided};
pub use certificate::Certificate;
pub use checkers::{
    BoundednessSubinvariantCert, FiringSequenceCert, ParikhVectorCert,
    PlaceBoundednessSubinvariantCert, SiphonTrapClaim, SiphonTrapCoverCert,
    TokenConservationCert,
};
#[cfg(feature = "serde")]
pub use format::{
    Cert, NamedFiringWord, NamedParikhVector, NamedQuery, NamedSiphonTrapCover,
};
pub use frontier::{frontier_table, CheckComplexity, FrontierEntry, Polarity, Property};
pub use ledger::{trusted_base, Decider, DeciderKind, TrustedBaseLedger};
pub use query::Query;
pub use registry::{
    boundedness_driver, coverability_driver, deadlock_freedom_driver, liveness_driver,
    reachability_driver, BareBooleanCert, BoundednessProof, Budget, CostClass, Decide,
    DeadlockFreeWitness, DefaultPolicy, Driver, Policy, UnboundedWitness,
};
pub use verdict::{Abstention, Proof, Refutation, Verdict};

// --- Per-property result and proof shapes, re-exported from their analyses. ---
// These are the five heterogeneous result shapes consolidated under one surface.
pub use crate::api::system::boundedness::{BoundednessAnalysis, BoundednessAnalysisMethod};
pub use crate::api::system::chc::CommonerHackCriterionResult;
pub use crate::api::system::coverability::{
    CoverabilityProof, CoverabilityResult, NonCoverabilityProof,
};
pub use crate::api::system::liveness::LivenessAnalysis;
pub use crate::api::system::reachability::{
    ReachabilityProof, ReachabilityResult, UnreachabilityProof,
};

#[cfg(test)]
mod a3a_surface_sentinel {
    //! Regression sentinel for the re-scoped M1 `[LINT]` gate (BACKLOG.md, 2026-06-20):
    //! *"the `petrivet::model` names `petrivet-wasm` imports from the A1-landed surface resolve."*
    //!
    //! `petrivet-wasm` is excluded from `default-members` and cannot compile (it targets an
    //! M2/M5/M6 API — see A3b and §F3″), so its build cannot guard the A1 contract surface it
    //! depends on. This test names the *exact* subset of `petrivet-wasm/src/lib.rs`'s
    //! `use petrivet::model::{…}` that the A1 milestone is responsible for making resolve. If any
    //! of these contract re-exports is renamed or dropped, this test fails to **compile** — the
    //! executable form of the gate the wasm build would otherwise have caught.
    //!
    //! The two deliberately-omitted names — `LivenessMethod`, `DeadlockAnalysisMethod` (and the
    //! `LivenessLevel` wasm also reaches for) — are A3b, gated on A6/M2 capability; their absence
    //! is *correct* at M1, so they are not asserted here.
    #[allow(unused_imports)]
    use crate::model::{
        BoundednessAnalysisMethod, CoverabilityResult, NonCoverabilityProof, ReachabilityProof,
        ReachabilityResult, UnreachabilityProof,
    };

    /// A compile-and-run anchor so the imports above are load-bearing (a bare `use` in a test
    /// module can be elided by some lint configurations; touching the paths in a `const` keeps
    /// the name-resolution obligation real).
    #[test]
    fn a1_landed_model_surface_for_wasm_resolves() {
        // Name each type in a path position; if any re-export regresses, this stops compiling.
        const _: fn() = || {
            let _ = core::marker::PhantomData::<BoundednessAnalysisMethod>;
            let _ = core::marker::PhantomData::<CoverabilityResult>;
            let _ = core::marker::PhantomData::<NonCoverabilityProof>;
            let _ = core::marker::PhantomData::<ReachabilityProof>;
            let _ = core::marker::PhantomData::<ReachabilityResult>;
            let _ = core::marker::PhantomData::<UnreachabilityProof>;
        };
    }
}
