# Rung 1 — The Empirical Hardness Ranker
### Learning a section of the cost bundle: cost-sensitive, per-instance decider selection over the certifying lattice

> Status: design spec / vision, and the **near-term highest-value rung** — the one to build first once the measurement harness exists. Exploratory work (Daniel Dyer, with Claude) on Michael's tool; the dreams are marked as such. Sits *above* the measurement substrate ([self-measurement harness plan](../self-measurement-harness-plan.md)) and realizes Rung 1 of the ladder in [Soundness as a Free Variable](soundness-as-a-free-variable.md).

A numbering note, to head off the collision seeded earlier: the implementation **branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the **ladder rungs** are the learning ambition. This document is the ladder's **Rung 1**, and it consumes what the harness produces. Rung 0 is today's hardcoded cascade; Rung 2 is the sequential policy; Rung 3 is the [planner over certified reductions](rung-3-certified-reductions.md).

---

## The move from Rung 0

Today the schedule is a constant. For each property, [`analyze_*`](../../petrivet/src/api/system/reachability.rs) runs a fixed, hand-ordered cascade of partial deciders — token-sum, marking-equation LP, ILP, CHC, Karp–Miller — gated by structural class. The order is the author's prior about what is usually cheapest, frozen into the source.

Rung 1 makes that prior a **learned function of the instance.** Given a net `N`, predict *which admissible decider will produce an accepted certificate fastest*, and try it first; on a miss (inconclusive, or a certificate that fails its check), fall through the remaining admissible deciders in predicted-cost order, ending — always — at the exhaustive backstop. Nothing about the *answer* changes; only the *order of attempts* does. It is the smallest possible step off Rung 0, and by the soundness theorem it is a step you can take blind.

## What is learned — an empirical hardness model

The object to learn is the one the **algorithm-selection** tradition has refined for decades (Rice, 1976; SATzilla, Xu–Hutter–Hoos–Leyton-Brown, 2008): a map from cheap instance features to a per-instance choice of solver. Two flavors, and we want the second:

- **Direct selection (classification).** Predict `argmax`-likely-fastest decider directly. Simple, but it throws away the cost structure and gives you no fallback ordering.
- **Runtime prediction (regression), then select.** Learn, per decider `d`, a model $\hat{c}_d(\varphi(N))$ of its cost-to-certificate (and its probability of concluding at all); then **rank deciders by predicted cost** and schedule in that order. This is the empirical-hardness-model lineage (Leyton-Brown et al.; Hutter et al., *AIJ* 2014), and it is richer: the output is a *predicted cost vector over deciders*, which is exactly a ranking *and* a fallback order *and* a confidence signal.

And here the harness's own framing pays off. The predicted cost vector is precisely a **learned section of the cost-torsor bundle** the harness measures: at each domain point `φ(N)` the bundle's fiber is the per-decider cost vector (defined only up to a global shift), and the ranker learns to pick the origin — *which decider is cheapest here.* The model does not need absolute times; it needs the **differential** structure the harness already persists as `FitnessComparison` (rankings and log-ratios). Rung 1 is, almost literally, "learn a good section of the bundle Rung-1-the-harness measures."

## The objective is cost, not accuracy

The tempting metric — "fraction of instances where we picked the truly-fastest decider" — is the wrong one, and getting this wrong is the classic failure of naive algorithm selection. Mispredictions are not equal: choosing a slightly-slower-but-still-cheap decider costs almost nothing; choosing a diverging exploration costs everything. So the training objective is **cost-sensitive regret**, not classification accuracy:

$$ \mathrm{regret}(N) \;=\; \mathrm{cost}\big(\text{chosen schedule}\big) \;-\; \mathrm{cost}\big(\text{oracle-best decider}\big), $$

minimized in expectation over the instance distribution. Two reference points frame the target (the SATzilla framing): the **single best solver (SBS)** — the one fixed decider that is best on average, i.e. roughly what a well-tuned Rung-0 cascade already is — is the *floor* a ranker must beat; the **virtual best solver (VBS)** — an oracle that always picks the per-instance best — is the *ceiling* it chases. Rung 1's whole value is the gap it closes between SBS and VBS. Note the regret is a **difference** — origin-free, machine-invariant — so it is exactly the torsor-quotient quantity the harness stores, and a regression model trained on `FitnessComparison` log-ratios optimizes it directly.

## The model class — no deep learning required

The feature-design doctrine (from the soundness paper, grounded in the effective-theory literature) says the policy should consume **macro-features that are approximately autonomous under the firing dynamics and sufficient for the hardness label** — structural class, strong-connectivity, P/T-invariant dimension, S-/T-component and siphon/trap counts, NUPN unit-tree shape, token-sum and concurrency summaries — *not* the raw marking. These are low-dimensional, tabular, and mostly already cached on `DenseNet`. The model class that fits this data is **gradient-boosted trees or random forests** — the SATzilla lineage — not neural networks. They are cheap to train (minutes on commodity hardware), robust on heterogeneous tabular features, and they need no GPU.

A bonus the tree models give for free: **interpretable feature importances.** Asking the trained ranker "which structural features predict that Karp–Miller will be slow?" yields a scientific readout — a learned, empirical answer to *what makes a net hard for which technique* — which feeds straight back into Michael's structural intuition. The ranker is not only a scheduler; it is an instrument that measures the hardness landscape.

## Soundness in action (the firewall, concretely)

The soundness theorem (every certifying-portfolio verdict is sound for *any* policy) applies here unchanged, and it is worth seeing it bite:

- The ranker only **reorders** the admissible deciders. The certificate gate is untouched: a verdict is accepted only if its certificate passes `check`.
- On a wrong top pick — the decider is inconclusive, or returns a certificate that fails its check — the schedule **falls through** to the next admissible decider in predicted-cost order, and ultimately to the exhaustive backstop.
- Therefore the verdict is sound regardless of how badly the model misjudges. A wrong ranker wastes time; it cannot produce a wrong answer.

The one precondition, carried from the soundness paper: this guarantee covers **certifying** deciders. The bare-boolean shortcuts and the two `Some(false)` stubs ([`is_covered_by_s_components`](../../petrivet/src/api/net/mod.rs), the marked-graph liveness arm) are *trusted*, not checked — so until they emit certificates, the ranker should either schedule only certificate-emitting deciders or treat those two as trusted with their known caveats. Fixing the stubs is upstream of trusting the ranker over them.

## Performance is NOT guaranteed monotone — and how to make it nearly so

The honest caveat that the soundness theorem does *not* cover: **a bad ranker can be slower than the Rung-0 cascade.** If the model confidently puts an expensive ILP or a diverging exploration first, you pay for it before falling through. Soundness is safe; *speed* is not automatically a Pareto improvement. Mitigations, in order of importance:

1. **Always run the (near-)free structural shortcuts first, unconditionally.** The `O(1)` token-sum / class checks and the polynomial CHC are nearly free and exact on their domains; let them fire before the ranker ever orders the *expensive* deciders. The ranker's job is to order the costly tail (LP vs ILP vs exploration), not the cheap head.
2. **Cap the first pick with a deadline / budget.** Bound the time the top-ranked expensive decider may run before escalating — a Rung-2 idea, but a static budget is a cheap Rung-1 safety net.
3. **Use the Rung-0 hand-order as a prior.** Blend the learned ranking with the hand-cascade order (or only deviate when the model is confident), so the worst case degrades toward Rung-0, not below it.

With (1)–(3), the realistic guarantee is: *sound always, and in expectation no worse than — usually meaningfully better than — Rung 0,* with a bounded, capped downside on adversarial instances.

## Training and evaluation

- **Offline, supervised, self-labeling.** Train on the harness's `Observation` / `FitnessComparison` JSONL. The label is "which admissible decider produced an accepted certificate, at what cost" — and *the certificate is the label*: no oracle needed (the MCC oracle remains an independent cross-check, not a training dependency). This is the cheap, self-supervised regime.
- **Split by net *family*, not by instance.** The single most important methodological trap: the MCC corpus has many parameterized instances per model family, and a random train/test split leaks family structure, inflating measured generalization. Cross-validate by holding out whole families, as the algorithm-selection community learned to.
- **Evaluate with the harness's differential fitness test.** The metric is the SBS→VBS gap closed on held-out families, reported as **cost-ratios** (origin-free, per the torsor principle), not absolute times — so the evaluation is portable across the machine it runs on. And the **soundness sentinel** keeps running underneath: every accepted verdict must still agree with the oracle. A performance experiment that quietly breaks soundness fails loudly.

## Deployment and domain separation

The trained model is a small artifact (a few hundred KB of trees), loaded at startup and consulted by the `Policy::next` seam. Crucially it stays **outside the core**, exactly like the observe crate: a downstream, optional (feature-gated) artifact that `petrivet`'s `analyze_*` never depends on. Without the model, the cascade simply uses the Rung-0 hand-order. The dependency arrow still points only toward the core; the learner is a leaf.

## Boundary with Rung 0 and Rung 2

- **Below (Rung 0):** the hand-ordered cascade. Rung 1 generalizes its frozen prior into a learned function of `φ(N)`, and degrades to it as a safety net.
- **What Rung 1 *is*:** a **one-shot, static selection** — it predicts a schedule *before* running anything and commits to that order (modulo fall-through). It does not adapt mid-search to what it has learned from the deciders it already ran.
- **Above (Rung 2):** make the section *adaptive*. Rung 2 turns the one-shot ranking into a **sequential policy** that updates its choice as deciders return inconclusive and as the budget burns — a contextual bandit, then RL, with deadlines and anytime parallel racing. Rung 1 hands Rung 2 a strong static prior and the trained value signal to bootstrap from.

## Where the dragons are (claim-honest)

1. **Performance is not provably monotone vs. Rung 0** (above). Soundness is; speed needs the mitigations.
2. **Distribution shift.** A model trained on MCC may misjudge novel nets — but benignly: by the theorem it wastes time, never correctness, degrading to the exhaustive backstop.
3. **Cold start.** The model needs enough corpus observations per `(property, decider)` cell; rare deciders on rare classes are data-starved. The Rung-0 prior covers the cold cells.
4. **Family leakage** in cross-validation (above) — the easiest way to fool yourself into reporting a ranker that doesn't generalize.
5. **Feature sufficiency is empirical.** §"model class" prescribes the *kind* of feature; whether the chosen `φ` is actually sufficient for hardness on a given corpus is measured (a mutual-information check against the hardness label), not assumed.
6. **The stubs precondition.** Until the two `Some(false)` deciders are certifying, the ranker must not trust them as if they were.

None of these touches correctness. They are the ordinary risks of a *performance* component — which is exactly the point: by the soundness theorem, Rung 1 confines the entire surface of machine-learning failure to the dimension where failure is cheap. This is why it is the rung to build first: maximal practical payoff (the SBS→VBS gap is real and often large), minimal risk (sound by construction), and minimal machinery (a tree model over features the code already computes, trained on data the harness already emits).

---

### References (curated)

- Rice. *The Algorithm Selection Problem.* Adv. Computers 15 (1976). Xu, Hutter, Hoos, Leyton-Brown. *SATzilla: Portfolio-based Algorithm Selection for SAT.* JAIR 32 (2008). Leyton-Brown, Nudelman, Shoham. *Empirical hardness models.* (2002/2009). Hutter, Xu, Hoos, Leyton-Brown. *Algorithm runtime prediction: Methods & evaluation.* AIJ 206 (2014). Kotthoff. *Algorithm Selection for Combinatorial Search: A Survey.* (2014). Lindauer et al. *AutoFolio.* JAIR (2015).
- Companion: the soundness theorem and the certifying-portfolio framing in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the cost-torsor data model in the [self-measurement harness plan](../self-measurement-harness-plan.md).
