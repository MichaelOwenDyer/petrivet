# Rung 3 — A Planner over Certified Reductions
### The far end of the learning ladder: structural reductions as actions in a certifying AND/OR proof search

> Status: speculative spec / dream. This is the most ambitious rung — research, not a near-term build. The near-term value is all at Rung 1 (the measurement substrate and the ranker). The structural-reduction theory it leans on is Michael's domain. Read it as the spire you raise last, once the foundation tells you where it goes.

A numbering note, to head off a collision seeded earlier: the implementation **phases / branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) are about building the *measurement harness*; the **ladder rungs** are the learning ambition from [Soundness as a Free Variable](soundness-as-a-free-variable.md) — Rung 0 hardcoded, Rung 1 ranker, Rung 2 sequential policy, **Rung 3: the planner that treats the net itself as something to transform.** This document is the ladder's Rung 3.

---

## The move from Rung 2

At Rungs 1–2 the net is fixed and the only actions are *which decider, in what order*. Rung 3 adds a second verb. The action set becomes **decide ∪ transform**, and the net stops being a constant — it becomes a **shrinking residual**. The question is no longer "which technique decides N?" but "what sequence of simplifications and decompositions makes N *cheap* to decide, and how do I lift the proof back?"

This is the move AlphaGo made from "evaluate this position" to "search the tree of positions" — except our moves don't play a game, they *simplify a proof obligation*.

## The central new object: the certified reduction

Everything hinges on one trait, and on getting its third method right:

```rust
trait Reduction {
    fn applicable(&self, net: &ResidualNet) -> Option<Witness>;   // precondition + soundness witness
    fn apply(&self, net: &ResidualNet) -> Residual;               // the smaller (or split) net
    fn lift(&self, residual_cert: Certificate) -> Certificate;    // proof on residual  →  proof on original
}
```

`applicable` and `apply` are the obvious part. **`lift` is the keystone.** Because each reduction maps a certificate on the *reduced* net back toward the *original*, a chain of reductions ending in a decider produces, after lifting back up the chain, **a certificate on the original net** — checkable by the original, unchanged checkers. A firing sequence found on an agglomerated net lifts by re-expanding the fused steps; an unreachability invariant on a net with an implicit place removed lifts by re-padding the removed coordinate. The reductions are scaffolding that comes down at the end, leaving a proof that stands on the real building.

And here is the payoff that makes Rung 3 the convergence point of the entire first essay: **the reduction library is just the structural apparatus, re-cast as actions.** Each reduction's `applicable` witness is one of the certificates dreamed about all along —

| Reduction (action) | Its soundness witness is… | …which is concern |
|---|---|---|
| remove an implicit place | the **P-invariant / Farkas dual** that determines it | ② linear algebra (the *discarded LP dual finally gets a job*) |
| agglomerate a transition sequence | the **cluster / siphon-trap** structure licensing the fusion | ③ the structural island |
| split into independent sub-nets | the **NUPN-unit / S-component** factorization, Φ_PN-bounded | ⑥ composition |

The order engine (①) supplies the deciders; the certificates (④) are the proof terms; the structure (②③⑥) becomes the *moves*; the decision policy (⑤) is the search. Rung 3 is the loop that eats the whole apparatus.

## The firewall theorem, extended — and a lovely robustness

> For any policy over {certifying deciders} ∪ {certifying reductions}, the lifted-and-checked verdict on the original net is sound — because each reduction's applicability witness is checked, the residual certificate is checked, and the final lifted certificate is checked **against the original net**.

The beautiful consequence: **even a *buggy* `lift` cannot break soundness.** A wrong lift produces a certificate that fails the original-net checker, and the search simply backtracks — it has wasted time, not correctness. So the trusted base stays *exactly* the original certificate checkers; the entire reduction library, however elaborate, lives outside it. This is strictly stronger than where MuZero sits (its learned dynamics model is *inside* the trust boundary and only empirically sound). We get MuZero's "plan over a model of the problem" with a checker that disposes of every leaf.

## The search is AlphaZero / HTPS in form

Decompositions make this an **AND/OR proof-tree search**, which is precisely the shape of HyperTree Proof Search (Lample et al., 2022) and AlphaProof (2024):

| Theorem proving | Rung 3 |
|---|---|
| goal / conjecture | (residual net, property) to decide |
| a tactic | a certified reduction |
| subgoals a tactic spawns (AND-node) | sub-nets a *decomposing* reduction spawns |
| proof term | the lifted certificate chain |
| kernel type-checks the proof | the original-net checker checks the lifted certificate |
| policy / value over proof states | policy / value over residual nets |
| self-play over a theorem corpus | self-solving over the MCC corpus |

**OR-nodes** are "which decider or non-splitting reduction"; **AND-nodes** are decompositions (all sub-nets must be decided, then composed). The **value network** estimates *expected cost-to-proof* of a residual node — it learns to look at a net and feel how far it is from a cheap certificate. And note where Φ_PN reappears: it is the **applicability oracle for the split action.** A decomposition is a good move exactly when the property factors over the cut (the interface coupling is benignly dischargeable); Φ_PN measures how badly it fails to. So the value net is, in part, *learning to predict Φ_PN* — to recognize the cuts along which the net falls apart cleanly versus the integrated cores where it must pay the exhaustive price. The first essay's capstone quantity becomes a learned heuristic in the third paper's search.

## The MuZero frontier (and the line not to cross)

You *could* go fully MuZero: learn an approximate reduction-*dynamics* — predict a residual net's relevant features without actually computing the reduction — to plan many moves ahead cheaply. The honest discipline: **plan with the learned model for speed, but *execute* only real certified reductions whose witnesses are checked.** The learned model accelerates the search; the certified execution + final check preserve soundness. That is the bright line between MuZero (trusted learned model) and this (learned *guidance* over certified operations). It is the same firewall, one level up.

## Training, and why it stays cheap

Solving the corpus yields **proof trees** — sequences of reductions terminating in a decider, lifted. Distill the policy on the search's improved action distribution (visit counts) and the value on realized cost-to-proof, AlphaZero-style. The certificate is still the label; no oracle, no human. A curriculum *emerges* — small nets resolve in one or two moves and teach the value net the easy gradients, harder nets stack reductions. And it stays small-and-cheap for the Rung-1 reason, now doubly true: the learner is outside trust, so its errors cost search time, never answers. The Rung-1 measurement harness extends almost unchanged — it now logs proof *trees* and φ-features of *residual* nodes, and the torsor/differential principle survives intact (cost-to-proof differences are still origin-free).

## Where the dragons are (claim-honest)

This is the speculative far-end, and three things are genuinely hard:

1. **The `lift` functions are the real work, and they are structural Petri-net theory** — Michael's domain, not tooling. Each property-preserving reduction comes condition-laden, and getting agglomeration's lift right under liveness vs. reachability is subtle. (Mitigated by the robustness above: a wrong lift is *caught*, not trusted — but a lift that's wrong *often* just makes the search thrash.)
2. **Search blow-up.** Reductions compose in many orders; the value net taming that is a *hope*, not a theorem. If the value signal is weak, you've built an expensive way to wander.
3. **It's research, not a sprint.** The near-term value is all at Rung 1 — the measurement substrate, the ranker. Rung 3 is the cathedral's spire; you raise it last, and only because the foundation told you where it goes.

But the reason it's *worth* dreaming: the contest already names this. The MCC protocol's `Technique` enum the harness declares includes `StructuralReduction` right alongside `Explicit` and `Topological` — structural reduction is a recognized, first-class technique, currently never wired in. The classical behaviour-preserving reductions are well-established (Berthelot; agglomeration à la Haddad–Pradat-Peyre; implicit-place removal via P-invariants), and used in tools like Tina, TAPAAL, ITS-Tools, LoLA, and GreatSPN. Rung 3 is simply *learning to apply them well*, with every step checkable. The tool would come to prove things about concurrent systems the way a mathematician does — **simplify, decompose, then decide** — and hand you, at the end, a proof on the original net that needs nothing but a small checker to believe.

---

*Open sub-thread worth playing with: whether a decomposing reduction's value estimate can be made to literally compute a cheap Φ_PN lower bound, which would turn the integrated-information idea into a live heuristic inside the planner rather than a capstone observation.*
