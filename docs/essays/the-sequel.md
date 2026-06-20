# The learned-selection sequel

### A three-rung program for learned selection over the certifying portfolio — the deferred sequel to the certificate-and-checker

> Status: design specification, deliberately deferred. Exploratory work (Daniel Dyer, with Claude) on Michael's tool; planned-but-unbuilt items are marked as such. The three rungs below are the **learning ladder** of [Soundness as a Free Variable](soundness-as-a-free-variable.md) — the *sequel* to petrivet's signature contribution, not its spine.

---

## Why the ladder is a sequel, and why deferring it is both safe and honest

The thesis of petrivet is not learned selection. It is an **empirical coverage map** — on the real MCC P/T corpus, a polynomial structural certificate decides a large, characterizable fraction of queries without state-space exploration, and where it abstains, it abstains honestly — carried by the **certificate-and-checker**: every verdict ships a machine-checkable proof object, re-validated by a small external checker that is the entire trusted base. Learned selection is the layer that comes *after* that stone is laid.

Deferring it is **safe** because of one structural fact, the **firewall**: over a portfolio of certifying deciders, soundness is independent of the selection policy. The theorem and its proof live in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the full inversion that motivates the whole program is in [README.md](README.md) and is not re-derived here. Any scheduler — hand-written, learned, or adversarial — can only change *which proofs are attempted and in what order*; it can never cause an unchecked verdict to be accepted. Mis-selection costs time, not correctness.

Deferring it is also **honest**, because the contribution is the map and the checker, not the ranker. So the program below is built last, after the certificate, the checker, and the coverage map — and only as far as each rung's measured payoff justifies. Each rung carries its own falsifier and is structured to stop at the simplest formulation the data supports.

A note on the lineage. This is the **SATzilla/Rice** tradition of algorithm selection (Rice, 1976; Xu–Hutter–Hoos–Leyton-Brown, 2008), and nothing more exotic. An earlier draft reached for an AlphaGo/MuZero analogy; that framing is **dropped**. petrivet has *checkable leaves* — every decider that concludes emits a proof a checker re-validates — which makes its selection problem strictly easier than game-tree search, where a leaf evaluation is an unverified guess that can lose the game. The effective-theory / cellular-automata material that appears elsewhere is **clearly-labeled speculation** about *why* instance-hardness might be learnable at all; it is never load-bearing for any design decision here.

A numbering note. The implementation **branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the **ladder rungs** below describe the learning program and consume what the harness produces. Rung 0 is the current hardcoded cascade.

---

## Rung 1 — the empirical hardness ranker

**The first experiment is to measure the gap, and it is allowed to say no.** Before any model, measure the SBS→VBS gap on the corpus. The **single best solver** (SBS) is the one fixed decider or schedule that is best on average — a well-tuned Rung-0 cascade approximates it, and it is the *floor* a ranker must beat. The **virtual best solver** (VBS) is the oracle that always picks the per-instance best decider — the unreachable *ceiling*. Rung 1's entire value is the fraction of that gap it closes.

The decisive falsifier sits at this very first step: **if the measured gap on the ~6-arm, class-gated portfolio is within noise, there is nothing for a ranker to win** — Rung-0 is the honest answer and the model is dead weight. The gap is measured first, reported as a number with its noise band, and only a gap clearing a pre-registered threshold justifies the model. This is the cheapest decisive experiment in the ladder.

**What is learned, if anything.** Suppose the gap clears the threshold. The object is the **empirical hardness model** (Leyton-Brown–Nudelman–Shoham 2002/2009; Hutter–Xu–Hoos–Leyton-Brown, *AIJ* 2014): per decider `d`, learn a model `ĉ_d(φ(N))` of its cost-to-certificate and its probability of concluding, then **rank deciders by predicted cost** and schedule in that order. The output is a predicted cost *vector* — simultaneously a ranking, a fallback order, and a confidence signal — preferred over direct `argmax` classification, which discards cost structure. The vector is a section of the harness's **cost-torsor bundle**: per-decider costs defined only up to a global additive shift (machine speed), so the model needs no absolute times, only the differential structure the harness persists as `FitnessComparison`.

**The objective is cost, not accuracy.** Choosing the truly-fastest decider on the most *instances* is the classic failure mode: a slightly-slower-but-cheap pick costs almost nothing, a diverging exploration costs a great deal. The training objective is **cost-sensitive regret**, `cost(chosen) − cost(oracle-best)`, minimized in expectation. The regret is a difference — origin-free, machine-invariant, exactly the torsor-quotient quantity the harness stores — and it is the same quantity as the SBS→VBS gap: first measured at the gate, then minimized.

**The model class.** Structural macro-features (structural class, strong-connectivity, P/T-invariant dimension, S-/T-component and siphon/trap counts, NUPN unit-tree shape, token-sum and concurrency summaries) over **gradient-boosted trees or random forests** — the SATzilla lineage, not neural networks: minutes to train on commodity hardware, robust on heterogeneous tabular features, no GPU. The trees also yield interpretable feature importances, so the ranker doubles as an instrument for charting the hardness landscape. A feature earns its place only if its mutual information with the hardness label is high at low description length — tested against the corpus, not assumed.

**Soundness in action.** The firewall applies unchanged: the ranker only *reorders* admissible deciders, the certificate gate is untouched, and on a wrong top pick the schedule falls through in predicted-cost order to the exhaustive backstop. A wrong ranker wastes time; it cannot produce a wrong answer. The one precondition (backlog **A2/A6**): the guarantee covers *certifying* deciders, so until the bare-boolean shortcuts and the two `Some(false)` stubs emit certificates, the ranker either schedules only certificate-emitting deciders or treats those two as trusted with their known caveats. The stubs — not the machine learning — are the real soundness risk.

**Performance is not provably monotone**, and the mitigations make it nearly so: (1) always run the near-free structural shortcuts first, unconditionally, so the ranker only orders the costly tail (LP vs. ILP vs. exploration); (2) cap the first pick with a static budget; (3) blend the learned ranking with the Rung-0 hand-order as a prior, so the worst case degrades toward Rung 0. The realistic guarantee: sound always, and in expectation no worse than Rung 0 with a bounded downside.

**Training and evaluation.** Offline, supervised, self-labeling on the harness's `Observation` / `FitnessComparison` JSONL — the accepted certificate *is* the label, so no oracle is required (the MCC oracle remains an independent cross-check, never a leaderboard). **Split by net *family*, not by instance**: a random split leaks family structure and inflates measured generalization. Evaluate as the fraction of the SBS→VBS gap closed on held-out families, reported as origin-free cost-ratios, with the soundness sentinel running throughout. The artifact is a few hundred KB of trees, loaded through the `Policy::next` seam, **outside the core**: a downstream, optional, feature-gated leaf that `analyze_*` never depends on.

**Falsifier.** The learned ranker does not beat the obvious ordering — on a 6-arm, class-gated portfolio, plausibly so. The lesser risks (distribution shift, cold start, family leakage, feature sufficiency, the stubs precondition) are all *performance* risks; none touches correctness.

---

## Rung 2 — the sequential policy

Rung 1 is one-shot and static: it reads `φ(N)`, commits to a cost-ordered schedule, and falls through on a miss. Rung 2 closes the loop into an **adaptive controller** that re-decides after each inconclusive return and while a long-running decider consumes budget without converging. The new operation is not "transform" (that is Rung 3) — the net stays fixed — it is **preempt**: start a decider, monitor it, abandon it for a better option when the evidence says it is unlikely to conclude. The motivating case is in the code today: a bounded-but-large Karp–Miller exploration or a branch-and-bound ILP can run well past where a cheaper decider would have concluded, and neither Rung 0 nor Rung 1 can detect and stop it.

**The formal object** is a sequential decision process over states `(φ(N), attempt history with costs, remaining budget)` and actions `{start, continue, abandon, allocate}`, reward `−cost`, minimizing expected time to an accepted certificate. Three formulations, in increasing ambition — **use the simplest the data supports**:

1. **Index / optimal-stopping baseline (no learning) — start here.** Choosing which decider to open next, paying an inspection cost to find a prize, is a *Pandora's-box* problem with known optimal structure (Weitzman, 1979): under independence the optimal policy is an index rule from each decider's predicted `(cost, P(conclude))`. Fed the Rung-1 predictions, it gives a principled, learning-free adaptive order with optimal abandonment. (Restart/timeout scheduling — Luby–Sinclair–Zuckerman; max-`k`-armed restart bandits — Streeter–Smith — are the same family.)
2. **Contextual bandit (intermediate).** Treat "commit to `d`" as an arm with context `φ(N)`, learn from online regret. Selection under feedback, but no mid-run preemption.
3. **Conservative offline RL (the target, only where warranted).** Model sequencing *and* preemption/escalation as an MDP over the full action space — the only formulation that learns *when* to abandon a diverging exploration (the dynamic-algorithm-portfolio problem, Gagliolo–Schmidhuber, 2006). Justified only where preemption timing demonstrably depends on rich state; absent that, the index rule is the answer and the RL is dead weight.

**The cancellation prerequisite (backlog D6 — the hard gate).** Rung 2 cannot exist without infrastructure the codebase lacks today. The only early-stop mechanisms are the ω short-circuit and process-level `catch_unwind`; there is no deadline, no budget, no way to interrupt a running exploration or a `microlp` solve and reclaim the time. An adaptive policy needs a **cooperative cancellation token** threaded into the exploration loop (check-and-yield at frontier steps, ideally with a checkpoint to resume rather than restart) and a **bounded LP/ILP** call. This is the one place Rung 2 reaches into the core, and it does so in a certifying manner: a cancelled decider returns `⊥`, which the trust boundary already handles. **Cancellation changes *when* a decider stops, never *what* is accepted** — its correctness is a robustness concern, never a soundness one.

**Anytime parallel racing.** Because every decider is a pure function of the net, the cheap ones run concurrently — first accepted certificate wins, the rest are cancelled (parallel scheduling, `aspeed`/SUNNY). This yields the **anytime** property: at any interruption the controller returns the best certificate found so far, or `Inconclusive`, never a wrong answer and never nothing-because-still-computing. Race when predicted costs are close or uncertain; run sequentially under a confident favorite, since racing has real overhead.

**Soundness over the richer action space.** The firewall extends without modification: preemption accepts nothing from an abandoned decider; racing still runs `check` against the original net on the winner; continue/allocate only move budget. Even a buggy preemption is at worst a resource leak or a hang (caught by the outer deadline), not an unsound verdict — see [Soundness as a Free Variable](soundness-as-a-free-variable.md).

**Training** is offline/off-policy from the harness's `(φ, decider, outcome, cost)` trajectories — fitted-Q or, preferably, **conservative offline RL** (CQL, Kumar et al. 2020) to control extrapolation error, no live experimentation required. **Bootstrap from Rung 1**: initialize with the static order as prior so Rung 2 learns only the adaptive corrections; because it can always reproduce Rung 1's schedule, a correctly trained Rung 2 dominates it in expectation. Reward (time-to-accept, optionally shaped against late abandonment) may be misspecified freely — by the firewall a wrong reward yields a slow policy, never a wrong verdict.

**Falsifier.** Adaptivity does not beat static selection — the index rule over Rung-1 predictions captures essentially all the gain, and the offline-RL controller adds machinery and extrapolation risk without a measured improvement on held-out families. Compounded with Rung 1's own falsifier and the hard cancellation prerequisite, the honest prior is that this rung earns the *index policy* and may never earn the *RL*. Lesser risks (extrapolation error, parallel contention, abandonment-timing reward shaping, from-scratch underperformance, distribution shift) are all performance risks.

---

## Rung 3 — the planner over certified reductions

> Status: speculative specification, the most deferred rung of all — a research direction, not a sprint. The structural-reduction theory it depends on is Michael's domain.

At Rungs 1–2 the net is fixed and the only actions are *which decider, in what order*. Rung 3 adds a second class of action: the action set becomes **decide ∪ transform**, and the net becomes a **shrinking residual**. The question is no longer "which technique decides N?" but "what sequence of simplifications and decompositions makes N cheap to decide, and how is the proof lifted back?" This is the standard method by which tools verify concurrent systems — **simplify, decompose, then decide** — recast so the choice of simplification is a learned policy and every step stays checkable.

**The central new object: the certified reduction.** The design hinges on one trait, and in particular its third method:

```rust
trait Reduction {
    fn applicable(&self, net: &ResidualNet) -> Option<Witness>;   // precondition + soundness witness
    fn apply(&self, net: &ResidualNet) -> Residual;               // the smaller (or split) net
    fn lift(&self, residual_cert: Certificate) -> Certificate;    // proof on residual → proof on original
}
```

`applicable` and `apply` are straightforward; **`lift` is the keystone.** Because each reduction maps a certificate on the *reduced* net back toward the *original*, a chain of reductions ending in a decider produces, after lifting back up the chain, **a certificate on the original net — checkable by the original, unchanged checkers**. The reductions are scaffolding, removed at the end. Each reduction's `applicable` witness is one of the structural certificate types built in Epic B — re-casting the structural apparatus as *moves*:

| Reduction (action) | Its applicability witness is… | …a certificate from |
|---|---|---|
| remove an implicit place | the **P-invariant / Farkas dual** that determines it | the LP dual (B1) |
| agglomerate a transition sequence | the **cluster / siphon-trap** structure licensing the fusion | the structural component (B2/B7) |
| split into independent sub-nets | the **NUPN-unit / S-component** factorization | composition (B8/B3) |

The implicit-place case is the clean example: the Farkas dual that today's LP computes and *discards* on the infeasible path (the `MarkingEquationNoRationalSolution` payload thrown away at `reachability.rs:177`) is exactly the witness that a place is implicit — the discarded dual finally gets a job.

**The trust boundary, and an honestly-bounded robustness property.** For any policy over {certifying deciders} ∪ {certifying reductions}, the lifted-and-checked verdict on the original net is sound, because the final lifted certificate is checked **against the original net** by the unchanged checkers. The trusted base remains *exactly* those original checkers. From this would follow "a buggy `lift` cannot break soundness" — but that claim does **not** hold uniformly, and the split is the load-bearing honesty of this rung:

- **For existential witnesses (firing words), it holds cleanly.** A lifted firing sequence is checked by *replay* on the original net: the checker fires the lifted word from the initial marking and confirms it reaches the target. A wrong `lift` produces a word that does not replay, the checker rejects it, the search backtracks. The check re-establishes the property from scratch, so robustness is a theorem and a buggy lift costs only time.
- **For compositional / invariant lifts, it is an open per-certificate obligation, not yet discharged.** Here the lifted object is re-checked against an algebraic condition, not replayed. The hazard is **checker-completeness**: a buggy `lift` could emit a *too-weak* certificate that passes a *too-weak* check — an interface correction dropping a coupling term, satisfying the local condition while failing to re-establish the global property. The original-net check catches this only if it is *complete* for that property. Whether each compositional checker is complete is a per-certificate-kind proof obligation (backlog **F1/F2**), **not yet discharged**.

**The ratified discipline: trusted lifts are restricted to existential witnesses until checker-completeness is proven for the compositional kinds.** Existential reductions may be wired into the planner now and trusted as robust. Compositional reductions (implicit-place removal's invariant lift, agglomeration under liveness, the split's interface correction) may be implemented and tested, but until the obligation is discharged their matching checker is, for those kinds, *part of the trusted base* and must be audited as such. This is strictly weaker than an earlier draft's unqualified "even a buggy lift is caught," and it is the honest position.

**The search is an AND/OR proof tree**, the structure of HyperTree Proof Search (Lample et al. 2022) and AlphaProof (2024): OR-nodes choose a decider or non-splitting reduction; AND-nodes are decompositions (all sub-nets decided, then composed); the lifted certificate chain is the proof term; the original-net checker is the kernel. The **value network** estimates expected cost-to-proof of a residual node. The factorization residual Φ_PN reappears as the **applicability oracle for the split**: a decomposition is a good move precisely when the property factors over the cut, and Φ_PN measures how badly it fails to — so the value net is, in part, learning to predict Φ_PN, recognizing clean cuts versus integrated cores where the exhaustive cost must be paid (see [the factorization residual](the-factorization-residual.md); Φ_PN is the factorization-residual mathematics of [principles.md](principles.md), a number-and-witness measuring failure-to-factor, nothing about minds — IIT is not in the repository). This is a proposer–checker architecture: learned *guidance* over *certified operations*, plan with a learned model for speed but execute only real certified reductions whose lifted certificates are checked against the original net, subject to the existential/compositional caveat.

**Training stays cheap** for the same reason as Rung 1: solving the corpus yields proof trees; distill the policy on visit counts and the value on realized cost-to-proof; the certificate is the label, no oracle. A curriculum emerges — small nets resolve in one or two moves, harder nets stack reductions. The harness extends almost unchanged, now logging proof *trees* and φ-features of *residual* nodes, and the torsor/differential principle survives (cost-to-proof differences are origin-free).

**Falsifier.** The sharpest teeth in the whole ladder: *a buggy compositional lift could pass a too-weak check and yield an unsound verdict* — which is exactly why such lifts are not trusted until checker-completeness is discharged (backlog F1/F2). The other hard parts: the `lift` functions are real structural Petri-net theory (Michael's domain — Berthelot; Haddad–Pradat-Peyre agglomeration; P-invariant implicit-place removal), and getting them right under liveness versus reachability is subtle; search blow-up if the value signal is weak; and the honest fact that this is research, with all near-term value at the certificate and the coverage map. The reason to pursue it: the MCC protocol already names `StructuralReduction` as a first-class `Technique`, currently never wired in (mis-tagged on the CHC path today; backlog **F0**), and the classical behaviour-preserving reductions are well-established across Tina, TAPAAL, ITS-Tools, LoLA, and GreatSPN. Rung 3 is *learning to apply them well*, with every step checkable.

*Open sub-thread: whether a decomposing reduction's value estimate can be made to literally compute a cheap Φ_PN lower bound, turning the factorization-residual idea into a live heuristic inside the planner rather than a capstone observation.*

---

### References (curated)

- **Algorithm selection:** Rice, *The Algorithm Selection Problem* (1976); Xu–Hutter–Hoos–Leyton-Brown, *SATzilla* (JAIR 2008); Leyton-Brown–Nudelman–Shoham, *Empirical hardness models* (2002/2009); Hutter–Xu–Hoos–Leyton-Brown, *Algorithm runtime prediction* (AIJ 2014); Kotthoff, *Algorithm Selection: A Survey* (2014); Lindauer et al., *AutoFolio* (JAIR 2015).
- **Sequential / portfolio:** Weitzman, *Optimal Search for the Best Alternative* (Econometrica 1979); Luby–Sinclair–Zuckerman (1993); Streeter–Smith (2008); Gagliolo–Schmidhuber, *Learning Dynamic Algorithm Portfolios* (2006); Hoos–Kaminski–Lindauer–Schaub, *aspeed* (2015); Amadini–Gabbrielli–Mauro, *SUNNY* (2014); Levine–Kumar–Tucker–Fu, *Offline RL* (2020); Kumar–Zhou–Tucker–Levine, *CQL* (NeurIPS 2020).
- **Proof search:** Lample et al., *HyperTree Proof Search* (2022); *AlphaProof* (2024).
- **Companion essays:** the firewall theorem in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the four-principle account and Φ_PN in [principles.md](principles.md); the coverage claim in [the coverage claim](the-coverage-claim.md); the full inversion in [README.md](README.md).
