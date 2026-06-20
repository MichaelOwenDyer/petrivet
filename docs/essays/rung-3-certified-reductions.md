# Rung 3 — A Planner over Certified Reductions
### Structural reductions as actions in a certifying AND/OR proof search

> Status: This is a speculative specification, not a near-term implementation. It is the most ambitious of the rungs and is best understood as a research direction. The near-term value lies entirely at Rung 1 (the measurement substrate and the ranker). The structural-reduction theory this document depends on is Michael's domain. It is presented here as the final component to build, once the lower rungs indicate that it is warranted.

A numbering note, to avoid an earlier collision: the implementation **phases / branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) concern building the *measurement harness*; the **ladder rungs** are the learning stages from [Soundness as a Free Variable](soundness-as-a-free-variable.md) — Rung 0 hardcoded, Rung 1 ranker, Rung 2 sequential policy, **Rung 3: the planner that treats the net itself as something to transform.** This document describes the ladder's Rung 3.

---

## The move from Rung 2

At Rungs 1–2 the net is fixed and the only actions are *which decider, in what order*. Rung 3 adds a second class of action. The action set becomes **decide ∪ transform**, and the net is no longer constant — it becomes a **shrinking residual**. The question is no longer "which technique decides N?" but "what sequence of simplifications and decompositions makes N cheap to decide, and how is the proof lifted back to the original net?"

This parallels the transition AlphaGo made from "evaluate this position" to "search the tree of positions," with the difference that these moves do not play a game; they simplify a proof obligation.

## The central new object: the certified reduction

The design hinges on one trait, and in particular on its third method:

```rust
trait Reduction {
    fn applicable(&self, net: &ResidualNet) -> Option<Witness>;   // precondition + soundness witness
    fn apply(&self, net: &ResidualNet) -> Residual;               // the smaller (or split) net
    fn lift(&self, residual_cert: Certificate) -> Certificate;    // proof on residual  →  proof on original
}
```

`applicable` and `apply` are straightforward. **`lift` is the key method.** Because each reduction maps a certificate on the *reduced* net back toward the *original*, a chain of reductions ending in a decider produces, after lifting back up the chain, **a certificate on the original net** — checkable by the original, unchanged checkers. A firing sequence found on an agglomerated net lifts by re-expanding the fused steps; an unreachability invariant on a net with an implicit place removed lifts by re-padding the removed coordinate. The reductions function as scaffolding that is removed at the end, leaving a proof that holds on the original net.

This is where the reduction library connects to the rest of the system: **the reduction library is the structural apparatus, re-cast as actions.** Each reduction's `applicable` witness is one of the certificate types described elsewhere in the design —

| Reduction (action) | Its soundness witness is… | …which is concern |
|---|---|---|
| remove an implicit place | the **P-invariant / Farkas dual** that determines it | ② linear algebra (the LP dual is put to use here) |
| agglomerate a transition sequence | the **cluster / siphon-trap** structure licensing the fusion | ③ the structural component |
| split into independent sub-nets | the **NUPN-unit / S-component** factorization, Φ_PN-bounded | ⑥ composition |

The order engine (①) supplies the deciders; the certificates (④) are the proof terms; the structure (②③⑥) becomes the *moves*; the decision policy (⑤) is the search. Rung 3 is the loop that integrates the whole apparatus.

## The extended trust-boundary theorem, and a robustness property

> For any policy over {certifying deciders} ∪ {certifying reductions}, the lifted-and-checked verdict on the original net is sound — because each reduction's applicability witness is checked, the residual certificate is checked, and the final lifted certificate is checked **against the original net**.

The consequence: **even a buggy `lift` cannot break soundness.** A wrong lift produces a certificate that fails the original-net checker, and the search backtracks — it has wasted time, not correctness. The trusted base therefore remains *exactly* the original certificate checkers; the entire reduction library, however elaborate, lies outside it. This is strictly stronger than the position MuZero occupies (its learned dynamics model is *inside* the trust boundary and only empirically sound). The result is MuZero's "plan over a model of the problem" combined with a checker that verifies every leaf.

## The search is AlphaZero / HTPS in form

Decompositions make this an **AND/OR proof-tree search**, which is the same structure as HyperTree Proof Search (Lample et al., 2022) and AlphaProof (2024):

| Theorem proving | Rung 3 |
|---|---|
| goal / conjecture | (residual net, property) to decide |
| a tactic | a certified reduction |
| subgoals a tactic spawns (AND-node) | sub-nets a *decomposing* reduction spawns |
| proof term | the lifted certificate chain |
| kernel type-checks the proof | the original-net checker checks the lifted certificate |
| policy / value over proof states | policy / value over residual nets |
| self-play over a theorem corpus | self-solving over the MCC corpus |

**OR-nodes** represent "which decider or non-splitting reduction"; **AND-nodes** represent decompositions (all sub-nets must be decided, then composed). The **value network** estimates the *expected cost-to-proof* of a residual node — it learns to assess how far a net is from a cheap certificate. Note where Φ_PN reappears: it is the **applicability oracle for the split action.** A decomposition is a good move precisely when the property factors over the cut (the interface coupling is benignly dischargeable); Φ_PN measures how badly it fails to. The value net is therefore, in part, *learning to predict Φ_PN* — to recognize the cuts along which the net separates cleanly versus the integrated cores where the exhaustive cost must be paid. The capstone quantity from the first essay becomes a learned heuristic in the search.

## The MuZero frontier, and the line not to cross

A fully MuZero-style approach is possible: learn an approximate reduction *dynamics* — predict a residual net's relevant features without actually computing the reduction — to plan many moves ahead cheaply. The required discipline: **plan with the learned model for speed, but execute only real certified reductions whose witnesses are checked.** The learned model accelerates the search; certified execution plus the final check preserve soundness. This is the distinction between MuZero (a trusted learned model) and the present design (learned *guidance* over certified operations). It is the same trust boundary, one level up.

## Training, and why it stays cheap

Solving the corpus yields **proof trees** — sequences of reductions terminating in a decider, lifted. Distill the policy on the search's improved action distribution (visit counts) and the value on realized cost-to-proof, AlphaZero-style. The certificate remains the label; no oracle and no human labeling are required. A curriculum *emerges*: small nets resolve in one or two moves and teach the value net the easy gradients, while harder nets stack reductions. It remains small and cheap for the Rung-1 reason, now doubly applicable: the learner is outside the trusted base, so its errors cost search time, never answers. The Rung-1 measurement harness extends almost unchanged — it now logs proof *trees* and φ-features of *residual* nodes, and the torsor/differential principle remains intact (cost-to-proof differences are still origin-free).

## Risks (stated plainly)

This is the most speculative part of the design, and three things are genuinely hard:

1. **The `lift` functions are the real work, and they are structural Petri-net theory** — Michael's domain, not tooling. Each property-preserving reduction comes with conditions, and getting agglomeration's lift right under liveness versus reachability is subtle. (This is mitigated by the robustness property above: a wrong lift is *caught*, not trusted — but a lift that is wrong *often* causes the search to thrash.)
2. **Search blow-up.** Reductions compose in many orders; the value net controlling this is a *hope*, not a theorem. If the value signal is weak, the result is an expensive way to wander.
3. **It is research, not a sprint.** The near-term value is all at Rung 1 — the measurement substrate and the ranker. Rung 3 is the final component to build, and only because the foundation indicates where it goes.

The reason it is worth pursuing: the contest already names this technique. The MCC protocol's `Technique` enum that the harness declares includes `StructuralReduction` alongside `Explicit` and `Topological` — structural reduction is a recognized, first-class technique, currently never wired in. The classical behaviour-preserving reductions are well-established (Berthelot; agglomeration per Haddad–Pradat-Peyre; implicit-place removal via P-invariants), and are used in tools such as Tina, TAPAAL, ITS-Tools, LoLA, and GreatSPN. Rung 3 is simply *learning to apply them well*, with every step checkable. The tool would prove properties of concurrent systems by the standard method — **simplify, decompose, then decide** — and produce, at the end, a proof on the original net that requires only a small checker to verify.

---

*Open sub-thread worth investigating: whether a decomposing reduction's value estimate can be made to literally compute a cheap Φ_PN lower bound, which would turn the integrated-information idea into a live heuristic inside the planner rather than a capstone observation.*
