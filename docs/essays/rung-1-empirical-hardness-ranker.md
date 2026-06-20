# Rung 1 — The Empirical Hardness Ranker
### Learning a section of the cost bundle: cost-sensitive, per-instance decider selection over the certifying lattice

> Status: design specification. This is the near-term highest-value rung and the one to build first once the measurement harness exists. Exploratory work (Daniel Dyer, with Claude) on Michael's tool; planned but unbuilt items are marked as such. It sits above the measurement substrate ([self-measurement harness plan](../self-measurement-harness-plan.md)) and realizes Rung 1 of the ladder in [Soundness as a Free Variable](soundness-as-a-free-variable.md).

A note on numbering, to avoid a collision in terminology. The implementation **branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the **ladder rungs** describe the learning program. This document is the ladder's **Rung 1**, and it consumes what the harness produces. Rung 0 is the current hardcoded cascade; Rung 2 is the sequential policy; Rung 3 is the [planner over certified reductions](rung-3-certified-reductions.md).

---

## The move from Rung 0

At present the schedule is a constant. For each property, [`analyze_*`](../../petrivet/src/api/system/reachability.rs) runs a fixed, hand-ordered cascade of partial deciders — token-sum, marking-equation LP, ILP, CHC, Karp–Miller — gated by structural class. The order encodes the author's prior about what is usually cheapest, fixed in the source.

Rung 1 replaces that prior with a **learned function of the instance.** Given a net `N`, the model predicts which admissible decider will produce an accepted certificate fastest, and that decider is tried first. On a miss (an inconclusive result, or a certificate that fails its check), the schedule falls through the remaining admissible deciders in predicted-cost order, ending in all cases at the exhaustive backstop. The *answer* does not change; only the *order of attempts* does. This is the smallest step away from Rung 0, and by the soundness theorem it can be taken without affecting correctness.

## What is learned: an empirical hardness model

The object to learn is the one the **algorithm-selection** tradition has developed over several decades (Rice, 1976; SATzilla, Xu–Hutter–Hoos–Leyton-Brown, 2008): a map from cheap instance features to a per-instance choice of solver. There are two variants, and the second is the one adopted here.

- **Direct selection (classification).** Predict the likely-fastest decider directly via `argmax`. This is simple, but it discards the cost structure and provides no fallback ordering.
- **Runtime prediction (regression), then select.** Learn, per decider `d`, a model $\hat{c}_d(\varphi(N))$ of its cost-to-certificate (and its probability of concluding at all); then **rank deciders by predicted cost** and schedule in that order. This is the empirical-hardness-model lineage (Leyton-Brown et al.; Hutter et al., *AIJ* 2014). It is richer because the output is a *predicted cost vector over deciders*, which simultaneously provides a ranking, a fallback order, and a confidence signal.

This connects directly to the harness's data model. The predicted cost vector is a **section of the cost-torsor bundle** that the harness measures: at each domain point `φ(N)` the bundle's fiber is the per-decider cost vector, defined only up to a global shift, and the ranker learns to select the origin — which decider is cheapest at that point. The model does not require absolute times; it requires the **differential** structure the harness already persists as `FitnessComparison` (rankings and log-ratios). Rung 1 therefore amounts to learning a section of the bundle that the harness measures.

## The objective is cost, not accuracy

The naive metric — the fraction of instances on which the truly-fastest decider was chosen — is incorrect, and using it is the classic failure mode of algorithm selection. Mispredictions are not equal: choosing a slightly slower but still cheap decider costs almost nothing, while choosing a diverging exploration costs a great deal. The training objective is therefore **cost-sensitive regret**, not classification accuracy:

$$ \mathrm{regret}(N) \;=\; \mathrm{cost}\big(\text{chosen schedule}\big) \;-\; \mathrm{cost}\big(\text{oracle-best decider}\big), $$

minimized in expectation over the instance distribution. Two reference points frame the target (the SATzilla framing). The **single best solver (SBS)** — the one fixed decider that is best on average, approximately what a well-tuned Rung-0 cascade already achieves — is the *floor* a ranker must beat. The **virtual best solver (VBS)** — an oracle that always selects the per-instance best — is the *ceiling* it approaches. Rung 1's value is the portion of the SBS–VBS gap it closes. The regret is a **difference**, hence origin-free and machine-invariant; it is exactly the torsor-quotient quantity the harness stores, and a regression model trained on `FitnessComparison` log-ratios optimizes it directly.

## The model class: no deep learning required

The feature-design doctrine (from the soundness paper, grounded in the effective-theory literature) holds that the policy should consume **macro-features that are approximately autonomous under the firing dynamics and sufficient for the hardness label** — structural class, strong-connectivity, P/T-invariant dimension, S-/T-component and siphon/trap counts, NUPN unit-tree shape, token-sum and concurrency summaries — rather than the raw marking. These features are low-dimensional, tabular, and in most cases already cached on `DenseNet`. The model class suited to this data is **gradient-boosted trees or random forests** — the SATzilla lineage — not neural networks. Such models are cheap to train (minutes on commodity hardware), robust on heterogeneous tabular features, and require no GPU.

The tree models also provide **interpretable feature importances**. Querying the trained ranker for which structural features predict that Karp–Miller will be slow yields a learned, empirical characterization of what makes a net hard for a given technique, which feeds back into structural intuition about the problem. The ranker thus functions both as a scheduler and as an instrument for characterizing the hardness landscape.

## Soundness in action

The soundness theorem (every certifying-portfolio verdict is sound under *any* policy) applies here unchanged. Its consequences are worth stating explicitly.

- The ranker only **reorders** the admissible deciders. The certificate gate is untouched: a verdict is accepted only if its certificate passes `check`.
- On a wrong top pick — the decider is inconclusive, or returns a certificate that fails its check — the schedule **falls through** to the next admissible decider in predicted-cost order, and ultimately to the exhaustive backstop.
- The verdict is therefore sound regardless of how badly the model misjudges. A wrong ranker wastes time; it cannot produce a wrong answer.

The one precondition, carried from the soundness paper: this guarantee covers **certifying** deciders. The bare-boolean shortcuts and the two `Some(false)` stubs ([`is_covered_by_s_components`](../../petrivet/src/api/net/mod.rs), the marked-graph liveness arm) are *trusted*, not checked. Until they emit certificates, the ranker should either schedule only certificate-emitting deciders or treat those two as trusted with their known caveats. Fixing the stubs is a prerequisite to trusting the ranker over them.

## Performance is not provably monotone, and how to make it nearly so

The soundness theorem does *not* cover performance: **a bad ranker can be slower than the Rung-0 cascade.** If the model confidently places an expensive ILP or a diverging exploration first, that cost is paid before the schedule falls through. Soundness is preserved; *speed* is not automatically a Pareto improvement. The mitigations, in order of importance:

1. **Always run the (near-)free structural shortcuts first, unconditionally.** The `O(1)` token-sum and class checks and the polynomial CHC are nearly free and exact on their domains; they should fire before the ranker orders the *expensive* deciders. The ranker's task is to order the costly tail (LP vs. ILP vs. exploration), not the cheap head.
2. **Cap the first pick with a deadline or budget.** Bound the time the top-ranked expensive decider may run before escalating. This is a Rung-2 idea, but a static budget is a cheap Rung-1 safety net.
3. **Use the Rung-0 hand-order as a prior.** Blend the learned ranking with the hand-cascade order, or deviate only when the model is confident, so that the worst case degrades toward Rung-0 rather than below it.

With (1)–(3), the realistic guarantee is: sound in all cases, and in expectation no worse than — and usually meaningfully better than — Rung 0, with a bounded, capped downside on adversarial instances.

## Training and evaluation

- **Offline, supervised, self-labeling.** Train on the harness's `Observation` / `FitnessComparison` JSONL. The label is which admissible decider produced an accepted certificate, and at what cost; the certificate itself is the label, so no oracle is required. (The MCC oracle remains an independent cross-check, not a training dependency.) This is a cheap, self-supervised regime.
- **Split by net *family*, not by instance.** This is the most important methodological point. The MCC corpus contains many parameterized instances per model family, and a random train/test split leaks family structure, inflating measured generalization. Cross-validation should hold out whole families, following standard practice in the algorithm-selection literature.
- **Evaluate with the harness's differential fitness test.** The metric is the fraction of the SBS→VBS gap closed on held-out families, reported as **cost-ratios** (origin-free, per the torsor principle) rather than absolute times, so the evaluation is portable across machines. The **soundness sentinel** runs throughout: every accepted verdict must still agree with the oracle. A performance experiment that quietly breaks soundness fails loudly.

## Deployment and domain separation

The trained model is a small artifact (a few hundred KB of trees), loaded at startup and consulted through the `Policy::next` seam. It remains **outside the core**, like the observe crate: a downstream, optional (feature-gated) artifact that `petrivet`'s `analyze_*` never depends on. Without the model, the cascade falls back to the Rung-0 hand-order. The dependency direction points only toward the core; the learner is a leaf.

## Boundary with Rung 0 and Rung 2

- **Below (Rung 0):** the hand-ordered cascade. Rung 1 generalizes its fixed prior into a learned function of `φ(N)`, and degrades to it as a safety net.
- **What Rung 1 is:** a **one-shot, static selection.** It predicts a schedule *before* running anything and commits to that order (modulo fall-through). It does not adapt mid-search based on what it has learned from the deciders already run.
- **Above (Rung 2):** making the section *adaptive*. Rung 2 turns the one-shot ranking into a **sequential policy** that updates its choice as deciders return inconclusive and as the budget is consumed — a contextual bandit, then RL, with deadlines and anytime parallel racing. Rung 1 supplies Rung 2 with a strong static prior and the trained value signal to bootstrap from.

## Risks

1. **Performance is not provably monotone vs. Rung 0** (above). Soundness is monotone; speed depends on the mitigations.
2. **Distribution shift.** A model trained on MCC may misjudge novel nets, but benignly: by the theorem it wastes time, never correctness, degrading to the exhaustive backstop.
3. **Cold start.** The model needs enough corpus observations per `(property, decider)` cell; rare deciders on rare classes are data-starved. The Rung-0 prior covers the cold cells.
4. **Family leakage** in cross-validation (above) — the easiest way to report a ranker that does not generalize.
5. **Feature sufficiency is empirical.** The "model class" section prescribes the *kind* of feature; whether the chosen `φ` is actually sufficient for hardness on a given corpus is measured (a mutual-information check against the hardness label), not assumed.
6. **The stubs precondition.** Until the two `Some(false)` deciders are certifying, the ranker must not trust them as if they were.

None of these affects correctness. They are the ordinary risks of a *performance* component, which is the central point: by the soundness theorem, Rung 1 confines the entire surface of machine-learning failure to the dimension where failure is cheap. This is why it is the rung to build first: maximal practical payoff (the SBS→VBS gap is real and often large), minimal risk (sound by construction), and minimal machinery (a tree model over features the code already computes, trained on data the harness already emits).

---

### References (curated)

- Rice. *The Algorithm Selection Problem.* Adv. Computers 15 (1976). Xu, Hutter, Hoos, Leyton-Brown. *SATzilla: Portfolio-based Algorithm Selection for SAT.* JAIR 32 (2008). Leyton-Brown, Nudelman, Shoham. *Empirical hardness models.* (2002/2009). Hutter, Xu, Hoos, Leyton-Brown. *Algorithm runtime prediction: Methods & evaluation.* AIJ 206 (2014). Kotthoff. *Algorithm Selection for Combinatorial Search: A Survey.* (2014). Lindauer et al. *AutoFolio.* JAIR (2015).
- Companion: the soundness theorem and the certifying-portfolio framing in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the cost-torsor data model in the [self-measurement harness plan](../self-measurement-harness-plan.md).
