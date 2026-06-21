# State of the Work — what the foundation accomplishes

*A summary of the development work since the last shared baseline (`origin/main`,
the "optimized liveness algorithms" commit), written to be read from the ground up:
each part is stated plainly first, to establish what it is and why it matters, then
in precise technical terms. The plain reading is for orientation; the technical
reading is the real claim. The detailed records are in
[`foundational-design.md`](foundational-design.md) (§1–§4.10), the two handbacks
([`HANDBACK.md`](../../HANDBACK.md), [`m3-b2-handback.md`](m3-b2-handback.md)), and
[`BACKLOG.md`](../../BACKLOG.md); this document is the overview that ties them
together.*

---

## 1. What the project is, and what it claims

**Plainly.** `petrivet` answers yes/no questions about whether a concurrent system
behaves correctly — *can it get permanently stuck? can it grow without limit? can
every action always eventually happen again? can it reach a particular situation?*
The systems are described as **Petri nets**, a standard model of events ("transitions")
that move resources ("tokens") between states ("places"). A tool that answers such
questions is a *model checker*. The hard part is twofold: answering cheaply (the
honest brute-force method — enumerate every reachable state — explodes), and
answering *trustworthily* (a fast shortcut is only useful if you can believe it).

**Precisely.** The five properties the tool decides are **reachability** (is a target
marking reachable from the initial one?), **coverability** (is some reachable marking
≥ a target?), **boundedness** (is the reachable state space finite?), **liveness**
(can every transition always eventually fire again?), and **deadlock-freedom** (is no
reachable marking a total deadlock?). The thesis's organizing claim — its *ratified
direction* — is an inversion of where the original vision put its emphasis:

- **The signature contribution is the certificate-and-checker.** Every verdict carries
  a machine-checkable *certificate* — a witness (a firing sequence, an invariant, a
  structural cover) that a *small, independent checker re-validates against the
  original net*, sharing no code with the procedure that produced it. That small
  checker is then the *entire* trusted base: you trust it, not the large solver.
- **The falsifiable headline is empirical.** On the real benchmark corpus (the Model
  Checking Contest P/T nets), what fraction of queries does a *polynomial structural
  tier* decide without state-space exploration? That number is called `f_struct`. It
  is a characterization plus a construction, not a leaderboard ranking.
- **The soundness firewall is the enabling property, not the headline.** "The answer
  is correct regardless of which algorithm or selection policy produced it" is, as a
  theorem, a short corollary; its real content is a *precondition the code must
  discharge* — and the figure of merit is `f`, the share of accepted verdicts that
  actually carry a checked certificate.

Everything below is in service of making those three things real, in code, rather
than described in prose. Learned algorithm-selection and the deeper compositional
("Φ") measures are explicitly *sequels*, not part of this foundation.

---

## 2. Where the code stands now

**Plainly.** Since the last shared commit, the work is **26 commits, 72 files,
about +14,400 lines and −300** — almost entirely additions. Roughly 9,400 lines of
that are code and 4,700 are documentation. The small deletion count is meaningful:
this is new structure built *underneath and alongside* the existing analyzer, not a
rewrite of it.

**Precisely.** The delta is three layers stacked on the shared baseline:

| Layer | Size | What it is |
|---|---|---|
| Vision & doctrine | ~3,500 lines, docs | The research essays and specs reconciled into one ratified direction (§1), plus `BACKLOG.md` (the dependency-ordered plan with falsifiable gates) and the *working doctrine* (the contract for how the code is built: falsifiability first, soundness before capability, let the type system carry the invariants) |
| The foundation | ~8,400 lines, code+docs | Milestones M0–M2 + M4/M5 + a soundness remediation pass + the A6 polarity gate — the verdict/certificate layer, the exact-arithmetic core, the checkers |
| The registry & keystone | ~2,500 lines, code+docs | Milestone M3 (the decider registry) + B2 (the cluster keystone) + a soundness fix found by independent verification + the handback |

The rest of this document is what those layers *accomplish*, told as five concrete
changes in what the tool can do.

---

## 3. What is now true that was not before

### 3.1 Every answer carries proof — and the code physically cannot skip it

**Plainly.** Before, the tool returned a bare "yes" or "no," and correctness rested
on the procedure being right. Now, an answer can only be produced by passing it
through a single gate that re-checks the supporting evidence against the original
system. We built this into the language's type system, so an attempt to fabricate a
verdict without that check **does not compile**. This is the heart of the thesis —
*trustworthy regardless of which algorithm produced it* — turned from an intention
into an enforced property.

**Precisely.** The foundation introduced a `model` module with three abstractions:

- `Verdict<P, N>` — a three-valued result: a checked positive proof `P`, a checked
  negative refutation `N`, or an honest `Inconclusive` that *carries the reason it
  abstained*. "Cannot decide" is now type-distinct from "provably false," which
  closes a real prior defect where an undecidable liveness query and a provably-dead
  transition were the same value.
- A `Certificate` trait whose one method, `check(net, m0, query) -> bool`,
  re-establishes the claimed property against the **original** net using only
  primitive access (firing transitions, exact dot products). Because it shares no
  code with the generator, a certificate produced by *any* procedure — or, in
  principle, another tool — re-validates identically.
- `accept` — the **sole** constructor of `Proven`/`Refuted`. It runs the certificate's
  `check`, *and* confirms the certificate's polarity matches the verdict's (a
  positive witness cannot mint a refutation), promoting the candidate only if both
  pass; otherwise it returns `Inconclusive`. There is no syntactic path from a failed
  check to a decided verdict.

The enforcement is structural, not conventional: the decided variants of `Verdict`
wrap their payload in `Proof`/`Refutation` types whose field is *private to the
module*, so `Verdict::Proven(x)` cannot be name-constructed from outside — proven by
compile-fail tests. The result is that the **trusted computing base is now a small,
named set**: a ledger enumerates every fast decider, tagged as *certifying* (a checker
re-validates it) or *bare-boolean* (trust rests on the decider), and reports `f = 4/7`
with the bare set pinned non-increasing. That ledger is the concrete, measurable form
of "how much do you actually have to trust."

### 3.2 No answer rests on approximate arithmetic

**Plainly.** The tool used to lean on floating-point (approximate decimal) math in
places that *decide* an answer. Approximate math can round a fraction off and flip a
conclusion — and it does so in a data-dependent, invisible way that is exactly the
hardest kind of bug to catch. We moved every answer-deciding calculation onto **exact**
arithmetic. Where exactness cannot be reached, the tool now abstains rather than guess.
An entire category of subtle wrong answers is closed off by construction.

**Precisely.** The foundation built an exact-rational core:

- `Rational` over `i128`, where every operation is *checked* and returns a `Result` —
  it **detects overflow and never silently wraps**. Equality and ordering are
  representation-independent and exact at every magnitude (no float ever decides an
  ordering).
- `Matrix` over `Rational`, with exact `rank`, null-spaces (the structural
  invariants), linear `solve`, and an exact **Farkas certificate** — the separating
  invariant that proves a target unreachable, which the prior code computed in
  floating point and then discarded.

The discipline this enforces is "floating point may *suggest*, never *decide*." The
fast LP/ILP filters remain, but only as suggesters; the verdict is re-derived exactly
or escalated to the always-terminating state-space method. This is the precondition
for §3.1 to be meaningful — a certificate is only as trustworthy as the arithmetic
that checks it.

### 3.3 Ten inputs on which the tool gave a wrong answer now give the right one

**Plainly.** Each of these was a real input — a specific net and question — where the
tool returned a *confidently wrong* result. The most dangerous kind is a *false
"proven"*: the tool says it has established something it has not. Each is now correct,
and locked in by a regression test that fails on the old code:

| # | The tool used to… | Now |
|---|---|---|
| 1 | fire an event but forget to update the system's state | updates correctly |
| 2 | scramble its internal graph and miscompute from it | stable and correct |
| 3 | call a healthy system broken (two hard-coded wrong answers) | abstains honestly instead |
| 4 | call a state unreachable because an approximate solver gave up | only when exact math proves it |
| 5 | call a target reachable from a category label, with no actual path | only with a real step-by-step path |
| 6 | call a system live/deadlock-free from incomplete evidence | re-derives the full condition itself |
| 7 | decide several properties with approximate math | exact arithmetic |
| 8 | give wrong answers when an internal counter overflowed | a wider counter |
| 9 | call a system "stays finite" when it actually grows forever | an exact check |
| 10 | call a system "never gets stuck" when its starting state is already stuck | checks the starting state |

**Precisely.** Grouped by kind, with the mechanism:

- **Engine-correctness defects (1–2).** `fire_unchecked` discarded the token delta it
  computed, so a fired transition left the marking unchanged. And `build()` populated
  the analysis graph in hash-map order, scrambling the node labels — which corrupted
  circuit enumeration and the SCC-based liveness analysis downstream. Both were
  silent miscomputations, not crashes.
- **Fabricated-verdict / abstention fixes (3, 4).** Two efficient-path constants
  returned a hard-coded `Some(false)` ("not covered by S-components"; the marked-graph
  liveness stub), reporting healthy systems as broken; both became honest abstention
  (the A2 north star). Separately, a negative reachability verdict rested on a
  floating-point LP merely *failing* to find a solution — a spurious infeasibility at
  a degenerate vertex was a silent false `Unreachable` (the B1a hole), now re-derived
  exactly over the rationals.
- **The certificate holes (5, 6) — the subtle, dangerous ones.** A marking-equation
  Parikh certificate accepted a witness on a *class label* alone, but the marking
  equation is only *necessary* without a liveness precondition — so a non-live net of
  the right class minted a false `Proven` reachable. It now accepts only by **realizing**
  the witness as an actual firing sequence, which is sound on any class. And a
  siphon/trap cover for liveness/deadlock-freedom validated only the *exhibited* pairs,
  not the **universal** condition (the Commoner–Hack criterion: *every* minimal siphon
  must contain a marked trap) — so an incomplete cover passed. It now re-derives the
  universal independently, and a typed `SiphonTrapClaim{Live, DeadlockFree}` makes the
  liveness-versus-deadlock-freedom conflation *unrepresentable* (the criterion proves
  liveness only on free-choice nets, deadlock-freedom on any net).
- **Arithmetic / overflow (7, 8, 9).** Three remaining float verdict paths
  (`Uncoverable`, the integer reachability arm, positive `is_bounded`) and the
  live-marked-graph reachability arm were closed over exact arithmetic or realization;
  token sums on verdict paths were widened from `u32` (which wraps in release builds —
  a wrap could make two different totals compare equal and mint a false `reachable`,
  `dead`, or `safe`) to `u64`; and an asymmetric-choice net was reported `bounded`
  from the Commoner–Hack criterion, which is *not* a boundedness theorem there (an
  always-enabled pump grows unboundedly while staying deadlock-free).
- **The deadlock seed (10).** Found by the independent verification of the latest work:
  the state-space search evaluated its predicate only on newly-discovered *successor*
  states, never on the *initial* state — so when the initial marking is itself a total
  deadlock, the tool reported the system deadlock-free. Fixed by testing the seed
  marking once, at the start.

Nine were found while building the foundation; #10 in verifying the registry. The
recurring lesson in the subtle ones (5, 6, 10) is the most important methodological
outcome: they passed the tests written by the same author who wrote the code, and were
caught only when an *independent* reviewer built running counterexamples against the
brute-force oracle. That independent adversarial review is now a standing step in the
process, not an afterthought.

### 3.4 The choice of which analysis to run is now a data structure, not hardcoded logic

**Plainly.** The tool's logic for picking which fast method to try for a given system
used to be hardcoded, property by property. We turned it into a *registry* — a list
you plug new methods into. Adding a new analysis is now writing one small, self-contained
piece and registering it, with no risk to the trustworthy core or to the existing
analyses.

**Precisely.** The decider registry (M3) makes the execution schedule a value. A
`Decide` trait represents each procedure with its metadata — the pole it can conclude,
its cost tier, and which net classes it applies to. A `Policy` orders the applicable
deciders; a `Driver` runs them and returns the first decided verdict, routing every
acceptance through `accept` (§3.1). The default policy *reproduces today's hand-coded
cascade exactly* — verified by a corpus regression test and by per-property tests that
the accepted result is invariant under reordering the deciders (the policy-independence
property, which is the soundness firewall made testable).

Two design points carry weight here. First, where a procedure already emits a
re-checkable witness, the registry routes it through the *real* checker — so the
free-choice liveness and deadlock-freedom verdicts are now *genuinely certifying at
runtime*, carrying the re-derived Commoner–Hack cover. Where a procedure has no compact
witness yet, it routes through a placeholder that performs no independent re-check but
keeps the boundary uniform and, crucially, *does not inflate `f`* — the ledger keeps
counting it as trusted-base. Second, the structural generators the thesis still needs
are now a *fill-in-the-blank*: each is one new `Decide` implementation slotted at its
position, with no change to the trait, the policy, the driver, or the trust boundary.
The hard problem was making the architecture admit them cleanly; that is done.

### 3.5 The free-choice structural theory has its keystone

**Plainly.** For a well-behaved class of these systems, there is deep theory that
yields cheap structural answers — no enumeration needed. That theory has one small,
central calculation everything else depends on. We built it. The remaining shortcuts
in that family are now fill-in-the-blank rather than a design problem.

**Precisely.** B2 is the cluster-quotient keystone. A near-linear union-find over the
net's consumption arcs computes the **cluster count `c`** (the Desel–Esparza clusters:
the connected components linking a place to the transitions that consume from it). The
keystone then relates `c` to the exact incidence-matrix rank from §3.2 via the **Rank
Theorem**, `rank(C) = c − 1`. A discernment worth flagging: that relation is *necessary
but not sufficient* for a free-choice system to be well-formed (live and bounded) — it
is one of four conditions, not an equivalence — so it is implemented and tested as the
forward direction only, validated against the state-space oracle. (An earlier draft
over-claimed the converse; building counterexamples by hand caught it.) The cluster
partition and the rank relation are exactly what the deferred structural deciders
(S/T-component decomposition, the simultaneous liveness-and-boundedness check) consume;
the keystone deliberately stops there and leaves those deciders for the next phase.

---

## 4. What this sets up, and what honestly remains

**Plainly.** The centerpiece of the thesis — cheap answers, each carrying
independently-checkable proof — now exists as working, tested code, and the bugs that
would have undermined it are gone. What remains is bounded and clear, not open-ended.

**Precisely.** The runway, in dependency order:

- **The headline number is not yet measured.** `f_struct` — the fraction of corpus
  queries the structural tier decides — is the falsifiable thesis claim. The substrate
  is in place (the floor is measured; the differential measurement harness exists); the
  full, family-held-out, two-denominator measurement is the largest open deliverable
  (Epic G). This is the step that turns the construction into a *result*.
- **The registry runs alongside the cascade, not yet in front of it.** This is
  deliberate: keeping the two parallel makes their proven equivalence the ground truth
  before any rewiring. Routing the public methods through the registry is a later,
  separately-gated step.
- **Certifying coverage is partial.** Some deciders still carry the trusted-base
  placeholder. Emitting their witnesses raises the runtime `f` and is the next
  increment, scheduled one decider at a time with the generator that produces the
  witness (the S-component cover for boundedness, the realized word for live
  state-machine reachability, the pumping-cycle witness for coverability).
- **The structural deciders the keystone unlocks** (S/T-component, the class-agnostic
  continuous and deadlock-freedom deciders) are each one registry slot; they also close
  B2's relation from *necessary* to the full Rank-Theorem equivalence.
- **The learned selection ladder** (the SATzilla-style sequel) is gated on first
  *measuring* a non-trivial gap between the hand-ordered cascade and the best-possible
  ordering. If the gap is within noise, the hand-ordered cascade is the honest answer
  and the machine learning is dead weight — a discipline the plan states up front.

A few smaller, separable items round it out: wiring the cancellation/budget seam into
the cascade; reconciling the WebAssembly crate against the consolidated API; a follow-up
audit for other instances of the seed blind spot (#10); and three pre-existing cleanups
left untouched here (a stale example, doctest re-export drift, and some missing
benchmark fixtures).

---

## 5. The bottom line

At the last shared baseline, the thesis's central claim was a described plan with
several latent correctness holes beneath it: verdicts that could be fabricated, answers
that could turn on a rounding error, and a handful of inputs the tool got outright
wrong. It is now a **built, type-enforced, independently-verified mechanism** — answers
that cannot be produced without checkable proof, computed in exact arithmetic, with the
known soundness holes closed and pinned by regression tests, and an analysis pipeline
that new methods extend by construction. The remaining work is *measurement and
coverage* — taking the headline number, and filling in the structural shortcuts the
keystone unlocks — not foundational uncertainty. The hard, load-bearing part is done
and verified; what is left is to run it forward.
