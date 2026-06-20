# Essays and research notes

This folder contains analysis and design writing about `petrivet`: a study of its current structure and proposals for its development. These are research and design artifacts, not authoritative project direction and not part of the thesis. They were produced by Daniel Dyer with Claude by reading the codebase, and they distinguish throughout between three categories: what is implemented, what is specified but not yet implemented, and what is proposed or speculative. The core Petri-net theory is Michael's.

Reading order (each document builds on the previous):

1. **[Latent architecture](latent-architecture.md)** — an analysis of six structures in the codebase (the order/WQO, linear algebra, the free-choice subclasses, evidence, decision, composition) and the architecture they imply. Central observation: `literature.rs` documents a module layer that is referenced but not yet implemented, which makes the planned design explicit.
2. **[Soundness as a free variable](soundness-as-a-free-variable.md)** — the framework: a sound learned algorithm-selection policy over a certificate-gated set of deciders. Main result: over certificate-emitting deciders, soundness is independent of the selection policy, so a learned scheduler cannot affect correctness. The next three documents develop this result.
3. **[Rung 1 — empirical hardness ranker](rung-1-empirical-hardness-ranker.md)** — the near-term, highest-value step: a cost-sensitive per-instance decider selector. Tree models over structural features; sound by the theorem, with the caveat that performance is not automatically monotone.
4. **[Rung 2 — sequential policy](rung-2-sequential-policy.md)** — static selection becomes an adaptive, preemptive, anytime controller: a Pandora's-box index baseline, then offline reinforcement learning from the harness logs. Requires a cancellation mechanism the core currently lacks.
5. **[Rung 3 — certified reductions](rung-3-certified-reductions.md)** — structural reductions as actions in an AND/OR proof-tree search, each reduction certifying, so the learned transformations remain outside the trusted base.

Summaries and companion documents:

- **[Core principles](core-principles.md)** — a condensed statement of the four organizing principles and the Φ_PN capstone, cross-referenced to the codebase.
- **[petrivet in four principles](petrivet-in-four-principles.md)** — the longer-form treatment of the same four principles.
- **[For Michael](for-michael.md)** — a personal companion letter to the technical notes.
- **[Self-measurement harness plan](../self-measurement-harness-plan.md)** — the observability layer that produces the differential cost measurements the learned components consume.
- **[Foundational design](../foundations/foundational-design.md)** and **[implementation backlog](../foundations/foundations-backlog.md)** — the components the future codebase requires, and the dependency-sequenced plan to build them.

### Status and provenance

The code is real and verifiable; the thesis document is currently a template; and Integrated Information Theory (the Φ thread in these documents) is not present in the repository. It is used here only in its factorization-residual form and is explicitly separated from any claim about cognition. Where these documents propose future work, each proposal is tied to a specific todo, stub, or unresolved reference in the code.
