# Essays & research notes

Exploratory writing about `petrivet` — its latent structure and its future potential. These are **vision/research artifacts**, not authoritative project direction and not part of the thesis proper. They were produced collaboratively (Daniel Dyer, with Claude) by studying the codebase, and they try hard to stay claim-honest: each marks what is *implemented* vs. *charted-but-unbuilt* vs. *dreamed/metaphorical*, and defers the core Petri-net theory to Michael's thesis work.

Read them as a sequence — each descends from the last.

1. **[The Charted Cathedral](charted-cathedral.md)** — a study of six structures in the codebase (order/WQO, linear algebra, the free-choice island, evidence, decision, composition) and the latent architecture they point at. The central observation: `literature.rs` is a blueprint drawn ahead of the stone, so "future potential" here is unusually legible.
2. **[Soundness as a Free Variable](soundness-as-a-free-variable.md)** — a paper on a *sound learned algorithm-selection policy* over the certifying decider lattice. Core result: over certificate-emitting deciders, soundness is independent of the selection policy, so machine learning can schedule without ever touching correctness. The AlphaGo move, with verified leaves.
3. **[Rung 3 — a planner over certified reductions](rung-3-certified-reductions.md)** — the far end of the learning ladder: treat structural reductions as actions in an AND/OR proof-tree search, each reduction certifying, so even the learned transformations stay outside the trusted base. The MuZero-grade dream, where the whole structural apparatus becomes the action set.

Companion implementation plan (lives at the base of the implementation stack, not here): **[Self-Measurement Harness — Rung 1 Implementation Plan](../self-measurement-harness-plan.md)**.

### A note on status and provenance
The code is real and carefully reasoned; the *thesis document* is currently scaffolding; and Integrated Information Theory (the Φ thread that recurs in these essays) appears nowhere in the repository — it is a lens brought from outside and developed here only in its rigorous, factorization-residual form, fenced off from any claim about minds. Where these essays dream, the dreams have coordinates: a `todo`, a stub, a dangling `literature.rs` doc-link, or a named-but-absent module that the existing code already points at.
