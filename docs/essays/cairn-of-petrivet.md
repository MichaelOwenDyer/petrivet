# THE CAIRN OF PETRIVET

The four roots converge on one capstone. Concreteness rises toward the top.

---

# PART I — THE CAIRN

> ## I. THE SUBSTRATE
> **A transition touches only its preset and its postset; the net is one matrix, C = Post − Pre. The whole's hardness lives nowhere in any single transition — it is EXPSPACE-hard from purely local arithmetic.**
> *— `fire` decrements `•t`, increments `t•`, sees nothing else. Local law, intractable consequence.*

> ## II. THE COMPLETION
> **Don't walk the state space — complete it. Omega is infinity made into a type, the ideal completion of ℕ hand-rolled without ever naming itself; the kernel is the matrix completed into its conservation laws. ω + anything = ω; the boundary between the two worlds is one line.**
> *— `enum Omega { Finite(u32), Unbounded }`. One engine runs over ℕ or over its completion. A finite witness stands in for the infinite fact.*

> ## III. THE STRUCTURAL ISLAND
> **Choose the algorithm by the net's shape before you compute. Shrink a set by one bad-place rule to its fixpoint, and one closure answers a question about every subset in a single shot. On the free-choice island the approximations stop approximating.**
> *— siphon and trap are exact De Morgan duals; `commoner_hack_criterion` settles an existential-over-subsets with one closure operator.*

> ## IV. THE EPISTEMIC LAW
> **Compute the reason, not the answer — a finite witness a skeptic can recheck. Then let any wild guess choose what to try, because only the witness is believed.**
> *— no zero-test, no Turing power, no undecidability; and the certificate, not the prover, is what is trusted.*

> ## ✦ THE CAPSTONE
> **Φ_PN — how much a net refuses to be a product.**
> *— a minimum, over every structural cut, of the gap between the whole's verdict and the tensor of its parts. The gap is the number; the cut is the witness; the residual is what nothing compresses. To name it you need all four roots at once.*

---

# PART II — THE ASCENT

*How the many become the few. Every leaf descends.*

---

## ROOT I — THE SUBSTRATE
### *"A transition touches only its preset and its postset; the net is one matrix. Everything else is read off this arithmetic, and the whole's hardness is something no transition can hold."*

The most concrete object and the parent of every carrier. A marking is a vector in ℕ^P; firing is `−Pre +Post`; the net's entire linear theory is **C = Post − Pre**. Locality is literal in the source — and locality is exactly why the *global* question is hard.

### → I.1 — The microphysics is a free commutative monoid of purely local updates.
- `petri-net-is-local-update-substrate` — `fire`/`is_enabled` at `state_space/mod.rs:149,157` consume one transition's pre/postset and nothing else
- `infinite-nets-simulate-rule-110` — the markings *are* the free commutative monoid ("Petri Nets Are Monoids")
- `structure-first-dispatch`, `subclass-recognition-as-gate` — the dispatch the substrate enables; head of Root III
- `cache-friendly-for-the-large`, `performance-as-a-design-commitment` — locality is *why* the structures are cache-friendly: neighbors are local
- `degenerate-nets-excluded-by-design`, `no-empty-nets-provision` — a substrate with no place or transition has no physics; excised by `BuildError`

### → I.2 — The one matrix is the shared spine: state equation, invariants, boundedness all read it.
- `incidence-matrix-definition`, `incidence-matrix-entry-definition`, `incidence-matrix-as-shared-substrate` — C = Post − Pre, the one object every algorithm reads
- `marking-equation-necessary-condition`, `state-equation-parikh-necessary`, `state-equation-is-necessary-not-sufficient`, `state-equation-murata-convention` — m = m₀ + C·σ, necessary not sufficient (the asymmetry the dispatch exploits)
- `algebra-outsourced-to-LP`, `library-phrases-algebra-as-feasibility` — the action phrased as LP feasibility
- `realization-deliberately-thin`, `incidence-matrix-only-new-and-get`, `no-rational-matrix-no-gaussian` — the matrix is present as a static object (`new`, `get`) and deliberately austere; its derived algebra is charted but unbuilt

### → I.3 — Local rules, intractable whole: the worst case is permanent.
- `worst-case-genuinely-untouched`, `decidable-but-ackermannian` — the gap between cheap law and expensive question is real, not an artifact: general-net reachability is Ackermann-complete

---

## ROOT II — THE COMPLETION
### *"Adjoin the limit you keep running into. Omega absorbs the finite; the kernel absorbs the matrix; the infinite fits in a finite type. One engine runs over ℕ or over its completion — the boundary is one line."*

Completion adds ideal points so a process that *almost* converges now *does*. petrivet performs it twice — on the order (ideals, ω) and on the algebra (kernel, invariants). Both over-approximate reachability; both are completions. Home of the first exemplar stone and the second.

### → II.1 — ONE state-space engine, parameterized by a single scalar trait.
- `one-state-space-engine-not-two`, `two-engines-search-reach-and-cover`
- `tokenops-abstracts-the-scalar`, `exactly-two-implementors`, `third-implementor-fits-lock` — `trait TokenOps`, with impls for `u32` and `Omega`
- `genericity-over-fiber-not-order` — the genericity is over the value in each place, not the order on states
- `karp-miller-is-reachability-over-N-omega`, `bounded-coverability-is-reachability` — a bounded coverability graph *is* a reachability graph, recovered by unwrapping each `Finite(k)`
- `omega-unifies-bounded-and-unbounded`

### →→ II.1.a — ★ Omega is the ideal completion of ℕ, hand-rolled without ever naming itself.
- `omega-is-infinity-made-a-type`, `omega-as-infinity-made-type` — `enum Omega { Finite(u32), Unbounded }`
- `omega-arithmetic-is-absorptive` — incrementing ω is a no-op; ω dominates every finite value
- `omega-is-ideal-completion-of-N`, `ideal-completion-hand-rolled-unnamed` — precisely the principal ideals ↓k together with the top ideal ℕ
- `cross-type-comparison-is-state-vs-ideal` — comparing a u32 marking against an Omega marking is comparing a state against an ideal
- `omega-is-acceleration`, `omega-accelerate-is-the-wsts-acceleration` — promote the strictly-greater coordinates to ω wherever a new marking dominates an ancestor
- `coverability-finiteness-guarantee`, `coverability-trees-always-finite`, `boundedness-iff-no-omega` — boundedness read off as the simple absence of the infinity-symbol
- `coverability-over-approximates-reachability`, `coverability-graph-decides-coverability`, `coverability-tree-introduces-omega-as-infinity`, `short-circuit-coverability-before-reachability` — an honest over-approximation: refutes with certainty, only approximates the exact yes; consult the cheap one first

### →→ II.1.b — The order is the one un-abstracted thing; Dickson's Lemma guarantees the completion terminates.
- `wqo-is-the-unabstracted-thing` — the blanket `impl<T:Ord> PartialOrd for IdxMarking<T>`; `merge-ordering-fold` — the product order returns `None` the instant two coordinates disagree in direction
- `dickson-guarantees-termination` — (ℕ^P, ≤) is a WQO, which is why ω-acceleration terminates
- `engine-is-a-wsts-fragment`, `no-wsts-vocabulary-in-repo`, `generalize-omega-unlocks-other-systems`, `order-future-one-sentence` — make the order abstract and the engine you already wrote becomes a WSTS framework
- `liveness-via-scc-structure`, `liveness-via-reachability-graph-sccs`, `liveness-as-a-ladder-l0-l4`, `liveness-levels-L0-to-L4` — liveness read off the SCC condensation (a quotient on the reachability graph): L4 iff in every terminal SCC; a rung, not a flag
- the charted next-completion at the unbounded frontier: `unbounded-nets-return-inconclusive`, `backward-coverability-over-upward-closed-sets`, `unique-sorted-slice-is-natural-basis`, `omega-witness-seeds-refinement-loop`

### → II.2 — The algebra completes the matrix into its kernel; the Farkas dual is the certificate of completion.
- `linear-semantics-completes-by-conservation`, `both-semantics-are-completions`, `two-faces-of-one-refusal`, `two-great-semantics-avoid-enumeration` — the order side completes upward into ideals; the algebra side completes into the kernel, into invariants
- `p-invariants-are-kernel-of-C-transpose`, `s-invariants-and-t-invariants-defined`, `s-and-t-invariants-are-dual`, `null-space-bases-are-semiflows` — invariants are the kernel of C and Cᵀ

### →→ II.2.a — ★ When the solver proves "No," it computes a conservation law that explains the impossibility — and discards it at the last line.
- `discarded-conservation-law`, `code-discards-the-dual` — the path returns the payload-free `MarkingEquationNoRationalSolution` at `reachability.rs:177` and `coverability.rs:124`; the dual y is never extracted
- `infeasible-LP-means-S-invariant-violated`, `farkas-yields-dual-certificate`, `farkas-dual-as-explanation`, `unreachability-carries-an-invariant` — yᵀC = 0 with y·(m′−m₀) ≠ 0: the place-weighting that witnesses the contradiction
- `extract-dual-makes-negative-carry-certificate`, `farkas-dual-checks-by-dot-product` — extracting it makes the negative verdict carry a certificate symmetric to the positive ones, checkable by one dot product
- `discarded-lp-dual-gets-a-job` — in the reduction calculus the discarded dual becomes a reduction's soundness witness

### →→ II.2.b — Every structural good-behavior is itself a conservation-law certificate.
- `conservativeness-is-s-invariant-coverage`, `conservativeness-is-positive-s-invariant-coverage`, `consistency-is-positive-t-invariant-coverage`, `consistency-is-t-invariant-coverage` — full conservativeness ⟺ S-invariant coverage; consistency ⟺ T-invariant coverage
- `structural-boundedness-via-lp`, `structural-boundedness-via-positive-subvariant` — structurally bounded iff some y > 0 with yᵀC ≤ 0
- `circuit-token-invariance`, `circuit-token-invariance-in-tnets` — token count on each circuit is closed under firing: an empty circuit is forever dead, a marked one live

### → II.3 — The completion is the named-but-unbuilt half; building it is completion, not invention.
- `invariants-not-computed-anywhere`, `first-completion-built-second-charted` — the order completion is built beautifully; the algebra completion is a charted absence
- `exact-rational-core-is-a-fulcrum`, `rank-is-missing-ingredient-for-rank-theorem`, `marking-equation-becomes-refinement-loop`, `not-yet-exploited-shortcuts` — rank (a completion product) is the missing ingredient for the Rank Theorem

---

## ROOT III — THE STRUCTURAL ISLAND
### *"Choose the algorithm by the net's shape before you compute. Shrink a set by one bad-place rule to its fixpoint — and one closure answers a question about every subset at once. Where the net is shaped, the approximations stop approximating."*

A closure is a monotone shrinking map iterated to its fixpoint; a quotient collapses a coupling into clusters. The island is where Root II's completions and these closures become *exact, polynomial* decisions — gated by recognizing the net's class. Home of the third exemplar stone. Duality runs through it as a reflection: siphon/trap, S/T-invariant, S/T-component, conservative/consistent.

### → III.1 — Try structure first; fall back to search only when shape cannot decide.
- `structure-first-dispatch`, `structure-first-then-search`, `subclass-recognition-as-gate`, `search-vs-structure-tension`, `structure-as-the-escape-from-explosion`
- `petrivet-routes-around-worst-case` — fast not by solving the worst case but by routing around it; land on the structural island first
- `petrivet-is-portfolio-solver`, `ascending-cost-cascade`, `reachability-cascade-ladder`, `cascade-gated-by-class` — an ascending-cost ladder of partial deciders, each rung gated by `self.class()`: try the cheap reason first

### → III.2 — On the free-choice island, liveness and boundedness become exact polynomial decisions.
- `island-where-approximations-stop-approximating`, `island-is-philosophical-heart`, `commoner-decides-fc-liveness-polynomially`, `subclass-exact-shortcuts`, `six-concerns-are-an-arc`
- the exact-domain catalog — each a region where the cheap reason *is* the truth:
  - S-nets: `snet-live-iff-strongly-connected-and-token`, `snet-safe-iff-at-most-one-token`, `s-system-live-iff-cycles-cover-with-tokens`, `s-system-exact-per-place-bounds`, `snet-reachability-rational-iff-reachable`, `marking-equation-rational-is-exact-for-s-nets` — total unimodularity ⇒ rational solve exact for S-nets
  - T-nets: `tnet-live-iff-token-on-every-circuit`, `tnet-place-bound-is-min-circuit-tokens`, `tnet-live-safe-iff-every-place-on-1-token-circuit`, `tnet-reachability-integer-plus-no-empty-circuit`, `t-net-reachability-needs-circuit-check`, `t-system-live-iff-nonempty-presets-and-cycle-tokens`
  - free-choice / asymmetric: `commoner-fc-live-iff-marked-trap-in-every-siphon`, `live-fc-safe-iff-covered-by-1-token-s-components`, `live-safe-fc-decomposes-into-s-components`, `fc-boundedness-from-s-component-token-sums`, `asymmetric-choice-marked-trap-sufficient-only`
  - coverage dualities: `s-coverage-conservative-t-coverage-consistent`, `t-component-coverage-implies-consistency`, `s-and-t-components-defined`

### →→ III.2.a — ★ Siphon and trap are exact De Morgan duals; one closure operator settles an existential over every subset of places in a single shot.
- `siphon-trap-de-morgan-duals`, `siphon-trap-are-exact-de-morgan-duals` — the same loop with preset↔postset swapped, at `literature.rs` Alg 6.19, lines 349–371
- `siphon-trap-are-closure-operators`, `maximal-siphon-algorithm-shrinks`, `maximal-siphon-closure-terminates` — `while some place is bad, remove it`; terminates because the set only shrinks; yields the unique maximal siphon (or trap) in any set
- `commoner-hack-cleverest-move`, `one-closure-answers-existential-over-subsets`, `one-closure-settles-all-subsets` — "the siphon contains a marked trap" iff "the maximal trap in the siphon is marked," replacing the naive enumerate-all-traps route
- `siphons-traps-govern-starvation-trapping`, `chc-needs-marked-trap-in-every-siphon`, `commoner-hack-liveness-iff`, `chc-sufficient-for-general-nets`
- the closure result carries proof or counterexample: `chc-result-carries-witness-or-counterexample`, `chc-positive-and-negative-evidence` — every siphon with its marking witness on success, the exact starving siphon (a deadlock certificate) on failure

### → III.3 — The single missing keystone is the cluster quotient: one partition unlocks both halves of the island.
*And it reaches forward — the same partition is the natural domain of composition, the lattice the capstone minimizes over.*
- `everything-is-order-completion-closure-quotient`, `four-primitives-enumerated`, `code-is-already-four-abstractions-longhand`, `future-is-making-four-constructions-abstract` — the meta-distillation that names the cairn's own operations: order, completion, closure, quotient
- `cluster-quotient-is-the-keystone`, `cluster-is-equivalence-of-place-transition-coupling`, `union-find-computes-clusters-cheaply`, `cluster-quotient-unlocks-both-halves`, `quotient-gives-cluster-count-c`, `quotient-gives-s-and-t-components`
- the Rank Theorem awaiting it: `rank-vs-clusters-decides-boundedness`, `cluster-appears-once-in-crate` — rank = c−1, c = number of clusters; `no-cluster-construction-no-rank`
- the absences at the waterline: `island-stops-at-waterline`, `s-component-decomposition-charted-and-absent`, `is-covered-by-s-components-hardcoded-false`

---

## ROOT IV — THE EPISTEMIC LAW
### *"Compute the reason, not the answer — a finite witness a skeptic can recheck. Then let any wild guess choose what to try, because only the witness is believed. Say only what you can show; 'I cannot yet decide' is one of the names a claim may wear."*

*— how it knows, and how it chooses.*

The root that explains *why* the other three are run. Roots I–III say what petrivet builds; this one says what a build is *for*: not to enumerate, but to construct a finite, checkable reason — and once the reason is checkable, the search for it may be reckless. Home of the firewall, the founding question answered, and "truth is what survives a quotient."

### → IV.1 — Refuse to enumerate; the *why* of the refusal is the decidability sweet spot.
- `sub-turing-no-zero-test`, `universality-traded-for-analyzability`, `decidability-sweet-spot` — a net cannot test a place for empty; the absence of a zero-test is exactly why the core questions stay decidable
- `inhibitor-arc-cliff` — one inhibitor arc installs a zero-test, restores Turing-completeness, and destroys decidability at one stroke: the cliff edge
- `universality-needs-infinite-nets`, `worst-case-vs-operative-question`, `roadmap-extensions-are-syntactic-sugar`, `extensions-chosen-not-collected` — expressive power is a cost spent deliberately; every gain in power is a loss of knowability

### → IV.2 — A verdict is a witness, not a bit; the boolean is the residue.
- `result-not-boolean`, `proof-carrying-stance`, `proof-carrying-as-deliberate-stance`, `evidence-types-per-subclass`, `results-cite-their-theorems`
- `verdicts-carry-different-witnesses`, `coverability-dual-witness`, `certifying-algorithms-vocabulary`, `certifying-decider-definition` — a scalar token-sum, a Parikh vector, a replayable firing sequence: the proof's shape is dictated by the theorem that earned it; a skeptic checks it trusting nothing
- the calculus not yet built (the witness made re-checkable): `instinct-not-calculus`, `five-properties-five-shapes`, `result-pattern-once`, `proofs-inert-no-verify`, `caller-must-trust-sequence`, `certificate-trait-implied`, `data-already-certificate-shaped`, `firing-sequence-checks-by-replay`, `marking-equation-proof-checks-by-recompute`, `siphon-trap-checks-by-closure`, `no-new-theory-needed`, `certificates-auditable-offline`, `certificate-verdict-coda-abstraction`
- the trusted base is the checker, not the decider: `checker-not-decider-is-trusted-base`, `shrink-then-verify-the-checker`

### → IV.3 — Say only what you can show: a claim is unshakable when it wears its true name.
- `claim-honesty-as-method`, `precision-makes-claims-unshakable`, `five-grades-of-claim` — SEE / READ-INTO / IMAGINE / BELIEVE / PROMISE; precision, not emphasis, removes every handhold for doubt
- `verdict-honesty-equivalence-vs-sufficiency` — `commoner-fc-live-iff-marked-trap-in-every-siphon` (iff on the island) vs `asymmetric-choice-marked-trap-sufficient-only` (sufficient, not equivalent — and it says which)
- honest abstention: `inconclusive-as-honest-verdict`, `ackermannian-honesty-on-unbounded`, `unbounded-nets-return-inconclusive` — the frontier is one line behind the omega check
- the hazard where a name is faked: `liveness-l0-soundness-hazard`, `verdict-and-not-verdict-same-face`, `liveness-hazard-type-enforced` — "we don't know" must be made un-confusable with "provably dead"
- confessing the gap: `not-yet-exploited-honesty`, `convention-reconciliation`, `three-sources-one-canon` — record the rejected alternative (the Primer's |P|×|T| over Murata's transpose)

### → IV.4 — The certificate is the firewall: the guess chooses what to try, the proof decides what is believed. *(the founding question, answered: both, with a wall)*
- `proof-or-guess-both-with-a-wall`, `certificate-firewall`, `certificate-is-the-firewall`, `guess-chooses-what-to-try`, `proof-decides-what-is-believed`, `learning-and-soundness-never-touch`, `heuristic-inside-verifier-without-anxiety`
- the theorem (policy-independence): `soundness-theorem`, `soundness-proof-mechanism`, `learner-outside-trusted-base`, `order-cannot-affect-answer`, `learning-confined-to-performance`, `flat-guarantee-rising-capability`, `wrong-guess-only-wastes-time`, `distribution-shift-benign`, `theorem-says-safe-not-helpful`
- every leaf is a verified proof (the asymmetry Go lacks): `every-leaf-can-be-checked`, `go-has-no-verified-leaf`, `muzero-inverse-of-petrivet`, `strictly-stronger-than-muzero`, `real-rollouts-verified-rewards`
- where the wall has holes (the real risk is trusted-but-wrong, not the ML): `firewall-strength-is-certifying-fraction`, `trusted-decider-is-trusted-base`, `two-some-false-stubs`, `two-some-false-hazards`, `real-soundness-risk-is-not-ml`, `prerequisite-fix-some-false-stubs`, `certificate-and-portfolio-one-project`

### →→ IV.4.a — The portfolio is the firewall made operational: the cascade as data, the order as a learnable parameter; the policy learns the effective theory of hardness — what *can* be coarse-grained.
- the cascade lifted into data: `routing-is-algorithm-selection`, `runtime-prediction-then-select`, `selection-as-sequential-decision-process`, `decider-set-already-exists`, `decider-table-cost-polarity-cert`, `soundness-domain-examples`, `cascade-hardcoded`, `no-decider-trait`, `decider-metadata-in-doccomments`, `lift-cascade-into-data`, `default-schedule-reproduces-behavior`, `one-trait-one-telemetry-hook-away`, `anytime-corollary-parallel-racing`, `learned-parallel-portfolio-natural-completion`, `ml-policy-schedules`, `learned-policy-predicts-which-fires`, `decider-learned-schedule-coda-abstraction`
- the embryo already in the code: `policy-already-exists-as-one-if`, `technique-tags-are-telemetry-embryo`, `only-learned-part-is-policy-next`, `little-new-machinery`
- the lineage and the unoccupied synthesis: `proposer-checker-lineage`, `fastforward-domain-precedent`, `generalize-fastforward`, `three-families-of-learners`, `petrivet-synthesis-of-one-and-two`, `contest-names-structural-reduction`, `composition-is-the-synthesis`
- the AlphaGo correspondence and the effective theory: `alphago-move-not-smaller`, `learn-distribution-collapse-difficulty`, `policy-value-triad`, `triad-exact-mapping`, `policy-is-effective-theory-of-hardness`, `policy-discovers-effective-theory`, `hardness-on-distribution-different-object`, `reachability-graph-is-micro-substrate`, `israeli-goldenfeld-coarse-grain`, `commuting-diagram-criterion`, `autonomous-means-self-predicting`, `causal-states-minimal-sufficient`, `information-bottleneck-lagrangian`
- which macro-variables to coarse-grain onto: `feature-design-doctrine`, `prefer-aggregate-descriptors`, `structural-features-are-autonomous-macrovariables`, `mutual-information-feature-test`, `feature-sufficiency-empirical`, `bach-borrowable-move`

### →→ IV.4.b — The rungs: rising ambition, flat soundness. Each rung dominates the last; the certificate keeps correctness constant.
- the ladder itself: `ladder-rung-0`, `ladder-rung-1`, `ladder-rung-2`, `ladder-rung-3`, `implementation-branches-vs-ladder-rungs`
- Rung 1 — static ranker: `rung1-makes-prior-a-learned-function`, `rung1-is-one-shot-static`, `failure-confined-to-cheap-dimension`, `objective-is-cost-sensitive-regret`, `sbs-floor-vbs-ceiling`, `tree-models-fit-tabular-features`, `ranker-is-an-instrument`, `ranker-only-reorders`, `performance-not-provably-monotone`, `run-free-shortcuts-first`, `cap-first-pick-with-deadline`, `blend-with-hand-order-prior`, `split-by-family-not-instance`, `cold-start-covered-by-prior`, `model-stays-outside-core`
- Rung 2 — adaptive controller: `rung2-closes-the-loop`, `new-verb-is-preempt`, `rung2-action-space`, `pandoras-box-index-baseline`, `three-formulations-increasing-ambition`, `reach-for-simplest-formulation`, `anytime-parallel-racing`, `race-only-under-uncertainty`, `offline-off-policy-from-logs`, `offline-rl-extrapolation-error`, `new-actions-harmless-to-soundness`, `rung2-dominates-rung1`, `rung2-monotone-only-if-bootstrapped`, `reward-design-is-performance-knob`, `certificate-seals-correctness-off`, `cancellation-is-hard-prerequisite`, `cancellation-reaches-toward-core-minimally`, `budget-cancellation-absent`, `deadlines-start-coarse`
- self-labeling — the certificate is the label: `certificate-is-the-training-label`, `self-play-against-cost`, `oracle-is-cross-check-not-training-dep`, `solving-corpus-yields-proof-trees`, `features-present-as-data-absent-as-vector`, `nupn-unit-tree-is-missing-feature`
- Rung 3 — certified reductions, the apparatus eaten as moves: `rung3-second-verb-transform`, `moves-simplify-a-proof-obligation`, `reduction-trait-three-methods`, `lift-is-the-keystone`, `reductions-are-scaffolding`, `reduction-library-is-apparatus-as-actions`, `rung3-eats-the-whole-apparatus`, `each-reduction-must-be-certifying`, `buggy-lift-cannot-break-soundness`, `muzero-frontier-bright-line`, `and-or-proof-tree-search`, `theorem-proving-correspondence`, `value-net-learns-cost-to-proof`, `curriculum-emerges`, `lift-functions-are-the-real-work`, `search-blowup-value-net-is-hope`, `rung3-is-the-spire-raised-last`, `prove-like-a-mathematician`

### → IV.5 — Truth is what survives a quotient: keep only what is invariant under the don't-cares.
*The method beneath the law — firing order, the prover's identity, the micro-detail are precisely the don't-cares a reason must be invariant under.*
- firing ORDER quotiented (the Parikh image): `state-equation-is-necessary-not-sufficient` — necessity-but-not-sufficiency is exactly the information the quotient loses; `marking-equation-becomes-refinement-loop`

### →→ IV.5.a — The machine origin is a don't-care: costs live in a torsor, fitness in the quotient.
- `measure-differences-record-the-bundle`, `absolute-timings-have-no-origin`, `schedule-is-a-choice-of-origin`, `log-costs-form-a-torsor`, `fitness-lives-in-quotient`, `torsor-glossary-group-forgot-identity`, `section-is-a-schedule`, `bundle-of-torsors-precise-not-loose`, `persist-raw-fibers-tagged-with-context`, `never-bake-schedule-into-measurement`, `fitness-tests-are-differential-assertions`, `committing-baseline-commits-a-section`, `no-regression-on-log-ratios`, `raw-cost-not-cross-comparable`, `prefer-invariant-counters`, `regret-is-torsor-quotient-quantity`, `ranker-needs-differential-not-absolute`, `ranker-learns-section-of-bundle`, `rung2-adaptive-section`, `torsor-survives-into-rung3`, `torsor-keeps-measurement-honest`, `timing-noise-mitigated-structurally`

### →→ IV.5.b — Observe, never act: the harness measures fitness and the dependency arrow points only one way.
- `observability-is-measurement-domain`, `harness-is-self-test-and-training-set`, `harness-describes-fitness-does-not-act`, `harness-observes-never-certifies-never-schedules`, `dependency-arrow-points-only-to-core`, `pure-downstream-observer`, `harness-hands-off-dataset-not-model`, `scope-creep-guarded-by-arrow-lint`, `seam-is-only-point-of-contact`, `two-record-types`, `phase1-no-core-change`, `phase2-minimal-additive-seam`, `each-phase-independently-shippable`, `loading-excluded-from-cost`, `timing-is-greenfield`, `harness-no-ops-on-missing-corpus`, `soundness-sentinel-runs-underneath`, `soundness-sentinel-catches-stubs`, `oracle-is-truth-signal-until-check-exists`, `phi-features-already-cheap-accessors`, `phi-recorded-as-raw-named-fields`

### → IV.6 — Name the room before the wall: the blueprint is committed to disk ahead of the stone.
*The meta-discipline — vision committed to disk before construction; the architecture making promises to itself.*
- `literature-tells-what-petrivet-is`, `literature-as-load-bearing-index`, `citation-index-binds-theorem-to-function`, `deeplinks-to-nonexistent-module`, `blueprint-drawn-ahead-of-stone`, `blueprint-ahead-of-the-stone`, `gap-is-self-authored-map`, `unbuilt-names-are-promise-to-self`, `architecture-promises-to-itself`, `seams-cut-with-next-generality`, `dream-is-completion-not-invention`, `every-abstraction-has-pointer-today`, `literature-organized-by-source`, `literature-imports-only-under-rustdoc`, `doc-links-as-cfg-doc-imports`
- the motto at the base — the union the whole structure embodies: `motto-theory-application-union`, `for-researchers-and-practitioners`, `readable-api-over-rigorous-impl`

---

## ✦ THE CAPSTONE — Φ_PN
### *"Φ is how much a net refuses to be a product. Cut it every way the structure allows; the smallest gap between the whole and its reassembled parts is the number, and the cut that achieves it is the witness."*

*— composition is the synthesis: the place all four roots meet.*

Not a fifth root — the fixpoint of the whole cairn, the one quantity whose definition requires all four roots at once. A minimum (Root II's order) over a decomposition lattice (Root III's quotient) of a distance between a verdict (Root IV) and a tensor of sub-verdicts (Root I's monoid, factored). It measures the residual the completion cannot complete and the coarse-graining cannot compress. The policy learns what *can* be coarse-grained; Φ measures what *cannot*.

### → The factorization residual: a minimum over a partition lattice.
- `phi-pn-honest-number`, `factorization-as-the-goal`, `phi-pn-definition`, `phi-is-minimum-over-cuts`, `iit-mathematical-kernel`, `phi-zero-means-reducible`, `phi-positive-means-irreducible-with-witness`, `phi-pn-is-number-and-witness`, `phi-pn-real-mathematics`, `delta-measures-shortfall`, `tensor-is-property-specific`, `phi-pn-resists-coarse-graining`, `learn-what-can-be-coarse-grained-phi-measures-rest`

### → Convergence: Φ needs every root.
- `phi-needs-all-five-prior-structures`, `phi-needs-order-engine` (Root II), `phi-needs-structural-decomposition` (Root III), `phi-needs-linear-algebra` (Root II — C completed into kernel and Farkas dual), `phi-needs-certificate-calculus` (Root IV), `phi-needs-decision-portfolio` (Root IV), `phi-is-what-architecture-built-to-compute`, `verdict-calculus-is-whole-behavior`, `decomposition-and-integration-same-lattice`

### → Where the partitions come from, and the cut as a learned action.
- `decomposition-lattice-source`, `nupn-unit-tree-is-candidate-partition`, `nupn-parsed-into-forest`, `fixtures-have-deep-unit-trees`, `nupn-unconsulted-decomposition-oracle`, `no-analysis-reads-nupn`, `unit-safe-flag-unused-invariant`, `no-composition-operator`, `composition-most-purely-potential`, `boundary-spec-is-software-not-nets`, `unittree-phi-coda-abstraction`
- the cut as a good move: `value-net-recognizes-clean-cuts`, `phi-pn-as-live-heuristic-subthread` — the cut is a good move exactly when the property factors over it; the value net is in part learning to predict Φ
- good behavior as clean factorization (Φ = 0): `s-component-coverage-implies-bounded`, `t-component-coverage-implies-consistent`

### → The fence: the residual is mathematics; the leap to minds is metaphor, and the descent stops here.
- `iit-absent-from-repository`, `consciousness-leap-is-the-metaphor`, `full-iit-needs-stochastic-semantics` — an honest "Petri-net Φ" is a number and a witness measuring failure-to-factor; it says nothing about minds

---

## THE FOUR ROOTS, AS A STANDING STONE

> **I — SUBSTRATE.** *A transition touches only its preset and its postset; the net is one matrix, C = Post − Pre. The whole's hardness lives nowhere in any single transition.*
>
> **II — COMPLETION.** *Don't walk the state space — complete it. Omega is infinity made into a type; the kernel is the matrix completed into its conservation laws. A finite witness stands in for the infinite fact.*
>
> **III — STRUCTURAL ISLAND.** *Choose by shape before you compute. One closure answers a question about every subset at once; on the free-choice island the approximations stop approximating.*
>
> **IV — EPISTEMIC LAW.** *Compute the reason, not the answer — a finite witness anyone can recheck. Then let a wild guess choose what to try; only the checked witness is believed. "I cannot yet decide" is a true name, not a failure.*

And the one number they were built to converge on:

> **✦ Φ_PN** — *how much a net refuses to be a product: the smallest gap, over every cut the structure allows, between the whole and its reassembled parts. The gap is the number; the cut is the witness; the residual is what nothing compresses.*

---

The arc reads as one sentence: **locality makes the question hard (I); completion makes it answerable without walking the trajectory (II); the right structural shape makes whole classes of it exact (III); and the certificate makes it safe to reach for those answers with a wild guess and still be right (IV) — converging on the one honest number the architecture was always built to compute (Φ_PN).**