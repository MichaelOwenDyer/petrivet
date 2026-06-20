# Rung 1 — The Empirical Hardness Ranker

### A SATzilla-style decider selector over the certifying portfolio — the sequel to the certificate, gated on a measured gap

> Status: design specification, deliberately deferred. This is the first rung of the **learning ladder**, which is the *sequel* to petrivet's signature contribution — the certificate-and-checker — not its spine. Exploratory work (Daniel Dyer, with Claude) on Michael's tool; planned-but-unbuilt items are marked as such. It sits above the measurement substrate ([self-measurement harness plan](../self-measurement-harness-plan.md)) and realizes Rung 1 of the ladder in [Soundness as a Free Variable](soundness-as-a-free-variable.md).

---

## Why the ladder is a sequel, and why it is honest to defer it

The thesis of petrivet is not this ranker. The thesis is an **empirical coverage map** — *on the real MCC P/T corpus, a polynomial structural certificate decides a large, characterizable fraction of queries without state-space exploration; where it abstains, it abstains honestly* — carried by petrivet's signature technical contribution, the **certificate-and-checker**: every verdict ships a machine-checkable proof object, re-validated by a small external checker that is the entire trusted base. Learned selection is the layer that comes *after* that stone is laid.

Deferring the ladder is **safe** because of one structural fact, the **firewall**: over a portfolio of certifying deciders, soundness is independent of the selection policy (the theorem in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the four-principle account in [Core Principles](core-principles.md), §4.4). Any scheduler — hand-written, learned, or adversarial — can only change *which proofs are attempted and in what order*; it can never cause an unchecked verdict to be accepted. The policy is a free variable. So the ranker can be built last, or never, without putting one grain of correctness at risk.

Deferring it is also **honest**, because the contribution is the map and the checker, not the ranker. The ranker's value is bounded by a quantity that has not yet been measured: the gap between the best fixed schedule and the per-instance-optimal one. If that gap is small, the hand-ordered cascade is the right answer and a learned model is dead weight. So this rung does not open by training a model. It opens by **measuring the gap** — and is willing to conclude that the model should not be built.

This is the SATzilla/Rice lineage of algorithm selection (Rice, 1976; Xu–Hutter–Hoos–Leyton-Brown, 2008), and nothing more exotic. An earlier draft reached for an AlphaGo/MuZero analogy; that framing is **dropped**. petrivet has *checkable leaves* — every decider that concludes emits a proof a checker re-validates — which makes its selection problem strictly easier than game-tree search and unlike AlphaGo, where a leaf evaluation is an unverified guess that can lose the game. The honest lineage is portfolio solving over verified outcomes. (The effective-theory / cellular-automata material that appears in the companion essay is **clearly-labeled speculation about why instance-hardness might be learnable at all**; it is never load-bearing for any design decision here, and this rung does not rest on it.)

A note on numbering, to avoid a collision in terminology. The implementation **branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the **ladder rungs** describe the learning program. This document is the ladder's **Rung 1**, and it consumes what the harness produces. Rung 0 is the current hardcoded cascade; Rung 2 is the [sequential policy](rung-2-sequential-policy.md); Rung 3 is the [planner over certified reductions](rung-3-certified-reductions.md).

---

## The first experiment is to measure the gap

Before any model: measure the SBS→VBS gap on the corpus. Two reference points frame it (the SATzilla framing).

- The **single best solver (SBS)** is the one fixed decider — or one fixed schedule — that is best on average over the corpus. A well-tuned Rung-0 cascade is approximately this. It is the *floor* a ranker must beat.
- The **virtual best solver (VBS)** is an oracle that always picks the per-instance best decider. It is the unreachable *ceiling*.

Rung 1's entire value is the fraction of the SBS→VBS gap it closes. The decisive falsifier sits here, at the very first step: **if the measured gap on the ~6-arm portfolio is within noise, there is nothing for a ranker to win.** The portfolio is small, and the structural cascade is already class-gated to the cheap technique in most cases; it is entirely possible that the hand-order is already near-VBS on the corpus that matters. In that case Rung-0 is the honest answer, the ML is dead weight, and this rung correctly terminates here. The gap is measured first (harness Phases 1–3, backlog D3/D4; reported as a backlog **D5** precondition) and reported as a number with its noise band. Only a gap that clears a pre-registered threshold justifies the model.

This is the cheapest decisive experiment in the whole ladder, and it is designed to be able to say *no*.

## What is learned, if anything: an empirical hardness model

Suppose the gap clears the threshold. The object to learn is then the one the **algorithm-selection** tradition has developed over several decades: a map from cheap instance features to a per-instance choice of decider. Two variants; the second is adopted.

- **Direct selection (classification).** Predict the likely-fastest decider directly via `argmax`. Simple, but it discards cost structure and gives no fallback ordering.
- **Runtime prediction (regression), then select.** Learn, per decider `d`, a model $\hat{c}_d(\varphi(N))$ of its cost-to-certificate (and its probability of concluding at all); then **rank deciders by predicted cost** and schedule in that order. This is the empirical-hardness-model lineage (Leyton-Brown, Nudelman, Shoham, 2002/2009; Hutter, Xu, Hoos, Leyton-Brown, *AIJ* 2014). It is richer: the output is a *predicted cost vector over deciders*, which simultaneously supplies a ranking, a fallback order, and a confidence signal.

This connects to the harness's data model. The predicted cost vector is a **section of the cost-torsor bundle** the harness measures: at each domain point `φ(N)` the bundle's fiber is the per-decider cost vector, defined only up to a global additive shift (machine speed), and the ranker learns to select the origin — which decider is cheapest at that point. The model needs no absolute times; it needs the **differential** structure the harness already persists as `FitnessComparison` (rankings and log-ratios). Rung 1 is therefore learning a section of the bundle the harness measures.

## The objective is cost, not accuracy

The naive metric — the fraction of instances on which the truly-fastest decider was chosen — is wrong, and using it is the classic failure mode of algorithm selection. Mispredictions are not equal: a slightly slower but still-cheap decider costs almost nothing, while a diverging exploration costs a great deal. The training objective is therefore **cost-sensitive regret**, not classification accuracy:

$$ \mathrm{regret}(N) \;=\; \mathrm{cost}\big(\text{chosen schedule}\big) \;-\; \mathrm{cost}\big(\text{oracle-best decider}\big), $$

minimized in expectation over the instance distribution. The regret is a **difference**, hence origin-free and machine-invariant; it is exactly the torsor-quotient quantity the harness stores, and a regression model trained on `FitnessComparison` log-ratios optimizes it directly. Note that the SBS→VBS gap of the previous section is precisely the expected regret of the *fixed* schedule relative to the oracle — so the gate and the objective are the same quantity, first measured, then minimized.

## The model class: no deep learning required

The policy should consume **structural macro-features** of the net — structural class, strong-connectivity, P/T-invariant dimension, S-/T-component and siphon/trap counts, NUPN unit-tree shape, token-sum and concurrency summaries — rather than the raw marking. These are low-dimensional, tabular, and in most cases already cached on `DenseNet`. The model class suited to this data is **gradient-boosted trees or random forests** — the SATzilla lineage — not neural networks: cheap to train (minutes on commodity hardware), robust on heterogeneous tabular features, and requiring no GPU.

(*Why structural features should suffice at all* is the question the companion essay's effective-theory discussion speculates about — the idea that an intractable micro-dynamics can admit a tractable macro-theory when the macro-variables are autonomous under the dynamics. That is offered as interpretation, not as justification; the operative claim here is the *measurable* one: a feature earns its place only if its mutual information with the hardness label is high at low description length, tested against the corpus, not assumed.)

The tree models also give **interpretable feature importances**. Asking the trained ranker which structural features predict that Karp–Miller will be slow yields a learned, empirical characterization of what makes a net hard for a technique, feeding back into structural intuition. The ranker thus doubles as an instrument for charting the hardness landscape.

## Soundness in action

The firewall (every certifying-portfolio verdict is sound under *any* policy) applies here unchanged. Its consequences, stated explicitly:

- The ranker only **reorders** admissible deciders. The certificate gate is untouched: a verdict is accepted only if its certificate passes `check` against the original net.
- On a wrong top pick — the decider is inconclusive, or returns a certificate that fails its check — the schedule **falls through** to the next admissible decider in predicted-cost order, and ultimately to the exhaustive backstop.
- The verdict is therefore sound regardless of how badly the model misjudges. A wrong ranker wastes time; it cannot produce a wrong answer.

The one precondition, carried from the soundness paper and ratified as backlog **A2/A6**: this guarantee covers **certifying** deciders. The bare-boolean shortcuts and the two `Some(false)` stubs ([`is_covered_by_s_components`](../../petrivet/src/api/net/mod.rs), the marked-graph liveness arm) are *trusted*, not checked — and are the real soundness risk in the construction, not the machine learning. Until they emit certificates, the ranker should either schedule only certificate-emitting deciders or treat those two as trusted with their known caveats. Fixing the stubs is the north-star precondition for the whole edifice, ranker included.

## Performance is not provably monotone, and how to make it nearly so

The firewall does *not* cover performance: **a bad ranker can be slower than the Rung-0 cascade.** If the model confidently places an expensive ILP or a diverging exploration first, that cost is paid before the schedule falls through. Soundness is preserved; *speed* is not automatically a Pareto improvement. The mitigations, in order of importance:

1. **Always run the (near-)free structural shortcuts first, unconditionally.** The `O(1)` token-sum and class checks and the polynomial CHC are nearly free and exact on their domains; they fire before the ranker orders the *expensive* deciders. The ranker's task is to order the costly tail (LP vs. ILP vs. exploration), not the cheap head.
2. **Cap the first pick with a deadline or budget.** Bound the time the top-ranked expensive decider may run before escalating. This is properly a Rung-2 idea, but a static budget is a cheap Rung-1 safety net.
3. **Use the Rung-0 hand-order as a prior.** Blend the learned ranking with the hand-cascade order, or deviate only when the model is confident, so the worst case degrades toward Rung-0 rather than below it.

With (1)–(3), the realistic guarantee is: sound in all cases, and in expectation no worse than — and, *if the gap was real*, meaningfully better than — Rung 0, with a bounded, capped downside on adversarial instances.

## Training and evaluation

- **Offline, supervised, self-labeling.** Train on the harness's `Observation` / `FitnessComparison` JSONL. The label is which admissible decider produced an accepted certificate, and at what cost; the certificate itself is the label, so no oracle is required. (The MCC oracle remains an independent cross-check, not a training dependency — consistent with the ratified position that the contest is the *crucible and labelling source*, never a leaderboard to climb.) A cheap, self-supervised regime.
- **Split by net *family*, not by instance.** The most important methodological point. The MCC corpus has many parameterized instances per model family; a random train/test split leaks family structure and inflates measured generalization. Cross-validation holds out whole families, per standard practice in the algorithm-selection literature.
- **Evaluate with the harness's differential fitness test.** The metric is the fraction of the SBS→VBS gap closed on held-out families, reported as **cost-ratios** (origin-free, per the torsor principle) rather than absolute times, so the evaluation is portable across machines. The **soundness sentinel** runs throughout: every accepted verdict must still agree with the oracle. A performance experiment that quietly breaks soundness fails loudly.

## Deployment and domain separation

The trained model is a small artifact (a few hundred KB of trees), loaded at startup and consulted through the `Policy::next` seam. It remains **outside the core**, like the observe crate: a downstream, optional, feature-gated artifact that petrivet's `analyze_*` never depends on. Without the model, the cascade falls back to the Rung-0 hand-order. The dependency arrow points only toward the core; the learner is a leaf.

## Boundary with Rung 0 and Rung 2

- **Below (Rung 0):** the hand-ordered cascade. Rung 1 generalizes its fixed prior into a learned function of `φ(N)`, and degrades to it as a safety net. **If the gap is within noise, Rung 0 *is* the answer.**
- **What Rung 1 is:** a **one-shot, static selection.** It predicts a schedule *before* running anything and commits to that order (modulo fall-through). It does not adapt mid-search.
- **Above (Rung 2):** making the section *adaptive*. Rung 2 turns the one-shot ranking into a [sequential policy](rung-2-sequential-policy.md) that updates its choice as deciders return inconclusive and as the budget is consumed. Rung 1 supplies Rung 2 with a strong static prior and the trained value signal to bootstrap from.

## The falsifier, and the lesser risks

**The falsifier of this rung is blunt: the learned ranker does not beat the obvious ordering.** The SBS→VBS gap may be small — on a 6-arm, class-gated portfolio, plausibly so — and then the model is dead weight that adds latency, a deployment artifact, and a training pipeline for no measured win. This is why the gap is measured *first* and the rung is *gated*: the experiment is built to be able to return "do not build the model," and that is a legitimate, even likely, outcome.

The remaining risks are the ordinary risks of a *performance* component:

1. **Performance is not provably monotone vs. Rung 0** (above). Soundness is monotone; speed depends on the mitigations.
2. **Distribution shift.** A model trained on MCC may misjudge novel nets — but benignly: by the firewall it wastes time, never correctness, degrading to the exhaustive backstop.
3. **Cold start.** The model needs enough observations per `(property, decider)` cell; rare deciders on rare classes are data-starved. The Rung-0 prior covers the cold cells.
4. **Family leakage** in cross-validation — the easiest way to report a ranker that does not generalize.
5. **Feature sufficiency is empirical.** Whether the chosen `φ` is sufficient for hardness is measured (a mutual-information check against the hardness label), not assumed.
6. **The stubs precondition.** Until the two `Some(false)` deciders are certifying, the ranker must not trust them as if they were.

None of these touches correctness. That is the point: the firewall confines the entire surface of machine-learning failure to the dimension where failure is cheap. Rung 1 is the rung to build *first within the ladder* — but the ladder itself is built last, after the certificate, the checker, and the coverage map, and only if the measured gap says it is worth building at all.

---

### References (curated)

- Rice. *The Algorithm Selection Problem.* Adv. Computers 15 (1976). Xu, Hutter, Hoos, Leyton-Brown. *SATzilla: Portfolio-based Algorithm Selection for SAT.* JAIR 32 (2008). Leyton-Brown, Nudelman, Shoham. *Empirical hardness models.* (2002/2009). Hutter, Xu, Hoos, Leyton-Brown. *Algorithm runtime prediction: Methods & evaluation.* AIJ 206 (2014). Kotthoff. *Algorithm Selection for Combinatorial Search: A Survey.* (2014). Lindauer et al. *AutoFolio.* JAIR (2015).
- Companion: the firewall theorem and the certifying-portfolio framing in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the four-principle account in [Core Principles](core-principles.md); the cost-torsor data model in the [self-measurement harness plan](../self-measurement-harness-plan.md).
</content>
</invoke>
