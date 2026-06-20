# Implementation Backlog
### Dependency-sequenced milestones, each gated by a provable invariant

> Status: implementation plan. The operational companion to the [foundational design](foundational-design.md): it sequences the eight components (F1–F8) into milestones (M0–M11), each with gates — invariant test cases that must pass before the next milestone begins. A milestone is complete when its specified invariants are proven, not when a representative example passes. Implementation occurs on a branch derived from `docs/vision`; this document is the specification that branch must satisfy.

---

## How to read this

Each milestone has a **Goal**, its **Depends** (milestones that must pass first), what it **Delivers**, a set of numbered **Gates**, and an **Exit** condition. A gate is an invariant plus the form of its proof:

| Tag | Proof form |
|---|---|
| `[PROP]` | property-based test — an invariant quantified over generated inputs (random small nets / matrices), e.g. via `proptest` |
| `[ORACLE]` | corpus cross-check — agreement with the MCC community oracle and/or brute-force ground truth (exhaustive reachability on bounded nets) |
| `[REGRESS]` | differential no-regression — equality against a committed baseline, or a recorded intentional change |
| `[LINT]` | static / CI assertion (dependency direction, `f64`-freedom, scope constraint) |
| `[UNIT]` | a worked example with a known answer |

### Standing invariants (must hold at every milestone)

Any milestone that violates one of these is blocked regardless of its own gates.

- **A1 — Soundness monitor.** Every accepted `Proven`/`Refuted` verdict's `certificate.check()` returns `true`, and the verdict agrees with the oracle / brute-force ground truth where that exists. `[ORACLE]`
- **A2 — No behavioral regression.** Public `analyze_*` verdicts match the committed baseline, except where a milestone explicitly and verifiably either (a) converts an `Inconclusive` to a `Decided`, or (b) corrects a known-incorrect stub. Decreases in decisiveness are not permitted. `[REGRESS]`
- **A3 — Dependency direction.** The core does not import the producer/observer set; the dependency lint passes. `[LINT]`
- **A4 — Clean build.** `cargo build`/`clippy` (pedantic + nursery, as the crate already configures) and the full test suite pass.

---

## M0 — Trust architecture and baseline capture

- **Goal.** Define the TCB boundary, add its lint, and record current behavior as the regression baseline and defect ledger.
- **Depends.** —
- **Delivers.** The enumerated TCB list (design §3); the dependency-direction CI lint; a committed snapshot of current `analyze_*` verdicts over the corpus; a defect ledger of current oracle disagreements (including the two `Some(false)` false negatives and the L0 defect).
- **Gates.**
  - **G0.1** `[REGRESS]` The baseline snapshot of all five property verdicts over the corpus is committed and reproduces exactly on re-run.
  - **G0.2** `[LINT]` The dependency-direction lint runs in CI and passes on the current tree.
  - **G0.3** `[ORACLE]` The current oracle-agreement rate is recorded, and each known disagreement is entered in the ledger as a case a later milestone must correct (and none may regress).
- **Exit.** Baseline and ledger committed; lint passing.

---

## M1 — Verdict and certificate calculus  *(F3, non-algebraic part)*

- **Goal.** Introduce the three-valued `Verdict`, the `Certificate` trait, and the `accept` constructor; implement checking for the witnesses that are already structured; make the stubs and the L0 case correct.
- **Depends.** M0.
- **Delivers.** The `crate::model` module; `Verdict<P,N> = Proven | Refuted | Inconclusive`; `Certificate::check` for `FiringSequence`, `Parikh` (still `f64`-sourced — flagged for M4), `SiphonTrap`, `OmegaMarking`; `accept()` as the only constructor of `Proven`/`Refuted`.
- **Gates.**
  - **G1.1** `[PROP]` **Checker soundness.** For random small nets, every accepted verdict's `check()` returns `true` and matches brute-force ground truth.
  - **G1.2** `[PROP]` **Replay fidelity.** A `FiringSequence` replays from `m₀` with every step enabled and terminates at the target.
  - **G1.3** `[UNIT]`/`[ORACLE]` **L0 distinguishability.** On a known unbounded net, `analyze_liveness` returns `Inconclusive`; "dead" is not derivable from it.
  - **G1.4** `[ORACLE]` **Stub correctness.** A known marked-graph live net is never reported dead; `is_covered_by_s_components` does not return an incorrect definitive `false` (returns `None`/`Inconclusive` pending M6). The ledger's two false-negative entries are resolved.
  - **G1.5** `[PROP]` **Constructor exclusivity.** No public path yields `Proven`/`Refuted` without a passing `check()`; a deliberately corrupted certificate is rejected.
- **Exit.** A1–A4 pass; the two false-negative ledger entries are resolved.

---

## M2 — Exact arithmetic kernel  *(F1, scalar)*

- **Goal.** A `Rational` type with a chosen, documented overflow policy.
- **Depends.** M0. (Independent of M1.)
- **Delivers.** `Rational`; the representation/overflow decision (`i128`-with-promotion vs. bignum) recorded.
- **Gates.**
  - **G2.1** `[PROP]` **Field axioms.** Associativity, commutativity, distributivity, identities, inverses over random rationals.
  - **G2.2** `[PROP]` **Canonical form and exact zero.** `a + (−a) == 0` exactly; equality is value-equality, independent of representation.
  - **G2.3** `[PROP]` **Overflow safety.** Operations either cannot overflow (bignum) or detect overflow and never wrap silently; cross-checked against a bignum reference on random inputs.
- **Exit.** Kernel passes; the representation decision documented.

---

## M3 — Exact linear algebra  *(F1, matrix)*

- **Goal.** Exact matrix/vector algebra over the incidence matrix.
- **Depends.** M2.
- **Delivers.** `Matrix`/`Vector` over `Rational`/`i64`; `rank`, `kernel`, `left_kernel`, `solve`, `farkas_certificate`.
- **Gates.**
  - **G3.1** `[PROP]` **Rank–nullity.** `rank + nullity == cols` on random integer matrices (vs. reference).
  - **G3.2** `[PROP]` **Kernel correctness.** `C·k == 0` exactly for each basis vector `k`.
  - **G3.3** `[PROP]` **Solve correctness.** A returned `x` satisfies `Cx == b` exactly; the solution set equals particular ⊕ kernel-span.
  - **G3.4** `[PROP]` **Farkas duality.** On infeasible `Cx=b, x≥0`, the returned `y` satisfies `yᵀC ≥ 0` and `yᵀb < 0` exactly; exactly one of {primal feasible, dual certificate} holds.
  - **G3.5** `[REGRESS]`/`[ORACLE]` **Float/exact reconciliation.** The exact marking-equation decider agrees with the prior `f64` LP wherever the LP was correct, and corrects every near-boundary disagreement (cross-checked vs. ILP / brute force).
- **Exit.** Algebraic verdicts are exact; the floating-point defect is quantified.

---

## M4 — Algebraic deciders made certifying  *(F3, algebraic part)*

- **Goal.** Replace the `f64` algebraic deciders with exact versions that emit checkable certificates.
- **Depends.** M1, M3.
- **Delivers.** `UnreachabilityProof` carries the extracted Farkas P-invariant; the structural-boundedness witness is an exact P-subinvariant; no `f64` on any path that constructs `Proven`/`Refuted`.
- **Gates.**
  - **G4.1** `[PROP]` **Negative certificates check.** Every algebraic `Refuted` carries an invariant whose `check()` (one exact dot product) returns `true`.
  - **G4.2** `[PROP]` **Exact boundedness witness.** The returned `y` satisfies `y > 0` and `yᵀC ≤ 0` exactly.
  - **G4.3** `[LINT]` **No `f64` reaches `accept()`.** The `Proven`/`Refuted` construction paths are `f64`-free.
  - **G4.4** `[REGRESS]` No verdict regresses vs. the M3 baseline; coverage (decided fraction) is non-decreasing.
- **Exit.** The §1 defect is resolved; A1 holds with exact checkers.

---

## M5 — Semiflows and invariants  *(F2)*

- **Goal.** The `Invariants` layer referenced by `literature.rs`.
- **Depends.** M3.
- **Delivers.** `compute_invariants`; minimal semiflows (Colom–Silva); coverage predicates.
- **Gates.**
  - **G5.1** `[PROP]` **Basis spans kernel.** `dim(semiflow span) == nullity`; each P-invariant satisfies `yᵀC == 0`, each T-invariant `Cx == 0`, exactly.
  - **G5.2** `[PROP]` **Minimality.** No returned semiflow's support strictly contains another's, and none admits a smaller-support non-negative generator.
  - **G5.3** `[ORACLE]` **Coverage implies property.** Positive-S-invariant coverage implies conservative and bounded; positive-T-invariant coverage implies consistent; both verified against state-space ground truth on bounded corpus nets.
- **Exit.** The invariant layer matches theory on the corpus.

---

## M6 — Cluster quotient and Rank Theorem  *(F4, quotient)*

- **Goal.** The cluster quotient and the certified well-formedness decision.
- **Depends.** M3 (rank), M1 (certified verdict).
- **Delivers.** Union-find clusters → `c`; `well_formed ⇔ rank(C) == c − 1`; the certified replacement for the `is_covered_by_s_components` decision path.
- **Gates.**
  - **G6.1** `[PROP]` **Cluster = flow-components.** The partition equals the connected components of the place-transition coupling (vs. reference).
  - **G6.2** `[ORACLE]` **Rank Theorem agreement.** `rank == c − 1` agrees with state-space (live and bounded) on free-choice corpus nets.
  - **G6.3** `[REGRESS]` The ledger's `is_covered_by_s_components` entry is replaced by a certified verdict; no previously-decided net changes answer except the known-incorrect `false`.
- **Exit.** The structural decidability result is certified; the second stub is resolved.

---

## M7 — Sub-net and S/T-components  *(F4, decomposition)*

- **Goal.** Induced sub-nets and the component decompositions.
- **Depends.** M5, M6.
- **Delivers.** `SubNet` (an induced `DenseNet`); S/T-component extraction; coverage.
- **Gates.**
  - **G7.1** `[PROP]` **Induced sub-net well-formed.** A `SubNet` is a valid `DenseNet`; analysis on it equals analysis on the parent restricted to that support (round-trip).
  - **G7.2** `[PROP]` **S-component characterization.** An S-component's support is a minimal P-semiflow with the state-machine property (one input, one output per in-component transition).
  - **G7.3** `[ORACLE]` **Coverage implies property.** S-component coverage implies bounded; T-component coverage implies consistent; vs. ground truth.
- **Exit.** Decomposition primitives ready for reductions (F5) and the lattice (F8).

---

## M8 — Decider, driver, and budget  *(F5)*

- **Goal.** Represent the cascade as data behind a `Decider` trait and a policy-driven `Driver`; add cooperative cancellation.
- **Depends.** M1 (gate), M4 (certifying deciders to reify).
- **Delivers.** `Decider`, `Outcome`, `Policy` (default = current hand-coded order), `Driver`; the `Budget`/`Cancellation` token.
- **Gates.**
  - **G8.1** `[REGRESS]` **Behavioral equivalence.** The `Driver` with the default policy returns identical verdicts to the current hardcoded cascade across the entire corpus.
  - **G8.2** `[PROP]` **Policy independence.** Over random admissible decider orderings, the accepted verdict is invariant (only time differs). This is the soundness theorem, tested.
  - **G8.3** `[PROP]` **Cancellation safety.** A cancelled or budget-exhausted decider returns `Inconclusive`, never `Proven`/`Refuted`; the certificate gate still blocks incorrect answers under preemption.
- **Exit.** The schedule is a parameter; the Rung-1 interface is available (and out of scope here).

---

## M9 — Observation crate  *(F6)*

- **Goal.** The differential measurement layer and the standing soundness monitor.
- **Depends.** M8 (deciders to measure), M1 (checked truth signal).
- **Delivers.** `petrivet-observe`; `Observation` + `FitnessComparison`; `φ(N)`; the soundness monitor.
- **Gates.**
  - **G9.1** `[LINT]` **Dependency direction.** The core's `Cargo.toml` never lists `observe`; the `cargo tree` lint passes.
  - **G9.2** `[PROP]` **Shift invariance.** Scaling all raw wall-times by any `k > 0` (a global log-shift) leaves every `FitnessComparison` ranking and log-ratio unchanged — performance assertions are origin-free and portable across machines.
  - **G9.3** `[ORACLE]` **Soundness monitor.** Over the corpus, every accepted verdict agrees with the oracle (`value=None` → unknown, skipped); a deliberately broken decider is detected.
- **Exit.** Self-labeling dataset and portable regression test in place; A1 is continuously enforced by the monitor.

---

## M10 — Order abstraction  *(F7, second phase)*

- **Goal.** Abstract the order; add the unbounded-net frontier.
- **Depends.** M0.
- **Delivers.** `WellQuasiOrder` trait, `Ideal<D>`; backward coverability over upward-closed sets.
- **Gates.**
  - **G10.1** `[REGRESS]` **Refactor equivalence.** The generalized engine over the ℕ-`Ideal` reproduces the current coverability/reachability graphs exactly (node/edge isomorphism); `Omega == Ideal<ℕ>` as a pure refactor.
  - **G10.2** `[ORACLE]` **Backward = forward on bounded nets.** Backward coverability agrees with the forward graph on all bounded corpus nets.
  - **G10.3** `[REGRESS]` **Monotone coverage.** The frontier converts some prior `Inconclusive` to `Decided` and changes no previously-decided verdict.
- **Exit.** WSTS substrate and increased coverage, soundness unchanged.

---

## M11 — Compositional analysis (Φ_PN)  *(F8, dependent)*

- **Goal.** The factorization residual and its witness cut.
- **Depends.** M3, M1, M7, M8.
- **Delivers.** `Φ_PN` over the partition lattice; the minimizing-cut witness.
- **Gates.**
  - **G11.1** `[ORACLE]` **Factorization soundness.** `Φ = 0` iff the per-unit verdicts recompose (via `⊗`) to the global verdict, on decomposable corpus nets with NUPN unit trees.
  - **G11.2** `[PROP]` **Witness minimality.** When `Φ > 0`, the returned cut attains the lattice minimum and identifies a genuine cross-unit coupling.
  - **G11.3** `[LINT]` **Scope constraint.** No stochastic or IIT apparatus is introduced; `Φ_PN` depends only on F1–F8 types.
- **Exit.** Compositional analysis computes; the scope constraint holds.

---

## Critical path

```
M0 ──┬── M1 ──────────────┐                         (certificate calculus)
     │                    ├── M4 ── M8 ── M9        (resolve defect → route → observe)
     └── M2 ── M3 ──┬── M4┘         │
                    ├── M5 ── M7 ───┘── M11         (invariants → components → Φ)
                    └── M6 ── M7
     M10  ── (independent, second phase) ──► coverage
```

M1 and M3 are the two components to implement first — the certificate calculus and the exact linear algebra. The remaining milestones compose under a constant guarantee: analytical capability increases across milestones while soundness, enforced by the standing invariants A1–A4, remains constant.

---

*Derived from the [foundational design](foundational-design.md), which is grounded in a line-by-line reading of the `docs/vision` codebase. Gates are stated as invariants: a milestone is complete when its invariants are proven and the standing invariants remain green. Implementation occurs on a branch derived from `docs/vision`; this backlog is the specification.*
