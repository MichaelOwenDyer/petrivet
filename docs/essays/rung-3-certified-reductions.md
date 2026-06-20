# Rung 3 — A Planner over Certified Reductions

### Structural reductions as actions in a certifying AND/OR proof search

> Status: speculative specification, the most deferred rung of all. The third rung of the **learning ladder** — the *sequel* to petrivet's certificate-and-checker, not its spine. It is the most ambitious of the rungs, best understood as a research direction; the near-term value is entirely at the certificate, the coverage map, and (if its gap is real) Rung 1. The structural-reduction theory this depends on is Michael's domain. It is presented as the final component to build, once the lower rungs indicate it is warranted.

A numbering note, to avoid an earlier collision: the implementation **branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the **ladder rungs** are the learning stages from [Soundness as a Free Variable](soundness-as-a-free-variable.md) — Rung 0 hardcoded, Rung 1 ranker, Rung 2 sequential policy, **Rung 3: the planner that treats the net itself as something to transform.** This document describes the ladder's Rung 3.

---

## Where this sits

As with the lower rungs, two facts hold. The thesis is the **coverage map and the certificate-and-checker**; this planner is the far end of the sequel. It is **safe to defer** because the firewall holds soundness independent of any policy — and, as sharpened below, independent of any *reduction*, with one honest caveat. It is **honest to defer** because the contribution is the certificate framework, not the search. Rung 3 is the last thing built, and only if the lower rungs justify it.

## The move from Rung 2

At Rungs 1–2 the net is fixed and the only actions are *which decider, in what order*. Rung 3 adds a second class of action. The action set becomes **decide ∪ transform**, and the net is no longer constant — it becomes a **shrinking residual**. The question is no longer "which technique decides N?" but "what sequence of simplifications and decompositions makes N cheap to decide, and how is the proof lifted back to the original net?"

This is the standard method by which tools verify concurrent systems — **simplify, decompose, then decide** — recast so that the choice of which simplification to apply becomes a learned policy, and every step remains checkable. It is not a game; the moves do not play against an opponent, they discharge a proof obligation.

## The central new object: the certified reduction

The design hinges on one trait, and in particular on its third method:

```rust
trait Reduction {
    fn applicable(&self, net: &ResidualNet) -> Option<Witness>;   // precondition + soundness witness
    fn apply(&self, net: &ResidualNet) -> Residual;               // the smaller (or split) net
    fn lift(&self, residual_cert: Certificate) -> Certificate;    // proof on residual  →  proof on original
}
```

`applicable` and `apply` are straightforward. **`lift` is the key method.** Because each reduction maps a certificate on the *reduced* net back toward the *original*, a chain of reductions ending in a decider produces, after lifting back up the chain, **a certificate on the original net** — checkable by the original, unchanged checkers. A firing sequence found on an agglomerated net lifts by re-expanding the fused steps; an unreachability invariant on a net with an implicit place removed lifts by re-padding the removed coordinate. The reductions are scaffolding, removed at the end, leaving a proof that holds on the original net.

This is where the reduction library connects to the rest of the system: **the structural apparatus, re-cast as actions.** Each reduction's `applicable` witness is one of the structural certificate types built in Epic B —

| Reduction (action) | Its applicability witness is… | …a certificate from |
|---|---|---|
| remove an implicit place | the **P-invariant / Farkas dual** that determines it | the LP dual (B1) — *finally getting a job* |
| agglomerate a transition sequence | the **cluster / siphon-trap** structure licensing the fusion | the structural component (B2/B7) |
| split into independent sub-nets | the **NUPN-unit / S-component** factorization | composition (B8/B3) |

The implicit-place case is the clean motivating example: the Farkas dual that today's LP computes and *discards* on the infeasible path (the `MarkingEquationNoRationalSolution` payload thrown away at `reachability.rs:177`) is exactly the witness that a place is implicit — so the discarded dual finally gets a job as a reduction's applicability proof. The order engine supplies the deciders; the certificates are the proof terms; the structure becomes the *moves*; the policy is the search. Rung 3 is the loop that integrates the whole apparatus.

## The trust boundary, and an honestly-bounded robustness property

> **Soundness (the firewall, extended).** For any policy over {certifying deciders} ∪ {certifying reductions}, the lifted-and-checked verdict on the original net is sound — because the residual certificate is checked, and the final lifted certificate is checked **against the original net** by the unchanged checkers.

The trusted base therefore remains *exactly* the original certificate checkers; the entire reduction library, however elaborate, lies outside it. From this follows a robustness claim — **a buggy `lift` cannot break soundness** — but the claim must be stated with a sharp line through it, because it does not hold uniformly:

- **For existential witnesses (firing words), it holds cleanly.** A lifted firing sequence is checked by *replay* on the original net: the checker fires the lifted word from the initial marking and confirms it reaches the target. A wrong `lift` produces a word that does not replay; the checker rejects it; the search backtracks. There is no way for a malformed existential witness to pass — the check re-establishes the property from scratch. For this class the robustness property is a theorem, and a buggy lift costs only time.

- **For compositional / invariant lifts, it is an open per-certificate obligation, not yet discharged.** Here the lifted object is not replayed but re-checked against an algebraic condition (e.g. a place-invariant inequality, or an interface correction recombining sub-net invariants). The hazard is **checker-completeness**: a buggy `lift` could emit a *too-weak* certificate that passes a too-weak check — an interface correction that drops a coupling term, say, satisfying the local invariant condition while failing to re-establish the global property. The original-net check catches a malformed witness only if the check is *complete* for that property — strong enough that nothing but a true witness passes it. Whether each compositional checker has this completeness is a **per-certificate-kind proof obligation** (backlog **F1/F2**), and it is *not* yet discharged.

**The ratified discipline, therefore: trusted lifts are restricted to existential witnesses until checker-completeness is proven for the compositional kinds.** Existential reductions (those whose lifted certificate is a firing word, replayed) may be wired into the planner now and trusted to be robust. Compositional reductions (implicit-place removal's invariant lift, agglomeration under liveness, the split's interface correction) may be *implemented and tested*, but their robustness is an obligation to be proven before they are trusted as fully outside the trusted base — until then the matching checker is, for those kinds, part of the trusted base and must be audited as such. This is strictly weaker than the unqualified "even a buggy lift is caught" of an earlier draft, and it is the honest position.

## The search is an AND/OR proof tree

Decompositions make this an **AND/OR proof-tree search**, the same structure as HyperTree Proof Search (Lample et al., 2022) and AlphaProof (2024):

| Theorem proving | Rung 3 |
|---|---|
| goal / conjecture | (residual net, property) to decide |
| a tactic | a certified reduction |
| subgoals a tactic spawns (AND-node) | sub-nets a *decomposing* reduction spawns |
| proof term | the lifted certificate chain |
| kernel type-checks the proof | the original-net checker checks the lifted certificate |
| policy / value over proof states | policy / value over residual nets |
| self-solving over a theorem corpus | self-solving over the MCC corpus |

**OR-nodes** represent "which decider or non-splitting reduction"; **AND-nodes** represent decompositions (all sub-nets must be decided, then composed). The **value network** estimates the *expected cost-to-proof* of a residual node — how far a net is from a cheap certificate. The factorization residual Φ_PN reappears here as the **applicability oracle for the split action**: a decomposition is a good move precisely when the property factors over the cut (the interface coupling is benignly dischargeable), and Φ_PN measures how badly it fails to. The value net is therefore, in part, *learning to predict Φ_PN* — to recognize the cuts along which the net separates cleanly versus the integrated cores where the exhaustive cost must be paid. The capstone quantity becomes a learned heuristic in the search. (Φ_PN here is the **factorization-residual mathematics** of [Core Principles](core-principles.md) — a number and a witness measuring failure-to-factor — and nothing about minds; IIT is not in the repository.)

This is a proposer–checker architecture: learned *guidance* over *certified operations*, with a kernel-style check at every leaf. The discipline that keeps the learned search outside the trusted base is the same one stated above — plan with a learned model for speed if desired, but execute only real certified reductions whose lifted certificates are checked against the original net, subject to the existential/compositional caveat. (A faster variant could learn an approximate reduction *dynamics* — predicting a residual's features without computing the reduction — to look ahead cheaply; the same rule applies: the learned model accelerates the search, certified execution plus the final check preserve correctness for the existential class and are an open obligation for the compositional class.)

## Training, and why it stays cheap

Solving the corpus yields **proof trees** — sequences of reductions terminating in a decider, lifted. Distill the policy on the search's improved action distribution (visit counts) and the value on realized cost-to-proof. The certificate remains the label; no oracle and no human labeling are required. A curriculum *emerges*: small nets resolve in one or two moves and teach the value net the easy gradients, while harder nets stack reductions. It stays small and cheap for the same reason as Rung 1, now doubly applicable: the learner is outside the trusted base, so its errors cost search time, never answers (for the existential class outright; for the compositional class, modulo the checker-completeness obligation). The harness extends almost unchanged — it now logs proof *trees* and φ-features of *residual* nodes, and the torsor/differential principle remains intact (cost-to-proof differences are still origin-free).

## Risks (stated plainly)

This is the most speculative part of the design, and four things are genuinely hard:

1. **The `lift` functions are the real work, and they are structural Petri-net theory** — Michael's domain, not tooling. Each property-preserving reduction comes with conditions, and getting agglomeration's lift right under liveness versus reachability is subtle. Mitigated *for the existential class* by replay-completeness; *for the compositional class* it is the open obligation of the previous section, and a lift that is wrong *often* causes the search to thrash even where it is caught.
2. **Checker-completeness for compositional lifts is unproven.** Until each invariant/interface checker is shown complete enough that a too-weak certificate cannot pass, those reductions' robustness is an obligation, not a theorem, and the matching checker is in the trusted base. This is the falsifier with the sharpest teeth: *a buggy compositional lift could pass a too-weak check and yield an unsound verdict* — which is exactly why such lifts are not trusted until the obligation is discharged.
3. **Search blow-up.** Reductions compose in many orders; the value net controlling this is a *hope*, not a theorem. If the value signal is weak, the result is an expensive way to wander.
4. **It is research, not a sprint.** The near-term value is all at the certificate and the coverage map. Rung 3 is the final component, built only because the foundation indicates where it goes.

The reason it is worth pursuing: the contest already names this technique. The MCC protocol's `Technique` enum the harness declares includes `StructuralReduction` alongside `Explicit` and `Topological` — a recognized, first-class technique, currently never wired in (indeed mis-tagged on the CHC path today; backlog **F0**). The classical behaviour-preserving reductions are well-established (Berthelot; agglomeration per Haddad–Pradat-Peyre; implicit-place removal via P-invariants), used in Tina, TAPAAL, ITS-Tools, LoLA, and GreatSPN. Rung 3 is *learning to apply them well*, with every step checkable — and, for the existential class, provably robust against its own bugs. The tool would prove properties of concurrent systems by the standard method — simplify, decompose, then decide — and produce, at the end, a proof on the original net that a small checker verifies.

---

*Open sub-thread worth investigating: whether a decomposing reduction's value estimate can be made to literally compute a cheap Φ_PN lower bound, turning the factorization-residual idea into a live heuristic inside the planner rather than a capstone observation.*
</content>
