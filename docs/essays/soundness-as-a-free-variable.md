# Soundness as a Free Variable
## A learned algorithm-selection policy over a certificate-gated decider lattice, extended from `petrivet`'s certifying apparatus

> Status: design paper / vision. This is exploratory work (Daniel Dyer, with Claude), built on Michael's tool. It realizes Rung 1 of the ambition ladder in its companion implementation plan, [`self-measurement-harness-plan.md`](../self-measurement-harness-plan.md). The deepest theory is Michael's; the dreams here are marked as such.

**Abstract.** The decision problems `petrivet` answers — reachability, liveness, boundedness — have worst-case complexities that no engineering removes: Ackermannian in the unbounded case, EXPSPACE-hard even when decidable. Yet the tool is fast on real instances, because it does not solve the worst case; it *routes around* it, dispatching each net to the cheapest technique its structure admits. This paper argues that this routing — today a hand-written cascade of `match self.class()` arms — is precisely an algorithm-selection problem of the kind that machine learning solved for game-tree search, and that `petrivet` occupies an unusually strong position to learn it: it is a *certifying* analyzer, in which every successful decider emits a machine-checkable proof. We prove that, over a portfolio of certifying deciders, **soundness is independent of the selection policy** — the policy is a free variable that may be optimized for cost by any means, including learning under a misspecified reward, without ever endangering correctness. This is the AlphaGo move (learn the distribution of good moves over an intractable search) with a property Go never had: *every leaf is a verified proof*, so the learner sits wholly outside the trusted base. We formalize the scheduling problem as a sequential decision process with verified terminal rewards, map the policy/value/search triad onto the existing decider set, show that the MCC harness is already a self-labeling training-data generator (the certificate is the label; no oracle required), give the minimal extension of the real code that realizes each rung of an ambition ladder from a SATzilla-style ranker to a MuZero-grade planner over structural reductions, and ground the *learnability* of instance-hardness in the effective-theory literature, which also dictates which features the policy should consume. We close with an honest account of the one precondition the theorem demands — that every fast decider be certifying, which today's bare-boolean shortcuts and two `Some(false)` stubs violate — and a disciplined coda on Petri nets as cellular automata and the policy as the *effective theory of verification hardness*.

---

## 1. The move

AlphaGo did not make Go's game tree smaller. The branching factor stayed ~250, the depth ~150, the tree astronomical. What changed is that two learned functions — a **policy network** giving a distribution over moves (narrowing *which* branches to explore) and a **value network** estimating the winner (truncating *how deep* to look) — let Monte-Carlo tree search concentrate its budget on the regions that matter (Silver et al., *Nature* 2016, 2017; *Science* 2018). The hard problem was never solved; the *distribution of good play* was learned, and the search rode it. Eric Jang's recent from-scratch reimplementation makes the sociological point that this is now cheap — a few thousand dollars of commodity compute, with KataGo's 40× efficiency gain as the load-bearing fact — but the conceptual point is older and exact: **when a search problem has intractable worst cases but a structured instance distribution, learning the distribution collapses the operative difficulty even though the worst case is untouched.**

`petrivet`'s analysis problems have this shape exactly. The README is admirably candid that unbounded reachability, liveness, and deadlock-freedom are decidable but Ackermannian, and the tool returns `Inconclusive` rather than pretend otherwise ([`reachability.rs`](../../petrivet/src/api/system/reachability.rs)). But on the *distribution* of nets people actually analyze — and the Model Checking Contest corpus is precisely such a distribution — the operative question is not "what is the worst case" but "which of my techniques will decide *this* instance cheaply." That meta-question is low-dimensional and statistical. It is the **Algorithm Selection Problem** (Rice, 1976), and learning it is what portfolio SAT solvers like SATzilla (Xu, Hutter, Hoos, Leyton-Brown, *JAIR* 2008) have done for two decades.

What makes `petrivet` special is not that it *could* host such a learner — anything could — but that it can host one **without putting machine learning anywhere near the trusted base.** Because every decider that concludes emits a checkable certificate (a firing sequence, a Parikh vector, an invariant, a siphon/trap pair), a learned scheduler can only ever change *which proofs are attempted and in what order*; it can never cause a false proof to be accepted. This is the property Go does not have. In AlphaGo the value network's estimate at a non-terminal leaf *is* the signal — a wrong estimate costs you the game. In `petrivet` a leaf is a proof: a wrong policy estimate costs you milliseconds before the next decider, or the exhaustive fallback, produces a real one. MuZero (Schrittwieser et al., *Nature* 2020) sharpens the contrast from the other side: it learns the *dynamics model itself*, inside the trust boundary, and its correctness is empirical, not certified. `petrivet` is the inverse — the learner is outside, and correctness is structural.

The rest of this paper makes that sentence a theorem, then builds outward from the code that already exists.

---

## 2. The scheduling problem, formally

Fix a property $P$ (say, reachability of a target marking) and a net $N$ with query $q$. A **decider** $d$ is a partial procedure that returns either $\bot$ (inconclusive) or a verdict $v \in \{\textsf{yes}, \textsf{no}\}$ together with a certificate $c$. Call $d$ **certifying** if there is a checker $\mathrm{chk}_P(N, q, v, c) \in \{\textsf{accept}, \textsf{reject}\}$ that is *sound*: $\textsf{accept} \Rightarrow v$ is the true answer. The checker — not the decider — is the trusted base, and the design goal is to keep it small (ideally formally verified, as the SAT community keeps only `gratchk` in its trusted base while the proof *generator* `gratgen` stays unverified; Lammich, *CADE* 2017).

`petrivet` already has the decider set: the `is_efficiently_*` / `analyze_*` family. For reachability the ordered set is — trivial equality; S-net token conservation; live-T-net integer marking equation; rational LP filter; integer ILP filter; Karp–Miller exploration — each gated by structural class, each with a known cost, polarity, and soundness domain:

| Decider | Cost | Polarity | Terminating? | Certificate |
|---|---|---|---|---|
| token-sum (S-net) | $O(|P|)$ | prove-NO; +YES if strongly connected | always | `LiveStateMachineTokenConservation` |
| integer marking eq. (live T-net) | ILP | exact | always | Parikh vector `HashMap<Transition,u32>` |
| rational LP filter | LP | prove-NO only | always | (Farkas dual — *discarded today*) |
| integer ILP filter | ILP | prove-NO only | always | (dual — discarded) |
| CHC siphon/trap | poly | exact on free-choice | always | `SiphonTrapPair`s / counterexample siphon |
| structural-boundedness LP | LP | prove-YES (bounded) | always | weight vector `Box<[f64]>` |
| Karp–Miller | exp | exact if bounded | coverability: yes; reachability: only if bounded | firing sequence / ω-marking |

We model selection as a **sequential decision process**. A *state* is $s = (N, q, H, \rho)$ where $H$ is the history of deciders already attempted and $\rho$ is any residual produced by structural reductions (initially $\rho = N$). An *action* is "run decider $d$" or, in the richest formulation, "apply reduction $r$" (producing a smaller residual). Running $d$ either reaches a **terminal** state (it returned $(v,c)$ and $\mathrm{chk}_P$ accepts — reward $= -\text{cost incurred}$, done) or continues. A **policy** $\pi(d \mid s)$ chooses the next action; a **value** $V(s)$ estimates the expected remaining cost to an accepted certificate. The mapping to the AlphaGo triad is now exact:

- **Policy network** $\leftrightarrow$ $\pi(d \mid s)$: which decider to attempt next — *narrows the breadth* of the portfolio search.
- **Value network** $\leftrightarrow$ $V(s)$: expected cost-to-certificate from here (e.g., "this net will hit ω; skip exploration") — *truncates the depth*, pruning hopeless branches.
- **MCTS / search** $\leftrightarrow$ the certificate-gated execution itself — but with **real rollouts** (running a decider *is* attempting a proof, not simulating one) and **verified terminal rewards** (a passing certificate is ground truth, not a learned estimate).
- **Self-play + distillation** $\leftrightarrow$ run the portfolio over the corpus, log $(\varphi(N), d, \text{accepted?}, \text{cost})$, train $\pi$ and $V$; the certificate is the label (§5).

---

## 3. The soundness theorem

> **Theorem (Soundness is policy-independent).** Let $D$ be a portfolio of certifying deciders for property $P$, each with a sound checker $\mathrm{chk}_P$. For every policy $\pi$, the certificate-gated execution of $D$ under $\pi$ returns a verdict $v$ only if $v$ is the true answer to $P$ on $(N,q)$. The returned answer is sound regardless of $\pi$.

**Proof.** The execution returns $v$ only if some decider produced $(v, c)$ and $\mathrm{chk}_P(N, q, v, c) = \textsf{accept}$. By soundness of the checker, $\textsf{accept} \Rightarrow v$ is the true answer. The policy $\pi$ influences only *which* deciders are run and in *what order* — hence which $(v,c)$ pairs are generated and tested — and cannot return an unaccepted certificate nor modify $\mathrm{chk}_P$. Therefore the returned verdict is true irrespective of $\pi$. $\blacksquare$

> **Corollary (Learning is confined to performance).** Since soundness holds for *all* $\pi$, the policy may be chosen to optimize any performance objective — expected time-to-accept, resource budget, anytime quality — by any procedure, including reinforcement learning under a possibly-misspecified reward. A wrong policy wastes time; it cannot produce a wrong answer. The learner lies entirely outside the trusted base $\{\mathrm{chk}_P\}$.

This is the formal content of "the certificate is the firewall." It is what separates this proposal from the unsound branch of the literature (NeuroSAT answering directly; data-driven Petri-reachability approximators; MuZero's uncertified learned model) and aligns it with the sound branch: SATzilla and Graph-Q-SAT (learning *outside* the trust boundary, scheduling or branching within a complete solver); and the *proposer–checker* systems — Code2Inv, which by its own description "takes a verification task **and a proof checker as input**" and accepts only SMT-certified invariants; Neural Termination and Neural Model Checking, which learn a candidate and then *SMT-check it* ("formally sound, and practically effective", Giacobbe et al., *NeurIPS* 2024); and the headline case, AlphaProof, an AlphaZero-style search whose every output is gated by the Lean kernel — *"if the Lean verifier accepts a proof, it is correct by construction"* (silver-medal at IMO 2024). The nearest *domain* neighbor is **FastForward** (Blondin, Haase, Offtermatt, *TACAS* 2021), which already uses Petri-net over-approximations — the state equation, the continuous relaxation — as **distance oracles for A\*** toward a target marking, returning the witnessing firing sequence on success. FastForward is the proof of concept that a *heuristic in the guidance slot* is sound-on-success because the found sequence is self-certifying. Our proposal is its generalization: put the *learned* heuristic in the slot, and let the certificate calculus extend the guarantee from one property to all of them.

> **Remark (the precondition — and where today's code violates it).** The theorem requires every decider in $D$ to be *certifying*. A **trusted** decider — one returning a bare verdict with no checkable certificate — is, in effect, part of the trusted base; scheduling it with a learned $\pi$ neither protects nor repairs it. The strength of the firewall is exactly the fraction of the decider set that is certifying. Today many `petrivet` deciders return bare booleans (`is_efficiently_bounded -> Some(bool)`, `is_live() -> bool`), and two of them — [`is_covered_by_s_components`](../../petrivet/src/api/net/mod.rs) and the marked-graph liveness arm in [`liveness.rs`](../../petrivet/src/api/system/liveness.rs) — return a *definitive* `Some(false)` where the theory gives an exact answer, i.e., they are trusted-but-wrong. **This is the real soundness risk in the whole construction, and it is not the machine learning.** Making every fast decider emit a certificate (the `Certificate::check` of the companion essay) is therefore not a parallel nicety but the *enabling precondition* for safe learning. The evidence calculus and the decision portfolio are one project.

---

## 4. Why the intractable problem becomes learnable

The Theorem says learning is *safe*. It does not say it *helps*. The case that it helps has a rigorous spine.

The worst case is genuinely untouched: finite P/T-net reachability is EXPSPACE-hard and, unbounded, Ackermann-complete (Czerwiński–Orlikowski; Leroux–Schmitz). No policy changes this. But hardness on a *distribution* is a different object. The effective-theory literature establishes precisely that an intractable micro-dynamics can admit a tractable, low-dimensional *macro*-theory — when the macro-variables are chosen well. Israeli and Goldenfeld (*PRL* 2004) showed that elementary cellular automata across all Wolfram classes — *including Rule 110, which is Turing-universal* — can be coarse-grained into predictable macro-rules, under one exact condition: the coarse-graining must commute with the dynamics,
$$ \mathrm{CoarseGrain} \circ \mathrm{MicroEvolve} \;=\; \mathrm{MacroEvolve} \circ \mathrm{CoarseGrain}, $$
i.e., the macro-variables must be *autonomous* — closed under the dynamics, predicting their own future without the micro-detail. Computational mechanics makes the optimal version a theorem: the *causal states* (Shalizi–Crutchfield, 2001) are the **coarsest partition of histories that remains sufficient for prediction** — the minimal sufficient statistic, the right macro-variables by construction. The information bottleneck (Tishby–Pereira–Bialek, 1999) gives the tunable Lagrangian form of the same trade-off.

Read into our setting, this is not philosophy but **feature-design doctrine**:

> A learned selection policy should consume macro-features of the net that are (i) approximately *autonomous under the firing dynamics* — structural and invariant quantities that summarize behavior without tracking the micro-marking — and (ii) *sufficient* for the hardness label, i.e., conditioning on them makes predicted difficulty approximately independent of the discarded micro-detail. Prefer aggregate descriptors — structural class, strong-connectivity, P/T-invariant dimension, S-/T-component and siphon/trap counts, NUPN unit-tree shape, concurrency and token-sum summaries — over the raw per-place marking. A feature earns its place to the extent that $I(\text{feature};\text{hardness})$ is high at low description length; this is a measurable selection test against the corpus, not a guess.

This is why the policy's input is the *structural* feature vector, and it is a happy fact that those are exactly the quantities `petrivet` already computes cheaply (§5). The raw reachability graph is the intractable micro-substrate; `NetClass`, `is_strongly_connected`, structural-boundedness, and the invariant/decomposition descriptors are the candidate autonomous macrostates. The policy is, in the most literal defensible sense, learning the *effective theory of verification hardness* over this net distribution.

---

## 5. Extending from the apparatus: what exists, what is one hook away

The proposal's credibility rests on how little new machinery it needs. The audit is encouraging: the *soundness substrate is nearly complete and the learning substrate is pure plumbing.*

**The decider set already exists** as the `is_efficiently_*` / `analyze_*` family. **The "policy" already exists too — as a single `if`.** The MCC harness's entire instance-dependent technique choice is one branch in [`run_liveness`](../../mcc-2026/petrivet-mcc/src/main.rs):

```rust
if system.class().is_free_choice() && system.commoner_hack_criterion().is_ok() {
    print_boolean_result(name, true, STRUCTURAL_TECHNIQUES);   // structural shortcut fired
    return Ok(());
}
let rg = system.try_build_reachability_graph()?;
print_boolean_result(name, rg.transition_liveness().is_live(), DEFAULT_TECHNIQUES); // explicit RG
```

The hardcoded `STRUCTURAL_TECHNIQUES` / `DEFAULT_TECHNIQUES` tags are a *faithful binary record of which of two deciders fired* — the embryo of a telemetry signal. The learned policy is the generalization of this one branch from a hand-coded predicate to a function of the full feature vector.

**The certificate is already the training label.** The MCC oracle ([`oracle.rs`](../../mcc-tests/src/oracle.rs)) parses the community-consensus verdict files (`value = None` encodes the literal `?`, "consensus not reached") — a useful independent cross-check, but *not required for training*. By the Theorem, whenever a decider's certificate passes its checker, the verdict is ground truth. So running the portfolio over the corpus emits $(\varphi(N), \text{property}, \text{decider}, \text{accepted?}, \text{cost})$ tuples that are **self-labeling**: no oracle, no human annotation. This is "self-play" against the cost objective. An accounting of the tuple against today's code:

| Field | Status | Where |
|---|---|---|
| features $\varphi(N)$ | present as *data*, absent as a *vector* | the cheap accessors below |
| property | present | `Examination` / `BK_EXAMINATION` |
| decider tried | present only for liveness (the technique tag); else internal | `main.rs`; `is_efficiently_*` |
| conclusive? | present | `RunResult`, `…Result::Inconclusive` |
| certificate | **present and rich** | the proof enums |
| wall-time | **absent** — no `Instant` anywhere in the workspace | — |

**The feature vector $\varphi(N)$ is assembled from accessors that already exist**, most cached on `DenseNet` at build time: `NetClass` and its four `const fn` sub-predicates; `is_strongly_connected` (cached `tarjan_scc(...).len() == 1`); place/transition/node/arc counts; `is_structurally_bounded` (one LP); the initial token sum; minimal-siphon counts (today only via the CHC side-effect). The one genuinely missing structural feature is the **NUPN unit-tree shape** (depth/width/`unit_count`/`unit_safe`) — the corpus carries it, but only `place_count_from_nupn` reads the `<size>` tag.

**What does not exist** is the thin learning scaffold, and it attaches to named code:

```rust
enum Outcome<V> { Decided { verdict: V, certificate: Cert }, Inconclusive }

trait Decider<Q, V> {
    fn cost_class(&self) -> CostClass;            // O1 | Lp | Ilp | Poly | Exp
    fn polarity(&self)  -> Polarity;             // ProveYes | ProveNo | Exact
    fn admissible(&self, phi: &Features) -> bool; // soundness domain
    fn run(&self, sys: &PetriNet, q: &Q, budget: Budget) -> Outcome<V>;
}

trait Policy { fn next(&self, st: &SearchState) -> Option<DeciderId>; }   // the ONLY learned part

fn decide<Q, V>(sys: &PetriNet, q: &Q, ds: &[Box<dyn Decider<Q,V>>], pi: &dyn Policy)
    -> Verdict<V>
{
    let mut st = SearchState::new(phi(sys));                 // phi(): assemble cached accessors
    while let Some(id) = pi.next(&st) {                      // policy schedules; soundness-irrelevant
        if let Outcome::Decided { verdict, certificate } = ds[id].run(sys, q, st.budget()) {
            if certificate.check(sys, q, &verdict) {         // the firewall — the trusted base
                return Verdict::Proven(verdict, certificate);
            }
        }
        st.record(id, /* outcome, cost */);                  // the telemetry tuple
    }
    Verdict::Inconclusive
}
```

Four additions, each with a home: the `Decider` trait wraps the `is_efficiently_*` ladder; `phi()` concatenates the cached accessors (plus a new NUPN parse); the telemetry `record` belongs in [`run_analysis`](../../mcc-tests/src/runner.rs) — the harness's explicitly "measured function" — wrapped in `Instant::now()/elapsed()`; and a `Budget`/deadline with cooperative cancellation, which the codebase entirely lacks today (the only "give up" is the ω short-circuit and process-level `catch_unwind`), threaded into the exploration loop and the LP/ILP solver calls. Note that the firewall line, `certificate.check(...)`, is the companion essay's `Certificate` trait: the precondition of §3 is the same line of code.

---

## 6. An ambition ladder

Calibrated to effort, because most of the value is cheap:

- **Rung 0 — today.** Hardcoded per-class `match`; one instance-dependent branch. Static, sound, leaves wins on the table.
- **Rung 1 — empirical hardness model.** A cost-sensitive ranker (gradient-boosted trees / random forest over $\varphi(N)$, SATzilla-style) predicting the fastest *admissible* decider. Sound by the Theorem, trains in minutes on the corpus, almost certainly captures most of the achievable speedup. **No deep learning required.**
- **Rung 2 — sequential policy.** A contextual bandit, then full RL, over decider *order* with a deadline budget: learns when to abandon a slow ILP or a diverging exploration and escalate, and supports anytime *parallel racing* of cheap deciders (needs the cancellation hook). The reward is wall-time; misspecify it freely — §3 protects soundness.
- **Rung 3 — the MuZero-grade dream.** Promote **structural reductions** (agglomeration, implicit-place removal, NUPN decomposition into independent sub-problems) to *actions*. Now the state is a shrinking residual net, the action space is heterogeneous (decide vs. transform), and we have a genuine branching game tree over reduced problems — policy + value + search, AlphaZero in form. The discipline that keeps it sound while MuZero is not: **each reduction must itself be certifying** — sound iff property-preserving, with a witness — so that even the learned *transformations* stay outside the trusted base. (Specced separately in [rung-3-certified-reductions.md](rung-3-certified-reductions.md).)

The through-line: difficulty is monotone in ambition, but soundness is *constant* across all four rungs — a flat guarantee under a rising capability curve. That is the shape the certificate gives you.

---

## 7. Related work, placed

Three families, distinguished by where the learner sits relative to the trusted base:

1. **Learner outside the boundary (schedules sound procedures).** Rice (1976); SATzilla (Xu et al., 2008); Kotthoff's survey (2014); Graph-Q-SAT replacing VSIDS inside complete CDCL (Kurin et al., 2020); FastForward's LP distance oracle for Petri reachability (Blondin et al., 2021). *Soundness preserved by construction.*
2. **Learner proposes, checker disposes.** AlphaProof + Lean kernel (2024); HyperTree Proof Search + ITP kernel (Lample et al., 2022); Code2Inv / CLN2INV / ICE invariant synthesis + SMT (2018–2020); Neural Termination (Giacobbe et al., 2022) and Neural Model Checking (Giacobbe et al., 2024) + SMT; underwritten theoretically by certifying algorithms (McConnell, Mehlhorn, Näher, Schweitzer, 2011) and exemplified in tooling by DRAT/GRAT, where the SAT community trusts only a tiny verified checker (Lammich, 2017). *Soundness from the checker.*
3. **Learner answers directly (unsound).** NeuroSAT standalone; data-driven Petri-reachability approximators; MuZero's uncertified learned dynamics. *Soundness empirical or absent.*

`petrivet`'s proposed position is the synthesis of (1) and (2): algorithm-*selection* in form, certificate-*checked* in guarantee, applied to Petri-net model checking, with FastForward as the domain-matched precedent and the existing proof calculus as the firewall that lifts FastForward's sound-on-success-for-one-property into sound-for-all-properties. To my knowledge that specific synthesis — a learned portfolio over a *full certifying* model-checking decider lattice — is not yet occupied in the literature.

---

## 8. Honest limitations

- **The precondition is real and currently unmet.** The Theorem covers *certifying* deciders; the bare-boolean shortcuts and the two `Some(false)` stubs are trusted, and one is a latent bug. The firewall must be built before the learner can be trusted to schedule those deciders. Until then, the learner should schedule *only* deciders whose results are independently checked, and the trusted base is honestly larger than `{chk_P}`.
- **Non-termination needs deadlines.** Bounded Karp–Miller exploration can run long with no cooperative cancellation today; anytime racing and RL with budgets both require it.
- **Distribution shift.** A policy trained on the MCC corpus may misjudge out-of-distribution nets — but, by §3, it *misjudges into wasted time, never wrong answers*, degrading gracefully to the exhaustive fallback. This is the benign failure mode the certificate buys.
- **Feature sufficiency is empirical.** §4 prescribes *which kind* of feature to prefer and gives a mutual-information test, but whether the chosen $\varphi$ is sufficient for hardness on a given corpus is a measured question, not a theorem.
- **The checker is the trusted base — shrink it, then verify it.** Soundness now rests entirely on the certificate checkers. They should be small, audited, and ideally formally verified (the GRAT discipline). This *concentrates* trust rather than eliminating it, which is the honest and the desirable outcome.

---

## 9. Coda — the effective theory of hardness

Strip the mysticism and the connection to cellular automata and to Joscha Bach's computational functionalism is technically load-bearing, in a way that can be fenced precisely.

A Petri net *is* a local-update concurrent substrate: a transition fires using only its preset and postset, touching nothing else. This is not metaphor — asynchronous cellular automata can implement Petri nets (Golze; Priese, 1982) and infinite Petri nets can simulate universal cellular automata such as Rule 110 in polynomial time (Zaitsev, 2015/2018), while markings form the free commutative monoid that makes composition itself algebraic (Meseguer–Montanari, *"Petri Nets Are Monoids,"* 1990). *(The honest caveat: those universality results need infinite nets; the finite nets the tool checks are decidable-but-intractable — which is exactly the regime where an effective theory is meaningful and a worst-case-free heuristic is the right tool.)* The micro-dynamics of this substrate — its reachability graph — is intractable. Yet the physics of coarse-graining says even a computationally irreducible local-update system can possess a closed, predictive macro-theory *when the macro-variables are autonomous under the dynamics* (Israeli–Goldenfeld's commuting-diagram criterion) and *sufficient for the target* (the causal-states / information-bottleneck criterion). On this reading — offered as interpretation, not theorem — **a learned selection policy is an attempt to discover the effective theory of verification hardness**: a compressible macrostructure over an intractable substrate, with structural decompositions (NUPN units, S-components, P-invariants) as the candidate coarse-grainings, and the policy's feature panel as the candidate autonomous macrostate.

This is where Bach's one defensible, borrowable move lives — *objects and agents as coarse-grained aggregate descriptions of a finer computational substrate* ("you only look at the aggregate dynamics … the operators that work in the limit"). We borrow that and quarantine the rest of his metaphysics ("only a simulation can be conscious," "the universe is all finite automata") as labeled speculation, not science. And it dovetails with the companion essay's capstone: the integrated-information residual $\Phi_{\mathrm{PN}}$ — the minimum over partitions of how badly the net fails to factor into independent components — is *exactly the quantity that resists this coarse-graining program*, the irreducible remainder the effective theory cannot compress. We import that strictly as the factorization-residual mathematics, and emphatically not as any claim about minds.

So the whole picture closes on itself. The policy learns what *can* be coarse-grained — the macro-structure of hardness that lets cheap techniques be routed to easy instances. $\Phi_{\mathrm{PN}}$ measures what *cannot* — the integrated, irreducible core that no decomposition recovers and no policy can shortcut, the instances where you must pay the exhaustive price. One learns the effective theory; the other measures its residual. And underwriting both, holding the entire learned apparatus safely outside the trusted base, is the single structural fact that a verdict here is never a guess but a proof — that in this tool, unlike in Go, every leaf can be checked.

![The certifying portfolio loop with its trust boundary: the learned policy schedules, only a checked certificate is accepted.](figures/certifying-portfolio.svg)

---

### References (curated, verified)

- Silver et al. *Mastering the game of Go with deep neural networks and tree search.* Nature 529 (2016); *…without human knowledge.* Nature 550 (2017); *A general RL algorithm…* Science 362 (2018). Schrittwieser et al. *Mastering Atari, Go, chess and shogi by planning with a learned model* (MuZero). Nature 588 (2020).
- Rice. *The Algorithm Selection Problem.* Adv. Computers 15 (1976). Xu, Hutter, Hoos, Leyton-Brown. *SATzilla.* JAIR 32 (2008). Kotthoff. *Algorithm Selection… A Survey* (2014). Kurin et al. *Graph-Q-SAT* (NeurIPS 2020).
- Blondin, Haase, Offtermatt. *Directed Reachability for Infinite-State Systems* (FastForward). TACAS 2021. Giacobbe et al. *Neural Model Checking.* NeurIPS 2024. Si et al. *Code2Inv.* NeurIPS 2018. Lample et al. *HyperTree Proof Search.* NeurIPS 2022. DeepMind. *AlphaProof* (Nature 2025).
- McConnell, Mehlhorn, Näher, Schweitzer. *Certifying algorithms.* Computer Science Review 5(2) (2011). Lammich. *Efficient Verified (UN)SAT Certificate Checking* (GRAT). CADE 2017 / JAR 2019.
- Israeli, Goldenfeld. *Computational irreducibility and the predictability of complex physical systems.* PRL 92 (2004). Shalizi, Crutchfield. *Computational Mechanics.* J. Stat. Phys. 104 (2001). Tishby, Pereira, Bialek. *The Information Bottleneck Method* (1999).
- Zaitsev. *Universality in Infinite Petri Nets.* LNCS 9288 (2015); *Simulating Cellular Automata by Infinite Petri Nets.* J. Cellular Automata 13 (2018). Meseguer, Montanari. *Petri Nets Are Monoids.* Inf. & Comput. 88(2) (1990). Wu. *Accelerating Self-Play Learning in Go* (KataGo). arXiv:1902.10565. Jang. *autogo*; Dwarkesh Podcast, 15 May 2026.

*A factual note carried from the research: the Jang figure is "a few thousand dollars" (per the episode transcript), not "a few hundred"; `autogo` is an admittedly-imperfect from-scratch Go agent, not a verified superhuman engine. The robust load-bearing fact is KataGo's 40× training-compute reduction (Wu, 2019). The underlying insight — ML collapsing an intractable search into statistical answerability — is correct and well-grounded.*
