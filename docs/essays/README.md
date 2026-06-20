# Essays and research notes

This folder is the **vision corpus** for `petrivet` (a Rust Petri-net analyzer and the master's thesis it supports): a set of design essays and research notes that read the codebase, name the structures it already has, and argue toward where it could go. They were produced by Daniel Dyer with Claude, by reading the source, the comments, the tests, and the git history. The core Petri-net theory is Michael's; the proposed extensions are marked as such throughout, and each distinguishes between what is *implemented*, what is *specified but not yet built*, and what is *proposed or speculative*.

## The ratified inversion (read this first)

The corpus has been reconciled to a single, committed orientation, which inverts the emphasis an earlier draft carried:

- **The certificate-and-checker is the stone.** The signature technical contribution is an interoperable, machine-checkable certificate for each verdict, re-validated by a *small external checker that is the entire trusted base*.
- **The structural-coverage claim is the falsifiable headline.** The empirical, testable claim is `f_struct`: on the real MCC P/T corpus, a polynomial structural certifying tier decides a large, characterizable fraction of *queries decided* without state-space exploration, and where it abstains, it abstains honestly. This is the number the thesis lives or dies by.
- **The soundness firewall is the enabling property, not the headline.** "Soundness is independent of the selection policy" is, as a theorem, a one-line corollary of certifying algorithms composed with algorithm selection; its real content is a *precondition* the code must first discharge (the two `Some(false)` stubs). The contribution is the firewall **built and measured**, with the certifying fraction `f` as its figure of merit.
- **Learned selection and the factorization residuals are the sequel and the horizon.** The SATzilla-style learned-selection ladder is the deferred next move, safe to defer precisely because the certificate makes mis-selection cost time and never correctness. The scalar net-level `Φ_PN` is **dissolved** to two honest, computable, per-property residuals, whose *measurement* — not their metaphysics — is the deliverable.

**Status — non-authoritative.** These essays are explicitly **not** the thesis proper and **not** project direction. They are vision and research artifacts, authored *toward* the thesis author, inviting contradiction. The authoritative, engineering-sequenced plan is [`BACKLOG.md`](../../BACKLOG.md); where the backlog and an essay disagree, the backlog's ratified position governs. The code is real and verifiable; the thesis document is at present a template; Integrated Information Theory is **not** present in the repository and is used here only in its factorization-residual form, fenced off from any claim about cognition.

## Reading order — the new spine

The order below follows the ratified spine: the enabling property, then the signature contribution, then the falsifiable headline, then the architecture and the dissolved residuals, then the sequel, then the companion letter.

1. **[Core principles](core-principles.md)** — the four organizing principles of the library (the substrate, the order, the structural shortcuts, the proof-carrying contract), cross-referenced to the codebase. The capstone is now read as the two measured residuals rather than a single scalar. (Its longer, more legible reading companion is **[petrivet in four principles](petrivet-in-four-principles.md)**.)
2. **[Soundness as a free variable](soundness-as-a-free-variable.md)** — the firewall, stated claim-honestly: the soundness theorem is nearly trivial, and that is the point — it relocates the work to the precondition (every fast decider must certify) and to the figure of merit `f`. This is the *enabling property*, not the headline.
3. **[The checkable frontier](the-checkable-frontier.md)** — the certificate-and-checker as the signature contribution: a small independent checker re-validating every verdict against the original net, an interoperable certificate format, and the **hardness map** of where compact checkable certificates exist and where complexity theory forbids them.
4. **[The coverage claim](the-coverage-claim.md)** — the falsifiable empirical headline: `f_struct`, the fraction of queries decided by the structural tier, measured two-denominator and family-held-out, with its explicit falsifiers (the fraction is small; the structural path is not cheaper; the certificates are not independently checkable).
5. **[The latent architecture](latent-architecture.md)** — the ratified architecture and the trust boundary: the six structures already in the codebase, the `literature.rs` blueprint of what is planned but unbuilt, and where the line between the trusted base and everything else is drawn.
6. **[The factorization residual](the-factorization-residual.md)** — `Φ_PN` dissolved: the single net-level scalar and its necessity claim are retired, and what survives is two computable, monotone, theorem-backed-zero, per-property residuals (Φ_bound, Φ_inv), whose corpus *measurement* is the deliverable.
7. **The learned-selection sequel** — the SATzilla-style ladder, strictly downstream of the certificate and gated so a mis-selection costs performance, never soundness:
   - **[Rung 1 — the empirical hardness ranker](rung-1-empirical-hardness-ranker.md)** — cost-sensitive, per-instance decider selection over structural features; justified only if a measured SBS→VBS gap warrants it.
   - **[Rung 2 — the sequential policy](rung-2-sequential-policy.md)** — static selection becomes an adaptive, preemptive, anytime controller; requires a cancellation seam the core currently lacks.
   - **[Rung 3 — a planner over certified reductions](rung-3-certified-reductions.md)** — structural reductions as actions in an AND/OR proof search, each lift re-checked against the original net so the transformations stay outside the trusted base.
8. **[For Michael](for-michael.md)** — the companion letter to the technical notes: the same findings told personally, with the work itself Michael's. Its own guidance — *start there: the two `Some(false)` stubs* — still holds, and is the near-term north star of the whole program.

## The bridge to the engineering plan

The **foundations** pair carries the corpus across from vision into the sequenced build:

- **[Foundational design](../foundations/foundational-design.md)** — the components the future codebase requires (the `Verdict`/`Certificate` contract, the checker, the decider registry, the measurement domain).
- **[Foundations backlog](../foundations/foundations-backlog.md)** — the dependency-ordered plan to build them.

These hand off to the authoritative engineering backlog, [`BACKLOG.md`](../../BACKLOG.md), whose epics realize the spine above: the trust boundary (Epic A/C), the structural generators that widen coverage (Epic B), the measured headline `f_struct`/`f` (Epic G), the learned-selection sequel (Epic D), the certified reductions (Epic F), and the dissolved residuals (Epic H). The companion **[self-measurement harness plan](../self-measurement-harness-plan.md)** is the experimental rig that produces the headline numbers.
