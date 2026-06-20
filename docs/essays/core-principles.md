# Core Principles of petrivet

> Status: condensed statement. This is the distilled, near-axiomatic companion to [petrivet in four principles](petrivet-in-four-principles.md) (the long legible reading). It states the four organizing principles compactly, then expands each with an index of reference identifiers anchored to the code. The four principles are the genuine spine and survive intact; what they converge on has been recast — see the capstone, and the note on the dissolved scalar Φ.

This document summarizes the four organizing principles of petrivet, then states the result they converge on. The first part states each principle compactly; the second expands each into detail with an index of reference identifiers. The principles describe a real, mostly-implemented architecture; the result they converge on is the project's ratified direction — a certificate that is the stone, a measured coverage claim that is the headline, and a soundness firewall that makes both trustworthy.

A note on what changed, stated once and plainly. Earlier drafts of this document ended on a single net-level scalar, Φ_PN, presented as "the one quantity the architecture was built to compute." That capstone is **dissolved**. The scalar/boolean Φ_PN, and the claim that the architecture exists to compute it, predicted nothing and are retired. What survives of that thread is narrow and honest: two computable per-property *factorization residuals*, treated as a measurement, developed in the dedicated essay [the factorization residual](the-factorization-residual.md). The four principles below are unchanged; only their capstone is corrected.

---

# Part I — Summary

## Principle 1: The substrate

A transition affects only its preset and its postset; the entire net is captured by a single matrix, C = Post − Pre. The hardness of analyzing the whole net is not located in any individual transition: the global problem is EXPSPACE-hard despite each transition obeying purely local arithmetic.

`fire` decrements the input places (`•t`), increments the output places (`t•`), and reads nothing else. The rule is local; its consequences are intractable.

## Principle 2: The completion

Rather than enumerating the state space, complete it. Omega represents infinity as a type — the ideal completion of ℕ, implemented directly without being named as such. The kernel is the incidence matrix completed into its conservation laws. The arithmetic rule is ω + anything = ω; the boundary between the finite and completed worlds is a single line of code.

`enum Omega { Finite(u32), Unbounded }`. One engine runs over ℕ or over its completion. A finite witness stands in for an infinite fact.

## Principle 3: The tractable structural subclasses

Select the algorithm from the net's shape before computing. Shrinking a set by a single bad-place rule to its fixpoint yields a closure operator that answers a question about every subset in one pass. On the class of free-choice nets, approximation methods become exact.

Siphon and trap are exact De Morgan duals; `commoner_hack_criterion` settles an existential quantifier over subsets using a single closure operator.

## Principle 4: The epistemic law

Compute the reason, not just the answer: a finite witness that an independent checker can re-verify. Because only the witness is trusted, any heuristic — however unprincipled — may choose what to attempt.

There is no zero-test, no Turing power, and no undecidability; and what is trusted is the certificate, not the prover.

## What the four converge on

The principles do not converge on a scalar. They converge on a program with three parts, in order of weight:

1. **The certificate is the stone.** Principle 4's witness, made into an interoperable, machine-checkable object re-validated by a small external checker that is the *entire* trusted base. This is the signature contribution. Each verdict carries a proof anchored to the original net; an independent checker re-establishes the property, sharing nothing with the prover. See [the checkable frontier](the-checkable-frontier.md).

2. **The measured coverage is the result.** The falsifiable headline is empirical: *on the real MCC P/T corpus, a polynomial structural certifying tier decides a large, characterizable fraction of queries — counted as queries decided, two-denominator, family-held-out — without state-space exploration; where it abstains, it abstains honestly.* The figures of merit are `f_struct` (the structural-coverage fraction) and `f` (the certifying fraction of accepted verdicts). See [the coverage claim](the-coverage-claim.md).

3. **The firewall is what makes both trustworthy.** Principle 4's separation of guess from proof — soundness independent of the selection policy — is the *enabling property*. As a theorem it is a one-line corollary of certifying algorithms composed with algorithm selection. Its non-trivial content is a precondition the code must discharge first: the two `Some(false)` stubs that today return a fabricated verdict with no certificate. See [soundness as a free variable](soundness-as-a-free-variable.md).

Two horizons sit past the headline, and are named as horizons, not spine. **Learned selection** — a SATzilla-style ranker over the certified deciders — is the sequel; it is safe precisely because the firewall holds, and it is justified only if a measured single-best-to-virtual-best gap warrants it (the rungs: [1](rung-1-empirical-hardness-ranker.md), [2](rung-2-sequential-policy.md), [3](rung-3-certified-reductions.md)). The **factorization residuals** are the far horizon: two per-property measurements, not a metaphysics. Neither is the result. The result is the stone and the number.

---

# Part II — Detail

This part expands each principle. Concrete, low-level facts are listed first under each principle, with higher-level claims building on them.

---

## Principle 1: The substrate

This is the most concrete object and the basis for every other construction. A marking is a vector in ℕ^P; firing applies `−Pre +Post`; the net's entire linear theory is captured by **C = Post − Pre**. Locality is explicit in the source code, and that locality is precisely why the global question is hard.

### 1.1 — The micro-level is a free commutative monoid of purely local updates.
- `petri-net-is-local-update-substrate` — `fire`/`is_enabled` at `state_space/mod.rs:149,157` consume one transition's pre/postset and nothing else
- `infinite-nets-simulate-rule-110` — the markings *are* the free commutative monoid ("Petri Nets Are Monoids")
- `structure-first-dispatch`, `subclass-recognition-as-gate` — the dispatch the substrate enables; head of Principle 3
- `cache-friendly-for-the-large`, `performance-as-a-design-commitment` — locality is *why* the structures are cache-friendly: neighbors are local
- `degenerate-nets-excluded-by-design`, `no-empty-nets-provision` — a substrate with no place or transition has no dynamics; excluded by `BuildError`

### 1.2 — The single matrix is the shared basis: state equation, invariants, and boundedness all read it.
- `incidence-matrix-definition`, `incidence-matrix-entry-definition`, `incidence-matrix-as-shared-substrate` — C = Post − Pre, the one object every algorithm reads
- `marking-equation-necessary-condition`, `state-equation-parikh-necessary`, `state-equation-is-necessary-not-sufficient`, `state-equation-murata-convention` — m = m₀ + C·σ, a necessary but not sufficient condition (the asymmetry the dispatch exploits)
- `algebra-outsourced-to-LP`, `library-phrases-algebra-as-feasibility` — the action phrased as LP feasibility
- `realization-deliberately-thin`, `incidence-matrix-only-new-and-get`, `no-rational-matrix-no-gaussian` — the matrix is present as a static object (`new`, `get`) and deliberately minimal; its derived algebra is documented but not yet implemented

### 1.3 — Local rules, intractable whole: the worst case is permanent.
- `worst-case-genuinely-untouched`, `decidable-but-ackermannian` — the gap between the cheap law and the expensive question is real, not an artifact: general-net reachability is Ackermann-complete

---

## Principle 2: The completion

Completion adds ideal points so that a process that almost converges now converges. petrivet performs completion twice — on the order (ideals, ω) and on the algebra (kernel, invariants). Both over-approximate reachability; both are completions. This principle contains the first and second illustrative results.

### 2.1 — A single state-space engine, parameterized by one scalar trait.
- `one-state-space-engine-not-two`, `two-engines-search-reach-and-cover`
- `tokenops-abstracts-the-scalar`, `exactly-two-implementors`, `third-implementor-fits-lock` — `trait TokenOps`, with implementations for `u32` and `Omega`
- `genericity-over-fiber-not-order` — the genericity is over the value in each place, not the order on states
- `karp-miller-is-reachability-over-N-omega`, `bounded-coverability-is-reachability` — a bounded coverability graph *is* a reachability graph, recovered by unwrapping each `Finite(k)`
- `omega-unifies-bounded-and-unbounded`

### 2.1.a — Example: Omega is the ideal completion of ℕ, implemented directly without being named as such.
- `omega-is-infinity-made-a-type`, `omega-as-infinity-made-type` — `enum Omega { Finite(u32), Unbounded }`
- `omega-arithmetic-is-absorptive` — incrementing ω is a no-op; ω dominates every finite value
- `omega-is-ideal-completion-of-N`, `ideal-completion-hand-rolled-unnamed` — precisely the principal ideals ↓k together with the top ideal ℕ
- `cross-type-comparison-is-state-vs-ideal` — comparing a u32 marking against an Omega marking is comparing a state against an ideal
- `omega-is-acceleration`, `omega-accelerate-is-the-wsts-acceleration` — promote the strictly-greater coordinates to ω wherever a new marking dominates an ancestor
- `coverability-finiteness-guarantee`, `coverability-trees-always-finite`, `boundedness-iff-no-omega` — boundedness read off as the absence of the infinity symbol
- `coverability-over-approximates-reachability`, `coverability-graph-decides-coverability`, `coverability-tree-introduces-omega-as-infinity`, `short-circuit-coverability-before-reachability` — an over-approximation: it refutes with certainty but only approximates an exact "yes"; consult the cheaper procedure first

This identification is **real**, not exposition: `Omega` *is* the ideal completion of (ℕ, ≤), and so the existing engine *is* a latent well-structured-transition-system. The payoff is concrete and near-term — abstracting the order into a `WellQuasiOrder` (the wqo obligation licenses termination) turns the blanket `Inconclusive` at the ω-frontier into a backward-coverability refinement (BACKLOG B9/E1), rather than the far-horizon WSTS zoo.

### 2.1.b — The order is the one component left un-abstracted; Dickson's Lemma guarantees the completion terminates.
- `wqo-is-the-unabstracted-thing` — the blanket `impl<T:Ord> PartialOrd for IdxMarking<T>`; `merge-ordering-fold` — the product order returns `None` the instant two coordinates disagree in direction
- `dickson-guarantees-termination` — (ℕ^P, ≤) is a WQO, which is why ω-acceleration terminates
- `engine-is-a-wsts-fragment`, `no-wsts-vocabulary-in-repo`, `generalize-omega-unlocks-other-systems`, `order-future-one-sentence` — making the order abstract turns the existing engine into a WSTS framework
- `liveness-via-scc-structure`, `liveness-via-reachability-graph-sccs`, `liveness-as-a-ladder-l0-l4`, `liveness-levels-L0-to-L4` — liveness read off the SCC condensation (a quotient on the reachability graph): L4 iff in every terminal SCC; a graded level, not a binary flag
- the documented but not-yet-implemented next completion at the unbounded frontier: `unbounded-nets-return-inconclusive`, `backward-coverability-over-upward-closed-sets`, `unique-sorted-slice-is-natural-basis`, `omega-witness-seeds-refinement-loop`

### 2.2 — The algebra completes the matrix into its kernel; the Farkas dual is the certificate of completion.
- `linear-semantics-completes-by-conservation`, `both-semantics-are-completions`, `two-faces-of-one-refusal`, `two-great-semantics-avoid-enumeration` — the order side completes upward into ideals; the algebra side completes into the kernel, into invariants
- `p-invariants-are-kernel-of-C-transpose`, `s-invariants-and-t-invariants-defined`, `s-and-t-invariants-are-dual`, `null-space-bases-are-semiflows` — invariants are the kernel of C and Cᵀ

### 2.2.a — Example: when the solver proves "No," it computes a conservation law explaining the impossibility — but discards it at the last line.
- `discarded-conservation-law`, `code-discards-the-dual` — the path returns the payload-free `MarkingEquationNoRationalSolution` at `reachability.rs:177` and `coverability.rs:124`; the dual y is never extracted
- `infeasible-LP-means-S-invariant-violated`, `farkas-yields-dual-certificate`, `farkas-dual-as-explanation`, `unreachability-carries-an-invariant` — yᵀC = 0 with y·(m′−m₀) ≠ 0: the place-weighting that witnesses the contradiction
- `extract-dual-makes-negative-carry-certificate`, `farkas-dual-checks-by-dot-product` — extracting it would make the negative verdict carry a certificate symmetric to the positive ones, checkable by a single dot product
- `discarded-lp-dual-gets-a-job` — in the reduction calculus the discarded dual becomes a reduction's soundness witness

One honesty caveat belongs here, because it is a soundness hole the firewall does *not* protect (there is no positive object to check on the negative path): today's `Unreachable` verdict rests on a *floating-point* LP failing to find a rational solution. A spurious floating "infeasible" yields a silent false `Unreachable`. The emitted invariant must be exact-rational and pass `y·C = 0 ∧ y·(m′−m₀) ≠ 0` in exact arithmetic before the verdict returns (BACKLOG B0/B1/B1a).

### 2.2.b — Every structural good-behavior property is itself a conservation-law certificate.
- `conservativeness-is-s-invariant-coverage`, `conservativeness-is-positive-s-invariant-coverage`, `consistency-is-positive-t-invariant-coverage`, `consistency-is-t-invariant-coverage` — full conservativeness ⟺ S-invariant coverage; consistency ⟺ T-invariant coverage
- `structural-boundedness-via-lp`, `structural-boundedness-via-positive-subvariant` — structurally bounded iff some y > 0 with yᵀC ≤ 0
- `circuit-token-invariance`, `circuit-token-invariance-in-tnets` — token count on each circuit is closed under firing: an empty circuit is permanently dead, a marked one live

### 2.3 — The algebraic completion is specified but not yet implemented; building it is completion of the design, not new invention.
- `invariants-not-computed-anywhere`, `first-completion-built-second-charted` — the order completion is fully built; the algebra completion is a documented absence
- `exact-rational-core-is-a-fulcrum`, `rank-is-missing-ingredient-for-rank-theorem`, `marking-equation-becomes-refinement-loop`, `not-yet-exploited-shortcuts` — rank (a product of the completion) is the missing ingredient for the Rank Theorem

---

## Principle 3: The tractable structural subclasses

A closure is a monotone shrinking map iterated to its fixpoint; a quotient collapses a coupling into clusters. This is the regime where Principle 2's completions and these closures become exact, polynomial-time decisions — conditioned on recognizing the net's class. It contains the third illustrative result. Duality runs through it: siphon/trap, S/T-invariant, S/T-component, conservative/consistent.

### 3.1 — Try structure first; fall back to search only when shape cannot decide.
- `structure-first-dispatch`, `structure-first-then-search`, `subclass-recognition-as-gate`, `search-vs-structure-tension`, `structure-as-the-escape-from-explosion`
- `petrivet-routes-around-worst-case` — fast not by solving the worst case but by avoiding it: classify into a tractable structural class first
- `petrivet-is-portfolio-solver`, `ascending-cost-cascade`, `reachability-cascade-ladder`, `cascade-gated-by-class` — an ascending-cost sequence of partial deciders, each stage gated by `self.class()`: try the cheap reason first

This structural tier is exactly the object the headline number measures: the fraction of corpus queries it decides, without exploration, is `f_struct` (BACKLOG G4a/G4). The generators of Principle 2 and Principle 3 are what widen that fraction.

### 3.2 — On the free-choice class, liveness and boundedness become exact polynomial-time decisions.
- `island-where-approximations-stop-approximating`, `island-is-philosophical-heart`, `commoner-decides-fc-liveness-polynomially`, `subclass-exact-shortcuts`, `six-concerns-are-an-arc`
- the catalog of exact domains — each a class where the cheap reason *is* the truth:
  - S-nets: `snet-live-iff-strongly-connected-and-token`, `snet-safe-iff-at-most-one-token`, `s-system-live-iff-cycles-cover-with-tokens`, `s-system-exact-per-place-bounds`, `snet-reachability-rational-iff-reachable`, `marking-equation-rational-is-exact-for-s-nets` — total unimodularity ⇒ rational solve exact for S-nets
  - T-nets: `tnet-live-iff-token-on-every-circuit`, `tnet-place-bound-is-min-circuit-tokens`, `tnet-live-safe-iff-every-place-on-1-token-circuit`, `tnet-reachability-integer-plus-no-empty-circuit`, `t-net-reachability-needs-circuit-check`, `t-system-live-iff-nonempty-presets-and-cycle-tokens`
  - free-choice / asymmetric: `commoner-fc-live-iff-marked-trap-in-every-siphon`, `live-fc-safe-iff-covered-by-1-token-s-components`, `live-safe-fc-decomposes-into-s-components`, `fc-boundedness-from-s-component-token-sums`, `asymmetric-choice-marked-trap-sufficient-only`
  - coverage dualities: `s-coverage-conservative-t-coverage-consistent`, `t-component-coverage-implies-consistency`, `s-and-t-components-defined`

### 3.2.a — Example: siphon and trap are exact De Morgan duals; one closure operator settles an existential over every subset of places in a single pass.
- `siphon-trap-de-morgan-duals`, `siphon-trap-are-exact-de-morgan-duals` — the same loop with preset↔postset swapped, at `literature.rs` Alg 6.19, lines 349–371
- `siphon-trap-are-closure-operators`, `maximal-siphon-algorithm-shrinks`, `maximal-siphon-closure-terminates` — `while some place is bad, remove it`; terminates because the set only shrinks; yields the unique maximal siphon (or trap) in any set
- `commoner-hack-cleverest-move`, `one-closure-answers-existential-over-subsets`, `one-closure-settles-all-subsets` — "the siphon contains a marked trap" iff "the maximal trap in the siphon is marked," replacing the naive enumerate-all-traps approach
- `siphons-traps-govern-starvation-trapping`, `chc-needs-marked-trap-in-every-siphon`, `commoner-hack-liveness-iff`, `chc-sufficient-for-general-nets`
- the closure result carries proof or counterexample: `chc-result-carries-witness-or-counterexample`, `chc-positive-and-negative-evidence` — every siphon with its marking witness on success, the exact starving siphon (a deadlock certificate) on failure

That siphon and trap are De Morgan-dual closures is another **real** identification — two implementors of one closure operation, with duality an incidence-direction flip — and it is exactly the closure that makes free-choice liveness a *polynomially checkable* certificate: the verifier checks the exhibited cover even though enumerating all siphons is hard (BACKLOG B7, and the frontier map in [the checkable frontier](the-checkable-frontier.md)).

### 3.3 — The cluster quotient: the missing construction that unlocks both halves of the class.
- `cluster-quotient-is-the-keystone`, `cluster-is-equivalence-of-place-transition-coupling`, `union-find-computes-clusters-cheaply`, `cluster-quotient-unlocks-both-halves`, `quotient-gives-cluster-count-c`, `quotient-gives-s-and-t-components`
- the Rank Theorem awaiting it: `rank-vs-clusters-decides-boundedness`, `cluster-appears-once-in-crate` — rank = c−1, c = number of clusters; `no-cluster-construction-no-rank`
- the unimplemented pieces: `island-stops-at-waterline`, `s-component-decomposition-charted-and-absent`, `is-covered-by-s-components-hardcoded-false`
- the meta-statement, kept as exposition only: `everything-is-order-completion-closure-quotient`, `four-primitives-enumerated`, `code-is-already-four-abstractions-longhand`, `future-is-making-four-constructions-abstract` — the reading that order, completion, closure, and quotient are the four constructions the code spells out by hand. This framing is *lovely and it is exposition*: it organizes the code, but it predicts nothing and licenses no new theorem. It is named as a reading, not a result. (Of the four, only `WellQuasiOrder` and `Closure` earn a trait now, each with two implementors; `Quotient`/`Completion` stay concrete — a trait over a population of one is aesthetics, not generality.)

---

## Principle 4: The epistemic law

This principle explains why the other three are run. Principles 1–3 describe what petrivet builds; this one describes the purpose of a build: not to enumerate, but to construct a finite, checkable reason — and, once the reason is checkable, the search producing it may use arbitrary heuristics. It contains the trust boundary, the answer to the project's founding question, and the discipline that only checkable content counts as believed.

### 4.1 — Refuse to enumerate; the reason for the refusal is exactly the decidability sweet spot.
- `sub-turing-no-zero-test`, `universality-traded-for-analyzability`, `decidability-sweet-spot` — a net cannot test a place for emptiness; the absence of a zero-test is exactly why the core questions remain decidable
- `inhibitor-arc-cliff` — one inhibitor arc installs a zero-test, restores Turing-completeness, and destroys decidability at a single stroke
- `universality-needs-infinite-nets`, `worst-case-vs-operative-question`, `roadmap-extensions-are-syntactic-sugar`, `extensions-chosen-not-collected` — expressive power is a deliberately incurred cost; every gain in power is a loss of decidability

### 4.2 — A verdict is a witness, not a bit; the boolean is a byproduct. The witness is the stone.
- `result-not-boolean`, `proof-carrying-stance`, `proof-carrying-as-deliberate-stance`, `evidence-types-per-subclass`, `results-cite-their-theorems`
- `verdicts-carry-different-witnesses`, `coverability-dual-witness`, `certifying-algorithms-vocabulary`, `certifying-decider-definition` — a scalar token-sum, a Parikh vector, a replayable firing sequence: the proof's shape is dictated by the theorem that produced it; an independent checker verifies it without trusting the prover
- the verification calculus to be built (the witness made re-checkable, and the signature contribution): `instinct-not-calculus`, `five-properties-five-shapes`, `result-pattern-once`, `proofs-inert-no-verify`, `caller-must-trust-sequence`, `certificate-trait-implied`, `data-already-certificate-shaped`, `firing-sequence-checks-by-replay`, `marking-equation-proof-checks-by-recompute`, `siphon-trap-checks-by-closure`, `no-new-theory-needed`, `certificates-auditable-offline`, `certificate-verdict-coda-abstraction`
- the trusted base is the checker, not the decider: `checker-not-decider-is-trusted-base`, `shrink-then-verify-the-checker`

This is the project's signature contribution and the reason it is *the stone* rather than a tendency: a `Certificate::check(net, m0, query)` that re-establishes the property against the **original** net, the `query` argument required (a firing-sequence witness is meaningless without the target it claims to reach), assuming nothing about which decider produced the witness. An interoperable, name-anchored serialization makes a certificate from one procedure check identically against another. The trusted base is `{checkers} ∪ {remaining bare-boolean deciders}`, measured and required non-increasing. See [the checkable frontier](the-checkable-frontier.md) for the per-property × polarity map, including the wall: general (non-free-choice) liveness has no known compact certificate, and integer-only infeasibility's honest witness is a cutting-plane derivation, not a single Farkas dual.

### 4.3 — State only what can be shown: a claim is robust when it is labeled with its true strength.
- `claim-honesty-as-method`, `precision-makes-claims-unshakable`, `five-grades-of-claim` — SEE / READ-INTO / IMAGINE / BELIEVE / PROMISE; precision, not emphasis, is what removes grounds for doubt
- `verdict-honesty-equivalence-vs-sufficiency` — `commoner-fc-live-iff-marked-trap-in-every-siphon` (an iff on the free-choice class) vs `asymmetric-choice-marked-trap-sufficient-only` (sufficient, not equivalent — and it states which)
- honest abstention: `inconclusive-as-honest-verdict`, `ackermannian-honesty-on-unbounded`, `unbounded-nets-return-inconclusive` — the frontier is one line behind the omega check
- the hazard of a falsely labeled verdict: `liveness-l0-soundness-hazard`, `verdict-and-not-verdict-same-face`, `liveness-hazard-type-enforced` — "unknown" (the L0 hazard) must be made indistinguishable in type from "provably dead" only when it genuinely is; a false label here is unsound
- recording the gap: `not-yet-exploited-honesty`, `convention-reconciliation`, `three-sources-one-canon` — record the rejected alternative (the Primer's |P|×|T| versus Murata's transpose)

### 4.4 — The certificate is the trust boundary: the heuristic chooses what to try, the proof decides what is believed. (This answers the founding question: use both a guess and a proof, separated by a boundary.)
- `proof-or-guess-both-with-a-wall`, `certificate-firewall`, `certificate-is-the-firewall`, `guess-chooses-what-to-try`, `proof-decides-what-is-believed`, `learning-and-soundness-never-touch`, `heuristic-inside-verifier-without-anxiety`
- the policy-independence property: `soundness-theorem`, `soundness-proof-mechanism`, `learner-outside-trusted-base`, `order-cannot-affect-answer`, `learning-confined-to-performance`, `flat-guarantee-rising-capability`, `wrong-guess-only-wastes-time`, `distribution-shift-benign`, `theorem-says-safe-not-helpful`
- where the boundary has gaps (the real risk is trusted-but-wrong code, not the ML): `firewall-strength-is-certifying-fraction`, `trusted-decider-is-trusted-base`, `two-some-false-stubs`, `two-some-false-hazards`, `real-soundness-risk-is-not-ml`, `prerequisite-fix-some-false-stubs`, `certificate-and-portfolio-one-project`

The firewall is the *enabling property*, not the headline — and it is worth being exact about its standing. As a **theorem** it is one line: a verdict is returned only if a checker accepted a certificate, and the policy cannot alter the acceptance predicate; therefore soundness is independent of the policy (the proof composes McConnell–Mehlhorn certifying algorithms with Rice/SATzilla algorithm selection — both mature). Its **non-trivial content is a precondition**, not the corollary: the theorem covers only the *certifying* fraction of the decider set, and today two arms — `is_covered_by_s_components` (`api/net/mod.rs:270`) and the marked-graph liveness arm (`liveness.rs:107`) — return a fabricated `Some(false)` with no certificate. They are trusted-but-wrong. Fixing them so a decider that cannot yet certify returns `None` and escalates is the **near-term north star** (BACKLOG A2). The figure of merit is the certifying fraction `f` (BACKLOG A6/C5/G6), measured and required to rise.

The framing that petrivet is "strictly stronger than MuZero / has a verified leaf where Go does not" is a *true and clarifying* observation about where the learner sits relative to the trusted base — but the AlphaGo/MuZero lineage is dropped as the design's spine. The honest lineage of selection is SATzilla and Rice. petrivet *has* checkable leaves, which makes its problem easier than and unlike AlphaGo; presenting it as AlphaGo-with-a-twist overclaims the kinship. The selection direction is the sequel, developed in [soundness as a free variable](soundness-as-a-free-variable.md) and the rungs.

### 4.4.a — Selection as an instrumented policy (the sequel, not the spine).
- the cascade lifted into data: `routing-is-algorithm-selection`, `runtime-prediction-then-select`, `selection-as-sequential-decision-process`, `decider-set-already-exists`, `decider-table-cost-polarity-cert`, `soundness-domain-examples`, `cascade-hardcoded`, `no-decider-trait`, `decider-metadata-in-doccomments`, `lift-cascade-into-data`, `default-schedule-reproduces-behavior`, `one-trait-one-telemetry-hook-away`, `anytime-corollary-parallel-racing`, `learned-parallel-portfolio-natural-completion`, `ml-policy-schedules`, `learned-policy-predicts-which-fires`, `decider-learned-schedule-coda-abstraction`
- the precursor already present in the code: `policy-already-exists-as-one-if`, `technique-tags-are-telemetry-embryo`, `only-learned-part-is-policy-next`, `little-new-machinery`
- the prior work and the unoccupied synthesis: `proposer-checker-lineage`, `fastforward-domain-precedent`, `generalize-fastforward`, `three-families-of-learners`, `petrivet-synthesis-of-one-and-two`, `contest-names-structural-reduction`, `composition-is-the-synthesis`
- the effective-theory reading, marked speculation, never a design justification: `policy-is-effective-theory-of-hardness`, `israeli-goldenfeld-coarse-grain`, `causal-states-minimal-sufficient`, `information-bottleneck-lagrangian`, `feature-design-doctrine`, `mutual-information-feature-test`. This material is an *interpretation* of why selection might help on a distribution; it is not evidence and not a justification for any design choice. Whether the chosen features suffice for the hardness label is a measured question (BACKLOG X4), gated on a measured single-best-to-virtual-best gap (BACKLOG D5).

### 4.4.b — The rungs: rising ambition, constant soundness. Each rung is a horizon, gated behind the certificate.
- the rungs themselves: `ladder-rung-0`, `ladder-rung-1`, `ladder-rung-2`, `ladder-rung-3`, `implementation-branches-vs-ladder-rungs` — see [rung 1](rung-1-empirical-hardness-ranker.md), [rung 2](rung-2-sequential-policy.md), [rung 3](rung-3-certified-reductions.md). MCC *ranking* is out of scope as a goal: the contest is the crucible (honest, protocol-correct abstention, cross-checked against an oracle) and the labelling source (the certificate is the training label), not a leaderboard to climb.
- self-labeling — the certificate is the label: `certificate-is-the-training-label`, `self-play-against-cost`, `oracle-is-cross-check-not-training-dep`, `solving-corpus-yields-proof-trees`, `features-present-as-data-absent-as-vector`, `nupn-unit-tree-is-missing-feature`
- Stage 3 — certified reductions as moves, the apparatus reused: `rung3-second-verb-transform`, `reduction-trait-three-methods`, `lift-is-the-keystone`, `each-reduction-must-be-certifying`, `buggy-lift-cannot-break-soundness`, `and-or-proof-tree-search`. The robustness here is clean for *existential* witnesses (a wrong `lift` produces a firing sequence the original-net checker rejects, so it costs time, not correctness); for compositional/invariant lifts it must be proven per certificate kind (BACKLOG F1).

### 4.5 — Truth is what survives a quotient: keep only what is invariant under the don't-cares.
Firing order, the prover's identity, and micro-level detail are precisely the don't-cares that a reason must be invariant under.
- firing ORDER quotiented (the Parikh image): `state-equation-is-necessary-not-sufficient` — necessity-but-not-sufficiency is exactly the information the quotient discards; `marking-equation-becomes-refinement-loop`

### 4.5.a — The machine origin is a don't-care: costs live in a torsor, fitness in the quotient.
- `measure-differences-record-the-bundle`, `absolute-timings-have-no-origin`, `schedule-is-a-choice-of-origin`, `log-costs-form-a-torsor`, `fitness-lives-in-quotient`, `torsor-glossary-group-forgot-identity`, `section-is-a-schedule`, `bundle-of-torsors-precise-not-loose`, `persist-raw-fibers-tagged-with-context`, `never-bake-schedule-into-measurement`, `fitness-tests-are-differential-assertions`, `committing-baseline-commits-a-section`, `no-regression-on-log-ratios`, `raw-cost-not-cross-comparable`, `prefer-invariant-counters`, `regret-is-torsor-quotient-quantity`, `ranker-needs-differential-not-absolute`, `ranker-learns-section-of-bundle`, `torsor-keeps-measurement-honest`, `timing-noise-mitigated-structurally`

### 4.5.b — Observe, never act: the harness measures fitness and the dependency arrow points only one way.
- `observability-is-measurement-domain`, `harness-is-self-test-and-training-set`, `harness-describes-fitness-does-not-act`, `harness-observes-never-certifies-never-schedules`, `dependency-arrow-points-only-to-core`, `pure-downstream-observer`, `harness-hands-off-dataset-not-model`, `scope-creep-guarded-by-arrow-lint`, `seam-is-only-point-of-contact`, `two-record-types`, `phase1-no-core-change`, `phase2-minimal-additive-seam`, `each-phase-independently-shippable`, `loading-excluded-from-cost`, `harness-no-ops-on-missing-corpus`, `soundness-sentinel-runs-underneath`, `soundness-sentinel-catches-stubs`, `oracle-is-truth-signal-until-check-exists`, `phi-features-already-cheap-accessors`, `phi-recorded-as-raw-named-fields`

The soundness sentinel here — every `Decided` row with a known oracle must agree — is the live regression that catches exactly the A2 stubs. It is the firewall's precondition turned into a test.

### 4.6 — Specify the interface before implementing it: the design is committed to disk ahead of the code.
- `literature-tells-what-petrivet-is`, `literature-as-load-bearing-index`, `citation-index-binds-theorem-to-function`, `deeplinks-to-nonexistent-module`, `blueprint-drawn-ahead-of-stone`, `blueprint-ahead-of-the-stone`, `gap-is-self-authored-map`, `unbuilt-names-are-promise-to-self`, `architecture-promises-to-itself`, `seams-cut-with-next-generality`, `dream-is-completion-not-invention`, `every-abstraction-has-pointer-today`, `literature-organized-by-source`, `literature-imports-only-under-rustdoc`, `doc-links-as-cfg-doc-imports`
- the project's stated goal — the union it embodies: `motto-theory-application-union`, `for-researchers-and-practitioners`, `readable-api-over-rigorous-impl`

---

## Capstone: the certificate, the coverage, the firewall

The four principles converge, and the convergence is the project's ratified direction — not a scalar but a program.

**The certificate is the stone.** Principle 4's witness, raised from a tendency into an interoperable, machine-checkable object re-validated by a small external checker that is the entire trusted base. Every verdict carries a proof anchored to the original net; an independent checker re-establishes the property without trusting the prover. This is the signature contribution, and it is what the architecture is actually built around.

**The measured coverage is the result.** The falsifiable headline is empirical: on the real MCC P/T corpus, the polynomial structural certifying tier (Principles 2 and 3) decides a large, characterizable fraction of queries — `f_struct`, two-denominator, family-held-out — without state-space exploration; where it abstains, it abstains honestly, and the boundary is predictable from cheap structural features. The certifying fraction `f` reports how much of what it accepts is independently checked. These two numbers, not a theorem, are the thesis.

**The firewall is what makes both trustworthy.** Soundness independent of the selection policy is the enabling property — a one-line corollary as a theorem, a real precondition as code (the two `Some(false)` stubs). It is why the structural tier can be reordered, raced, or eventually learned without endangering correctness, and why honest abstention is always available.

### The factorization residuals are a measurement, not a metaphysics.

The earlier capstone — a single net-level scalar Φ_PN, "the one quantity the architecture was built to compute" — is dissolved. The scalar/boolean Φ_PN, and the necessity claim that it requires all four principles, predicted nothing and are retired. What survives is two computable per-property residuals: Φ_bound (a boundedness-factorization residual: how much a net is bounded only through cross-block synchronization, provably zero on live bounded free-choice nets and strongly-connected T-nets) and Φ_inv (an invariant rank-defect residual: the count of conservation laws no single block can see, linked to the Rank Theorem's cluster count). Each is monotone, theorem-backed-zero, non-vacuous, and emits a minimizing cut as a C1-checkable witness. The deliverable is their *distribution over the corpus*, indexed by net class — a measurement. This is a far horizon (BACKLOG H2a/H2b), recorded, not scheduled. It is developed in [the factorization residual](the-factorization-residual.md).

### Scope limit: the residual is mathematics; any extension to minds is metaphor and lies outside this work.
- `iit-absent-from-repository`, `consciousness-leap-is-the-metaphor`, `full-iit-needs-stochastic-semantics` — a "Petri-net Φ" is a number and a witness measuring failure-to-factor; it makes no claim about minds. IIT is not present in the repository.

---

## The four principles, restated

- **Principle 1 — Substrate.** A transition affects only its preset and postset; the net is one matrix, C = Post − Pre. The hardness of the whole net resides in no single transition.
- **Principle 2 — Completion.** Rather than enumerate the state space, complete it. Omega represents infinity as a type; the kernel is the matrix completed into its conservation laws. A finite witness stands in for an infinite fact.
- **Principle 3 — Structural subclasses.** Choose the algorithm by net shape before computing. One closure answers a question about every subset at once; on the free-choice class, approximations become exact.
- **Principle 4 — Epistemic law.** Compute the reason, not just the answer — a finite witness anyone can re-verify. A heuristic chooses what to try; only the checked witness is believed. "Cannot yet decide" is a valid verdict, not a failure.

And what they converge on:

- **The certificate** — the witness made interoperable and externally checkable, the entire trusted base reduced to a small checker. *The stone.*
- **The coverage** — `f_struct` and `f`, measured on the MCC corpus, queries-decided and certifying-fraction. *The result, and the falsifier: the fraction is small, or the structural path is not cheaper, or the certificates are not independently checkable.*
- **The firewall** — soundness independent of selection, an enabling property whose real content is the precondition the code discharges first.

---

The overall structure: locality makes the question hard (Principle 1); completion makes it answerable without traversing the trajectory (Principle 2); the right structural shape makes whole classes of it exact (Principle 3); and the certificate makes it safe to reach those answers via an arbitrary heuristic while remaining correct (Principle 4). They converge not on a number-to-compute but on a thing-to-trust and a thing-to-measure: a certificate that any third party can check, and a coverage fraction that says how far the cheap, checkable tier reaches.
