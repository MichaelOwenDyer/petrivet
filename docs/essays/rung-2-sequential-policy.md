# Rung 2 — The Sequential Policy
### From a static section to an adaptive controller: preemptive, anytime scheduling over the certifying lattice

> Status: design spec / vision. Exploratory work (Daniel Dyer, with Claude) on Michael's tool. Sits above Rung 1 and the measurement harness; realizes Rung 2 of the ladder in [Soundness as a Free Variable](soundness-as-a-free-variable.md). Build Rung 1 first — Rung 2 is the next step *after* the static ranker is earning its keep, and it has a real infrastructure prerequisite (§"The cancellation prerequisite").

A numbering note, once more: the implementation **branches** (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the **ladder rungs** are the learning ambition. This document is the ladder's **Rung 2**. Rung 1 is the [empirical hardness ranker](rung-1-empirical-hardness-ranker.md); Rung 3 is the [planner over certified reductions](rung-3-certified-reductions.md).

---

## The move from Rung 1

Rung 1 is a **one-shot, static selection**: it reads `φ(N)`, predicts a cost-ordered schedule, and commits to that order, falling through on a miss. It decides everything *before running anything*, and it never revises its plan in light of what the deciders it has already run actually did.

Rung 2 closes the loop. It is an **adaptive controller**: after each decider returns inconclusive — or as a long-running decider burns budget without converging — the policy updates its belief and re-decides the next action, *including when to give up on a running decider and escalate.* The new verb is not "transform" (that is Rung 3); the net is still fixed. The new verb is **preempt**: start a decider, watch it, and abandon it for a better bet when the evidence says it is going nowhere. This is the difference between a schedule and a scheduler — between a plan and a feedback controller.

The motivating case is concrete and lives in the code today: a bounded-but-large Karp–Miller exploration, or a branch-and-bound ILP, can run far past the point where a cheaper decider would have concluded — and Rung 0 and Rung 1 both have no way to *notice and bail.* Rung 2 is the rung that learns to say "this one is diverging; cut it, try the structural route."

## The formal object

This is now a genuine **sequential decision process**. A *state* is `(φ(N), history of attempts and their outcomes/costs so far, remaining budget)`. The *actions* are richer than a bare choice of decider:

- **start** decider `d`;
- **continue** a running decider for another time-slice `δ`;
- **abandon** a running decider (give up, free its budget);
- **(parallel) allocate** budget across several concurrently-running deciders (§"Anytime parallel racing").

An episode ends when a certificate is accepted, or the budget is exhausted (escalate to the exhaustive backstop, or report `Inconclusive`). The reward is `−cost`; the objective is to minimize expected time-to-accepted-certificate. Three formulations, in increasing ambition and decreasing assumption:

1. **An index / optimal-stopping baseline (no learning).** The "which decider to open next, paying inspection cost, to find a prize" problem is a *Pandora's-box* problem with classical optimal structure (Weitzman, 1979): under independence, the optimal policy is a simple **index rule** computed from each decider's predicted cost and success probability. Feed it the Rung-1 model's predicted `(cost, P(conclude))` per decider and you get a *principled, learning-free* adaptive order with optimal abandonment — a strong, honest baseline before any RL. (Restart/timeout scheduling, Luby et al.; max-`k`-armed restart bandits, Streeter–Smith, are the same family.)
2. **A contextual bandit (the stepping stone).** Treat "commit to decider `d`" as an arm, context `φ(N)`, and learn from observed regret online — essentially Rung 1 with online updating. It captures *selection* under feedback but not *mid-run preemption*.
3. **Full reinforcement learning (the target).** Model the sequencing *and* the preemption/escalation as an MDP and learn a policy over the full action space. This is the only formulation that learns *when to abandon a diverging exploration* — the thing the cheaper formulations can only approximate. This is the dynamic-algorithm-portfolio problem (Gagliolo–Schmidhuber, 2006): online time-allocation across a set of algorithms.

Reach for the simplest formulation the data supports. Most of the adaptive value is in the index policy; RL earns its complexity only when preemption timing genuinely depends on rich state.

## The cancellation prerequisite

Rung 2 cannot exist without **cooperative cancellation / deadlines** — and the codebase has none today. The only ways analysis stops early are the ω short-circuit and process-level `catch_unwind`; there is no deadline, no budget, no way to interrupt a running exploration or a `microlp` solve and reclaim the time. So Rung 2's hard prerequisite is the budget/cancellation hook that the harness's per-decider-fibers work (the `rung2/observe-per-decider` *branch*) introduces in coarse form (thread/process timeouts → `Timeout`). A real adaptive policy wants more than a coarse outer timeout:

- a **cooperative cancellation token** threaded into the exploration loop (check-and-yield at frontier steps), and ideally a **checkpoint** so a paused exploration can resume rather than restart;
- a **bounded LP/ILP** call (solver iteration/time limits) so the algebraic deciders are interruptible too.

This is the one place Rung 2 reaches *toward* the core — and it should reach minimally and certifying-ly: a cancelled decider simply returns `⊥` (inconclusive), which the firewall already handles. Cancellation changes *when* a decider stops, never *what is accepted*.

## Anytime parallel racing

Because every decider is a pure function of the net, the cheap ones can be **raced concurrently** — first accepted certificate wins, cancel the rest. So Rung 2 also chooses *resource allocation across a concurrent portfolio*, not merely a sequence (parallel algorithm scheduling, à la `aspeed`/SUNNY). The policy's budget becomes a quantity to *split* across simultaneously-running deciders, weighted by their predicted promise. This yields the **anytime** property: at any interruption the controller returns the best certificate found so far, or `Inconclusive` — never a wrong answer, and never nothing-because-it-was-still-thinking. Race when the Rung-1 predicted costs are close or uncertain; run sequentially when one decider is a confident favorite (racing has real overhead and contention, wasted on easy instances).

## Soundness — still free, and now over a much richer action space

The soundness theorem extends verbatim, and it is worth seeing *why the new actions are harmless*:

- **Preemption** cannot cause a wrong answer: an abandoned decider simply produced no certificate, so nothing of its is accepted. Cutting a diverging exploration loses work, never correctness.
- **Parallel racing** cannot: the first *accepted* certificate wins (acceptance still runs `check`), and the losers' partial work is discarded.
- **Continue/allocate** decisions only move budget around; they never touch the acceptance predicate.

So the entire adaptive, preemptive, parallel controller — index rule, bandit, or deep RL — stays wholly outside the trusted base. Even a *buggy* preemption is, at worst, a resource leak or a hang (caught by the outer deadline), not an unsound verdict. This is the same firewall as Rung 1, now holding over `{start, continue, abandon, allocate}` instead of just `{select}`. Learning is confined to performance; performance is the only thing that can break.

## Training

Now it is reinforcement learning, but the cheap and safe regime is **offline**:

- **Offline / off-policy from the harness logs.** The measurement harness already emits `(φ, decider, outcome, cost)` trajectories over the corpus. A policy can be learned **off-policy from those logs** — fitted-Q or, better, *conservative* offline RL (CQL, Kumar et al., 2020) to control the extrapolation error that plagues naive off-policy value estimates — *without* expensive live exploration (each live "rollout" runs real deciders). This is the regime to start in: the data already exists, and no live experimentation is needed.
- **Bootstrap from Rung 1.** Initialize the policy's behavior with the Rung-1 ranker's static order as the prior; Rung 2 then learns only the *adaptive corrections* — when to deviate, when to abandon, when to race. Because the policy can always reproduce Rung 1's static schedule, a correctly-trained Rung 2 **dominates Rung 1 in expectation**: it has strictly more options and a no-worse default.
- **Reward = time-to-accept**, optionally shaped to penalize late abandonment. And as ever: misspecify the reward freely — by the theorem, a wrong reward yields a slow policy, never a wrong verdict. Reward design is a performance knob, not a safety one.

## Continuity with the torsor framing

Rung 1 learned a *static* section of the cost-torsor bundle; Rung 2 learns an **adaptive section** — it re-picks the origin (the next decider) as new fiber information arrives, after each outcome. The reward is still a differential (cost-ratios, origin-free), so it is still the harness's persisted invariant, and the *budget* is the resource the controller allocates across the bundle's fibers. Same geometry, now traversed with feedback.

## Where the dragons are (claim-honest)

1. **Offline-RL extrapolation error.** Off-policy value estimates over-credit unseen action sequences; this is the central failure mode. Mitigate with conservative methods, or stay at the index-policy/bandit rung when data is thin — most of the adaptive win is there anyway.
2. **The cancellation infrastructure is a real, gating dependency** (§"prerequisite"), and it must be low-overhead and correct. (Correctness here is a *robustness* concern, not a soundness one — a bad preemption wastes work or hangs; the firewall still blocks wrong answers.)
3. **Parallel overhead and contention** can erode or reverse the racing benefit on small instances; race only under predicted-cost uncertainty.
4. **Reward shaping for abandonment timing** is delicate, and over-eager abandonment can starve a decider that was about to finish.
5. **Performance is monotone over Rung 1 only if bootstrapped from it** (above); a from-scratch policy can underperform the static ranker until trained.
6. **Distribution shift remains benign** — by the theorem, an out-of-distribution net costs the policy time, never correctness, degrading to the exhaustive backstop.

As with Rung 1, none of these touches the answer. Rung 2 spends a great deal more machinery — feedback, preemption, concurrency, RL — entirely inside the performance dimension, because the certificate has already sealed correctness off from all of it. That sealing is what makes it *safe* to let a controller this elaborate loose inside a verification tool.

## Boundary with Rung 1 and Rung 3

- **Below (Rung 1):** static, one-shot selection — a fixed section chosen up front. Rung 2 adds feedback, preemption, and parallel allocation, and is initialized from Rung 1 so it can only improve.
- **What Rung 2 is *not*:** it does not transform the problem. The net is fixed; Rung 2 schedules, sequences, preempts, and races deciders *over that fixed net*.
- **Above (Rung 3):** add the **transform** action — certified reductions that shrink or split the net — turning the fixed-net controller into a [planner over a tree of reduced problems](rung-3-certified-reductions.md). Rung 2's adaptive control loop is exactly the substrate Rung 3 generalizes: Rung 3 = Rung 2's action space `+` reductions, with policy and value now defined over *residual* nets and an AND/OR proof tree. Build Rung 2 well and Rung 3 is "the same controller, over a richer state space."

---

### References (curated)

- Weitzman. *Optimal Search for the Best Alternative* (Pandora's box). Econometrica 47 (1979). Luby, Sinclair, Zuckerman. *Optimal speedup of Las Vegas algorithms.* (1993). Streeter, Smith. *New techniques for algorithm portfolio design / max-k-armed bandit restart scheduling.* (2008).
- Gagliolo, Schmidhuber. *Learning Dynamic Algorithm Portfolios.* Annals of Math. & AI (2006). Hoos, Kaminski, Lindauer, Schaub. *aspeed: Solver scheduling via answer set programming.* (2015). Amadini, Gabbrielli, Mauro. *SUNNY* portfolio scheduling. (2014).
- Levine, Kumar, Tucker, Fu. *Offline Reinforcement Learning: Tutorial, Review, and Perspectives.* (2020). Kumar, Zhou, Tucker, Levine. *Conservative Q-Learning (CQL).* NeurIPS (2020).
- Companion: the soundness theorem in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the static ranker it builds on in [Rung 1](rung-1-empirical-hardness-ranker.md); the cost-torsor data model and the cancellation/deadline hook in the [self-measurement harness plan](../self-measurement-harness-plan.md).
