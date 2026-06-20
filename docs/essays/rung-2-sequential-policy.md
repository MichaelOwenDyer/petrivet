# Rung 2 — The Sequential Policy

### From static selection to an adaptive controller: preemptive, anytime scheduling over the certifying deciders

> Status: design specification, deferred. The second rung of the **learning ladder** — itself the *sequel* to petrivet's certificate-and-checker, not its spine. Exploratory work (Daniel Dyer, with Claude) on Michael's tool. It sits above Rung 1 and the measurement harness, realizes Rung 2 of the ladder in [Soundness as a Free Variable](soundness-as-a-free-variable.md), and has a hard infrastructure prerequisite (the cancellation seam, backlog **D6**) the codebase lacks today.

A note on numbering: the implementation branches (`rung1/observe-public-api`, `rung2/observe-per-decider`) build the *measurement harness*; the *ladder rungs* describe the learning objective. This document is the ladder's Rung 2. Rung 1 is the [empirical hardness ranker](rung-1-empirical-hardness-ranker.md); Rung 3 is the [planner over certified reductions](rung-3-certified-reductions.md).

---

## Where this sits, and why it is safe to defer

The same two facts that frame Rung 1 frame this rung. The thesis is the **coverage map and the certificate-and-checker**; the learning ladder is the sequel. Building this controller is **safe to defer** because the firewall — soundness is independent of the selection policy — makes any scheduler, however elaborate, unable to affect a verdict; and it is **honest to defer** because the contribution is the certificate, not the controller. Rung 2 is built only after Rung 1, only if Rung 1's measured gap justified a learned selector at all, and only once its hard prerequisite — a cooperative cancellation seam — exists in the core.

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

1. **An index / optimal-stopping baseline (no learning) — start here.** Choosing which decider to open next, paying an inspection cost, to find a prize, is a *Pandora's-box* problem with known optimal structure (Weitzman, 1979): under independence, the optimal policy is an index rule computed from each decider's predicted cost and success probability. Supplying it with the Rung-1 model's predicted `(cost, P(conclude))` per decider yields a principled, learning-free adaptive order with optimal abandonment — a strong baseline that precedes any reinforcement learning. Restart and timeout scheduling (Luby, Sinclair, Zuckerman) and max-`k`-armed restart bandits (Streeter, Smith) belong to the same family.
2. **A contextual bandit (intermediate step).** Treat "commit to decider `d`" as an arm, with context `φ(N)`, and learn from observed regret online. This is essentially Rung 1 with online updating. It captures selection under feedback but not mid-run preemption.
3. **Conservative offline reinforcement learning (the target, and only where warranted).** Model both the sequencing and the preemption/escalation as a Markov decision process and learn a policy over the full action space. This is the only formulation that learns *when* to abandon a diverging exploration, which the simpler formulations can only approximate. It is the dynamic-algorithm-portfolio problem (Gagliolo, Schmidhuber, 2006).

**Use the simplest formulation the data supports.** Most of the adaptive value resides in the index policy. Reinforcement learning is justified *only where preemption timing demonstrably depends on rich state* — i.e., where the right moment to abandon is not a fixed budget but a function of what the run has revealed so far. Absent that demonstration, the index rule over Rung-1 predictions is the answer, and the RL machinery is, like a ranker over a small gap, dead weight.

## The cancellation prerequisite (backlog D6 — the hard gate)

Rung 2 has a hard prerequisite the codebase **does not have today**, and the rung cannot exist without it. The only mechanisms that stop analysis early are the ω short-circuit and process-level `catch_unwind`; there is no deadline, no budget, and no way to interrupt a running exploration or a `microlp` solve and reclaim the time. The harness's per-decider-fibers work (the `rung2/observe-per-decider` branch) introduces coarse thread/process timeouts mapping to `Timeout`, but an adaptive policy needs more:

- a **cooperative cancellation token** threaded into the exploration loop (check-and-yield at frontier steps), and ideally a **checkpoint** so a paused exploration can resume rather than restart;
- a **bounded LP/ILP** call (solver iteration or time limits) so the algebraic deciders are interruptible as well.

This is the one place where Rung 2 reaches into the core, and it must do so minimally and in a certifying manner: a cancelled decider simply returns `⊥` (inconclusive), which the trust boundary already handles. **Cancellation changes *when* a decider stops, never *what* is accepted.** This seam is the author's call (backlog D6) and is the precondition for everything below; correctness of the seam is a *robustness* concern (a bad preemption wastes work or hangs), never a soundness one (the firewall still blocks wrong answers).

## Anytime parallel racing

Because every decider is a pure function of the net, the cheap ones can run concurrently — the first accepted certificate wins, the rest are cancelled. Rung 2 therefore also chooses resource allocation across a concurrent portfolio, not merely a sequence (parallel algorithm scheduling, as in `aspeed`/SUNNY). The policy's budget becomes a quantity to split across simultaneously running deciders, weighted by predicted promise. This yields the **anytime** property: at any interruption the controller returns the best certificate found so far, or `Inconclusive`. It never returns a wrong answer, and it never returns nothing on the grounds that it was still computing. Race when the Rung-1 predicted costs are close or uncertain; run sequentially when one decider is a confident favorite, since racing has real overhead and contention wasted on easy instances.

## Soundness over the richer action space

The firewall extends without modification, and it is useful to see why the new actions are harmless:

- **Preemption** cannot cause a wrong answer: an abandoned decider produced no certificate, so nothing of its is accepted. Cutting a diverging exploration loses work, never correctness.
- **Parallel racing** cannot cause a wrong answer: the first *accepted* certificate wins (acceptance still runs `check` against the original net), and the losers' partial work is discarded.
- **Continue/allocate** decisions only move budget around; they never touch the acceptance predicate.

The entire adaptive, preemptive, parallel controller — index rule, bandit, or offline RL — therefore stays wholly outside the trusted base. Even a buggy preemption is, at worst, a resource leak or a hang (caught by the outer deadline), not an unsound verdict. This is the same trust boundary as in Rung 1, now holding over `{start, continue, abandon, allocate}` rather than just `{select}`. Learning is confined to performance, and performance is the only thing that can break.

## Training

Training is now reinforcement learning, but the inexpensive and safe regime is offline:

- **Offline / off-policy from the harness logs.** The harness already emits `(φ, decider, outcome, cost)` trajectories over the corpus. A policy can be learned off-policy from those logs — fitted-Q, or preferably **conservative offline RL** (CQL, Kumar et al., 2020) to control the extrapolation error that affects naive off-policy value estimates — without expensive live exploration, since each live rollout runs real deciders. This is the regime to start in: the data exists, no live experimentation required.
- **Bootstrap from Rung 1.** Initialize the policy's behavior with the Rung-1 ranker's static order as the prior; Rung 2 then learns only the adaptive corrections — when to deviate, when to abandon, when to race. Because the policy can always reproduce Rung 1's static schedule, a correctly trained Rung 2 dominates Rung 1 in expectation: strictly more options, a no-worse default.
- **Reward = time-to-accept**, optionally shaped to penalize late abandonment. The reward may be misspecified freely: by the firewall, a wrong reward yields a slow policy, never a wrong verdict. Reward design is a performance parameter, not a safety one.

## Continuity with the torsor framing

Rung 1 learned a *static* section of the cost-torsor bundle; Rung 2 learns an *adaptive* section — it re-selects the origin (the next decider) as new fiber information arrives, after each outcome. The reward is still a differential (cost-ratios, origin-free), so it remains the harness's persisted invariant, and the *budget* is the resource the controller allocates across the bundle's fibers. The geometry is the same, now traversed with feedback.

## The falsifier, and the lesser risks

**The falsifier of this rung is that adaptivity does not beat static selection** — that the index rule over Rung-1 predictions captures essentially all the available gain, and the offline-RL controller adds machinery and extrapolation risk without a measured improvement on held-out families. Compounded with Rung 1's own falsifier (the static gap may already be within noise) and the hard cancellation prerequisite, the honest prior is that this rung earns the *index policy* and may never earn the *RL*. The rung is structured to stop at the simplest formulation the data supports.

The remaining risks:

1. **Offline-RL extrapolation error.** Off-policy value estimates over-credit unseen action sequences; the central failure mode. Mitigate with conservative methods, or stay at the index-policy/bandit level when data is thin — where most of the adaptive benefit lives anyway.
2. **The cancellation infrastructure is a gating dependency** (backlog D6). It must be low-overhead and correct; correctness here is robustness, not soundness.
3. **Parallel overhead and contention** can erode or reverse the racing benefit on small instances; race only under predicted-cost uncertainty.
4. **Reward shaping for abandonment timing** is delicate; over-eager abandonment can starve a decider that was about to finish.
5. **Monotonicity over Rung 1 holds only when bootstrapped from it**; a from-scratch policy can underperform the static ranker until trained.
6. **Distribution shift remains benign**: by the firewall, an out-of-distribution net costs time, never correctness.

As with Rung 1, none of these affects the answer. Rung 2 introduces substantially more machinery — feedback, preemption, concurrency, reinforcement learning — entirely within the performance dimension, because the certificate has already separated correctness from all of it. That separation is what makes it safe to run a controller this elaborate inside a verification tool, and what makes it honest to build it last.

## Boundary with Rung 1 and Rung 3

- **Below (Rung 1):** static, one-shot selection — a fixed section chosen up front. Rung 2 adds feedback, preemption, and parallel allocation, and is initialized from Rung 1 so it can only improve.
- **What Rung 2 is not:** it does not transform the problem. The net is fixed; Rung 2 schedules, sequences, preempts, and races deciders over that fixed net.
- **Above (Rung 3):** add the **transform** action — certified reductions that shrink or split the net — turning the fixed-net controller into a [planner over a tree of reduced problems](rung-3-certified-reductions.md). Rung 2's adaptive control loop is the substrate Rung 3 generalizes: Rung 3 = Rung 2's action space `+` reductions, with policy and value now defined over *residual* nets and an AND/OR proof tree. Build Rung 2 well and Rung 3 is the same controller over a richer state space.

---

### References (curated)

- Weitzman. *Optimal Search for the Best Alternative* (Pandora's box). Econometrica 47 (1979). Luby, Sinclair, Zuckerman. *Optimal speedup of Las Vegas algorithms.* (1993). Streeter, Smith. *New techniques for algorithm portfolio design / max-k-armed bandit restart scheduling.* (2008).
- Gagliolo, Schmidhuber. *Learning Dynamic Algorithm Portfolios.* Annals of Math. & AI (2006). Hoos, Kaminski, Lindauer, Schaub. *aspeed: Solver scheduling via answer set programming.* (2015). Amadini, Gabbrielli, Mauro. *SUNNY* portfolio scheduling. (2014).
- Levine, Kumar, Tucker, Fu. *Offline Reinforcement Learning: Tutorial, Review, and Perspectives.* (2020). Kumar, Zhou, Tucker, Levine. *Conservative Q-Learning (CQL).* NeurIPS (2020).
- Companion: the firewall theorem in [Soundness as a Free Variable](soundness-as-a-free-variable.md); the four-principle account in [Core Principles](core-principles.md); the static ranker it builds on in [Rung 1](rung-1-empirical-hardness-ranker.md); the cost-torsor data model and the cancellation/deadline hook in the [self-measurement harness plan](../self-measurement-harness-plan.md).
</content>
