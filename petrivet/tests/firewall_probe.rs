//! The firewall, probed from *outside* the crate.
//!
//! This is the external-consumer regression for the M1 `[PROP]` gate: a
//! downstream crate (which this integration test stands in for) can construct an
//! abstaining `Verdict` freely, but has **no** path to a decided
//! `Proven`/`Refuted`. The adversarial review found that `pub enum Verdict {
//! Proven(P), .. }` left a public name-constructor open
//! (`petrivet::model::Verdict::Proven(payload)` compiled with no certificate);
//! the fix wraps the decided payloads in `Proof`/`Refutation` whose field is
//! private to `crate::model`. The negative (does-not-compile) half of the proof
//! lives in the `compile_fail` doctests on `api/model/verdict.rs`; this file is
//! the positive half — what an external consumer *can* still do.

use petrivet::model::{Abstention, Verdict};

/// An external consumer can still mint an abstention (it carries no proof
/// obligation), and it reads back as inconclusive — type-distinct from refuted.
#[test]
fn external_consumer_can_construct_inconclusive() {
    let v: Verdict<&str, &str> = Verdict::inconclusive(Abstention::Unbounded);
    assert!(v.is_inconclusive());
    assert!(!v.is_proven());
    assert!(!v.is_refuted());
    assert_eq!(v.abstention(), Some(Abstention::Unbounded));
}

/// An external consumer can *read* a decided verdict's witness through the
/// accessors, but cannot *forge* one — the only constructor of `Proof`/
/// `Refutation` is the private field-fill inside `accept`. (That the forge does
/// not compile is asserted by the `compile_fail` doctests; here we confirm the
/// read surface a real consumer needs is intact.)
#[test]
fn decided_verdict_accessors_are_public() {
    // We cannot build a Proven here (no public constructor — that is the point),
    // so we exercise the accessor contract on an inconclusive verdict: proof and
    // refutation are absent, abstention is present.
    let v: Verdict<u32, String> = Verdict::inconclusive(Abstention::NotApplicable);
    assert_eq!(v.proof(), None);
    assert_eq!(v.refutation(), None);
    assert_eq!(v.abstention(), Some(Abstention::NotApplicable));
}
