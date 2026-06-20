# The Coverage Claim

## A falsifiable headline: how often a polynomial structural certificate decides a real query without searching the state space

> Status: vision essay, the most falsifiability-disciplined of the corpus. This is exploratory writing (Daniel Dyer, with Claude), authored to the thesis author and inviting contradiction; the core theory is Michael's. Unlike the other essays in this folder, which argue for designs, this one argues for *a number and the experiment that could kill it*. Every claim below names its falsifier and the minimal test that would settle it. If the number comes back wrong, the essay has done its job: it will have told you exactly which sentence to delete.

---

## 0. The claim this essay stakes

This essay owns the corpus's one *empirically falsifiable* headline. The certificate-and-checker ([the-checkable-frontier.md](the-checkable-frontier.md)) and the soundness firewall ([soundness-as-a-free-variable.md](soundness-as-a-free-variable.md)) are both true but both *theorems*; the framing that makes coverage the falsifiable headline is stated once in [README.md](README.md). The claim a referee could refute is the empirical one:

> **The coverage claim.** On the real Model Checking Contest P/T corpus, a polynomial structural certifying tier decides a large, characterizable fraction of *queries* — counted as queries decided, not nets-in-class — each with an independently checkable certificate and without any state-space exploration; where it abstains, it abstains honestly; and the structural-versus-search boundary is predictable from cheap structural features.

The figure of merit is **`f_struct`**: the fraction of queries the structural tier decides, reported against two denominators, with whole families held out. The certificate makes a decided query *believable*; the firewall makes the routing *safe*; but the thesis — the part that could be wrong — is that `f_struct` is large, characterizable, cheap, and predictable on real models.

---

## 1. Why "high performance" needs a theorem behind it

"High performance Petri net model checker" is the thesis title. On its own, "high performance" is the kind of phrase a benchmark can flatter and a referee can dismiss. The coverage claim makes it rigorous by anchoring it to a place where the performance gap is not an engineering accident but a *complexity-theoretic theorem*.

The worst case is genuinely immovable. General P/T-net reachability is **Ackermann-complete** (Leroux–Schmitz 2021 for the upper bound's matching hardness; Czerwiński–Orlikowski 2021 for the Ackermannian lower bound) — not "hard" in the loose sense but provably non-elementary, beyond every primitive-recursive bound. Liveness and boundedness are **EXPSPACE-hard** (Lipton; Rackoff for the matching space bound). No data structure, no cache, no constant factor touches these. A tool that claimed to be "fast at reachability" in general would be claiming something false.

And yet, on the *free-choice* class, liveness and boundedness are **polynomial** — Commoner–Hack's theorem (a free-choice system is live iff every proper siphon contains a marked trap) turns an EXPSPACE-hard question into one closure computation, and the rank/cluster theorem (`rank C = c − 1`) decides simultaneous liveness-and-boundedness from linear algebra alone ([principles.md](principles.md); [latent-architecture.md](latent-architecture.md)). The gap between Ackermannian search and polynomial structure is *the width of a theorem*.

So the structural tier is not "a clever optimization." It is a trade with a proof on each side: **it exchanges a provably hard search for a polynomial certificate exactly at the instances where the hardness gap is a theorem.** The performance claim inherits the rigor of that gap.

But — and this is the whole point — *a theorem about a class is not a measurement of a corpus.* Commoner–Hack tells you free-choice liveness is polynomial. It does not tell you what fraction of the queries people actually pose, on the nets people actually build, fall into the reach of such a tier. That fraction is `f_struct`, and it is an empirical fact about the world, not a corollary. The complexity theory makes the trade *rigorous when it applies*; the headline number measures *how often it applies on real models*.

> **Falsifier (the framing).** If, on inspection, the structural tier's decisions turn out to be confined to instances that are *also* polynomial for every mainstream tool — if the trade applies only where no one needed it — then "trades a hard search for a cheap certificate" is rhetoric, not result. **Minimal test:** §6's hardness-stratification check. This is the framing's own falsifier, and it is the one I most expect to bite (§7).

---

## 2. The discipline that makes the number honest

A coverage number is easy to inflate and easy to fool yourself with. Four disciplines, each adopted because the naive alternative lies, make `f_struct` decisive rather than decorative. These are not stylistic preferences; each is a guard against a specific way the number could be a lie.

### 2.1 Count queries decided, not nets-in-class

The unit of evidence is the **query** — a (model, examination) pair, or a (model, examination, formula) triple where the contest poses several — not the net, and emphatically not the net-class. Reporting "92% of free-choice nets" is a category error three times over: it counts membership in a class rather than questions answered; it lets one trivially-decided net count the same as a hard one; and it silently excludes the queries the tier abstained on. The honest denominator is queries, because a query is what a user actually asks and what the contest actually scores.

> **Falsifier of the metric's honesty.** If `f_struct` is high counted by nets-in-class but collapses counted by queries-decided, the headline was an artifact of the unit. **Minimal test:** report both; if they diverge, the queries number is the real one and the nets number is retired.

### 2.2 Two denominators

Report `f_struct` against **two** denominators, always both:

- **In-scope:** queries the tool admits — the implemented examinations on the supported PNML subset (BACKLOG.md E5/E7). This measures the tier's reach *within its declared scope*.
- **All-MCC:** every query in the corpus, with every out-of-scope query (CTL/LTL, unsupported arcs, over-`u32` markings — E3/E4/E7) counted as an **abstain**. This measures the tier's reach *against the whole contest*, refusing to launder scope-narrowing into apparent coverage.

A single denominator is a choice about what to hide. The in-scope number alone flatters by excluding the things the tool cannot do; the all-MCC number alone punishes design decisions that were made for soundness (E3's CTL/LTL exclusion is a boundary, not a failure). Reporting both, side by side, removes the degree of freedom an author could use — even unconsciously — to make the number say what they want.

> **Falsifier.** If the in-scope and all-MCC numbers tell *opposite* stories — the tier looks strong in-scope but negligible against all-MCC — the honest headline is the all-MCC number and the thesis must say so. **Minimal test:** the two-denominator table is the deliverable; both columns ship or neither does.

### 2.3 Hold out families, not instances

The MCC corpus is organized into *families* (a parameterized model — a philosophers ring, a token ring, a Petri-net encoding of a protocol — instantiated at many scales). Instances within a family share structure to the point of near-identity. A random instance-level train/test split therefore **leaks family structure**: the model "sees" `philosophers-50` in training and is asked to predict `philosophers-100`, which is not prediction but interpolation within a known shape. Any claim that the structural/search boundary is *predictable* (the fourth clause of the coverage claim) is only meaningful under a **family-held-out** protocol: whole families are withheld, so the test asks whether the boundary generalizes to structures never seen ([the-factorization-residual.md](the-factorization-residual.md) on why structural features should travel across families; [the-sequel.md](the-sequel.md) `split-by-family-not-instance`).

This applies with full force *only* to the predictability clause (the feature→boundary map). The raw coverage fraction `f_struct` is a census, not a prediction, and does not need a held-out split — but it must still be *reported per family*, because a coverage number dominated by one enormous family is a fact about that family, not about the corpus.

> **Falsifier (predictability).** If a cheap-feature classifier separates structural-decided from search-needed queries well under a random split but **collapses to chance under a family-held-out split**, then the boundary is *memorized, not predicted*, and the fourth clause of the claim is refuted. **Minimal test:** report classifier skill (AUC, or balanced accuracy) under both splits; the family-held-out number is the one that counts. This is the cleanest single falsifier in the essay.

### 2.4 Origin-free cost, and the structural-tier ablation as the internal baseline

Two cost disciplines, because "cheaper" is as easy to fake as "more coverage."

First, costs are **origin-free**. Absolute wall-times have no canonical origin (machine speed and load are an arbitrary additive shift in log-cost); only *ratios* and *rankings* survive across machines. The cost claim is therefore stated as a ratio — structural-path cost over search-path cost on the same instance, same machine, same run — never as an absolute milliseconds figure ([self-measurement-harness-plan.md](../self-measurement-harness-plan.md) §2.2, the torsor principle; [principles.md](principles.md)).

Second, the baseline is the **structural-tier ablation**, and it is *internal*. The decisive experiment is not "petrivet versus tool X" (a benchmark-ranking comparison the project has committed *out* — BACKLOG.md *Committed decisions*; MCC ranking is the crucible and the labelling source, not a leaderboard). The decisive experiment is: **turn the structural shortcuts off, force the same query through state-space exploration, and measure the delta** (BACKLOG.md G2). This is the honest counterfactual to "decided structurally without exploration," because it holds everything else fixed and isolates exactly the quantity the claim is about: what the structural certificate *bought*, in coverage and in cost, over doing it the explicit way.

> **Falsifier (the cost half of the claim).** If, on the queries the structural tier decides, the ablation shows the forced state-space path is **no slower** (the cost-ratio distribution straddles 1), then "without state-space exploration" is a true but *empty* statement — the structural path saved nothing — and the performance half of the headline is refuted even if `f_struct` is large. **Minimal test:** the per-query cost-ratio distribution from the ablation; the claim needs its mass well below 1.

---

## 3. The cheapest decisive experiment exists today

The strongest move available is also the cheapest, and it can run *now*, before a single structural generator of Epic B is written.

A **coverage floor** is readable from the code as it stands. Every analysis already returns *which tier concluded* via its method/proof tag — `BoundednessAnalysisMethod`, `LivenessMethod`, the reachability `…Proof` enums, `CommonerHackCriterionResult`. One `Instant` wrapper around the public `analyze_*` surface, one pass over the corpus, and you can already report, per (model, examination): did the structural tier decide it, did the search tier decide it, or did the tool abstain — and at what cost (BACKLOG.md G4a; [self-measurement-harness-plan.md](../self-measurement-harness-plan.md) Phase 1, *zero core change*). That pass yields a floor `f_struct` under both denominators today.

Why "floor"? Because the structural tier is presently *under*-built — `is_covered_by_s_components` is the A2 stub, S-component decomposition and the rank theorem are charted-but-absent ([principles.md](principles.md); BACKLOG.md B2–B6). Whatever coverage the current tier achieves, Epic B can only raise. So the floor is not a weak result to apologize for; it is the **baseline against which every B generator is measured**. It converts Epic B from "build the structural theory because the literature has it" into "build the structural theory because it moves *this measured number*, by *this much*, on *this corpus*." Each generator's acceptance criterion becomes: how many previously-abstained or search-decided queries did it convert, and at what cost-ratio (BACKLOG.md, recommended sequencing, step 4).

This is the move to make first. It is decisive (it can already refute the claim — see below), it is cheap (one timer, one pass, no core change), and it sequences the entire rest of the program around a number that exists instead of a number that is hoped for.

> **Falsifier, available now.** If the floor `f_struct` comes back small under the all-MCC denominator — say, the structural tier decides a single-digit percentage of queries and the rest fall to search or abstention — then either the claim is already in trouble or its weight rests entirely on Epic B closing a large gap, and that gap becomes the thesis's central risk, stated as such. **Minimal test:** run G4a this week; read the two-denominator floor.

---

## 4. The three ways the claim dies, and what the thesis becomes if it does

A claim that cannot fail is not a thesis; it is a slogan. This essay's spirit is to name, in advance and without flinching, the outcomes that refute the headline — and to say what honest story replaces it in each case. The replacement is never "nothing"; it is the stone, which stands regardless.

**Death 1 — the fraction is small.** If `f_struct` is small on both denominators even after Epic B lands, the structural tier is a minor contributor and "decides a *large* fraction" is false. *The honest replacement:* the contribution narrows to the certificate-and-checker ([the-checkable-frontier.md](the-checkable-frontier.md)) and to honest abstention — petrivet decides less than hoped, but *everything it decides it can prove, and where it cannot it says so*. That is a real, defensible, smaller thesis. **Minimal test:** §3's floor, then the post-Epic-B number.

**Death 2 — the decided instances are the easy ones.** If the structurally-decided queries are exactly the ones every mainstream tool also solves in polynomial time — if `f_struct` is large but lands entirely on instances no one found hard — then the coverage is real but *uninteresting*: the tier is fast where speed was free. *The honest replacement:* the same narrowing, plus an explicit boundary statement — the structural tier's reach coincides with the easy region, and the hard region remains search-bound. **Minimal test:** §6's hardness stratification — cross the structural-decided set against an independent hardness signal (oracle-reported difficulty, or the search tier's own cost on the same query). This is the falsifier I rank as most dangerous (§7).

**Death 3 — the structural path is not cheaper.** If the ablation shows forcing state-space exploration costs about the same as the structural certificate on the decided queries (§2.4), then "without state-space exploration" is technically true and practically vacuous. *The honest replacement:* the certificate's value is *epistemic* (a checkable proof) rather than *computational* (a speedup); the thesis re-centers on checkability, and the performance claim is dropped or sharply qualified. **Minimal test:** the ablation cost-ratio distribution.

In all three deaths the firewall still holds and the checker still checks; the project does not collapse, it *re-centers*. That is precisely why naming these deaths is safe to do and necessary to do. A thesis whose author has pre-registered its own falsifiers, and has a defensible position in each branch, is a thesis and not a hope. **This essay exists to invite the refutation it most fears.**

---

## 5. Why the number is trustworthy at all: the certificate underneath every counted query

A coverage fraction is only as honest as the verdicts it counts. `f_struct` counts a query as "decided" — and a decided query is worth counting only if its verdict is *true*. Two structures, built elsewhere in the corpus, are what license the count.

First, **every decided query carries a checked certificate** ([the-checkable-frontier.md](the-checkable-frontier.md); BACKLOG.md C1/C2/G6), re-validated against the *original* net by the in-band check (BACKLOG.md C4) before it is counted. The certifying fraction `f` — the share of accepted verdicts that carry a checked certificate — is the companion figure of merit. Coverage you cannot check is coverage you cannot report.

Second, **the firewall holds** ([soundness-as-a-free-variable.md](soundness-as-a-free-variable.md)): the policy-independence theorem decouples *which tier fired* from *whether the verdict is true*, so routing a query to the structural tier cannot turn a wrong answer into an accepted one. This is what lets `f_struct` be a coverage number rather than a coverage-and-correctness gamble. The two-denominator coverage table and the certifying-fraction table (G4/G6) therefore measure orthogonal things — reach and trustworthiness — and both must be reported.

The dependency is one-directional and load-bearing: **the coverage claim is trustworthy only because the checkable-frontier work and the soundness firewall hold.** This is also why the precondition matters so much. The two `Some(false)` stubs and the bare-boolean shortcuts (BACKLOG.md A2/A6) are queries currently counted as "decided" with *no checkable certificate behind them* — and at least one is trusted-but-wrong. Until A2 lands, any `f_struct` that includes those paths is counting verdicts the firewall does not protect. The soundness work is not parallel to the coverage claim; it is the **precondition** for the coverage claim to mean anything. Measure `f` and `f_struct` together, or neither is honest.

---

## 6. Beyond the structural classes: extending the decided fraction soundly

The coverage claim says "structural certifying tier," and the named theorems are the free-choice classics. But two further deciders extend the *decided* fraction beyond the classical structural classes without leaving the polynomial, certificate-carrying, no-exploration regime — and each widens `f_struct` honestly.

**The continuous (fluid) relaxation as a class-agnostic prove-NO decider** (BACKLOG.md B10). Continuous reachability, coverability, and boundedness — markings in ℝ≥0, fractional firing — are **PTIME** (Fraca–Haddad 2015), and the continuous relaxation soundly over-approximates the discrete question. It is the natural apex of the LP→ILP cascade already in the code, strictly tighter than the state-equation LP, and — uniquely — **class-agnostic**: it can decide *general, unbounded* instances at the ω-frontier where reachability currently returns `Inconclusive`. Its witness is the same Farkas/place-invariant `y` (for the algebraic refutation) or a maximal firing set with a blocking empty siphon (for the firing-set refutation); its checker is a dot product or a polynomial firing-set fixpoint recompute, both against the original net. This is `f_struct` widening into the general-net region the classical structural theorems cannot reach.

**The exact-rational core** (BACKLOG.md B0/B1a) is the other extension, and it works by *subtraction of false positives* rather than addition. The negative reachability verdict currently rests on a *floating-point* solver failing to find a rational solution — a spurious floating "infeasible" on a genuinely feasible system yields a silent **false `Unreachable`** the firewall does not protect (there is no positive object to check on the negative path). Re-deriving infeasibility over ℚ — null-space membership of `m′−m₀` in `ker(Cᵀ)`, or an exact recheck of the rationalized dual — makes every negative verdict exact-certified. This does not raise the raw count; it makes the count *honest*, by ensuring the queries `f_struct` reports as decided-NO are decided-NO in exact arithmetic, not in `f64`.

> **Falsifier (B10's payoff).** If the continuous decider converts essentially *no* currently-abstained `ReachabilityCardinality`/`UpperBounds` queries to sound verdicts on the corpus, the "class-agnostic extension" is theory without corpus payoff, and the coverage claim does not get to lean on it. **Minimal test:** the conversion-count table (B10's stated acceptance criterion), with zero soundness violations against the oracle as the hard gate.

---

## 7. The biggest empirical risk, named

If I had to bet on which sentence of the coverage claim breaks, it is not "a large fraction" — Epic B has real theorems behind it and the floor experiment will tell us early. It is the hidden adjective in **"a large, *characterizable* fraction"** colliding with **Death 2 (§4): the decided instances are the easy ones.**

The danger is specific and plausible. The free-choice and state-machine and marked-graph subclasses are exactly the *well-behaved* corner of net space — and well-behaved is correlated with easy-for-everyone. It is entirely possible that `f_struct` comes back large and clean, every decided query carries a beautiful checkable certificate, the ablation even shows a speedup — and yet the whole decided set sits in the region that explicit state-space tools, partial-order reduction, and structural-reduction front-ends already dispatch cheaply. In that world the number is *true* and the contribution is *thin*: petrivet would be fast and certifying exactly where speed and certification were already available, and silent exactly where the hard queries live.

This is why §6's **hardness stratification** is the experiment I would run *immediately after the floor*, ahead of building more generators. Cross the structural-decided set against an independent hardness signal and look at where the coverage lands. If `f_struct`'s mass sits on the genuinely hard queries — the ones the search tier struggles with, the ones the oracle marks as contested or expensive — the claim is not just true but *interesting*, and the thesis is strong. If `f_struct`'s mass sits on the easy queries, the headline survives literally and dies in substance, and the honest move (§4) is to narrow to the certificate-and-checker and say so in plain words.

I flag this as the biggest risk precisely because it is the failure mode that a careless coverage number would *hide*: a large fraction looks like success, and only the stratification reveals whether it is. The discipline of §2 exists to make that hiding impossible. The essay's whole spirit — name the falsifier, run the cheap decisive test, pre-register the honest fallback — is aimed at this one risk above all.

---

## 8. The claim, with its falsifiers, on one page

**The coverage claim.** On the real MCC P/T corpus, a polynomial structural certifying tier decides a large, characterizable fraction of *queries* (counted queries-decided, two denominators, families held out), each with an independently checkable certificate and without state-space exploration; where it abstains, it abstains honestly; and the structural/search boundary is predictable from cheap structural features. Figure of merit: `f_struct`, reported beside the certifying fraction `f`.

| Sub-claim | Falsifier | Minimal test |
|---|---|---|
| "large fraction" | `f_struct` small on both denominators after Epic B | G4a floor (§3), then post-B number |
| "characterizable / interesting" | decided set = the easy-for-everyone instances | hardness stratification (§6, §7) — **the biggest risk** |
| "without state-space exploration" (cost) | ablation shows forced search no slower | per-query cost-ratio distribution (§2.4) |
| "queries, not nets" (honest unit) | high by nets-in-class, collapses by queries | report both units (§2.1) |
| "two denominators" (no laundering) | in-scope and all-MCC tell opposite stories | the two-denominator table (§2.2) |
| "predictable boundary" | feature classifier collapses to chance family-held-out | both-split classifier skill (§2.3) |
| "independently checkable" (trust) | a counted verdict has no passing certificate | in-band check, certifying fraction `f` (§5) |
| B10 extension | no abstained query converted to a sound verdict | conversion-count table (§6) |

If every row survives, the headline is a thesis. If a row breaks, the table tells you which sentence to delete and §4 tells you what stands in its place. Either way, the stone remains: the certificate-and-checker, and honest abstention. That is what makes this a claim worth staking — and worth trying to break.

---

### A note on register

This essay is the corpus's falsifiability anchor, so it has avoided the metaphors the other essays permit themselves. There is no cathedral here, no song, no spire. There is a number, `f_struct`; a corpus, the MCC; a counterfactual, the ablation; and eight ways the number could be a lie, each with the cheapest experiment that would expose it. The claim sings in tune with the mathematics only if the mathematics is allowed to refute it. That is the difference between a thesis and a slogan, and it is the whole of the discipline this essay was written to keep.
