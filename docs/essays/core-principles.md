# Core Principles of petrivet

This document summarizes the four organizing principles of petrivet, followed by a capstone quantity, Φ_PN, whose definition depends on all four. The first part states each principle compactly; the second part expands each into detail with an index of reference identifiers.

---

# Part I — Summary

## Principle 1: The substrate

A transition affects only its preset and its postset; the entire net is captured by a single matrix, C = Post − Pre. The hardness of analyzing the whole net is not located in any individual transition: the global problem is EXPSPACE-hard despite each transition obeying purely local arithmetic.

`fire` decrements the input places (`•t`), increments the output places (`t•`), and reads nothing else. The rule is local; its consequences are intractable.

## Principle 2: The completion

Rather than enumerating the state space, complete it. Omega represents infinity as a type — the ideal completion of ℕ, implemented directly without being named as such. The kernel is the incidence matrix completed into its conservation laws. The arithmetic rule ω + anything = ω; the boundary between the finite and completed worlds is a single line of code.

`enum Omega { Finite(u32), Unbounded }`. One engine runs over ℕ or over its completion. A finite witness stands in for an infinite fact.

## Principle 3: The tractable structural subclasses

Select the algorithm from the net's shape before computing. Shrinking a set by a single bad-place rule to its fixpoint yields a closure operator that answers a question about every subset in one pass. On the class of free-choice nets, approximation methods become exact.

Siphon and trap are exact De Morgan duals; `commoner_hack_criterion` settles an existential quantifier over subsets using a single closure operator.

## Principle 4: The epistemic law

Compute the reason, not just the answer: a finite witness that an independent checker can re-verify. Because only the witness is trusted, any heuristic — however unprincipled — may choose what to attempt.

There is no zero-test, no Turing power, and no undecidability; and what is trusted is the certificate, not the prover.

## Capstone: Φ_PN

Φ_PN measures how far a net is from being a product of independent parts. It is a minimum, taken over every structural cut, of the gap between the verdict for the whole net and the tensor of the verdicts for its parts. The gap is the number; the cut that achieves it is the witness; the residual is the part that no decomposition compresses. Defining it requires all four principles simultaneously.

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

### 3.3 — The cluster quotient: the missing construction that unlocks both halves of the class.
The same partition is also the natural domain of composition — the lattice over which the capstone minimizes.
- `everything-is-order-completion-closure-quotient`, `four-primitives-enumerated`, `code-is-already-four-abstractions-longhand`, `future-is-making-four-constructions-abstract` — the meta-statement naming the four operations these principles rest on: order, completion, closure, quotient
- `cluster-quotient-is-the-keystone`, `cluster-is-equivalence-of-place-transition-coupling`, `union-find-computes-clusters-cheaply`, `cluster-quotient-unlocks-both-halves`, `quotient-gives-cluster-count-c`, `quotient-gives-s-and-t-components`
- the Rank Theorem awaiting it: `rank-vs-clusters-decides-boundedness`, `cluster-appears-once-in-crate` — rank = c−1, c = number of clusters; `no-cluster-construction-no-rank`
- the unimplemented pieces: `island-stops-at-waterline`, `s-component-decomposition-charted-and-absent`, `is-covered-by-s-components-hardcoded-false`

---

## Principle 4: The epistemic law

This principle explains why the other three are run. Principles 1–3 describe what petrivet builds; this one describes the purpose of a build: not to enumerate, but to construct a finite, checkable reason — and, once the reason is checkable, the search producing it may use arbitrary heuristics. It contains the trust boundary, the answer to the project's founding question, and the principle that only quotient-invariant content counts as truth.

### 4.1 — Refuse to enumerate; the reason for the refusal is exactly the decidability sweet spot.
- `sub-turing-no-zero-test`, `universality-traded-for-analyzability`, `decidability-sweet-spot` — a net cannot test a place for emptiness; the absence of a zero-test is exactly why the core questions remain decidable
- `inhibitor-arc-cliff` — one inhibitor arc installs a zero-test, restores Turing-completeness, and destroys decidability at a single stroke
- `universality-needs-infinite-nets`, `worst-case-vs-operative-question`, `roadmap-extensions-are-syntactic-sugar`, `extensions-chosen-not-collected` — expressive power is a deliberately incurred cost; every gain in power is a loss of decidability

### 4.2 — A verdict is a witness, not a bit; the boolean is a byproduct.
- `result-not-boolean`, `proof-carrying-stance`, `proof-carrying-as-deliberate-stance`, `evidence-types-per-subclass`, `results-cite-their-theorems`
- `verdicts-carry-different-witnesses`, `coverability-dual-witness`, `certifying-algorithms-vocabulary`, `certifying-decider-definition` — a scalar token-sum, a Parikh vector, a replayable firing sequence: the proof's shape is dictated by the theorem that produced it; an independent checker verifies it without trusting the prover
- the verification calculus not yet built (the witness made re-checkable): `instinct-not-calculus`, `five-properties-five-shapes`, `result-pattern-once`, `proofs-inert-no-verify`, `caller-must-trust-sequence`, `certificate-trait-implied`, `data-already-certificate-shaped`, `firing-sequence-checks-by-replay`, `marking-equation-proof-checks-by-recompute`, `siphon-trap-checks-by-closure`, `no-new-theory-needed`, `certificates-auditable-offline`, `certificate-verdict-coda-abstraction`
- the trusted base is the checker, not the decider: `checker-not-decider-is-trusted-base`, `shrink-then-verify-the-checker`

### 4.3 — State only what can be shown: a claim is robust when it is labeled with its true strength.
- `claim-honesty-as-method`, `precision-makes-claims-unshakable`, `five-grades-of-claim` — SEE / READ-INTO / IMAGINE / BELIEVE / PROMISE; precision, not emphasis, is what removes grounds for doubt
- `verdict-honesty-equivalence-vs-sufficiency` — `commoner-fc-live-iff-marked-trap-in-every-siphon` (an iff on the free-choice class) vs `asymmetric-choice-marked-trap-sufficient-only` (sufficient, not equivalent — and it states which)
- honest abstention: `inconclusive-as-honest-verdict`, `ackermannian-honesty-on-unbounded`, `unbounded-nets-return-inconclusive` — the frontier is one line behind the omega check
- the hazard of a falsely labeled verdict: `liveness-l0-soundness-hazard`, `verdict-and-not-verdict-same-face`, `liveness-hazard-type-enforced` — "unknown" (the L0 hazard) must be made indistinguishable in type from "provably dead" only when it genuinely is; a false label here is unsound
- recording the gap: `not-yet-exploited-honesty`, `convention-reconciliation`, `three-sources-one-canon` — record the rejected alternative (the Primer's |P|×|T| versus Murata's transpose)

### 4.4 — The certificate is the trust boundary: the heuristic chooses what to try, the proof decides what is believed. (This answers the founding question: use both a guess and a proof, separated by a boundary.)
- `proof-or-guess-both-with-a-wall`, `certificate-firewall`, `certificate-is-the-firewall`, `guess-chooses-what-to-try`, `proof-decides-what-is-believed`, `learning-and-soundness-never-touch`, `heuristic-inside-verifier-without-anxiety`
- the policy-independence theorem: `soundness-theorem`, `soundness-proof-mechanism`, `learner-outside-trusted-base`, `order-cannot-affect-answer`, `learning-confined-to-performance`, `flat-guarantee-rising-capability`, `wrong-guess-only-wastes-time`, `distribution-shift-benign`, `theorem-says-safe-not-helpful`
- every leaf is a verified proof (an asymmetry the game of Go lacks): `every-leaf-can-be-checked`, `go-has-no-verified-leaf`, `muzero-inverse-of-petrivet`, `strictly-stronger-than-muzero`, `real-rollouts-verified-rewards`
- where the boundary has gaps (the real risk is trusted-but-wrong code, not the ML): `firewall-strength-is-certifying-fraction`, `trusted-decider-is-trusted-base`, `two-some-false-stubs`, `two-some-false-hazards`, `real-soundness-risk-is-not-ml`, `prerequisite-fix-some-false-stubs`, `certificate-and-portfolio-one-project`

### 4.4.a — The portfolio operationalizes the trust boundary: the cascade as data, the ordering as a learnable parameter; the policy learns an effective theory of hardness — what can be coarse-grained.
- the cascade lifted into data: `routing-is-algorithm-selection`, `runtime-prediction-then-select`, `selection-as-sequential-decision-process`, `decider-set-already-exists`, `decider-table-cost-polarity-cert`, `soundness-domain-examples`, `cascade-hardcoded`, `no-decider-trait`, `decider-metadata-in-doccomments`, `lift-cascade-into-data`, `default-schedule-reproduces-behavior`, `one-trait-one-telemetry-hook-away`, `anytime-corollary-parallel-racing`, `learned-parallel-portfolio-natural-completion`, `ml-policy-schedules`, `learned-policy-predicts-which-fires`, `decider-learned-schedule-coda-abstraction`
- the precursor already present in the code: `policy-already-exists-as-one-if`, `technique-tags-are-telemetry-embryo`, `only-learned-part-is-policy-next`, `little-new-machinery`
- the prior work and the unoccupied synthesis: `proposer-checker-lineage`, `fastforward-domain-precedent`, `generalize-fastforward`, `three-families-of-learners`, `petrivet-synthesis-of-one-and-two`, `contest-names-structural-reduction`, `composition-is-the-synthesis`
- the AlphaGo correspondence and the effective theory: `alphago-move-not-smaller`, `learn-distribution-collapse-difficulty`, `policy-value-triad`, `triad-exact-mapping`, `policy-is-effective-theory-of-hardness`, `policy-discovers-effective-theory`, `hardness-on-distribution-different-object`, `reachability-graph-is-micro-substrate`, `israeli-goldenfeld-coarse-grain`, `commuting-diagram-criterion`, `autonomous-means-self-predicting`, `causal-states-minimal-sufficient`, `information-bottleneck-lagrangian`
- which macro-variables to coarse-grain onto: `feature-design-doctrine`, `prefer-aggregate-descriptors`, `structural-features-are-autonomous-macrovariables`, `mutual-information-feature-test`, `feature-sufficiency-empirical`, `bach-borrowable-move`

### 4.4.b — The stages: rising ambition, constant soundness. Each stage dominates the previous one; the certificate holds correctness fixed.
- the stages themselves: `ladder-rung-0`, `ladder-rung-1`, `ladder-rung-2`, `ladder-rung-3`, `implementation-branches-vs-ladder-rungs`
- Stage 1 — static ranker: `rung1-makes-prior-a-learned-function`, `rung1-is-one-shot-static`, `failure-confined-to-cheap-dimension`, `objective-is-cost-sensitive-regret`, `sbs-floor-vbs-ceiling`, `tree-models-fit-tabular-features`, `ranker-is-an-instrument`, `ranker-only-reorders`, `performance-not-provably-monotone`, `run-free-shortcuts-first`, `cap-first-pick-with-deadline`, `blend-with-hand-order-prior`, `split-by-family-not-instance`, `cold-start-covered-by-prior`, `model-stays-outside-core`
- Stage 2 — adaptive controller: `rung2-closes-the-loop`, `new-verb-is-preempt`, `rung2-action-space`, `pandoras-box-index-baseline`, `three-formulations-increasing-ambition`, `reach-for-simplest-formulation`, `anytime-parallel-racing`, `race-only-under-uncertainty`, `offline-off-policy-from-logs`, `offline-rl-extrapolation-error`, `new-actions-harmless-to-soundness`, `rung2-dominates-rung1`, `rung2-monotone-only-if-bootstrapped`, `reward-design-is-performance-knob`, `certificate-seals-correctness-off`, `cancellation-is-hard-prerequisite`, `cancellation-reaches-toward-core-minimally`, `budget-cancellation-absent`, `deadlines-start-coarse`
- self-labeling — the certificate is the label: `certificate-is-the-training-label`, `self-play-against-cost`, `oracle-is-cross-check-not-training-dep`, `solving-corpus-yields-proof-trees`, `features-present-as-data-absent-as-vector`, `nupn-unit-tree-is-missing-feature`
- Stage 3 — certified reductions, the apparatus used as moves: `rung3-second-verb-transform`, `moves-simplify-a-proof-obligation`, `reduction-trait-three-methods`, `lift-is-the-keystone`, `reductions-are-scaffolding`, `reduction-library-is-apparatus-as-actions`, `rung3-eats-the-whole-apparatus`, `each-reduction-must-be-certifying`, `buggy-lift-cannot-break-soundness`, `muzero-frontier-bright-line`, `and-or-proof-tree-search`, `theorem-proving-correspondence`, `value-net-learns-cost-to-proof`, `curriculum-emerges`, `lift-functions-are-the-real-work`, `search-blowup-value-net-is-hope`, `rung3-is-the-spire-raised-last`, `prove-like-a-mathematician`

### 4.5 — Truth is what survives a quotient: keep only what is invariant under the don't-cares.
Firing order, the prover's identity, and micro-level detail are precisely the don't-cares that a reason must be invariant under.
- firing ORDER quotiented (the Parikh image): `state-equation-is-necessary-not-sufficient` — necessity-but-not-sufficiency is exactly the information the quotient discards; `marking-equation-becomes-refinement-loop`

### 4.5.a — The machine origin is a don't-care: costs live in a torsor, fitness in the quotient.
- `measure-differences-record-the-bundle`, `absolute-timings-have-no-origin`, `schedule-is-a-choice-of-origin`, `log-costs-form-a-torsor`, `fitness-lives-in-quotient`, `torsor-glossary-group-forgot-identity`, `section-is-a-schedule`, `bundle-of-torsors-precise-not-loose`, `persist-raw-fibers-tagged-with-context`, `never-bake-schedule-into-measurement`, `fitness-tests-are-differential-assertions`, `committing-baseline-commits-a-section`, `no-regression-on-log-ratios`, `raw-cost-not-cross-comparable`, `prefer-invariant-counters`, `regret-is-torsor-quotient-quantity`, `ranker-needs-differential-not-absolute`, `ranker-learns-section-of-bundle`, `rung2-adaptive-section`, `torsor-survives-into-rung3`, `torsor-keeps-measurement-honest`, `timing-noise-mitigated-structurally`

### 4.5.b — Observe, never act: the harness measures fitness and the dependency arrow points only one way.
- `observability-is-measurement-domain`, `harness-is-self-test-and-training-set`, `harness-describes-fitness-does-not-act`, `harness-observes-never-certifies-never-schedules`, `dependency-arrow-points-only-to-core`, `pure-downstream-observer`, `harness-hands-off-dataset-not-model`, `scope-creep-guarded-by-arrow-lint`, `seam-is-only-point-of-contact`, `two-record-types`, `phase1-no-core-change`, `phase2-minimal-additive-seam`, `each-phase-independently-shippable`, `loading-excluded-from-cost`, `timing-is-greenfield`, `harness-no-ops-on-missing-corpus`, `soundness-sentinel-runs-underneath`, `soundness-sentinel-catches-stubs`, `oracle-is-truth-signal-until-check-exists`, `phi-features-already-cheap-accessors`, `phi-recorded-as-raw-named-fields`

### 4.6 — Specify the interface before implementing it: the design is committed to disk ahead of the code.
The vision is committed to disk before construction, so the architecture's later parts are constrained by promises made early.
- `literature-tells-what-petrivet-is`, `literature-as-load-bearing-index`, `citation-index-binds-theorem-to-function`, `deeplinks-to-nonexistent-module`, `blueprint-drawn-ahead-of-stone`, `blueprint-ahead-of-the-stone`, `gap-is-self-authored-map`, `unbuilt-names-are-promise-to-self`, `architecture-promises-to-itself`, `seams-cut-with-next-generality`, `dream-is-completion-not-invention`, `every-abstraction-has-pointer-today`, `literature-organized-by-source`, `literature-imports-only-under-rustdoc`, `doc-links-as-cfg-doc-imports`
- the project's stated goal — the union it embodies: `motto-theory-application-union`, `for-researchers-and-practitioners`, `readable-api-over-rigorous-impl`

---

## Capstone: Φ_PN

Φ_PN measures how far a net is from being a product of independent parts. Cut the net every way the structure permits; the smallest gap between the whole and its reassembled parts is the number, and the cut that achieves it is the witness.

Φ_PN is not a fifth principle but the fixpoint of the whole — the one quantity whose definition requires all four principles at once. It is a minimum (Principle 2's order) over a decomposition lattice (Principle 3's quotient) of a distance between a verdict (Principle 4) and a tensor of sub-verdicts (Principle 1's monoid, factored). It measures the residual that the completion cannot complete and the coarse-graining cannot compress. The policy learns what can be coarse-grained; Φ_PN measures what cannot.

### The factorization residual: a minimum over a partition lattice.
- `phi-pn-honest-number`, `factorization-as-the-goal`, `phi-pn-definition`, `phi-is-minimum-over-cuts`, `iit-mathematical-kernel`, `phi-zero-means-reducible`, `phi-positive-means-irreducible-with-witness`, `phi-pn-is-number-and-witness`, `phi-pn-real-mathematics`, `delta-measures-shortfall`, `tensor-is-property-specific`, `phi-pn-resists-coarse-graining`, `learn-what-can-be-coarse-grained-phi-measures-rest`

### Convergence: Φ_PN requires every principle.
- `phi-needs-all-five-prior-structures`, `phi-needs-order-engine` (Principle 2), `phi-needs-structural-decomposition` (Principle 3), `phi-needs-linear-algebra` (Principle 2 — C completed into kernel and Farkas dual), `phi-needs-certificate-calculus` (Principle 4), `phi-needs-decision-portfolio` (Principle 4), `phi-is-what-architecture-built-to-compute`, `verdict-calculus-is-whole-behavior`, `decomposition-and-integration-same-lattice`

### Where the partitions come from, and the cut as a learned action.
- `decomposition-lattice-source`, `nupn-unit-tree-is-candidate-partition`, `nupn-parsed-into-forest`, `fixtures-have-deep-unit-trees`, `nupn-unconsulted-decomposition-oracle`, `no-analysis-reads-nupn`, `unit-safe-flag-unused-invariant`, `no-composition-operator`, `composition-most-purely-potential`, `boundary-spec-is-software-not-nets`, `unittree-phi-coda-abstraction`
- the cut as a candidate move: `value-net-recognizes-clean-cuts`, `phi-pn-as-live-heuristic-subthread` — a cut is a good move exactly when the property factors over it; the value net is in part learning to predict Φ_PN
- good behavior as clean factorization (Φ_PN = 0): `s-component-coverage-implies-bounded`, `t-component-coverage-implies-consistent`

### Scope limit: the residual is mathematics; any extension to minds is metaphor and lies outside this work.
- `iit-absent-from-repository`, `consciousness-leap-is-the-metaphor`, `full-iit-needs-stochastic-semantics` — a "Petri-net Φ" is a number and a witness measuring failure-to-factor; it makes no claim about minds. IIT is not present in the repository.

---

## The four principles, restated

- **Principle 1 — Substrate.** A transition affects only its preset and postset; the net is one matrix, C = Post − Pre. The hardness of the whole net resides in no single transition.
- **Principle 2 — Completion.** Rather than enumerate the state space, complete it. Omega represents infinity as a type; the kernel is the matrix completed into its conservation laws. A finite witness stands in for an infinite fact.
- **Principle 3 — Structural subclasses.** Choose the algorithm by net shape before computing. One closure answers a question about every subset at once; on the free-choice class, approximations become exact.
- **Principle 4 — Epistemic law.** Compute the reason, not just the answer — a finite witness anyone can re-verify. A heuristic chooses what to try; only the checked witness is believed. "Cannot yet decide" is a valid verdict, not a failure.

And the quantity they converge on:

- **Φ_PN** — how far a net is from being a product: the smallest gap, over every cut the structure permits, between the whole and its reassembled parts. The gap is the number; the cut is the witness; the residual is what nothing compresses.

---

The overall structure: locality makes the question hard (Principle 1); completion makes it answerable without traversing the trajectory (Principle 2); the right structural shape makes whole classes of it exact (Principle 3); and the certificate makes it safe to reach those answers via an arbitrary heuristic while remaining correct (Principle 4) — converging on Φ_PN, the single quantity the architecture was built to compute.
