# The Working Doctrine — how to engage petrivet

*Read this before touching the backlog or the codebase. It is the contract for how agents (and humans) work here — the subtext beneath every item in [`BACKLOG.md`](BACKLOG.md). The principles mirror the codebase's own epistemic law, deliberately: **work on petrivet the way petrivet works on nets — compute the reason, not just the answer; believe only the checked witness; "cannot yet decide" is a valid result.***

1. **Falsifiability first.** Every claim names its falsifier and its cheapest test. Label strength honestly — observed / inferred / proposed. Demote near-trivial results to their true standing (a theorem that is a one-line corollary is reported as one). Prefer a runnable measurement to an assertion.

2. **Soundness before capability.** Fix trusted-but-wrong before adding reach — A2 (the two `Some(false)` stubs) is the north star. Abstention is honest; a fabricated verdict is the cardinal sin. Floating-point is a soundness concern, not a convenience.

3. **The trust boundary is sacred.** For any change to analysis, the load-bearing question is: does the certificate `check` against the *original* net, sharing no code with the generator? Never enlarge the trusted base silently. The thesis collapses to this one invariant — protect it above features.

4. **Let Rust carry the invariants.** Make illegal states unrepresentable — the type system is the cheapest checker. A `Proven` verdict constructible only through a passing `check`; `polarity` and `admissible` on the `Decider` trait; newtype handles over raw indices; `#[must_use]` on verdicts; exhaustive matches; ownership marking the trust boundary; sealed traits where a closed set is meant. The compiler should refuse the bug before the test does. Honor the pedantic/nursery clippy contract `lib.rs` already opts into.

5. **Discernment over deference.** The docs — including this vision corpus — are input, not authority. The strongest defensible position wins, even against the docs' own lean. Red-team your own proposal; report what survives, and in what weakened-but-defensible form.

6. **Measure, don't assert.** The thesis is two numbers — `f_struct` (structural coverage) and `f` (certifying fraction) — not a theorem. Costs are differential (the torsor discipline: rankings and log-ratios, never raw wall-time across machines). The certificate is the label.

7. **The blueprint is real.** `literature.rs`'s dangling links (`structural::*`, `Invariants`, `SComponent`, `crate::model`) are a self-authored spec to *build*, not doc-debt to suppress. Every backlog item is gated by a **provable invariant**, not a representative example — a milestone is done when its invariant is proven (`[PROP]`/`[ORACLE]`/`[REGRESS]`/`[LINT]`), not when one case passes.

8. **Keep the rationale with the work.** When a finding becomes an item, record the discernment behind it — the *why*, the complexity bound, the rejected alternative — in [`foundations/foundational-design.md`](docs/foundations/foundational-design.md), never only in a transcript. An item without its reason is a decision no successor can re-derive.

9. **Lean.** State things once; reference, don't re-derive. Trim, don't bloat. The passion goes in the ideas; the discipline goes in the claims — sing in tune with the mathematics.
