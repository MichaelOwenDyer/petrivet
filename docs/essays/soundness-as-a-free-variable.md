# Soundness as a Free Variable
## The firewall is the enabling property, not the headline — and that honesty relocates the work

> Status: design essay. Exploratory work (Daniel Dyer, with Claude), built on Michael's tool. The core Petri-net theory is Michael's; the proposed extensions are marked as such. This essay owns the **soundness firewall**: the theorem that the verdict's correctness is independent of which decider is selected, why that theorem is nearly trivial, where the code violates its precondition, and the certifying fraction `f` that measures it. The broader inversion it sits inside — certificate-as-stone, coverage-as-headline, firewall-as-enabling-property — is stated once in [README.md](README.md). The companion headline `f_struct` is in [the-coverage-claim.md](the-coverage-claim.md); the certificate format and hardness map are in [the-checkable-frontier.md](the-checkable-frontier.md).

---

## 1. The firewall, stated

`petrivet` is a *certifying* analyzer: every decider that concludes can, in principle, emit a machine-checkable witness, and a small external checker re-validates it against the original net. The decision problems it answers — reachability, liveness, boundedness — have a worst case no engineering removes (EXPSPACE-hard where decidable, Ackermannian in the unbounded case), so the tool dispatches each net to the cheapest technique its structure admits and returns `Inconclusive` where none applies ([`reachability.rs`](../../petrivet/src/api/system/reachability.rs)).

The **soundness firewall** is the property this essay owns: *no matter which decider the tool selects, the answer it returns is sound.* It is the enabling property beneath the headline coverage fraction `f_struct` ([the-coverage-claim.md](the-coverage-claim.md)) — what makes that measurement *trustworthy* and the eventual learned-selection sequel *safe*. Stating it as a theorem is easy; the work is in earning its precondition and measuring its reach.

---

## 2. The theorem, stated precisely

Fix a property $P$ (say, reachability of a target marking) and a net $N$ with query $q$. A **decider** $d$ is a partial procedure returning either $\bot$ (inconclusive) or a verdict $v \in \{\textsf{yes}, \textsf{no}\}$ together with a certificate $c$. Call $d$ **certifying** if there is a checker $\mathrm{chk}_P(N, q, v, c) \in \{\textsf{accept}, \textsf{reject}\}$ that is *sound*: $\textsf{accept} \Rightarrow v$ is the true answer to $P$ on $(N, q)$. The checker — not the decider — is the trusted computing base. A **policy** $\pi$ chooses which decider to run next, as a function of the net, the query, and the history of attempts.

> **Theorem (soundness is policy-independent).** Let $D$ be a portfolio of certifying deciders for $P$, each with a sound checker $\mathrm{chk}_P$. Run them under any policy $\pi$, accepting a verdict only when its certificate passes $\mathrm{chk}_P$. Then the accepted verdict is the true answer to $P$ on $(N, q)$, for every $\pi$.

**Proof.** The execution returns $v$ only when some decider produced $(v, c)$ and $\mathrm{chk}_P(N, q, v, c) = \textsf{accept}$. By soundness of the checker, $\textsf{accept} \Rightarrow v$ is true. The policy $\pi$ governs only *which* $(v, c)$ pairs are generated and *in what order* they are tested; it cannot accept an unchecked certificate nor alter $\mathrm{chk}_P$. Hence the accepted verdict is true irrespective of $\pi$. $\blacksquare$

> **Corollary (learning is confined to performance).** Because the conclusion holds for *all* $\pi$, the policy may be optimized for any performance objective — expected time, resource budget, anytime quality — by any procedure, including reinforcement learning under a misspecified reward. A wrong policy wastes time; it cannot return a wrong answer. The learner lies entirely outside the trusted base $\{\mathrm{chk}_P\}$.

---

## 3. The theorem is nearly trivial — and that is the point

The proof is one paragraph, and every step is a definition unfolding: *certifying* was defined as "$\textsf{accept} \Rightarrow$ true"; the policy was defined as "chooses order, nothing else"; the conclusion is their conjunction. No induction, no case analysis, no inequality. It is a corollary of two results that predate this project by decades:

- **Certifying algorithms** (McConnell, Mehlhorn, Näher, Schweitzer, *Computer Science Review*, 2011): an algorithm should emit a witness a simple checker can verify, moving trust from the (complex, possibly buggy) solver to the (simple, auditable) checker. The SAT community's DRAT/GRAT discipline is the canonical instance — trust only the tiny verified `gratchk`, never the unverified `gratgen` (Lammich, *CADE* 2017).
- **Algorithm selection** (Rice, *Advances in Computers*, 1976; operationalized by SATzilla — Xu, Hutter, Hoos, Leyton-Brown, *JAIR* 2008): choose, per instance, which of several procedures to run, using cheap features of the instance.

Compose them — "select among procedures, each of which is certifying" — and the theorem falls out with no further mathematics. **As a theorem it earns no credit.** Saying so forces the question: *if the theorem is free, where is the work?*

The work is in the **precondition**: the hypothesis that *every decider in the portfolio is certifying.* A decider that returns a bare verdict with no checkable witness is, in effect, part of the trusted base; scheduling it under any $\pi$ neither protects nor repairs it. The strength of the firewall is therefore *exactly* the fraction of the decider set that is certifying — and nothing in the theorem makes that fraction large. Making it large is engineering, and it is the actual project.

---

## 4. Where the code violates the precondition today

The precondition is not satisfied. Two deciders return a *definitive* `Some(false)` — a confident negative verdict — where the underlying theory gives an exact answer they have simply not computed. They are not abstaining; they are asserting, and one of them asserts a falsehood.

**The first** is `is_covered_by_s_components`, which returns `false` unconditionally:

```rust
// petrivet/src/api/net/mod.rs:270
pub fn is_covered_by_s_components(&self) -> bool {
    // todo
    false
}
```

This is consumed on the live-free-choice boundedness path at [`boundedness.rs:67`](../../petrivet/src/api/system/boundedness.rs): a live free-choice net is bounded iff every place lies in an S-component, so a hardcoded `false` reports a genuinely bounded net as *not* efficiently bounded. Here the damage is contained — the caller falls through to `is_structurally_bounded` and the coverability graph ([`boundedness.rs:141`](../../petrivet/src/api/system/boundedness.rs)) — but the *shape* is the violation: a fast decider asserting a verdict it has not earned.

**The second** is the marked-graph arm of `is_efficiently_live`:

```rust
// petrivet/src/api/system/liveness.rs:106-108
NetClass::MarkedGraph => {
    Some(false) // todo  ← liveness.rs:107, the fabricated negative verdict
},
```

A marked graph is live iff every circuit is marked — an exact, polynomial structural test. Returning `Some(false)` reports *every* marked graph as non-live. Because `is_live` short-circuits on `is_efficiently_live` ([`liveness.rs:118`](../../petrivet/src/api/system/liveness.rs)) before consulting the reachability graph, this is **trusted-but-wrong**: a live marked graph is reported non-live with no fallback. This is the real soundness defect in the present construction, and **it is not the machine learning** — no learned policy is anywhere near it. It is a `// todo` in a `match` arm.

The near-term first move is therefore not to build a learner. It is to **demote both stubs to abstention**: a decider that cannot yet certify must return `None` and escalate, never a fabricated `Some(false)`. This is item **A2** in the [backlog](../../BACKLOG.md) — the precondition of the firewall — and the stated first move in [for-michael.md](for-michael.md). Demotion is the floor; the ceiling is to make each arm emit a certificate (the S-component cover; the circuit token counts — backlog B3, B4), converting abstention into a checked positive verdict.

There is a quieter cousin worth naming for honesty's sake. The `Unreachable` verdict on the general path rests on the *floating-point* LP failing to find a rational solution ([`reachability.rs:172`](../../petrivet/src/api/system/reachability.rs)). A spurious numerical "infeasible" on a genuinely feasible system would be a silent false `Unreachable` — and the firewall does **not** protect it, because on the negative path there is no positive witness to re-check (backlog B1a). This is a subtler obligation than the two stubs: negative verdicts must be re-derived in exact arithmetic before they are trusted. It is flagged here so the firewall's coverage is not overstated.

---

## 5. The trusted base, the checker, and the figure of merit `f`

Once the precondition is discharged, the firewall has a precise extent, and that extent is measurable. The trusted base is not "the tool"; it is

$$ \text{TCB} \;=\; \{\text{certificate checkers}\} \;\cup\; \{\text{remaining bare-boolean deciders}\}. $$

The design goal, following the GRAT discipline, is to shrink the right-hand union to empty and the left-hand set to something small enough to audit and eventually to verify formally. Each checker must re-establish the property against the **original** $(N, q)$ — assuming nothing about which decider or reduction produced the witness, sharing no code with the generators beyond primitive net access (backlog C1). That original-net discipline is what holds the trusted base constant under reduction-lifting (§7) and what would let the certificate format be tool-agnostic. The trust boundary is a single line in the decision loop:

```rust
if certificate.check(net, m0, query) {        // the trusted base — and the precondition of §3
    return Verdict::Proven(verdict, certificate);
}
```

The figure of merit for the firewall is the **certifying fraction**

$$ f \;=\; \frac{\#\{\text{accepted verdicts carrying a checked certificate}\}}{\#\{\text{accepted verdicts}\}}. $$

`f = 1` means the firewall is total: every committed answer is backed by a witness an independent checker accepted, and the bare-boolean trusted base is empty. `f < 1` names exactly how far short of that the tool falls — the honest accounting of the gap between the theorem's hypothesis and the code's reality. The discipline (backlog A6, C5) is to **report `f` over the corpus and require the bare-boolean trusted base to be non-increasing in CI**: every release may certify more, never less. This is the firewall as a *built and measured* artifact rather than a stated theorem, and it is the contribution.

Two further measurements complete the picture, both independent of any learner: the **check-pass rate** (every certificate produced in testing must re-validate, or CI fails — backlog C2, G6), and the **map of the checkable frontier** ([the-checkable-frontier.md](the-checkable-frontier.md)) — the per-property, per-polarity table of where compact certificates exist and where complexity theory forbids them, where the firewall meets its limits with the same candor it meets its successes.

---

## 6. "A free variable" — stated exactly, and why deferral is honest

The slogan in the title is precise and worth stating without ornament. *Soundness is a free variable* means: **the learner is free to be arbitrarily wrong about cost while being structurally incapable of being wrong about truth.** Cost — which decider is fastest on this net, when to abandon a slow solve, which order minimizes expected time — is the variable the learner optimizes, and it may misjudge it badly: a bad ranker can be slower than the hand-ordered cascade. Truth — whether the returned verdict is correct — is held constant by the checker, the same value for every $\pi$. The learner moves the first freely and cannot touch the second.

This is what makes the eventual selection sequel *safe* and deferring it *honest*. The sequel's lineage is **SATzilla / Rice algorithm selection** — a cost-sensitive ranker over structural features, sound by the theorem, no deep learning required — escalating later to a sequential policy and, at the far end, a planner over certified reductions ([the-sequel.md](the-sequel.md)). It is gated behind the firewall (a mis-selection must cost time, never soundness — backlog D5–D8) and justified only by a *measured* gap between the single-best and virtual-best decider on the corpus. If that gap is within noise on a six-arm portfolio, the hand-ordered cascade is the honest answer and the learner is dead weight. Because the firewall makes the verdict sound regardless, we can *afford* to wait for the measurement rather than build on a hope.

The AlphaGo / MuZero analogy is kept as a clarifying contrast, not a thesis: Go's leaves are *estimated*, so a wrong value network costs the game, whereas `petrivet`'s leaves are *checked*, so a wrong policy costs milliseconds — which makes `petrivet`'s problem *easier and different*, not grander. Likewise the effective-theory material is a *feature-design heuristic* (prefer structural macro-features whose mutual information with hardness is high at low description length; backlog X4), **labelled speculation, never a soundness argument**. The firewall stands on the checker alone.

---

## 7. The one place the firewall must be proven, not assumed

There is a single point in the construction where the firewall is not free, and intellectual honesty requires naming it. It is **certified reductions** ([the-sequel.md](the-sequel.md); backlog Epic F).

A reduction is a property-preserving transformation carrying an applicability witness and a `lift` that maps a certificate on the *residual* net back to the *original*. The firewall's promise is that even a buggy `lift` cannot break soundness, because the lifted certificate is re-checked against the original net: a wrong lift produces a certificate the original-net checker rejects, the search backtracks, and the cost is time, not correctness. That argument is clean — *for existential witnesses.* A firing sequence either fires on the original net or it does not; the checker replays it and the bug is caught.

It is **not** automatically clean for **compositional or invariant** lifts. A buggy interface correction could, in principle, produce a *too-weak* certificate that the checker accepts — the witness checks out, but it witnesses less than the property requires. There the robustness property is no longer a corollary of "re-check against the original net"; it becomes a per-certificate-kind **checker-completeness obligation** that must be *proven*. The disciplined response (backlog F1): restrict the *trusted* reduction lifts to existential witnesses until that obligation is discharged for each compositional certificate kind. This is the one open theoretical liability in an otherwise free firewall, recorded rather than papered over. The boundary between what the checker buys for free and what it does not is mapped in [the-checkable-frontier.md](the-checkable-frontier.md).

---

## 8. What this essay commits to, and what it hands off

The reconciled position, stated as plainly as the mathematics allows:

- The soundness theorem is true and nearly trivial — a corollary of certifying algorithms composed with algorithm selection. Its value is not as a result but as a *signpost* to its precondition.
- The precondition — every fast decider is certifying — is **violated today** in the two `Some(false)` stubs ([`api/net/mod.rs:270`](../../petrivet/src/api/net/mod.rs); [`liveness.rs:107`](../../petrivet/src/api/system/liveness.rs)). Demoting them to honest abstention (backlog A2) is the near-term first move and the precondition of everything downstream.
- The contribution is the **firewall built and measured**: the trusted base reduced to the certificate *checker* (the GRAT discipline), with the certifying fraction `f` reported and held non-increasing in CI, and the checkable frontier mapped.
- "Soundness is a free variable" means the learner is free to be wrong about cost while structurally unable to be wrong about truth. This makes the [selection sequel](the-sequel.md) safe and makes deferring it honest.
- The one place the firewall must be *proven* rather than assumed is the compositional `lift` ([the-sequel.md](the-sequel.md)); existential lifts are sound for free, compositional ones carry an open obligation.

The empirical headline `f_struct` is the companion claim, carried in [the-coverage-claim.md](the-coverage-claim.md). The factorization residual that measures what *cannot* be coarse-grained is in [the-factorization-residual.md](the-factorization-residual.md). The components this design presupposes are in [foundational-design.md](../foundations/foundational-design.md); the organizing principles in [principles.md](principles.md), and the architectural reading in [latent-architecture.md](latent-architecture.md).

The single fact under all of it: a verdict here is never a guess but a proof — and unlike in Go, every leaf can be checked. The theorem that says so is free. Earning its hypothesis, and measuring how much of it the code has earned, is the work.

![The certifying portfolio loop with its trust boundary: any policy may schedule the deciders; only a verdict whose certificate the checker accepts is returned, and the certifying fraction f is the share of accepted verdicts that reach that checked state.](figures/certifying-portfolio.svg)

---

### References (curated, verified)

- McConnell, Mehlhorn, Näher, Schweitzer. *Certifying algorithms.* Computer Science Review 5(2) (2011). Lammich. *Efficient Verified (UN)SAT Certificate Checking* (GRAT). CADE 2017 / JAR 2019.
- Rice. *The Algorithm Selection Problem.* Advances in Computers 15 (1976). Xu, Hutter, Hoos, Leyton-Brown. *SATzilla.* JAIR 32 (2008). Kotthoff. *Algorithm Selection for Combinatorial Search Problems: A Survey* (2014).
- Blondin, Haase, Offtermatt. *Directed Reachability for Infinite-State Systems* (FastForward). TACAS 2021 — the domain-matched precedent: an LP/continuous over-approximation as a sound-on-success distance oracle for Petri reachability, the witnessing firing sequence self-certifying.
- Si et al. *Code2Inv.* NeurIPS 2018; Giacobbe et al. *Neural Model Checking.* NeurIPS 2024 — learner-proposes / checker-disposes, soundness from the SMT checker. DeepMind. *AlphaProof* (Nature 2025) — every output gated by the Lean kernel.
- Leroux, Schmitz; Czerwiński, Orlikowski — the reachability complexity frontier (Ackermann-complete unbounded; EXPSPACE-hard where decidable), the worst case no policy removes.
- Silver et al. *Mastering the game of Go…* Nature 529 (2016), 550 (2017); Science 362 (2018). Schrittwieser et al. *…planning with a learned model* (MuZero). Nature 588 (2020) — retained only as the clarifying contrast (estimated leaves vs. checked leaves), not as a design justification.

*A note on register, carried from the inversion: where an earlier draft reached for the AlphaGo framing and the effective-theory metaphysics as organizing arguments, this rewrite keeps them as, respectively, one clarifying contrast and one labelled feature-design heuristic. The load-bearing claims here name their falsifiers — the two stubs (a regression test), the fraction `f` (a CI gate), the compositional-lift obligation (a checker-completeness proof) — and rest on the checker alone.*
