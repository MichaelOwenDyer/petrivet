# Petrivet — Backlog

> **Agents — read [`working-doctrine.md`](working-doctrine.md) first.** It is the contract for *how* we work on petrivet: falsifiability first, soundness before capability, the trust boundary is sacred, let Rust carry the invariants. Every item below assumes it.

## Provenance and scope

This backlog was built in three passes: (1) a reading of the Rust sources only; (2)
enrichment against the project's vision corpus on the `docs/vision` git branch (the
`docs/essays/*`, the boundary spec, the self-measurement harness plan, the
thesis-springboard prompt, the Typst thesis scaffold, the viz roadmap); and (3) a
multi-agent **falsifiability pass** that sharpened and adversarially red-teamed the
core claims, ratified the architecture, optimized the core algebra, introspected the
project's trajectory, and surveyed new certifying deciders, a certificate standard,
and the Φ capstone. Where an item cites a vision file, the path is relative to the
`docs/vision` branch.

### The organizing thesis — a *proposed reframing* (awaiting Michael's ratification)

> **This is a proposal authored *for* Michael, not a decision he has made.** It reframes the project
> around one possible thesis contribution (a proof-carrying, structural-coverage spine). Michael owns
> the thesis framing — accept, refine, or reject it. The library-facing work this backlog schedules
> (structural deciders that decide without state-space exploration, exact-arithmetic correctness, the
> per-class dispatch registry, the bug fixes) stands on its own regardless of which framing he chooses.

- **The certificate-and-checker is the stone.** The signature technical contribution
  is an interoperable, machine-checkable certificate for each property verdict,
  re-validated by a *small external checker that is the entire trusted base*. (Epic C.)
- **The falsifiable headline claim is empirical.** *On the real MCC P/T corpus, a
  polynomial structural certificate decides a large, characterizable fraction of
  queries without state-space exploration; where it abstains, it abstains honestly.*
  (Epic G, measured as `f_struct`.)
- **The soundness firewall is the enabling property, not the headline.** "Soundness is
  independent of the selection policy" is, as a theorem, a one-line corollary of
  certifying-algorithms (McConnell–Mehlhorn) composed with algorithm-selection
  (Rice/SATzilla); its non-trivial content is a *precondition* the code must first
  discharge (A2/A6). The contribution is the firewall *built and measured*, with the
  **certifying fraction `f`** (share of accepted verdicts carrying a checked
  certificate) as its figure of merit.
- **Learned selection (Epic D ladder) and the Φ residuals (Epic H) are sequels and
  horizons**, not the spine.

### Recommended decisions (for Michael's call)

- **MCC ranking is *recommended* OUT as a thesis goal.** The contest is the *crucible* (honest,
  protocol-correct abstention with an oracle cross-check) and the *labelling source*
  (the certificate is the training label); it is not a leaderboard to climb. The unit
  of evidence is a characterization plus a construction, not a benchmark ranking.
- **The primary thesis claim is the structural-coverage characterization** (G1),
  carrying the proof-carrying-certificate contribution. The exact wording remains the
  author's to finalize; the framing is *proposed* — Michael's to ratify, refine, or reject.

### Status of the vision documents

The essays and plans are explicitly non-authoritative (`docs/essays/README.md`,
`for-michael.md`): vision artifacts, not project direction, authored to the thesis
author and inviting contradiction. Every `INFERRED`/`RESERVED`/`EXPLORATORY` item
defers to the author's judgement. Several items below deliberately *contradict* the
docs' lean where the falsifiability pass found a stronger defensible position; these
are flagged.

### Project context (load-bearing for priority)

- **The thesis.** *"High Performance Petri Net Model Checker"*, TUM Information Systems
  master's (supervisor Prof. Rinderle-Ma; advisors Dr. Mangler, Prof. Esparza). Fixed
  window: start **2026-05-02**, submission **2026-11-02**
  (`typst/masters/common/metadata.typ`). The prose is template-stage with no committed
  primary claim — the largest open deliverable (Epic G).
- **Ownership lanes.** The core library and structural theory are the author's; the
  downstream measurement/observer lane (Epic D's harness sub-track) is the
  collaborator's, governed by a one-directional dependency. Two items are the author's
  call: the per-decider enumeration seam (D4) and the cancellation seam (D6).
- **Stated first move.** Fix the two `Some(false)` stubs so every fast check carries
  its proof (`docs/essays/for-michael.md`: "Start there."). This is **A2**, the
  near-term north star and the precondition of the firewall.

### How to read an item

**Observation** (Rust and/or corpus, with references) · **Proposed work**
(recommendation) · **Acceptance criteria** (testable) · **Dependencies** ·
**Confidence**.

### Status taxonomy

`OBSERVED` · `INFERRED` · `RESERVED` (deferred to the author) · `EXPLORATORY`.

---

## Epic A — The decision/certificate contract (foundational)

The firewall holds only over the *certifying* fraction of the decider set; a decider
returning a bare verdict is, in effect, part of the trusted base. Epic A makes the
certifying property structural — the precondition for the headline coverage claim to
be trustworthy and for any learned selection to be safe.

### A1 — Consolidate result and evidence types under a single contract
- **Status:** `OBSERVED` / `INFERRED`
- **Observation:** Result/evidence types are five heterogeneous shapes across
  `api/system/*`; `petrivet-wasm` imports a non-existent `petrivet::model::*`; the
  `#[cfg(doc)] use crate::model` at `literature.rs:409` is broken.
  `docs/essays/charted-cathedral.md` §④ specifies the target: a generic `Verdict<P, N>`
  with an inhabited `Inconclusive`, and a `Certificate` trait. The witnesses are
  already owned, serializable data.
- **Proposed work:** Land the `model` module around three abstractions: (a)
  `Verdict<P, N>` with a type-distinct `Inconclusive`; (b) a `Certificate` trait whose
  sole method re-establishes the property — signature
  `check(&self, net, m0, query) -> bool` (the **`query` argument is required**: a
  firing-sequence witness is meaningless without the target it claims to reach; a
  query-free property passes a unit `Query`); (c) the `Decider` trait deferred to D1.
  Require certificate payloads to be owned and serializable (they are the training
  label and the external-checker input).
- **Acceptance criteria:** all five analyses return `Verdict<P,N>` with "inconclusive"
  type-distinct from "false"; every evidence variant names its theorem; certificates
  serializable; `literature.rs:409` resolves.
- **Dependencies:** none.
- **Confidence:** high.

### A2 — Demote unsound efficient-path constants to abstention · *near-term north star*
- **Status:** `OBSERVED` (confirmed defect)
- **Observation:** `Net::is_covered_by_s_components()` returns `false` unconditionally
  (`api/net/mod.rs:270`), consumed for live FC nets at `boundedness.rs:67`;
  `is_efficiently_live` returns `Some(false) // todo` for marked graphs
  (`liveness.rs:107`). The corpus names both as "trusted-but-wrong" and the real
  soundness risk of the whole construction.
- **Proposed work:** Make a decider that cannot yet certify return `None` (escalate),
  never a fabricated `Some(false)`.
- **Acceptance criteria:** no `is_efficiently_*` returns a verdict not backed by a
  certificate; regressions confirm a live FC net is not reported unbounded and a live
  marked graph not reported non-live; the D3 sentinel guards recurrence.
- **Dependencies:** A1 (preferred); doable as a stopgap now.
- **Confidence:** high. **Gates the firewall, the figure of merit `f`, and all of
  Epic D.**

### A3 — Reconcile `petrivet-wasm` against the consolidated API
- **Status:** `OBSERVED` (split A3a / A3b after measurement, 2026-06-20)
- **Observation:** The wasm crate is written against a *future, post-capability* API, not merely the
  post-`model` one. Measured at M1 (`cargo build -p petrivet-wasm`): **28 compile errors.** They split:
  - **A3a (M1-bounded, A1 dependency — MET):** the `petrivet::model` names wasm imports from the
    A1-landed contract surface (`CoverabilityResult`, `NonCoverabilityProof`, `ReachabilityProof`,
    `ReachabilityResult`, `UnreachabilityProof`, `BoundednessAnalysisMethod`) resolve. This is the
    A1 unblock the M1 gate names.
  - **A3b (capability-gated — NOT a function of A1):** the remaining errors need API that M1 must not
    build. (1) Method-tracking enums `LivenessMethod` / `DeadlockAnalysisMethod` / `LivenessLevel`
    (do not exist — A6/M2). (2) The `ReachabilityProof` witness redesign: variants
    `StronglyConnectedStateMachine`, `StateMachineMarkingEquationRationalSolution`,
    `MarkedGraphMarkingEquationIntegerSolution`, the `.firing_sequence()` accessor, and the
    `StateMachineTokenConservation` field layout (M5/M6 witness shapes). (3) Downstream type
    mismatches that follow (`place_position(i)`/`support()` signatures; `PetriArc`/`BuilderArc` names).
    (4) The `parse_pnml` `labels`/`graphics` unbound locals are themselves capability-gated, not a
    one-line fix: M0 reworked `to_pt_system` to return a bare `PetriNet<Net>` (convert.rs:503), so
    the labels/graphics the struct fields need are no longer in the conversion's return — re-exposing
    them is a core-API change beyond M1's A1 surface.
- **Proposed work:** Treat the crate as the target-surface spec. **A3a** lands with A1 (done). **A3b**
  — the full compile + CI build + `parse_pnml` fix — is armed at M2 (A6 enums) and closed at M5/M6
  (witness shapes + the `to_pt_system` labels/graphics reconciliation).
- **Acceptance criteria:** *A3a* — the A1-landed `model` surface wasm imports resolves. *A3b* —
  `petrivet-wasm` compiles; a CI job builds it; `parse_pnml` binds real labels/graphics.
- **Dependencies:** A3a: A1 (**done**). A3b: A1 + A6/M2 (method enums) + M5/M6 (witness redesign and
  the `to_pt_system` triple). **Confidence:** high (the split is measured, not estimated). *The prior
  "Dependencies: A1, Confidence: high" was the optimistic estimate the M1 build falsified — recorded
  here so the correction is re-derivable (doctrine #1).*

### A4 — Route the boundedness LP proof through the API as a general-net decider
- **Status:** `OBSERVED` (reframed: not a refactor but a polynomial general-net decider)
- **Observation:** `analyze_boundedness` always builds the coverability graph, though
  `find_positive_place_subvariant` already returns a witness `y` with `yᵀN ≤ 0` valid
  for *any* net class including General (`semi_decision.rs:270`); the per-place bound
  `⌊(y·M₀)/y[p]⌋` is in the doc-comment. The certificate is computed and discarded.
- **Proposed work:** Emit `PositivePlaceSubvariant(y)` with derived bounds whenever the
  structural LP decides — a polynomial *prove-bounded* certificate on general nets —
  falling back to the coverability graph only when it does not.
- **Acceptance criteria:** structurally bounded nets (incl. general) return the LP
  certificate without building the coverability graph; bounds re-checkable by C1.
- **Dependencies:** A1; the exact-rational handling of `y` (B0/B1a). **Confidence:** high.

### A5 — Type-enforce "inconclusive" versus "provably dead" liveness
- **Status:** `OBSERVED`
- **Observation:** The unbounded-liveness path returns every transition at L0 — the
  value a provably dead transition gets — so "unknown" and "provably non-live" are
  indistinguishable (`liveness.rs:126`–137).
- **Proposed work:** Make the inconclusive case type-distinct (subsumed by A1's
  `Verdict`).
- **Acceptance criteria:** no path returns L0 as a proxy for "unknown."
- **Dependencies:** A1. **Confidence:** high.

### A6 — Certifying-property audit; surface polarity; define the figure of merit `f`
- **Status:** `INFERRED`
- **Observation:** Many `is_efficiently_*` return bare `Option<bool>`/`bool`, silently
  enlarging the trusted base. The firewall's strength is exactly the certifying
  fraction `f`. Polarity (`ProveYes | ProveNo | Exact`) is latent in the split proof
  enums and the prove-NO-only LP filters.
- **Proposed work:** Enumerate every fast decider; route through A1 or mark explicit
  trusted-base. Surface polarity as a `Decider` field (D1). Define and report `f` (the
  certifying fraction of accepted verdicts) and require the bare-boolean trusted-base
  set to be **non-increasing** in CI.
- **Acceptance criteria:** an inventory test asserts each decider is certifying or
  trust-listed; `f` is computed over the corpus; the trust-list is non-increasing.
- **Dependencies:** A1; feeds C5, D3, G6. **Confidence:** high.

### A7 — Input fidelity at the PNML boundary is a soundness concern
- **Status:** `OBSERVED` (wrong-net-in / confident-answer-out, before selection runs)
- **Observation:** Initial markings above `u32::MAX` are silently clamped
  (`convert.rs:287`) and weighted arcs are silently linearised to weight-1
  (`convert.rs:262`–267), each producing a structurally different net than the input —
  contrary to the converter's own discipline (`convert.rs:37`). Detailed as E4/E7;
  recorded here because they rank, in priority, with A2.
- **Proposed work:** Make both hard `PnmlConversionError`s (or explicit caveats); see
  E4/E7.
- **Acceptance criteria:** import never fabricates a different net; each case rejected
  or flagged, with regression fixtures.
- **Dependencies:** coordinate with E4/E7. **Confidence:** high.

---

## Epic B — Structural certificate generators (the free-choice frontier and beyond)

`literature.rs`'s dangling links to `structural::*`/`Invariants`/`SComponent` are the
project's self-authored blueprint (X2). These generators have a triple role: they are
certificates (Epic C), φ-features for the ranker (D5/X4), and the applicability
witnesses of certified reductions (Epic F). Of the "four constructions" the corpus
names, only **two earn a trait now** — `WellQuasiOrder` (B9) and `Closure` (B7), each
with two implementors; `Quotient`/`Completion` stay concrete (a trait over a
population of one is aesthetics, not generality).

### B0 — Exact-rational matrix core (soundness precondition, not merely substrate)
- **Status:** `INFERRED`
- **Observation:** `IncidenceMatrix` exposes only `new`/`get`; all linear algebra is
  outsourced to floating `microlp`. The Farkas dual on the infeasible path is computed
  and discarded — and a *floating* dual is unsound to emit (B1a). Exact rank/null-space
  are discontinuous and cannot be obtained by rounding `f64`.
- **Proposed work:** A fraction-free (Bareiss) exact-rational core over the dense
  `IncidenceMatrix` providing `rank`, `nullspace_basis` (the P/T-semiflows), and
  exact Farkas-dual extraction. Defer Smith/Hermite normal form to a scoped **B0b**
  used only by the integer-marking refinement and minimal-semiflow extraction; SNF is
  the wrong default cost.
- **Acceptance criteria:** exact rank/null-space for the `class.rs` examples; an exact
  dual extractable from an infeasible marking equation.
- **Dependencies:** none. **Confidence:** high.

### B1 — Invariants and the exact Farkas-dual negative certificate
- **Status:** `INFERRED` / `OBSERVED` (the discarded dual)
- **Observation:** `compute_invariants`/`Invariants` are referenced but unbuilt. The
  rational LP computes a Farkas dual on the infeasible path and returns the
  payload-free `MarkingEquationNoRationalSolution` (`reachability.rs:177`,
  `coverability.rs:124`) — a P-invariant witnessing unreachability, re-checkable by one
  dot product, and the applicability witness for implicit-place removal (F2).
- **Proposed work:** Implement S/T-invariant computation. Split: **B1-certificate** —
  attach a *single separating* invariant to the negative verdict, polynomial, on the
  fast path; the emitted invariant must be **exact-rational and pass
  `y·C = 0 ∧ y·(m'−m₀) ≠ 0` in exact arithmetic before the verdict returns**.
  **B1-coverage** — `is_covered_by_s/t_invariants` via minimal-semiflow generators
  (Colom–Silva), which are worst-case exponential; compute lazily and capped, off the
  fast path.
- **Acceptance criteria:** negative reachability carries a checkable exact S-invariant;
  the fast path stays polynomial; coverage is capped.
- **Dependencies:** B0. **Confidence:** high (certificate), medium (coverage tractability).

### B1a — Audit the floating-point infeasibility *verdict* (silent false-`Unreachable`)
- **Status:** `OBSERVED` (suspected soundness hole; A2-priority)
- **Observation:** The `Unreachable` verdict rests on `microlp` *failing* to find a
  rational solution (`reachability.rs:177`). A spurious floating "infeasible" on a
  genuinely feasible rational system yields a silent **false `Unreachable`** — worse
  than the A2 stubs because it is data-dependent and invisible, and the firewall does
  not protect it (there is no positive object to check on the negative path).
- **Proposed work:** Re-derive the infeasibility verdict over ℚ via B0 (null-space
  membership of `m'−m₀` in `ker(Cᵀ)`), or recheck the rationalised dual in exact
  arithmetic, before returning `Unreachable`/`Uncoverable`.
- **Acceptance criteria:** an ill-conditioned net with a feasible rational solution at a
  degenerate vertex is *not* reported `Unreachable`; every negative verdict is
  exact-certified.
- **Dependencies:** B0. **Confidence:** high that the raw `f64` dual is not emittable;
  medium-high that a false-infeasible verdict is reachable (the minimal test is the
  falsifier — confirm against `microlp` empirically).

### B2 — Cluster quotient via union-find (cheapest keystone)
- **Status:** `INFERRED`
- **Observation:** No cluster construction exists; one union-find over the
  preset/postset `UniqueSortedSlice`s yields the partition near-linearly and unlocks
  *both* the Rank-Theorem count `c` (B5) and S-/T-component extraction (B3).
- **Proposed work:** Compute the cluster partition and count `c`; expose to B3 and B5.
- **Acceptance criteria:** partition and `c` for the `class.rs` FC examples, checked
  against `rank(C) = c − 1` once B0 lands.
- **Dependencies:** none. **Confidence:** high. *Highest leverage-per-line in the
  algebraic core.*

### B3 — S-component decomposition and exact free-choice bounds
- **Status:** `INFERRED`
- **Observation:** `is_covered_by_s_components` is the A2 stub; Hack's FC boundedness
  and exact per-place bounds from S-component token sums are unbuilt.
- **Proposed work:** Implement S-component decomposition; replace the A2 stub with a
  certificate; derive exact bounds for live FC systems.
- **Acceptance criteria:** live FC nets covered by S-components decide in polynomial
  time with a checkable certificate; the A2 FC-boundedness regression passes via the
  efficient path.
- **Dependencies:** A1, A2, B1, B2. **Confidence:** medium.

### B4 — Exact T-net bounds and circuit-based T-net liveness
- **Status:** `INFERRED`
- **Observation:** T-net place bounds (min circuit token count) and liveness (every
  circuit marked) are realised only via the reachability-graph fallback; the marked-graph
  efficient path is the A2 stub. `efficient_place_boundedness` already enumerates
  circuits (`boundedness.rs:121`).
- **Proposed work:** Circuit-based liveness and exact-bound certificates for T-nets
  without state-space exploration; resolve the A2 marked-graph case.
- **Acceptance criteria:** the documented T-net examples decide structurally with
  checkable certificates; circuit enumeration restricted to circuits containing the
  place of interest.
- **Dependencies:** A1, A2. **Confidence:** medium-high.

### B5 — Rank/cluster theorem for simultaneous liveness and boundedness
- **Status:** `INFERRED`
- **Observation:** `class.rs:293`–298 states the FC simultaneous L+B characterisation
  (positive S/T-invariants; `rank C = c − 1`; every proper siphon marked).
- **Proposed work:** Combine B0 (rank), B2 (`c`), B1 (invariants) into a combined
  certificate for FC systems.
- **Acceptance criteria:** decides the documented FC examples; certificate checkable.
- **Dependencies:** A1, B0, B1, B2. **Confidence:** medium.

### B6 — Free-choice reachability via marking equation plus unmarked-trap check
- **Status:** `OBSERVED`
- **Observation:** `LiveBoundedFreeChoiceMarkingEquationWithTrapCheck` is declared but
  never produced; the dispatch arm is commented out (`reachability.rs:103`).
- **Proposed work:** Implement the polynomial FC reachability decision (integer
  marking-equation solution + unmarked-trap check on the unfired-transition subnet).
- **Acceptance criteria:** live and bounded FC systems decide reachability structurally
  with the trap-check certificate.
- **Dependencies:** A1, B3, B7. **Confidence:** medium.

### B7 — Consolidate the siphon/trap engines (the `Closure` family)
- **Status:** `OBSERVED`
- **Observation:** A backtracking `minimal_siphons` (used by CHC) sits beside three
  `#[expect(unused)]` engines. Siphon and trap are De Morgan-dual closures (two
  implementors today). The real scaling risk is that **minimal-siphon enumeration is
  worst-case exponential** and sits on the CHC liveness path — not the choice among
  engines.
- **Proposed work:** Introduce a `Closure` trait (`maximal_siphon_in`/`maximal_trap_in`
  as instances; duality = an incidence-direction flip) to dedup the two shrinking loops.
  Decide enumeration policy: cap, or restrict CHC to the structural subclass where the
  minimal-siphon count is bounded. Benchmark; retire the unused engines.
- **Acceptance criteria:** no unexplained `#[expect(unused)]` engines; the exponential
  enumeration is capped or scoped with a logged bound; CHC behaviour unchanged.
- **Dependencies:** none. **Confidence:** high.

### B8 — Exploit the NUPN unit tree and `unit_safe` invariant
- **Status:** `OBSERVED`
- **Observation:** NUPN is parsed into a unit forest with a `unit_safe`
  one-token-per-unit invariant, then flattened and ignored (`models.rs` reads only
  `<size places>`). A free, checkable safety/boundedness certificate, and a candidate
  partition for F2 and H2.
- **Proposed work:** Surface `unit_safe` as a certificate under A1/C1; preserve the unit
  forest through conversion.
- **Acceptance criteria:** `unit_safe` inputs emit a checked certificate; the forest
  survives conversion.
- **Dependencies:** A1. **Confidence:** medium-high.

### B9 — Lift the state-space order to `WellQuasiOrder`/`Ideal` (WSTS, near-term payoff)
- **Status:** `INFERRED`
- **Observation:** The engine is generic over the fibre (`TokenOps`) but the *order* is
  the hand-rolled `impl<T:Ord> PartialOrd for IdxMarking<T>`; `Omega` is the ideal
  completion of ℕ and the coordinatewise ω-promotion is the *ideal join*, neither
  captured by `TokenOps`. The extra structure a domain must supply is a
  `WellQuasiOrder` (with the test-enforced wqo obligation that licenses termination)
  plus an `Ideal<D>` with a `join`-based acceleration. The near-term payoff is *not* the
  WSTS zoo (that is H1) but converting the blanket `Inconclusive` at the ω-frontier into
  an Abdulla-style **backward-coverability refinement** (E1).
- **Proposed work:** Abstract the order into `WellQuasiOrder` + `Ideal<D>` with `join`;
  generalise `Omega` to `Ideal<ℕ>`; implement the backward-coverability loop for E1.
- **Acceptance criteria:** the existing explorer drives a second trivial WQO domain
  unchanged; E1's `Inconclusive` frontier becomes a refinement carrying a partial
  over-approximation certificate.
- **Dependencies:** A1. Below B0/B1/B2 in priority. **Confidence:** medium-high.

### B10 — Continuous (fluid) relaxation as a class-agnostic prove-NO decider *(new)*
- **Status:** `INFERRED`
- **Observation:** The continuous relaxation (markings in ℝ≥0, fractional firing) is a
  sound over-approximation of discrete reachability/coverability, and continuous
  reachability/coverability/boundedness are **PTIME** (Fraca–Haddad 2015). It is the
  natural apex of the LP→ILP cascade already in `semi_decision.rs`, strictly tighter
  than the state-equation LP, and — uniquely — **class-agnostic**: it can decide
  general, unbounded instances at the ω-frontier where `reachability.rs:197` returns
  `Inconclusive` today.
- **Proposed work:** A `ProveNo` continuous-reachability/coverability decider. Witness:
  the Farkas/place-invariant `y` (B1) for the algebraic refutation, or the maximal
  firing set + blocking empty siphon for the firing-set refutation. Checker: a dot
  product, resp. a polynomial firing-set fixpoint recompute — both original-net.
- **Acceptance criteria:** zero soundness violations against the oracle; a measured
  fraction of currently-abstained `ReachabilityCardinality`/`UpperBounds` instances
  converted to sound verdicts (the falsifier is a corpus table). A unit test: a net
  where the state-equation LP passes but continuous reachability fails must return
  `Unreachable`.
- **Dependencies:** B0, B1; polarity `ProveNo` (A6). Feeds E5(b). **Confidence:**
  medium-high (soundness textbook; PTIME is Fraca–Haddad; corpus payoff is the open
  empirical question).

### B11 — General-net deadlock-*free* siphon certificate *(new)*
- **Status:** `OBSERVED`
- **Observation:** The CHC engine (`siphon_trap.rs:370`) is, for *general* nets, a
  sound *sufficient* condition for deadlock-freedom (the `Ok` arm: every minimal siphon
  contains a marked trap). The dispatch discards this certifying value outside
  free-choice. (The *converse* — an unmarked siphon as a reachable-deadlock witness — is
  **not** sound in general and is explicitly excluded.)
- **Proposed work:** Expose CHC's `Ok` result as a certifying *deadlock-free* verdict
  for general nets. Checker: confirm each exhibited place set is a siphon and contains a
  marked trap — linear per pair; the generator bears the enumeration cost.
- **Acceptance criteria:** a general (non-FC) net with every siphon holding a marked trap
  yields a checked deadlock-free verdict without exploration; a measured fraction of
  general-net `ReachabilityDeadlock` instances converted.
- **Dependencies:** B7 (the `Closure`/enumeration), `deadlock_freedom.rs` dispatch.
  **Confidence:** medium-high.

---

## Epic C — Independent checking and the certificate standard (the signature contribution)

With the inversion ratified, this epic is the project's signature technical
contribution: a small, independent, near-linear-time **checker** that re-validates every
verdict, an interoperable **certificate format**, and a **map of the checkable
frontier** (where compact certificates exist and where complexity theory forbids them).
The trusted base reduces to `{C1 checkers} ∪ {remaining bare-boolean deciders}` (A6/C5).

### C1 — Per-certificate checkers, validating against the *original* net
- **Status:** `INFERRED`
- **Observation:** No `check` exists. The single most load-bearing decision in the whole
  architecture: each checker must re-establish the property against the **original**
  `(net, query)`, assuming nothing about which decider or reduction produced the witness,
  and sharing no code with generators beyond primitive net access. This is what holds the
  trusted base constant under reduction-lifting (F) and makes the format tool-agnostic.
- **Proposed work:** Implement a checker per certificate kind; signature
  `check(&self, net, m0, query) -> bool`.
- **Acceptance criteria:** every certificate variant has a checker; a certificate from a
  *different* generator (or a lifted certificate) for the same `(net, query, verdict)`
  validates identically; dot-product/replay checkers invoke no solver/graph machinery.
- **Dependencies:** A1. **Confidence:** medium-high.

### C2 — Checking as a test invariant
- **Status:** `INFERRED`
- **Proposed work:** Every certificate produced during testing must re-validate (CI gate).
- **Acceptance criteria:** CI fails if any emitted certificate fails its checker.
- **Dependencies:** C1. **Confidence:** high.

### C3 — Surface certificates and their checks in the front-end
- **Status:** `INFERRED`
- **Observation:** `petrivet-viz/ROADMAP.md` prescribes per-transition liveness colour,
  per-place bound badges, and firing-sequence replay; several are `⚙`-gated on prior
  `petrivet`/`petrivet-wasm` changes.
- **Proposed work:** Render firing-sequence replay as the visual `Certificate::check`;
  render the Farkas S-invariant (B1) for unreachability rather than a bare "no."
- **Acceptance criteria:** the front-end shows method, a positive check, and a witness
  animation per analysis.
- **Dependencies:** A3b, C1, B1, the `⚙` WASM methods. **Confidence:** medium-high.

### C4 — In-band certificate checking (verify-on-return); mandatory for lifted certificates
- **Status:** `INFERRED`
- **Observation:** The decision loop should return a verdict only after
  `certificate.check(...)` accepts on the decision path, not merely in tests — the line
  that makes a buggy reduction `lift` cost time, never correctness.
- **Proposed work:** A verified-decision entry point running the C1 checker before
  returning; mandatory on any lifted-certificate path.
- **Acceptance criteria:** no decided verdict returns without a passing check.
- **Dependencies:** C1; F1 for the lifted case. **Confidence:** medium-high.

### C5 — Track and minimise the trusted base; report `f`
- **Status:** `INFERRED`
- **Observation:** The GRAT discipline: an unverified generator, a small (eventually
  formally verified) checker. The trusted base is `{C1 checkers} ∪ {bare-boolean
  deciders from A6}`.
- **Proposed work:** Measure the trusted-base surface in CI and assert it
  **non-increasing**; report the certifying fraction `f`; record the formal-verification
  aspiration as non-binding.
- **Acceptance criteria:** trusted base enumerated, its size reported and non-increasing;
  `f` reported over the corpus.
- **Dependencies:** A6, C1. **Confidence:** medium-high.

### C6 — An interoperable, machine-checkable certificate format *(new)*
- **Status:** `INFERRED`
- **Observation:** Petri-net model checking lacks the DRAT/LRAT/GRAT/VeriPB analogue the
  SAT/ILP world has. Petrivet's proof objects are already owned, serializable, borrow-free
  — the format is one `serde` derive and one net-anchoring convention away. Anchor
  certificates to PNML place/transition *names* (not internal indices) to make them
  tool-agnostic.
- **Proposed work:** Define a canonical serialization
  `Cert = (net_id, query, polarity, witness, theorem_id)` over PNML-referenced names; the
  C1 checkers consume it.
- **Acceptance criteria:** a certificate round-trips and checks; a hand-authored
  certificate for a verdict produced by a different procedure (or, in principle, another
  tool) checks identically against the original net.
- **Dependencies:** A1, C1. **Confidence:** high intra-tool; cross-tool adoption is a
  position, recorded as future work.

### C7 — Map the checkable frontier (per-property × polarity), incl. the hardness boundary
- **Status:** `INFERRED`
- **Observation:** Certificate strength is sharply non-uniform. Near-linear-checkable:
  positive reachability/coverability (firing words), LP-refuted unreachability/
  uncoverability (Farkas P-semiflows — one dot product), structural boundedness (place
  invariants), unboundedness (Karp–Miller self-covering lassos), k-safety, deadlock
  existence. Polynomial: free-choice liveness (an *exhibited* siphon/trap cover —
  checking the cover is linear even though enumerating all siphons is hard). **The wall:**
  general (non-FC) liveness has no known compact checkable certificate, and
  *integer-only* infeasibility has no single Farkas dual (its honest witness is a
  cutting-plane / VeriPB-shaped derivation, worst-case super-polynomial). Two emitted
  witnesses are currently incomplete: the coverability ω-witness lacks the **pumping
  cycle** the checker needs, and `LivenessMethod::MarkedGraph{}` carries no circuit-token
  data.
- **Proposed work:** Produce the per-property × polarity table as a thesis deliverable;
  enrich the ω-witness with its lasso and the marked-graph witness with circuit tokens;
  state the hardness boundary as a claim (general liveness; ILP-infeasibility →
  cutting-plane).
- **Acceptance criteria:** the frontier table is reported with checker complexities; the
  boundary is stated with its complexity-theoretic justification; a checkable liveness
  certificate for one class strictly beyond free-choice is the standout open target.
- **Dependencies:** C1, C6; relates to G6. **Confidence:** high on the map; the
  beyond-FC liveness certificate is the genuinely hard, high-novelty item.

---

## Epic D — Selection as an instrumented policy (the SATzilla sequel, not the headline)

Selection is the *sequel* to the certificate-and-checker, not the spine. The honest
lineage is **SATzilla/Rice algorithm selection** — the AlphaGo/MuZero framing is dropped
(petrivet *has* checkable leaves, which makes the problem easier and unlike AlphaGo). The
effective-theory / cellular-automata material in the essays is labelled speculation, never
a design justification. The harness sub-track (D1–D4) is the collaborator's downstream
lane; the ladder (D5–D8) is gated behind C2 so a mis-selection costs performance, never
soundness.

### D1 — Decider registry with applicability guards, polarity, cost — *sequence before Epic B*
- **Status:** `INFERRED`
- **Observation:** Dispatch is hardcoded `match self.class()`. B's generators are most
  cleanly *born* as `Decider` impls and `Reduction` witnesses, so the registry refactor
  should precede them.
- **Proposed work:** A registry of `Decider`s behind A1, each with an applicability guard,
  polarity, cost class, certificate kind, and a `Policy::next` seam; the default policy
  reproduces today's cascade exactly. Land it **after Phase 2 (C1/C2), before B1**.
- **Acceptance criteria:** adding/reordering a decider needs no change to public analysis
  methods; default behaviour preserved.
- **Dependencies:** A1. **Confidence:** medium.

### D2 — `petrivet-observe`: measurement crate, JSONL schema, φ extractor (Phase 0)
- **Status:** `INFERRED`
- **Observation:** `docs/self-measurement-harness-plan.md` specifies a downstream-only
  crate with two record types — `Observation` and the differential `FitnessComparison`
  (rankings, log-ratios, Pareto fronts) — under a torsor discipline: absolute timings are
  non-comparable across runs, so only the differential object crosses runs.
- **Proposed work:** Create `petrivet-observe` depending on `petrivet`, both schemas
  (versioned, run-context-tagged), and a φ extractor over cheap accessors. No core change.
- **Acceptance criteria:** crate builds; both record types round-trip; a CI `cargo tree`
  lint asserts `petrivet` never depends on it; φ emits the documented fields.
- **Dependencies:** A1. **Confidence:** high.

### D3 — Corpus driver, soundness sentinel, differential fitness test (Phases 1 & 3)
- **Status:** `OBSERVED` / `INFERRED`
- **Observation:** `mcc-tests/{runner,oracle}.rs` are the self-labelling substrate. The
  plan adds a driver folding `Observation` into `FitnessComparison`, an always-on
  **soundness sentinel** (every `Decided` row with a known oracle must agree — the live
  regression for A2), and a baseline-relative no-regression test.
- **Proposed work:** Implement the Phase-1 driver over the public surface, the
  cross-check, the fold, and (Phase 3) committed-baseline differential reporting.
- **Acceptance criteria:** a reproducible dataset + snapshot without altering verdicts;
  the sentinel fails on a known trusted-but-wrong stub; the differential test is invariant
  to CI machine speed.
- **Dependencies:** D2; A1; guards A2; strengthens with C1. **Confidence:** medium-high.

### D4 — Per-decider fibres via a public decider enumeration + coarse deadlines (Phase 2)
- **Status:** `RESERVED` (additive core seam — the author's call)
- **Observation:** Full cost vectors need each partial decider callable in isolation; the
  plan proposes a public, read-only, side-effect-free enumeration.
- **Proposed work (conditional):** Expose the seam; run each admissible decider under a
  coarse per-decider timeout; assemble cost vectors.
- **Acceptance criteria:** if pursued, full per-decider fibres emitted; the seam adds no
  scheduling logic; degrades to Phase-1 without it.
- **Dependencies:** D2, D3; the seam decision. **Confidence:** n/a (decision) / medium.

### D5 — Rung 1: empirical hardness ranker — *gated on a measured SBS→VBS gap*
- **Status:** `EXPLORATORY`
- **Observation:** A SATzilla-lineage runtime-regression-then-rank model over φ(N),
  trained self-supervised on the harness JSONL, optimising cost-sensitive **regret**
  (not accuracy), gradient-boosted trees (no GPU). The honesty ledger: a bad ranker can
  be slower than the Rung-0 cascade, and the SBS→VBS gap on a ~6-arm portfolio may be
  within noise — so **the ranker is justified only if the measured gap exceeds a
  threshold**; otherwise the hand-ordered cascade is the honest answer and the ML is dead
  weight.
- **Proposed work:** First *measure* the SBS→VBS gap on the corpus (D3/D4). If
  non-trivial, train the ranker; schedule in predicted-cost order with fall-through to the
  exhaustive backstop; mitigations: near-free shortcuts first, a first-pick budget cap, a
  Rung-0 prior. Strictly downstream, feature-gated.
- **Acceptance criteria:** the gap is reported first; the ranker (if built) is gated behind
  C2; evaluated by gap-closed on **held-out families**; the sentinel stays green; absent
  the model, the cascade is Rung-0.
- **Dependencies:** C1/C2, D1, D3/D4, A2; the evaluation protocol (G4). **Confidence:**
  medium.

### D6 — Core seam: cooperative cancellation, deadlines, bounded LP/ILP (Rung 2 prerequisite)
- **Status:** `OBSERVED` (gap) / the author's call
- **Observation:** No cooperative cancellation exists ("the only early stops are the ω
  short-circuit and process-level `catch_unwind`"). Rung 2 cannot exist without it.
- **Proposed work:** A cancellation token threaded into the exploration loop, a per-call
  budget, iteration/time limits on `microlp`; a cancelled decider returns inconclusive.
- **Acceptance criteria:** a running exploration/solve can be cancelled and reclaim its
  time; cancellation changes only *when* a decider stops, never *what* is accepted.
- **Dependencies:** A1. Gates D7. **Confidence:** medium-high.

### D7 — Rung 2: adaptive sequential policy (preempt / continue / abandon / race)
- **Status:** `EXPLORATORY`
- **Observation:** An adaptive controller over `{start, continue, abandon, allocate}` —
  a learning-free Weitzman/Pandora's-box index rule, then a contextual bandit, then
  conservative offline RL — off-policy from harness logs, anytime racing of cheap
  deciders.
- **Proposed work:** Start with the index policy over Rung-1 predictions; escalate only
  where preemption timing depends on rich state.
- **Acceptance criteria:** preemption/racing never alter a verdict; anytime; cost
  improvement over Rung 1 on held-out families.
- **Dependencies:** D5, D6, C2. **Confidence:** low-to-medium.

### D8 — Rung 3: planner over certified reductions
- **Status:** `EXPLORATORY`
- **Observation:** A `transform` action over a shrinking residual; an AND/OR proof-tree
  search; the reduction *apparatus* is Epic F, the *planner* is D8.
- **Proposed work (conditional):** Learn a policy/value over residual nets; every lifted
  certificate checked against the original net.
- **Acceptance criteria:** a wrong reduction/`lift` causes backtracking, never an unsound
  verdict.
- **Dependencies:** Epic F, C1/C2/C4, D7. **Confidence:** low.

---

## Epic E — Scope boundaries and honest degradation

The authoritative scope content is the boundary spec plus
`docs/thesis-springboard-prompt.md` §D. With MCC ranking committed OUT, the abstaining
examinations are recorded as boundaries, not coverage debt.

### E1 — Honest degradation for general nets (→ backward coverability)
- **Status:** `OBSERVED`
- **Observation:** General reachability returns `Inconclusive` on ω; coverability has a
  "todo: backwards coverability" note. The LP/ILP filters (now the Farkas certificate, B1)
  produce infeasibility evidence.
- **Proposed work:** Degrade to `Inconclusive` with whatever partial certificate exists;
  via B9, convert the blanket frontier into an Abdulla-style backward-coverability
  refinement carrying an over-approximation witness.
- **Acceptance criteria:** no general-net path fails silently; partial certificates
  surfaced.
- **Dependencies:** A1; B9 for the refinement. **Confidence:** medium-high.

### E2 — General-purpose fallback — *not pursued (ranking OUT)*
- **Status:** `OBSERVED` (decision settled)
- **Observation:** The reading that motivated a general fallback was the MCC ranking goal,
  now committed OUT. The crucible/labelling readings need no such fallback; honest
  abstention is the correct move.
- **Proposed work:** None. Recorded as decided.
- **Dependencies:** none. **Confidence:** n/a.

### E3 — Temporal logic (CTL/LTL) is out of scope
- **Status:** `OBSERVED`
- **Observation:** No formula parser or temporal engine; the MCC binary `DoNotCompete`s
  for all four CTL/LTL examinations (`main.rs:98`–101). The corpus names temporal/symbolic/
  unfolding methods as a "conspicuously absent" reference class. The structural-certificate
  thesis has no CTL/LTL analogue.
- **Proposed work:** Record as out of current scope; re-open only as an explicit new
  direction.
- **Dependencies:** none. **Confidence:** high.

### E4 — `u32` marking ceiling, including the silent import-saturation defect
- **Status:** `OBSERVED`
- **Observation:** Token counts are `u32`; the runner opts out of `unfinite`/
  `large_marking`. PNML import silently clamps over-`u32::MAX` markings (`convert.rs:287`),
  producing a wrong net (see A7).
- **Proposed work:** Document the ceiling; make over-`u32::MAX` markings a hard
  `PnmlConversionError`.
- **Acceptance criteria:** ceiling documented; overflow errors; a `>u32::MAX` fixture is
  rejected.
- **Dependencies:** A7. **Confidence:** high.

### E5 — MCC examination coverage
- **Status:** `OBSERVED`
- **Observation:** Implemented (6): `StateSpace, ReachabilityDeadlock, OneSafe,
  QuasiLiveness, StableMarking, Liveness`. The abstaining 7 split into (a) decided-OUT —
  CTL/LTL ×4 (E3); (b) `UpperBounds`, `ReachabilityFireability`, `ReachabilityCardinality`
  — reachable within the structural + state-space engine (and via B10's continuous
  prove-NO) but, with ranking OUT, **future work**, not a thesis goal.
- **Proposed work:** Record the (a)/(b) split; (b) is opportunistic future coverage tied to
  B10, not committed.
- **Dependencies:** E3; B10. **Confidence:** high.

### E6 — Connected-nets-only precondition
- **Status:** `OBSERVED`
- **Observation:** `classify` returns `None` for weakly-disconnected nets ("this library
  only supports connected nets", `class.rs:498`,504–524) — a *weak*-connectivity gate
  distinct from the *strong* connectivity several shortcuts require; the corpus flags the
  possible mismatch as unverified.
- **Proposed work:** Document the precondition; audit each shortcut's weak-vs-strong
  requirement; ensure a disconnected instance degrades honestly, not to a panic/silent
  `None`.
- **Acceptance criteria:** precondition documented; requirements recorded; a disconnected
  fixture yields an honest abstention.
- **Dependencies:** A1; E1. **Confidence:** high (boundary), medium (mismatch).

### E7 — Supported PNML subset and dropped arc semantics
- **Status:** `OBSERVED`
- **Observation:** Only `ptnet` converts to runnable; arc weights, inhibitor/read/reset
  arcs, and colored nets are parsed-but-dropped or refused. The silent weighted-arc
  linearisation is the A7 soundness defect.
- **Proposed work:** Document the supported subset; decide whether a non-unit-weight P/T
  arc errors (consistent with the colored-net refusal) or is flagged.
- **Acceptance criteria:** subset documented; weighted-arc P/T inputs rejected or flagged,
  not silently linearised.
- **Dependencies:** A7. **Confidence:** high.

### E8 — Determinism, reproducibility, per-run budget
- **Status:** `INFERRED`
- **Observation:** No recorded budget/determinism contract; the engine appears
  deterministic (no RNG, fixed BFS). The harness plan: absolute timings are non-canonical.
- **Proposed work:** Record the MCC budget; state the determinism contract; express
  performance claims as differential ratios.
- **Acceptance criteria:** determinism stated; budget recorded; claims differential.
- **Dependencies:** D2; G5. **Confidence:** medium.

---

## Epic F — Certified reductions (Rung 3 apparatus)

A property-preserving transformation carrying a checkable applicability witness and a
`lift` that maps a residual certificate back to the original net, checked by the
*unchanged* C1 checkers. Because a wrong `lift` is caught against the original net, the
whole reduction library lives outside the trusted base — but the *robustness* property
holds cleanly for **existential** witnesses (firing sequences) and must be *proven per
certificate kind* for **compositional/invariant** lifts (a buggy interface correction
could pass a too-weak check). Restrict trusted lifts to existential witnesses until the
compositional checker-completeness obligation is discharged.

### F0 — Correct the MCC `STRUCTURAL_REDUCTION` mis-tag
- **Status:** `OBSERVED`
- **Observation:** `Technique::StructuralReduction` is printed on the CHC liveness
  shortcut (`main.rs:31`) — a structural *decision*, not a reduction; no reduction code
  exists.
- **Proposed work:** Stop tagging it on the CHC path, or reserve it until a real reduction
  fires.
- **Acceptance criteria:** emitted only when a certified reduction fired.
- **Dependencies:** none (immediate). **Confidence:** high.

### F1 — The `Reduction` trait and the lifting-firewall robustness test
- **Status:** `EXPLORATORY`
- **Observation:** A three-method `Reduction { applicable, apply, lift }`; soundness rests
  on the original-net check.
- **Proposed work:** Introduce the trait; prove the loop with an identity reduction and a
  **deliberately wrong `lift` that the C1 checker rejects** (robustness tested, not
  assumed). Split acceptance by witness polarity (existential trusted now; compositional
  requires a checker-completeness proof).
- **Acceptance criteria:** identity reduction round-trips; a wrong `lift` is caught; the
  trusted-base size (C5) is unchanged.
- **Dependencies:** A1, C1, C4. **Confidence:** medium.

### F2 — First certified reductions, reusing Epic B witnesses
- **Status:** `EXPLORATORY`
- **Observation:** Implicit-place removal's witness is the Farkas dual (B1) — "the
  discarded LP dual finally gets a job"; agglomeration reuses cluster/siphon-trap;
  independent-subnet split reuses NUPN/S-components. Classical (Berthelot;
  Haddad–Pradat-Peyre).
- **Proposed work:** Implement implicit-place removal first, with a correct `lift`
  (re-pad the removed coordinate via the invariant).
- **Acceptance criteria:** each lifted certificate passes the original-net checker; verdict
  equivalence with the non-reduced path on bounded fixtures (X1).
- **Dependencies:** F1; B1. **Confidence:** low-to-medium (the `lift`s are hard theory).

---

## Epic G — Thesis, evaluation, and scientific contribution

The committed primary claim is the **structural-coverage characterization**. The
experimental rig largely exists (Criterion benches, the `mcc-tests` differential harness,
the Nix VM), so most items are promotions to thesis-grade. The headline number is
`f_struct` — the fraction of *queries decided* by the structural tier, two-denominator,
family-held-out.

### G1 — Commit the falsifiable thesis claim (the coverage characterization)
- **Status:** `RESERVED` (the exact wording is the author's; the framing is *proposed*, his to ratify)
- **Observation:** The thesis scaffold is the stock template (residue at `thesis.typ:55`–148
  to delete). The committed claim: *on the MCC P/T corpus, a polynomial structural certifying
  tier decides a large, characterizable fraction of queries with an independently checkable
  certificate and without state-space exploration; where it abstains, it abstains honestly,
  and the boundary is predictable from cheap structural features.* MCC ranking is OUT; the
  unit of evidence is a characterization + a construction (the certificate framework). The
  learned-selection direction is future work (G9).
- **Proposed work:** Commit the claim with its falsifier (the fraction is small, or the
  structural path is not cheaper, or certificates are not independently checkable), the
  named baseline (the structural-tier ablation), and the unit of evidence; delete the
  residue.
- **Acceptance criteria:** the Introduction/Objectives state the falsifiable claim; the
  abstract is real prose; no residue remains.
- **Dependencies:** none (informs G2–G8). **Confidence:** high on the gating role.

### G2 — Evaluation design: hypotheses, the structural-tier ablation, baselines
- **Status:** `INFERRED`
- **Proposed work:** Hypotheses from G1; the **structural-tier ablation** (run each
  analysis with shortcuts disabled, forcing state-space; measure the delta) as the primary
  internal baseline; the naive-frontier baseline at `core/state_space/mod.rs:111`–127.
- **Acceptance criteria:** every hypothesis maps to a G1 falsifier and a named baseline.
- **Dependencies:** G1. **Confidence:** medium-high.

### G3 — Curate a versioned, provenance-tracked benchmark corpus
- **Status:** `OBSERVED` / `INFERRED`
- **Proposed work:** Define the corpus (15 in-tree PNML fixtures + MCC models + synthetic
  bench families) with provenance, class labels, ground-truth verdicts, and in/out-of-scope
  status (per E4/E5/E7); pin to a manifest.
- **Acceptance criteria:** a manifest is the single source the harness reads.
- **Dependencies:** G2. **Confidence:** high.

### G4 — Promote the harness to the rig; family-held-out, two-denominator protocol
- **Status:** `OBSERVED` / `INFERRED`
- **Proposed work:** Extend the harness (unifying D2's telemetry with timing) to emit, per
  (model, examination): verdict, certificate kind, decider path (structural vs. search),
  cost, abstention reason. Implement **family-held-out** cross-validation, SBS/VBS
  computation, and the **two-denominator** coverage table (in-scope; and all-MCC counting
  out-of-scope as abstain), counted in **queries decided**, not nets-in-class.
- **Acceptance criteria:** one run produces the machine-readable table; correctness
  assertions still pass; results are origin-free and machine-portable.
- **Dependencies:** G2, G3; D2; A1. **Confidence:** medium-high.

### G4a — Structural-coverage floor (runnable now, before Epic B) *(new)*
- **Status:** `INFERRED`
- **Observation:** The headline number has a *floor* obtainable today: which tier fires is
  already readable from the `*Method`/`*Proof` tag, and one `Instant` wrapper suffices. This
  measures the coverage the *current* structural tier achieves, which Epic B then raises.
- **Proposed work:** A minimal pass over the corpus reporting (structural-decided %, abstain
  %, search-decided %) and per-tier time, before any B item lands.
- **Acceptance criteria:** a floor `f_struct` is reported with its two denominators; it is
  the baseline against which B's generators are measured.
- **Dependencies:** D3-lite. **Confidence:** high. *Cheapest decisive experiment; run early.*

### G5 — Reproducibility and artifact package
- **Status:** `INFERRED`
- **Proposed work:** Deterministic runs (seed any selection randomness); a one-command
  reproduction (pinned toolchain, the G3 manifest, the G4 harness); the Nix VM as the
  artifact baseline.
- **Acceptance criteria:** one command reproduces the table (within Criterion CIs); every
  thesis number traces to a harness output.
- **Dependencies:** G4. **Confidence:** medium.

### G6 — Certificate coverage and check-pass rate (= `f`) as a measured result
- **Status:** `INFERRED`
- **Proposed work:** Report, over the corpus, certificate coverage (fraction of verdicts
  certificate-backed) and the independent-check pass rate; the proof-carrying contribution
  is stated with this number.
- **Acceptance criteria:** coverage and a 100%-or-explained pass rate reported; ties to C5's
  `f`.
- **Dependencies:** C1, C2; G4. **Confidence:** medium-high.

### G7 — Related work, positioning, bibliography
- **Status:** `OBSERVED`
- **Observation:** `thesis.bib` has two entries; the Related Work chapter is template.
- **Proposed work:** Position against the structural lineage (Murata; Best–Devillers;
  Commoner–Hack; Desel–Esparza), the certifying-algorithms lineage (McConnell–Mehlhorn–
  Näher), the proof-logging lineage (DRAT/LRAT, VeriPB, GRAT), the continuous-net theory
  (David–Alla; Fraca–Haddad), reduction theory (Berthelot; Haddad–Pradat-Peyre), the
  Kronecker/compositional school (Buchholz; Donatelli) for Epic H, the complexity frontier
  (Leroux–Schmitz; Czerwiński–Orlikowski), and — for the future selection direction —
  SATzilla/Leyton-Brown/Hutter. State the unfolding/symbolic absence as an explicit
  boundary.
- **Acceptance criteria:** every baseline cited; complexity claims backed; the boundary
  stated.
- **Dependencies:** G1. **Confidence:** high.

### G8 — Write-up milestones and figure tracking against the deadline
- **Status:** `OBSERVED`
- **Proposed work:** Chapters as milestones mapped to subsystems; a figure list
  (certifying-portfolio diagram; the two-denominator coverage chart; the ablation speedup;
  the per-property frontier table); freeze the rig (G4) before write-up; the **2026-11-02**
  endpoint fixed.
- **Acceptance criteria:** a chapter schedule with the deadline as endpoint; figures trace
  to G4 outputs.
- **Dependencies:** G2–G6; G1. **Confidence:** high.

### G9 — Selection as a measured result — *out of scope for this thesis (future / 3-year)*
- **Status:** `EXPLORATORY` (recorded OUT for the thesis window)
- **Observation:** Direction 3 is unbuildable to thesis-grade evaluation by November and is
  contingent on a measured SBS→VBS gap (D5). The certifying spine makes it *safe* to defer
  without risk.
- **Proposed work (future):** If pursued later, the experiment is regret-vs-oracle and
  beating the fixed ordering, with every verdict still certificate-checked.
- **Dependencies:** D1, D3/D5, C2; G4. **Confidence:** low-to-medium; deferred.

---

## Epic H — Generality and composition horizons (recorded, not scheduled)

The far end. The Φ capstone, ruthlessly examined, is two-thirds mirage and one-third real:
the single net-level scalar Φ_PN, the boolean-verdict Φ (which is just assume-guarantee
reasoning, a mature field), and the "needs all four roots / built to compute it" necessity
claim are **dissolved**. What survives are two computable, monotone, theorem-backed-zero,
non-vacuous *per-property* factorization residuals, and the deliverable is their
**measurement over the corpus**, not the metaphysics. The IIT fence — a statement about
nets, not minds; IIT absent from the repo — is kept verbatim.

### H1 — Reuse the lifted engine for other WSTS classes (far horizon)
- **Status:** `EXPLORATORY`
- **Observation:** Once B9 abstracts the order, the engine could reuse for lossy channel
  systems, ν-nets, broadcast protocols, BVASS — distinct from B9's in-scope backward-
  coverability payoff.
- **Proposed work:** Instantiate one non-Petri WQO domain (e.g. Higman order on words)
  against the unchanged explorer.
- **Acceptance criteria:** a third order/fibre implementor drives the engine with no engine
  changes.
- **Dependencies:** B9. **Confidence:** low-to-medium.

### H2 — Compositional recombination operators (per property; cite the prior art)
- **Status:** `EXPLORATORY`
- **Observation:** No composition operator exists. The recombination is *not* a free tensor
  — it is the Kronecker-descriptor "direct-sum-plus-interface-correction" the
  compositional/modular school (open nets, Baldan et al.; Kronecker descriptors, Buchholz,
  Donatelli) already studies, where the interface correction is where the difficulty lives.
- **Proposed work:** Define `compose` along shared-interface places for *boundedness* and
  *invariants* specifically (drop the boolean-verdict version); cite the prior art.
- **Acceptance criteria:** part-wise analysis recombines to the whole on documented
  fixtures for boundedness and invariants.
- **Dependencies:** A1, B2, B5, B8, B1, C1. **Confidence:** low.

### H2a — Φ_bound: the boundedness-factorization residual *(the one to schedule first)*
- **Status:** `EXPLORATORY`
- **Observation:** Φ_bound = min over cuts (NUPN units / S-component cover) of
  `Σ_p (b⊗(p) − b(N)_p)`, where `b⊗(p)` is the in-block structural bound (interface
  transitions made free). Monotone (`b⊗ ≥ b`), so well-signed; **provably 0** on live
  bounded FC nets (Hack) and strongly-connected T-nets (circuit theorem); **>0** (indeed ∞)
  on nets bounded only through cross-block synchronization. The minimizing cut is a
  C1-checkable witness.
- **Proposed work:** Define Φ_bound over the NUPN antichain lattice + S-component cover; prove
  monotonicity; verify the zero-set on `class.rs` fixtures; construct a Φ_bound>0 fixture;
  **measure the distribution over a sample of the MCC NUPN corpus, indexed by `NetClass`** —
  the actual deliverable.
- **Acceptance criteria:** monotonicity proof; zero/positive separation verified; minimizing
  cut emitted as a witness; a corpus distribution reported.
- **Dependencies:** B3/B4 (bounds), B8 (NUPN survival), C1. **Confidence:** medium-high.

### H2b — Φ_inv: the invariant rank-defect residual (the novel Rank-Theorem link)
- **Status:** `EXPLORATORY`
- **Observation:** Φ_inv = min over cuts of `dim ker(Cᵀ) − dim(⊕ block-local invariants,
  interface-corrected)` — the count of conservation laws no single block can see. Integer,
  basis-free, non-negative (with the explicit interface correction zeroing interface-coupled
  coordinates). Its link to the Rank Theorem (`rank C = c − 1`, hence to the cluster count
  `c`) is the genuinely novel object, pending a full literature check against Kronecker /
  compositional methods.
- **Proposed work:** Define Φ_inv with the interface correction made explicit; relate to `c`;
  emit the minimizing cut.
- **Acceptance criteria:** integer-valued, basis-free; the Rank-Theorem link established; cut
  emitted.
- **Dependencies:** B0, B1, B2. **Confidence:** medium on well-definedness; medium on novelty
  surviving a literature check.

---

## Cross-cutting quality

### X1 — Property-based testing against state-space ground truth
- **Status:** `INFERRED`
- **Proposed work:** For bounded fixtures, compare every decider's certificate-backed answer
  against exhaustive state-space construction — the oracle for B, C, F.
- **Acceptance criteria:** a property-test suite across generated nets within a size envelope.
- **Dependencies:** A1; complements C2. **Confidence:** high.

### X2 — Resolve the `literature.rs` blueprint by building it
- **Status:** `OBSERVED`
- **Observation:** `literature.rs` deep-links to unbuilt items (`structural::*`, `Invariants`,
  `SComponent`, `crate::model::*`, broken at line 409) — the project's self-authored
  specification, not doc-debt.
- **Proposed work:** Treat it as the canonical spec for A/B; resolve each link by building the
  module; demote to prose only as an interim.
- **Acceptance criteria:** `cargo doc` builds without broken intra-doc warnings.
- **Dependencies:** tracks A1/B. **Confidence:** high.

### X3 — Lint and documentation build clean across the workspace
- **Status:** `OBSERVED`
- **Proposed work:** Maintain clean `clippy`/`cargo doc` across all crates, including the
  non-default members once A3b restores their build.
- **Acceptance criteria:** CI enforces it workspace-wide.
- **Dependencies:** A3b. **Confidence:** high.

### X4 — φ feature artifact and sufficiency test
- **Status:** `INFERRED`
- **Observation:** The feature doctrine prescribes structural macro-features with a measurable
  sufficiency criterion (mutual information with the hardness label). φ richness grows as B
  lands; the NUPN unit-tree shape is the missing coordinate.
- **Proposed work:** Define φ(N) as a versioned artifact in `petrivet-observe`; add the NUPN
  unit-forest parser (observe-side, no core touch); run the sufficiency check.
- **Acceptance criteria:** φ recorded as raw named fields; a sufficiency report produced;
  B-derived coordinates wired in as they land.
- **Dependencies:** D2; B1/B2/B3. **Confidence:** medium-high.

---

## Build plan — gated milestones

The epics above are the *catalog* (what each item is). This section is the *build*: a
dependency-ordered sequence of milestones, **each gated by a provable invariant, not a
representative example** (doctrine #7). A milestone is complete when its gates are *proven* and
the standing invariants stay green. The rationale behind the components is in
[`docs/foundations/foundational-design.md`](docs/foundations/foundational-design.md).

### Status (2026-06-20) — Workflow 1 complete; M3 + B2 are next (Workflow 2)

**Landed and green** on two stacked local branches — `foundation-docs` (docs) and `foundation-code`
(code, stacked on it), both off `f3356bc`, **never pushed**: **M0, M1, M2, M4, M5**, plus the post-M2
**A6 polarity-coherence gate** and the doctest-suite modernization. Green gate: lib **218/0**, doctests
**25/0**, `checker_invariants` 9/0, `firewall_probe` 2/0; `cargo build`/`clippy -p petrivet --all-targets`
clean (no new warnings). The complete next-phase brief is
[`docs/foundations/m3-b2-handoff.md`](docs/foundations/m3-b2-handoff.md).

What landed *beyond* the verbatim gates (build record in `foundational-design.md` §1.2/§F3″/§F3‴/§4.x;
the soundness remediation in [`m2-soundness-remediation.md`](docs/foundations/m2-soundness-remediation.md)):
- **M0** also fixed two engine bugs found closing the green gate: an inert `fire_unchecked` (it discarded
  its token delta) and a scrambled petgraph mirror in `build()` (`HashMap`-order node labels corrupted
  `circuits()`/SCC liveness).
- **M1** the firewall was hardened from a grep-checked convention to a **type invariant** (private
  `Proof`/`Refutation` fields; `Verdict` is `Serialize` but not `Deserialize`); **A3** split into A3a
  (done) / A3b (wasm, deferred).
- **M2** the first cut of two checkers shipped **unsound** (false `Proven` — the cardinal sin), caught by
  an independent oracle-counterexample review and **remediated**: `ParikhVectorCert` now accepts by
  **realization** (replay), sound on any class; `SiphonTrapCoverCert` re-derives the Commoner–Hack
  **universal** and carries a typed `SiphonTrapClaim { Live, DeadlockFree }` (`Live` class-gated to
  free-choice). Three legacy **f64 verdict paths** (`Uncoverable`, the reachability ILP arm, positive
  `is_bounded`) were closed over ℚ. **A6** added the polarity-coherence gate on `accept`.
- **M5** B1a closed; A4's exact checker exists but rewiring the *live* boundedness decider through it is
  the M6 completion.

**Next — Workflow 2:** **M3** (the decider registry, D1) and the **B2 cluster-quotient keystone** (the B2
portion of M6 — *keystone only*: the union-find quotient, the count `c`, the `rank(C) == c−1` invariant;
**not** the S/T-component deciders it unlocks). Standing instruction: decompose the M3+B2 work into
legible commits for Michael at the end.

**Open theory ratifications for Michael** (his domain — do not re-litigate; flag new ones): T1–T8 in
`m2-soundness-remediation.md` §4, and the vending-machine S-net correction in `doctest-modernization.md`.

**Note on the epic catalog (Epics A–H):** the per-item `Status` fields below are the *pre-build* analysis
(`OBSERVED`/`INFERRED`/…). For what has actually *landed*, this milestone status and the
`foundational-design.md` build records are authoritative.

A gate is an invariant plus the form of its proof:

| Tag | Proof form |
|---|---|
| `[PROP]` | property-based test — an invariant quantified over generated inputs (`proptest`) |
| `[ORACLE]` | corpus cross-check — agreement with the MCC oracle and/or brute-force ground truth |
| `[REGRESS]` | differential no-regression — equality vs. a committed baseline, or a recorded change |
| `[LINT]` | static / CI assertion (dependency direction, `f64`-freedom on verdict paths, TCB size) |
| `[UNIT]` | a worked example with a known answer |
| `[MEASURE]` | a corpus measurement reported as a result (the falsifier is a table, not a pass/fail) |

### Standing invariants (must hold at every milestone)

Any milestone that violates one of these is blocked regardless of its own gates.

- **S1 — Soundness monitor.** Every accepted `Proven`/`Refuted` verdict's
  `certificate.check(net, m0, query)` returns `true` against the *original* net, and agrees with
  the oracle / brute-force ground truth where it exists. `[ORACLE]`
- **S2 — No decisiveness regression.** Public `analyze_*` verdicts match the committed baseline,
  except where a milestone explicitly converts an `Inconclusive` to a `Decided` or corrects a
  known-incorrect stub. Decreases in decisiveness are not permitted. `[REGRESS]`
- **S3 — Dependency direction and a non-increasing trusted base.** The core never imports the
  observer set; the bare-boolean trusted-base set (A6) is reported and never grows. `[LINT]`
- **S4 — Clean build.** `cargo build`/`clippy` (pedantic + nursery) and the full suite pass.

### Milestones

**M0 ✅ DONE — Soundness defects fixed; trust boundary defined; floor measured.** *(north star)*
Depends: —. Items: **A2** (the two `Some(false)` stubs → abstention) · **A5** (type-distinct
inconclusive-vs-dead) · **A7/E4/E7** (PNML fidelity → hard errors) · **B1a** (the
float-`Unreachable` audit) · **F0** (the `STRUCTURAL_REDUCTION` mis-tag) · **G4a** (the
`f_struct` floor). Gates: `[ORACLE]` no `is_efficiently_*` returns a verdict without a
certificate; a live FC net is not reported unbounded, a live marked graph not non-live ·
`[UNIT]/[REGRESS]` a `>u32::MAX` marking and a non-unit-weight P/T arc are rejected or flagged ·
`[REGRESS]` a corpus baseline of all five property verdicts is committed and reproduces ·
`[LINT]` the dependency-direction lint passes · `[MEASURE]` a floor `f_struct` is reported with
**both denominators**, counted in queries-decided. *Gates the firewall, `f`, and all structural work.*

**M1 ✅ DONE — Verdict/certificate contract.** Depends: M0. Items: **A1** (`Verdict<P,N>`,
`Certificate::check(net, m0, query)`, owned/serializable payloads, the `model` module resolving
`literature.rs:409`) · **A3a** (the A1-bounded wasm surface) · resolves **A5**. Gates: `[PROP]` no
public path yields `Proven`/`Refuted` without a passing `check`; a corrupted certificate is rejected ·
`[UNIT]/[ORACLE]` on a known unbounded net `analyze_liveness` returns `Inconclusive`, not L0 ·
`[PROP]` a `FiringSequence` checked against a *different* query fails (the target is load-bearing) ·
`[LINT]` certificates round-trip; the `petrivet::model` names `petrivet-wasm` imports from the
A1-landed surface resolve. *(The full `petrivet-wasm` compile + CI build was the A3 estimate's
optimistic dependency — it is **A3b**, gated on capability M1 must not build: see A3, and the M2/M5/M6
gates that now carry it. Re-scoped 2026-06-20 after the M1 adversarial gate found the verbatim
"`petrivet-wasm` builds in CI" criterion unmeetable without breaching the milestone boundary; rationale
in `foundational-design.md` §F3″.)*

**M2 ✅ DONE (incl. soundness remediation + A6) — Per-certificate checkers against the original net.** *(the signature contribution)*
Depends: M1. Items: **C1** (checkers, original-net) · **C2** (checking as a test invariant) ·
**C4** (in-band verify-on-return) · **C5** (trusted base, `f`) · **C6** (interchange format) ·
**C7** (frontier map) · **A6** (certifying audit, polarity) · **A3b** (the full `petrivet-wasm`
compile + CI build, relocated here from M1 — A6's polarity/method surface supplies the
`LivenessMethod`/`DeadlockAnalysisMethod`/`LivenessLevel` enums wasm reaches for; the residual
`ReachabilityProof` witness-shape variants finish at M5/M6, so A3b's CI gate is *armed* here and
*closed* once the witness redesign lands). Gates: `[PROP]` every accepted
verdict's `check` returns `true` and matches brute force · `[PROP]` a certificate from a
*different* generator (or a lifted one) for the same `(net, query, verdict)` validates
identically (original-net independence) · `[LINT]` CI fails if any emitted certificate fails its
checker; the trusted base is reported and **non-increasing** · `[UNIT]` format round-trip ·
`[LINT]` `petrivet-wasm` compiles and a CI job builds it; `parse_pnml` binds real labels/graphics
(needs the M0-changed `to_pt_system` conversion to re-expose them — the A3b/M5–M6 reconciliation) ·
`[MEASURE]` the per-property × polarity frontier table with checker complexities and the stated
wall (general liveness; ILP→cutting-plane). *Precedes the generators that feed it.*

**M3 ▶ NEXT (Workflow 2) — Decider registry.** *(before the structural generators)* Depends: M2. Items: **D1**
(registry with polarity/cost/admissible; a `Policy` whose default reproduces today's cascade
exactly). Gates: `[REGRESS]` the default-policy driver returns identical verdicts to the current
cascade across the corpus · `[PROP]` over random admissible orderings the accepted verdict is
invariant (the soundness theorem, tested — the *enabling* property, not the headline). *B's
generators are born as `Decider`s.*

**M4 ✅ DONE — Exact arithmetic kernel.** Depends: M0 (independent of M1–M3). Items: **B0** (scalar half:
a `Rational` with a documented overflow policy). Gates: `[PROP]` field axioms; `a + (−a) == 0`
exactly; value-equality independent of representation; overflow detected, never silently wrapped.

**M5 ✅ DONE — Exact linear algebra (Bareiss); negative-path audit closed.** Depends: M4. Items: **B0**
(matrix half: `rank`, `kernel`, `left_kernel`, exact `farkas_certificate`; the `f64` LP assembly
becomes an inexact filter that never constructs `Proven`/`Refuted`) · **B1a** (the
float-`Unreachable` hole closed: negative verdicts re-derived over ℚ) · **A4** (structural
boundedness as an exact P-subinvariant decider). Gates: `[PROP]` rank–nullity; `C·k == 0`
exactly; Farkas duality exact · `[ORACLE]` the ill-conditioned feasible-at-a-degenerate-vertex
net is *not* reported `Unreachable` · `[REGRESS]/[ORACLE]` exact agrees with the prior `f64` LP
where it was correct and corrects every near-boundary disagreement; coverage non-decreasing.
*The §1 design defect is resolved; the silent negative-path hole is closed.*

**M6 — Cluster quotient and Rank Theorem.** *(B2 keystone = the Workflow 2 target; B8 NUPN + the S-component decider that B2 unlocks are deferred)* Depends: M5 (rank), M2 (certified verdict). Items:
**B2** (union-find clusters → `c`; `well_formed ⇔ rank(C) == c−1`; certifies the
`is_covered_by_s_components` half of A2) · **B8** (NUPN `unit_safe`, forest preserved). Gates:
`[PROP]` cluster = flow-components · `[ORACLE]` `rank == c−1` agrees with state-space on FC nets ·
`[REGRESS]` the stub's ledger entry becomes a certified verdict · `[UNIT]` a `unit_safe` input
emits a checked certificate.

**M7 — Semiflows, invariants, the closure family.** Depends: M5. Items: **B1** (the
certificate/coverage split: a *single separating* P-invariant on the fast path, exact-checked;
minimal-semiflow coverage lazy and **capped**) · **B7** (the `Closure` trait; the exponential
minimal-siphon enumeration capped or scoped with a logged bound). Gates: `[PROP]` each invariant
satisfies `yᵀC == 0` / `Cx == 0` exactly; the separating invariant passes `y·(m'−m₀) ≠ 0` ·
`[PROP]` minimality · `[ORACLE]` coverage ⇒ the property vs. ground truth · `[LINT]` no
unexplained `#[expect(unused)]` engines; the enumeration bound is logged.

**M8 — Free-choice and T-net structural deciders.** Depends: M3, M6, M7. Items: **B3**
(S-components, exact FC bounds — certifies the FC-boundedness half of A2) · **B4** (T-net bounds,
circuit-based liveness) · **B5** (Rank/cluster simultaneous L+B) · **B6** (FC reachability +
unmarked-trap check). Gates: `[ORACLE]` each class decides structurally with a checkable
certificate · `[MEASURE]` `f_struct` re-measured against the M0 floor; the delta this milestone
contributes is reported. *The number moves, measurably.*

**M9 — The two class-agnostic deciders.** Depends: M3, M5, M7. Items: **B10** (continuous/fluid
relaxation, `ProveNo`, Fraca–Haddad PTIME — decides general/unbounded instances at the ω-frontier)
· **B11** (general-net deadlock-free siphon certificate; the converse excluded as unsound). Gates:
`[PROP]/[ORACLE]` zero soundness violations; a net where the state-equation LP passes but
continuous reachability fails returns `Unreachable` · `[ORACLE]` deadlock-free soundness ·
`[MEASURE]` a measured fraction of previously-abstained instances converted — the falsifier is a
table.

**M10 — Order abstraction and backward coverability.** Depends: M1 (independent of M5–M9). Items:
**B9** (`WellQuasiOrder` + `Ideal<D>` with `join`; `Omega == Ideal<ℕ>`; the Abdulla-style backward
loop) · **E1** (honest general-net degradation). Gates: `[REGRESS]` the generalized engine
reproduces the current graphs exactly (a pure refactor); a second trivial WQO domain drives it
unchanged · `[ORACLE]` backward = forward on bounded nets · `[REGRESS]` some prior `Inconclusive`
becomes a refinement carrying an over-approximation certificate; no decided verdict changes.

**M11 — Observation crate and the soundness sentinel.** Depends: M3, M2. Items: **D2**
(`petrivet-observe`, JSONL schema, φ) · **D3** (corpus driver, soundness sentinel, differential
fitness). (**D4** per-decider fibres is the author's-call seam.) Gates: `[LINT]` the core never
imports `observe` · `[PROP]` scaling all raw wall-times by `k>0` leaves every `FitnessComparison`
ranking/log-ratio unchanged (origin-free, torsor) · `[ORACLE]` the sentinel detects a deliberately
broken decider (the live regression for the M0 stub fixes).

**M12 — The thesis and evaluation rig.** *(alongside M6–M11)* Depends: M0, M2, M11. Items: **G1**
(commit the claim) · **G2** (the structural-tier ablation baseline) · **G3** (versioned corpus) ·
**G4** (family-held-out, two-denominator protocol) · **G5** (reproducibility) · **G6** (certificate
coverage = `f`) · **G7** (related work) · **G8** (write-up to 2026-11-02). **G9** is OUT for the
thesis window. Gates: `[MEASURE]` one run produces the `f_struct` table and the `f` check-pass
rate; every thesis number traces to a harness output · `[REGRESS]/[ORACLE]` family-held-out CV and
SBS/VBS computed; results origin-free · `[LINT]` the thesis residue deleted; the Introduction
states the falsifiable claim with its falsifier and named baseline. *MCC ranking is not a goal.*

### The gated sequel and the horizon (off the thesis-critical path)

Dependency-gated by the spine and the checker, so a wrong choice costs time, never correctness.

- **Certified reductions (Epic F).** F1 (the `Reduction { applicable, apply, lift }` trait + the
  lifting-firewall test) · F2 (implicit-place removal reusing the B1 Farkas dual). Depends: M2, M5,
  M3. Gate: `[PROP]` an identity reduction round-trips; a **deliberately wrong `lift` is caught by
  the unchanged original-net checker**; the trusted-base size is unchanged. Trusted lifts are
  restricted to *existential* witnesses until the compositional checker-completeness obligation is
  discharged.
- **The learned-selection ladder (D5–D8).** D6 (cancellation seam) · D5 (Rung 1 ranker) · D7 (Rung
  2) · D8 (Rung 3 planner). Depends: M2, M3, M11, **and a measured SBS→VBS gap**. Gate: `[MEASURE]`
  the gap is reported *first*; the ranker is built only if it exceeds a threshold (else Rung-0 is the
  honest answer). `[PROP]` if built, every verdict is still certificate-checked. **MCC ranking is OUT.**
- **Generality and the residuals (Epic H).** H1 (the WSTS zoo, once M10 abstracts the order) · H2a
  (Φ_bound, scheduled first) · H2b (Φ_inv, the Rank-Theorem link). Gate: `[ORACLE]/[MEASURE]`
  Φ_bound monotone and zero on FC/T-net fixtures; the **distribution measured over the corpus,
  indexed by `NetClass`** is the deliverable; `[LINT]` no stochastic/IIT apparatus introduced.

### Critical path

```
M0 ──┬── M1 ── M2 ── M3 ───────────────────────────┐   (defects+floor → contract → checkers → registry)
     │                    │                         │
     └── M4 ── M5 ──┬── M6 ─┬── M8 ── M9 ───────────┤   (exact LA → quotient/invariants → FC/T-net → new deciders)
                    └── M7 ─┘                        │
                                                     ├── M11 ── M12   (observe → the rig → f_struct, f)
     M10 ── (order abstraction, second phase) ───────┘

   sequel/horizon (gated, off the thesis-critical path):
     Epic F (reductions)   <- M2, M5, M3
     Epic D5-D8 (ladder)   <- M2, M3, M11, a measured SBS->VBS gap
     Epic H (Phi, WSTS)    <- M10 (H1); M5/M6/M7 (Phi_bound, Phi_inv)
```

**M2** (the checkers) and **M5** (the exact linear algebra) are the two load-bearing milestones —
the signature contribution and the soundness precondition. Capability and measured coverage rise
across the milestones while soundness, enforced by S1–S4 and the original-net checking invariant,
stays constant. The thesis is the two numbers — `f_struct` (coverage) and `f` (certifying
fraction) — not the soundness theorem.
