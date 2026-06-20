# Rung 2 — The Sequential Policy
### From static selection to an adaptive controller: preemptive, anytime scheduling over the certifying deciders

> Status: design specification. Exploratory work (Daniel Dyer, with Claude) on Michael's tool. It sits above Rung 1 and the measurement harness, and realizes Rung 2 of the ladder described in [Soundness as a Free Variable](soundness-as-a-free-variable.md). Rung 1 should be built first. Rung 2 is the step that follows once the static ranker is in productive use, and it has an infrastructure prerequisite described in "The cancellation prerequisite" below.

A note on numbering: the implementation branches (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the *ladder rungs* describe the learning objective. This document is the ladder's Rung 2. Rung 1 is the [empirical hardness ranker](rung-1-empirical-hardness-ranker.md); Rung 3 is the [planner over certified reductions](rung-3-certified-reductions.md).

---

## The move from Rung 1

Rung 1 performs one-shot, static selection: it reads `φ(N)`, predicts a cost-ordered schedule, commits to that order, and falls through to the next decider on a miss. It decides everything before running anything, and it does not revise its plan based on what the deciders it has already run actually did.

Rung 2 closes the loop. It is an adaptive controller: after each decider returns inconclusive — or while a long-running decider consumes budget without converging — the policy updates its belief and re-decides the next action, including whether to stop a running decider and escalate. The new operation is not "transform" (that is Rung 3); the net remains fixed. The new operation is **preempt**: start a decider, monitor it, and abandon it for a better option when the evidence indicates it is unlikely to conclude. This is the distinction between a schedule and a scheduler, and between a fixed plan and a feedback controller.

The motivating case is present in the code today: a bounded-but-large Karp–Miller exploration, or a branch-and-bound ILP, can run well past the point where a cheaper decider would have concluded, and neither Rung 0 nor Rung 1 has any mechanism to detect this and stop. Rung 2 is the rung that learns to identify a diverging computation, cancel it, and try the structural route instead.

## The formal object

Rung 2 is a sequential decision process. A *state* is `(φ(N), history of attempts with their outcomes and costs so far, remaining budget)`. The *actions* are richer than a single choice of decider:

- **start** decider `d`;
- **continue** a running decider for another time-slice `δ`;
- **abandon** a running decider, freeing its budget;
- **(parallel) allocate** budget across several concurrently running deciders (see "Anytime parallel racing").

An episode ends when a certificate is accepted, or when the budget is exhausted (escalating to the exhaustive backstop, or reporting `Inconclusive`). The reward is `−cost`; the objective is to minimize the expected time to an accepted certificate. There are three formulations, in increasing order of ambition and decreasing order of assumptions:

1. **An index / optimal-stopping baseline (no learning).** The problem of choosing which decider to open next, paying an inspection cost, in order to find a prize is a *Pandora's-box* problem with known optimal structure (Weitzman, 1979): under independence, the optimal policy is an index rule computed from each decider's predicted cost and success probability. Supplying it with the Rung-1 model's predicted `(cost, P(conclude))` per decider yields a principled, learning-free adaptive order with optimal abandonment — a strong baseline that precedes any reinforcement learning. Restart and timeout scheduling (Luby, Sinclair, Zuckerman) and max-`k`-armed restart bandits (Streeter, Smith) belong to the same family.
2. **A contextual bandit (intermediate step).** Treat "commit to decider `d`" as an arm, with context `φ(N)`, and learn from observed regret online. This is essentially Rung 1 with online updating. It captures selection under feedback but not mid-run preemption.
3. **Full reinforcement learning (the target).** Model both the sequencing and the preemption/escalation as a Markov decision process and learn a policy over the full action space. This is the only formulation that learns when to abandon a diverging exploration, which the simpler formulations can only approximate. It is the dynamic-algorithm-portfolio problem (Gagliolo, Schmidhuber, 2006): online time-allocation across a set of algorithms.

Use the simplest formulation the data supports. Most of the adaptive value resides in the index policy; reinforcement learning is justified only when preemption timing genuinely depends on rich state.

## The cancellation prerequisite

Rung 2 requires cooperative cancellation and deadlines, which the codebase does not currently have. The only mechanisms that stop analysis early are the ω short-circuit and process-level `catch_unwind`; there is no deadline, no budget, and no way to interrupt a running exploration or a `microlp` solve and reclaim the time. Rung 2's hard prerequisite is therefore the budget/cancellation hook that the harness's per-decider-fibers work (the `rung2/observe-per-decider` branch) introduces in coarse form (thread/process timeouts mapping to `Timeout`). An adaptive policy needs more than a coarse outer timeout:

- a **cooperative cancellation token** threaded into the exploration loop (check-and-yield at frontier steps), and ideally a **checkpoint** so that a paused exploration can resume rather than restart;
- a **bounded LP/ILP** call (solver iteration or time limits) so that the algebraic deciders are interruptible as well.

This is the one place where Rung 2 reaches into the core, and it should do so minimally and in a certifying manner: a cancelled decider simply returns `⊥` (inconclusive), which the trust boundary already handles. Cancellation changes *when* a decider stops, never *what is accepted*.

## Anytime parallel racing

Because every decider is a pure function of the net, the cheap ones can be run concurrently — the first accepted certificate wins, and the rest are cancelled. Rung 2 therefore also chooses resource allocation across a concurrent portfolio, not merely a sequence (parallel algorithm scheduling, as in `aspeed`/SUNNY). The policy's budget becomes a quantity to split across simultaneously running deciders, weighted by their predicted promise. This yields the **anytime** property: at any interruption the controller returns the best certificate found so far, or `Inconclusive`. It never returns a wrong answer, and it never returns nothing on the grounds that it was still computing. Race when the Rung-1 predicted costs are close or uncertain; run sequentially when one decider is a confident favorite, since racing has real overhead and contention that are wasted on easy instances.

## Soundness over the richer action space

The soundness theorem extends without modification, and it is useful to see why the new actions are harmless:

- **Preemption** cannot cause a wrong answer: an abandoned decider produced no certificate, so nothing of its is accepted. Cutting a diverging exploration loses work, never correctness.
- **Parallel racing** cannot cause a wrong answer: the first *accepted* certificate wins (acceptance still runs `check`), and the losers' partial work is discarded.
- **Continue/allocate** decisions only move budget around; they never touch the acceptance predicate.

The entire adaptive, preemptive, parallel controller — index rule, bandit, or deep reinforcement learning — therefore stays wholly outside the trusted base. Even a buggy preemption is, at worst, a resource leak or a hang (caught by the outer deadline), not an unsound verdict. This is the same trust boundary as in Rung 1, now holding over `{start, continue, abandon, allocate}` rather than just `{select}`. Learning is confined to performance, and performance is the only thing that can break.

## Training

Training is now reinforcement learning, but the inexpensive and safe regime is offline:

- **Offline / off-policy from the harness logs.** The measurement harness already emits `(φ, decider, outcome, cost)` trajectories over the corpus. A policy can be learned off-policy from those logs — fitted-Q, or preferably conservative offline reinforcement learning (CQL, Kumar et al., 2020) to control the extrapolation error that affects naive off-policy value estimates — without expensive live exploration, since each live rollout runs real deciders. This is the regime to start in: the data already exists and no live experimentation is required.
- **Bootstrap from Rung 1.** Initialize the policy's behavior with the Rung-1 ranker's static order as the prior; Rung 2 then learns only the adaptive corrections — when to deviate, when to abandon, and when to race. Because the policy can always reproduce Rung 1's static schedule, a correctly trained Rung 2 dominates Rung 1 in expectation: it has strictly more options and a no-worse default.
- **Reward = time-to-accept**, optionally shaped to penalize late abandonment. As before, the reward may be misspecified freely: by the theorem, a wrong reward yields a slow policy, never a wrong verdict. Reward design is a performance parameter, not a safety one.

## Continuity with the torsor framing

Rung 1 learned a *static* section of the cost-torsor bundle; Rung 2 learns an *adaptive* section — it re-selects the origin (the next decider) as new fiber information arrives, after each outcome. The reward is still a differential (cost-ratios, origin-free), so it remains the harness's persisted invariant, and the *budget* is the resource the controller allocates across the bundle's fibers. The geometry is the same, now traversed with feedback.

## Limitations and risks

1. **Offline-RL extrapolation error.** Off-policy value estimates over-credit unseen action sequences; this is the central failure mode. Mitigate it with conservative methods, or remain at the index-policy/bandit level when data is thin, since most of the adaptive benefit is there in any case.
2. **The cancellation infrastructure is a gating dependency** (see "The cancellation prerequisite"), and it must be low-overhead and correct. Correctness here is a robustness concern, not a soundness one: a bad preemption wastes work or hangs, while the trust boundary still blocks wrong answers.
3. **Parallel overhead and contention** can erode or reverse the racing benefit on small instances; race only under predicted-cost uncertainty.
4. **Reward shaping for abandonment timing** is delicate, and over-eager abandonment can starve a decider that was about to finish.
5. **Performance is monotone over Rung 1 only when bootstrapped from it** (see "Training"); a from-scratch policy can underperform the static ranker until it is trained.
6. **Distribution shift remains benign**: by the theorem, an out-of-distribution net costs the policy time, never correctness, degrading to the exhaustive backstop.

As with Rung 1, none of these affects the answer. Rung 2 introduces substantially more machinery — feedback, preemption, concurrency, reinforcement learning — entirely within the performance dimension, because the certificate has already separated correctness from all of it. That separation is what makes it safe to run a controller this elaborate inside a verification tool.

## Boundary with Rung 1 and Rung 3

- **Below (Rung 1):** static, one-shot selection — a fixed section chosen up front. Rung 2 adds feedback, preemption, and parallel allocation, and is initialized from Rung 1 so that it can only improve.
- **What Rung 2 is not:** it does not transform the problem. The net is fixed; Rung 2 schedules, sequences, preempts, and races deciders over that fixed net.
- **Above (Rung 3):** add the **transform** action — certified reductions that shrink or split the net — turning the fixed-net controller into a [planner over a tree of reduced problems](rung-3-certified-reductions.md). Rung 2's adaptive control loop is the substrate that Rung 3 generalizes: Rung 3 = Rung 2's action space `+` reductions, with policy and value now defined over *residual* nets and an AND/OR proof tree. Build Rung 2 well and Rung 3 is the same controller over a richer state space.

---

### References (curated)

- Weitzman. *Optimal Search for the Best Alternative* (Pandora's box). Econometrica 47 (1979). Luby, Sinclair, Zuckerman. *Optimal speedup of Las Vegas algorithms.* (1993). Streeter, Smith. *New techniques for algorithm portfolio design / max-k-armed bandit restart scheduling.* (2008).
- Gagliolo, Schmidhuber. *Learning Dynamic Algorithm Portfolios.* Annals of Math. & AI (2006). Hoos, Kaminski, Lindauer, Schaub. *aspeed: Solver scheduling via answer set programming.* (2015). Amadini, Gabbrielli, Mauro. *SUNNY* portfolio scheduling. (2014).
- Levine, Kumar, Tucker, Fu. *Offline Reinforcement Learning: Tutorial, Review, and Perspectives.* (2020). Kumar, Zhou, Tucker, Levine. *Conservative Q-Learning (CQL).* NeurIPS (2020).
- Companion: the soundness theorem in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the static ranker it builds on in [Rung 1](rung-1-empirical-hardness-ranker.md); the cost-torsor data model and the cancellation/deadline hook in the [self-measurement harness plan](../self-measurement-harness-plan.md).
