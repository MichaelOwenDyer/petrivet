# The Work Ahead — what remains, and where it goes

*Companion to [`state-of-the-work.md`](state-of-the-work.md). That document covered
what the foundation accomplishes; this one is the forward view — the remaining
backlog, told the same way: plainly first, to establish what each piece is for, then
precisely. The plain reading gives the direction; the technical reading is the plan.
The authoritative source is [`BACKLOG.md`](../../BACKLOG.md) (the dependency-ordered
milestones M6–M13 and Epics A–H); this is the narrative that makes its shape legible.*

> **Framing note.** The *committed spine* below — the structural deciders that decide without
> state-space exploration, and the rig that measures their coverage — advances the library Michael
> described (his "structural analysis" feature and his roadmap toward handling more real nets,
> faster). The *thesis claim* this lane is framed around (the two numbers `f_struct`/`f`) is a
> **proposal he owns**, not a settled goal; the structural-and-measurement work is useful to the
> tool regardless of which framing he chooses.

---

## 1. The shape of what's left

**Plainly.** The hard part — a trustworthy foundation where every answer carries
checkable proof — is built. What remains is three kinds of work, in order of how
committed they are: **raise the number** (add cheap structural shortcuts so the tool
decides more questions without brute force), **measure the number** (turn the
construction into the thesis's headline result), and a set of **deliberately gated
extensions** (more ambitious capabilities that are only built if a measurement first
shows they are worth building). The discipline running through all of it is the same
one that governed the foundation: claim only what a measurement or a proof supports,
and prefer an honest "we don't know yet" to an overstated result.

**Precisely.** The *proposed* thesis (Michael's to ratify) would rest on **two numbers**, not a theorem:

- **`f_struct`** — the fraction of benchmark-corpus queries the *polynomial structural
  tier* decides without state-space exploration. This is the falsifiable headline.
- **`f`** — the fraction of accepted verdicts that carry an independently-checked
  certificate. This is the firewall's figure of merit, the *enabling* property.

The starting line is measured and deliberately low: today `f_struct` is **0.167** of
in-scope queries (**0.133** of the whole corpus) — only the two marked-graph fixtures
decide structurally; everything else falls to state-space search or honest abstention.
The entire remaining structural lane exists to *raise* that number, and the evaluation
rig exists to *re-measure* it honestly. The work divides into:

- **A committed spine** — the structural deciders (Epic B, milestones M6–M9) and the
  measurement-and-thesis lane (the observation crate at M11, the evaluation rig at M12).
- **Gated sequels** — certified reductions (Epic F) and learned algorithm-selection
  (the Epic D ladder), each off the critical path and built only when justified.
- **Horizons** — recorded, not scheduled: the compositional "Φ" residuals, the reuse
  of the engine for other infinite-state system classes, and one genuinely hard open
  research target.

One fixed constraint shapes everything: the thesis window closes **2026-11-02**, and
the writing must begin well before then. The critical chain to the headline result is
**the observation crate → the rig → the certifying-fraction report → the write-up**.
Slips there cost write-up time directly.

---

## 2. Raise the number — the structural deciders (Epic B, M6–M9)

**Plainly.** Right now the tool answers most questions the slow way (enumerate states)
or abstains. This lane adds *fast structural shortcuts*: for well-behaved classes of
system, a cheap calculation on the net's shape decides a property — bounded? live?
reachable? — with no enumeration, and emits a small proof. Each shortcut that lands
both raises the coverage number `f_struct` and, because it replaces a trusted answer
with a checkable one, raises the certifying fraction `f`.

**Precisely.** The cluster keystone (B2) is built; this lane is the deciders it and
the exact-arithmetic core unlock, sequenced across three milestones:

- **M7 — the algebraic layer.** B1 attaches a *single separating place-invariant* to a
  negative reachability/coverability verdict — a one-line algebraic receipt re-checkable
  by a single exact dot product — plus the (lazily computed, capped) invariant-coverage
  predicates. B7 consolidates the siphon/trap machinery into one operator and gives its
  worst-case-exponential enumeration a logged cap, so a pathological net yields an honest
  `Inconclusive` rather than an unbounded hang.
- **M8 — the free-choice and T-net deciders, where the number moves.** B3 decomposes a
  free-choice net into state-machine-like components and emits *exact per-place token
  bounds* (replacing today's abstention on boundedness with a checkable answer — and
  finally routing the boundedness decider through the certificate path). B4 decides
  marked-graph liveness and bounds directly from the net's circuits. B5 combines the
  cluster count, the rank, and the invariants into the *simultaneous* liveness-and-
  boundedness ("well-formedness") certificate. B6 decides free-choice reachability by
  the marking equation plus a trap check. The milestone's terminal gate is
  `[MEASURE]`: **re-measure `f_struct` against the floor and report the delta.** "The
  number moves, measurably."
- **M9 — reaching beyond free-choice.** Two *class-agnostic* deciders, which is what
  makes them strategically distinct: B10, a continuous ("fluid") relaxation that is a
  fast, always-polynomial way to *disprove* reachability on *any* net — including the
  general, unbounded instances the tool abstains on today — and B11, which exposes the
  deadlock-freedom guarantee the siphon/trap engine already computes for general nets
  but currently discards.

**Two things to hold honestly.** First, every one of these deciders is *sound by
construction* — it cannot give a wrong answer — but its *coverage* (how many real
corpus nets fall in its decidable subclass) is an open empirical question; several
carry `Confidence: medium` in the backlog for exactly this reason. The value of the
lane is therefore itself a measurement, which is the point. Second, this lane lives in
precisely the territory where the foundation's subtlest soundness bugs lived (a
"necessary" condition mistaken for "sufficient," an incomplete cover trusted as a
universal). The guardrails the foundation paid for — *realize the witness rather than
trust a class label, re-derive the universal from primitives, carry the precondition
as checkable data* — are non-negotiable for every decider here.

> **Open theory ratification (Michael's call).** B5 is where B2's relation
> `rank(C) = c − 1` becomes the full well-formedness *equivalence*. B2 deliberately
> proved only the necessary direction (and a hand-built counterexample shows the
> converse fails on the rank relation alone — it is one of four conditions). The
> backlog writes the `⇔` as shorthand; B5 is what earns it, by supplying the other
> three conditions. Confirm that reading.

---

## 3. Measure the number — the observation crate and the rig (M11, M12 / Epic G)

**Plainly.** This is where the construction becomes a *result*. First the tool learns
to record its own behavior — for every net, which shortcut answered, what it cost, and
whether the answer was right — as a structured dataset, with no change to how anything
is decided. Then that dataset is promoted into a scientific instrument that produces
the headline coverage table, measured honestly so the tool cannot simply "memorize"
families of nets.

**Precisely.** Two stages:

- **M11 — the measurement substrate.** A downstream-only crate (`petrivet-observe`,
  whose dependency arrow points only *at* the core, enforced in CI) records two kinds of
  data under a strict discipline: raw per-decider cost fibers, tagged with their machine
  context and *never compared across runs* (absolute timings have no common origin —
  only differences are portable), and a differential comparison object that *is*
  cross-run. On top sits an **always-on soundness sentinel**: every decided result with
  a known ground truth must agree, so a "trusted-but-wrong" shortcut fails the build the
  instant it reappears. The link that makes this lane self-sufficient: **the certificate
  is the training label.** Because an accepted, checked certificate is itself ground
  truth, the dataset is self-labeling — no oracle required to learn from it. The
  contribution of this lane is the *harness*, not any model built on it.
- **M12 — the rig and the thesis.** The harness is promoted to emit, per (net, property):
  the verdict, the certificate kind, whether a structural shortcut or search decided it,
  the cost, and the abstention reason. It computes **family-held-out** cross-validation
  (split by net family, so coverage cannot leak across related nets) and the
  **two-denominator** coverage table (in-scope queries; and all-corpus, counting
  out-of-scope as abstention), **counted in queries decided, not nets**. This is where
  `f_struct` is *measured* — the single largest remaining deliverable — and where `f`
  (the certifying fraction) is reported over the corpus. Around it: the falsifiable
  claim committed in writing with its falsifier, the structural-tier ablation that makes
  the "cheaper" claim non-circular, the versioned corpus, reproducibility, and the
  write-up schedule against the deadline.

**The honest core of this lane.** The headline is a number that *may come back small*,
and the rig is built to report it either way. That is not a risk to manage but the
definition of a falsifiable claim: a small `f_struct`, a thin speed advantage, or
certificates that don't independently check would each refute the thesis, and the
instrument is designed to surface exactly that. The exact wording of the claim is
Michael's to finalize; the framing (a characterization plus a construction, not a
benchmark ranking) is settled.

---

## 4. The gated sequels — built only when a measurement justifies them

**Plainly.** Two more ambitious capabilities are designed but deliberately held back,
each behind a gate that asks "is this worth building?" before any of it is built. They
are safe to defer because the firewall guarantees correctness no matter which method
runs — so a deferred or mis-chosen method costs *speed, never correctness*.

**Precisely.**

- **Certified reductions (Epic F).** The ability to *shrink* a hard net to a smaller
  equivalent, solve the small one, and lift the answer back — with the original net's
  own checker re-validating the lifted proof, so the entire reduction library lives
  *outside* the trusted base. The apparatus (F1) is built and *proven* by deliberately
  writing a wrong transformation and confirming the checker catches it; the first
  reduction (F2) removes a redundant place, certified by an algebraic dual the tool
  already computes and currently discards. The honest caveat is sharp and worth stating:
  the firewall is *clean* for existential witnesses (a wrong firing sequence simply fails
  replay), but for *compositional* lifts (re-padding an invariant across a cut) a buggy
  lift could pass a too-weak check — so trust is restricted to existential lifts until a
  per-kind completeness proof is discharged, and the very first reduction is a
  compositional one. It is sound *at runtime* (a wrong lift wastes time, never
  correctness), but extending the *static* firewall guarantee to it is open theory.

- **Learned algorithm-selection (the Epic D ladder).** Predicting which shortcut will be
  cheapest on a given net from cheap structural features, then trying them in that order;
  later, adapting mid-analysis. The entire ladder is **gated on first measuring the gap**
  between the tool's fixed ordering and the best-possible ordering. If that gap is within
  noise — which it may be, on a small portfolio — then there is nothing to win, the
  hand-ordered cascade is the honest answer, and the model is dead weight that is not
  built. This is a feature: the flagship learned component is conditional on a
  pre-registered measurement that is *allowed to say no*, and a null result is a
  publishable honest finding, not a failure. The model, if built, trains on the
  self-labeled harness data (the certificate as label), runs entirely downstream of the
  core, and every verdict it selects is still independently checked.

---

## 5. The horizons — recorded, not scheduled

**Plainly.** A few directions are written down because they are real and worth marking,
but they are explicitly *not* on the path to submission. They are labeled as research,
some of it genuinely novel and some of it honestly uncertain.

**Precisely.**

- **One hard open research target.** A checkable liveness certificate for a class of net
  *strictly beyond* free-choice. General liveness is a known wall — no compact checkable
  certificate is known to exist — so the target is to push the boundary by one class, not
  to break the wall. It is flagged repeatedly in the backlog as "the genuinely hard,
  high-novelty item." It may not succeed, and is framed as open research.

- **The compositional residuals (Epic H).** The original single "Φ" capstone was
  dissolved — the grand scalar and its necessity claim did not survive scrutiny — leaving
  two precise, measurable, per-property quantities. **Φ_bound** measures how much a net's
  boundedness comes from genuine cross-part synchronization rather than from each part
  being bounded alone (provably zero when the net factors cleanly). **Φ_inv** counts the
  conservation laws that exist for the whole net but that no single part can see — and its
  link to the Rank Theorem and the cluster count `c` the foundation already computes is,
  potentially, a genuinely new object. The deliverable, if pursued, is their *measured
  distribution over the corpus*, not the metaphysics; no stochastic or
  integrated-information apparatus is introduced.

  > **Open ratification (Michael's call).** Φ_inv's *novelty* is a conjecture pending a
  > literature check against the Kronecker / compositional-modeling school. Present it as
  > pending verification, not as an established result — the link to the keystone is real
  > and partly built, but whether it is *new* is the open question.

- **Reusing the engine elsewhere.** Once the state-space engine is abstracted over its
  ordering (a near-term refactor that also lets the tool give partial answers where it now
  abstains), the *same* engine could in principle verify entirely different infinite-state
  system classes with no engine changes. The near-term payoff (the partial answers) is
  in scope; the broad reuse is the far horizon, named for completeness.

---

## 6. Reading the map honestly

Three distinctions are worth keeping straight, because they are what make the plan
defensible rather than aspirational:

- **Committed vs gated vs horizon.** The structural deciders and the rig are committed —
  they are the spine. The reductions and the learned ladder are gated sequels, safe to
  defer because the firewall makes selection a performance question, not a correctness
  one. The Φ residuals, the WSTS reuse, and the beyond-free-choice certificate are
  horizons — recorded, off the critical path.
- **Sound vs covered.** Every committed decider is sound by construction; what is *not*
  yet known is how much of the real corpus each one covers. The honest framing of the
  structural lane is "committed mechanisms with measured, possibly-modest payoff" — and
  measuring that payoff is the thesis.
- **The falsifier is a table, not a pass/fail.** The headline gates are measurements. The
  honest outcome is to report whatever the number is. The project is falsifiable precisely
  because a disappointing number would refute its claim, and the rig is built to show it.

Two decisions remain genuinely Michael's, both flagged above: whether B5 closes the
well-formedness equivalence as intended, and whether Φ_inv is novel against the
compositional literature.

---

## 7. The bottom line

The trustworthy foundation — the hard, load-bearing part — is built and verified. The
remaining *committed* work is conceptually clear: add the structural shortcuts that raise
the coverage number, and build the instrument that measures it honestly, against a fixed
deadline. The more ambitious extensions — reductions, learned selection, the compositional
residuals — are deliberately gated, so that each is built only when a measurement or a
proof earns it, and a null result anywhere is a publishable honest answer rather than a
failure. The direction is set and the foundation supports it; what remains is to raise the
number, measure it, and write down what it says.
