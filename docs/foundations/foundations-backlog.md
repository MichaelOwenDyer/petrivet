# Foundations Backlog
### Dependency-sequenced milestones, each gated by a provable invariant

> Status: implementation plan (specification-grade). The operational companion to the [foundational design](foundational-design.md): it sequences the components (F1–F8) into milestones, each with **gates** — invariant test cases that must pass before the next milestone begins. A milestone is complete when its specified invariants are *proven*, not when a representative example passes.
>
> This backlog is **coherent with the ratified engineering plan** in [`BACKLOG.md`](../../BACKLOG.md) (epics A–H, X, and its dependency-ordered sequencing). It is the foundations-document view of the same plan: same direction, same invariants, no contradictions. Each milestone below cites the ratified epic items it discharges; the [crosswalk in the design doc §11](foundational-design.md#11-crosswalk-the-fm-numbering-against-the-ah-epics) gives the full F/M ↔ A–H mapping. Where the in-progress milestone numbering diverged from the ratified epics, the ratified epic governs and this document has been brought into line — see the divergence notes at the end.

---

## The spine (the ratified order, stated once)

The inversion (design §0) puts the trust boundary before the learned superstructure, and a *measured* number before any theorem. The spine, in order:

1. **Fix the soundness defects and measure the floor now.** The two `Some(false)` stubs → abstention; the silent PNML corruptions → hard errors; the floating-`Unreachable` audit. *In parallel,* measure the structural-coverage floor `f_struct` before building anything. **[A2/A5/A7, B1a; G4a]**
2. **The contract and the certifying audit.** The `Verdict`/`Certificate::check(net, m0, query)` contract with the original-net invariant; the certifying audit that defines and measures `f` and the trusted base. **[A1, A6]** (with the wasm unblock **[A3]**)
3. **The checkers (the signature contribution).** Per-certificate checkers validating against the *original* net; in-band checking; the trusted-base ledger; the interchange format; the per-property × polarity frontier map. **[C1–C7]** *This precedes the generators that feed it.*
4. **The decider registry — before the structural generators.** So B's generators are born as `Decider`s. **[D1]**
5. **The exact-rational core, as a soundness precondition, then the generators.** The Bareiss core and the float-`Unreachable` audit **[B0/B1/B1a]**; the cluster quotient **[B2]**; onward **[B3–B9]**; the two new deciders **[B10 continuous, B11 deadlock-free]**.
6. **The thesis / evaluation rig, alongside.** **[Epic G]**, headline `f_struct`.
7. **The gated sequel and the horizon.** Certified reductions **[Epic F]** and the learned ladder **[D5–D8]**; the WSTS reuse and the two Φ residuals **[Epic H: B9→H1; H2a/H2b]**.

**MCC ranking is OUT** as a thesis goal (the contest is the crucible and the label source, not a leaderboard). **The scalar Φ_PN is dissolved** to two measured per-property residuals, Φ_bound and Φ_inv. Both decisions are carried throughout.

---

## How to read this

Each milestone has a **Goal**, its **Depends** (milestones that must pass first), the ratified **Epic** items it discharges, what it **Delivers**, numbered **Gates**, and an **Exit** condition. A gate is an invariant plus the form of its proof:

| Tag | Proof form |
|---|---|
| `[PROP]` | property-based test — an invariant quantified over generated inputs (random small nets / matrices), e.g. via `proptest` |
| `[ORACLE]` | corpus cross-check — agreement with the MCC community oracle and/or brute-force ground truth (exhaustive reachability on bounded nets) |
| `[REGRESS]` | differential no-regression — equality against a committed baseline, or a recorded intentional change |
| `[LINT]` | static / CI assertion (dependency direction, `f64`-freedom, trusted-base size, scope constraint) |
| `[UNIT]` | a worked example with a known answer |
| `[MEASURE]` | a corpus measurement reported as a result (the falsifier is a table, not a pass/fail) |

### Standing invariants (must hold at every milestone)

Any milestone that violates one of these is blocked regardless of its own gates. (Named S1–S4 to avoid collision with the ratified Epic A.)

- **S1 — Soundness monitor.** Every accepted `Proven`/`Refuted` verdict's `certificate.check(net, m0, query)` returns `true` against the *original* net, and the verdict agrees with the oracle / brute-force ground truth where it exists. `[ORACLE]`
- **S2 — No behavioral regression.** Public `analyze_*` verdicts match the committed baseline, except where a milestone explicitly and verifiably either (a) converts an `Inconclusive` to a `Decided`, or (b) corrects a known-incorrect stub. Decreases in decisiveness are not permitted. `[REGRESS]`
- **S3 — Dependency direction and a non-increasing trusted base.** The core does not import the producer/observer set; the dependency lint passes; the bare-boolean trusted-base set (A6) is reported and never grows. `[LINT]`
- **S4 — Clean build.** `cargo build`/`clippy` (pedantic + nursery, as configured) and the full test suite pass.

---

## M0 — Soundness defects fixed; trust boundary defined; floor measured  *(north star)*

- **Goal.** Discharge the soundness defects that make every later number untrustworthy, define the TCB boundary and its lint, capture the regression baseline and defect ledger — and, in parallel, measure the structural-coverage floor *now*.
- **Depends.** —
- **Epic.** **A2** (the two `Some(false)` stubs → abstention) · **A5** (type-distinct inconclusive-vs-dead) · **A7 / E4 / E7** (PNML fidelity: silent over-`u32::MAX` clamp and silent weighted-arc linearisation → hard `PnmlConversionError`) · **B1a** (the floating-`Unreachable` verdict audit) · **F0** (the `STRUCTURAL_REDUCTION` mis-tag) · **G4a** (the `f_struct` floor).
- **Delivers.** Each non-certifying `is_efficiently_*` returns `None`/`Inconclusive`, never a fabricated `Some(false)`. The PNML converter rejects or flags both corruptions with regression fixtures. The negative-path audit (B1a) is recorded as a falsifier task. The enumerated TCB list (design §3); the dependency-direction CI lint; a committed snapshot of current `analyze_*` verdicts over the corpus; a defect ledger of current oracle disagreements (the two false negatives, the L0 defect, any false-`Unreachable`). A minimal pass reporting the floor (structural-decided %, abstain %, search-decided %) and per-tier time, from the existing `*Method`/`*Proof` tags plus one `Instant` wrapper.
- **Gates.**
  - **G0.1** `[ORACLE]` No `is_efficiently_*` returns a verdict not backed by a certificate; a live free-choice net is not reported unbounded and a live marked graph not reported non-live.
  - **G0.2** `[UNIT]`/`[REGRESS]` A `> u32::MAX` marking fixture and a non-unit-weight P/T-arc fixture are each rejected or flagged; import never fabricates a different net.
  - **G0.3** `[REGRESS]` The baseline snapshot of all five property verdicts over the corpus is committed and reproduces exactly on re-run.
  - **G0.4** `[LINT]` The dependency-direction lint runs in CI and passes on the current tree.
  - **G0.5** `[ORACLE]` The current oracle-agreement rate is recorded; each known disagreement is entered in the ledger as a case a later milestone must correct (none may regress). The B1a false-`Unreachable` hypothesis has a minimal falsifying test attempted (an ill-conditioned net with a feasible rational solution at a degenerate vertex must not be reported `Unreachable`).
  - **G0.6** `[MEASURE]` A floor `f_struct` is reported with its **two denominators** (in-scope; and all-MCC counting out-of-scope as abstain), counted in *queries decided*. This is the baseline against which every Epic-B generator is measured.
- **Exit.** The stubs and corruptions are fixed; baseline, ledger, and lint are in place; the floor number exists. *This is the near-term north star: it gates the firewall, the figure of merit `f`, and all of the structural work.*

---

## M1 — Verdict and certificate contract  *(F3, non-algebraic part)*

- **Goal.** Introduce the three-valued `Verdict`, the `Certificate` trait with the `query` argument, and the `accept` constructor; consolidate the result types; make the L0 case correct.
- **Depends.** M0.
- **Epic.** **A1** (the contract) · **A3** (wasm reconciled against it) · resolves **A5** (L0) structurally.
- **Delivers.** The `crate::model` module (resolving the broken `literature.rs:409` and the five heterogeneous result shapes across `api/system/*`); `Verdict<P,N> = Proven | Refuted | Inconclusive` with `Inconclusive` type-distinct from `Refuted`; `Certificate::check(&self, net, m0, query) -> bool` with the **required `query` argument** (a query-free property passes a unit `Query`); `accept()` as the only constructor of `Proven`/`Refuted`; owned, serializable certificate payloads. `petrivet-wasm` compiles and joins CI.
- **Gates.**
  - **G1.1** `[PROP]` **Constructor exclusivity.** No public path yields `Proven`/`Refuted` without a passing `check()`; a deliberately corrupted certificate is rejected.
  - **G1.2** `[UNIT]`/`[ORACLE]` **L0 distinguishability.** On a known unbounded net, `analyze_liveness` returns `Inconclusive`; "dead" is not derivable from it; no path returns L0 as a proxy for "unknown."
  - **G1.3** `[LINT]` Certificates serialize and round-trip; every evidence variant names its theorem; `literature.rs:409` resolves; `petrivet-wasm` builds in CI.
  - **G1.4** `[PROP]` **Query necessity.** A `FiringSequence` certificate checked against a *different* query fails; the target is load-bearing.
- **Exit.** S1–S4 pass; the contract is the sole verdict surface.

---

## M2 — Per-certificate checkers against the original net  *(F3 / C1 — the signature contribution)*

- **Goal.** Implement a checker per certificate kind, each re-establishing the property against the **original** net, sharing no code with generators beyond primitive net access; make checking a test and decision-path invariant; measure and bound the trusted base; define the interchange format and the frontier map.
- **Depends.** M1.
- **Epic.** **C1** (checkers, original-net) · **C2** (checking as a test invariant) · **C4** (in-band verify-on-return) · **C5** (trusted base, `f`) · **C6** (interchange format) · **C7** (frontier map) · **A6** (certifying audit; polarity surfaced) · **C3** (front-end surfacing, where the WASM seams allow).
- **Delivers.** A checker per certificate variant with signature `check(&self, net, m0, query) -> bool`; dot-product/replay checkers that invoke no solver or graph machinery; a verified-decision entry point running the checker *before* returning (mandatory on any lifted-certificate path); the trusted base enumerated as `{checkers} ∪ {bare-boolean deciders}`, its size reported and **non-increasing**, with `f` (the certifying fraction) reported over the corpus; the canonical serialization `Cert = (net_id, query, polarity, witness, theorem_id)` over PNML *names*; the per-property × polarity frontier table.
- **Gates.**
  - **G2.1** `[PROP]` **Checker soundness.** For random small nets, every accepted verdict's `check()` returns `true` and matches brute-force ground truth.
  - **G2.2** `[PROP]` **Original-net independence.** A certificate from a *different* generator (or a lifted certificate) for the same `(net, query, verdict)` validates identically; the checker assumes nothing about its producer.
  - **G2.3** `[PROP]` **Replay fidelity.** A `FiringSequence` replays from `m₀` with every step enabled and terminates at the target; a P-invariant certificate checks by one exact dot product (pending M5's exact core for the algebraic variants).
  - **G2.4** `[LINT]` **CI checking gate.** CI fails if any emitted certificate fails its checker; no decided verdict returns without a passing check.
  - **G2.5** `[LINT]` **Trusted base non-increasing.** An inventory test asserts each decider is certifying or trust-listed; the bare-boolean set's size is reported and never grows.
  - **G2.6** `[UNIT]` **Format round-trip.** A certificate round-trips through serialization and re-checks; a hand-authored certificate for a verdict produced by a different procedure checks identically against the original net.
  - **G2.7** `[MEASURE]` The per-property × polarity frontier table is reported with checker complexities; the hardness boundary (general liveness; ILP-infeasibility → cutting-plane) is stated with its justification. The ω-witness lasso and the marked-graph circuit-token enrichments are scheduled (their absence is recorded as a known incompleteness).
- **Exit.** Every verdict is independently checkable; the trusted base is measured and minimal; `f` is a reported number. *This is the project's signature technical contribution, and it precedes the generators.*

---

## M3 — Decider registry  *(F5 / D1 — before the structural generators)*

- **Goal.** Represent the cascade as data behind a `Decider` trait and a policy-driven `Driver`, so Epic-B generators are born as `Decider`s rather than retrofitted.
- **Depends.** M2.
- **Epic.** **D1** (registry with applicability guards, polarity, cost) · surfaces **A6** polarity.
- **Delivers.** `trait Decider { polarity; cost_class; admissible(NetClass); run(net, query, budget) -> Outcome<Verdict> }`; a `Policy` whose default reproduces today's cascade exactly; a `Driver` that selects admissible deciders, orders them, gates on the certificate, and returns the first `Proven`/`Refuted` or else `Inconclusive`. (The `Budget`/`Cancellation` token is deferred to the sequel, M-Reductions/D6, where the adaptive policy needs it.)
- **Gates.**
  - **G3.1** `[REGRESS]` **Behavioral equivalence.** The `Driver` with the default policy returns identical verdicts to the current hardcoded cascade across the entire corpus; adding/reordering a decider needs no change to public analysis methods.
  - **G3.2** `[PROP]` **Policy independence.** Over random admissible decider orderings, the accepted verdict is invariant (only time differs). This is the soundness theorem, tested — and, per the inversion, the *enabling* property, not the headline.
- **Exit.** The schedule is a parameter; B's generators have a home.

---

## M4 — Exact arithmetic kernel  *(F1, scalar / B0)*

- **Goal.** A `Rational` type with a chosen, documented overflow policy.
- **Depends.** M0. (Independent of M1–M3.)
- **Epic.** **B0** (the scalar half).
- **Delivers.** `Rational`; the representation/overflow decision (`i128`-with-promotion vs. bignum) recorded.
- **Gates.**
  - **G4.1** `[PROP]` **Field axioms.** Associativity, commutativity, distributivity, identities, inverses over random rationals.
  - **G4.2** `[PROP]` **Canonical form and exact zero.** `a + (−a) == 0` exactly; equality is value-equality, independent of representation.
  - **G4.3** `[PROP]` **Overflow safety.** Operations either cannot overflow (bignum) or detect overflow and never wrap silently; cross-checked against a bignum reference on random inputs.
- **Exit.** Kernel passes; the representation decision documented.

---

## M5 — Exact linear algebra (Bareiss); negative-path audit closed  *(F1, matrix / B0, B1a)*

- **Goal.** Exact fraction-free matrix/vector algebra over the incidence matrix; close the floating-`Unreachable` hole.
- **Depends.** M4.
- **Epic.** **B0** (the matrix half, fraction-free Bareiss) · **B1a** (the float-`Unreachable` audit, resolved) · **A4** (the structural-boundedness witness becomes an exact P-subinvariant decider).
- **Delivers.** `Matrix`/`Vector` over `Rational`/`i64`; `rank`, `kernel`, `left_kernel`, `solve`, `farkas_certificate` (exact dual); the `f64::from(...)` LP assembly in `semi_decision.rs` replaced by exact solves, with `microlp` retained only as an inexact filter that never constructs `Proven`/`Refuted`; negative reachability/coverability re-derived over ℚ (null-space membership of `m'−m₀` in `ker(Cᵀ)`, or exact recheck of the rationalised dual) before returning. SNF/Hermite deferred to a scoped **B0b** for integer refinement only.
- **Gates.**
  - **G5.1** `[PROP]` **Rank–nullity.** `rank + nullity == cols` on random integer matrices (vs. reference).
  - **G5.2** `[PROP]` **Kernel correctness.** `C·k == 0` exactly for each basis vector `k`.
  - **G5.3** `[PROP]` **Farkas duality.** On infeasible `Cx=b, x≥0`, the returned `y` satisfies `yᵀC ≥ 0` and `yᵀb < 0` exactly; exactly one of {primal feasible, dual certificate} holds.
  - **G5.4** `[ORACLE]` **False-`Unreachable` closed.** The ill-conditioned feasible-at-a-degenerate-vertex net from M0's B1a test is *not* reported `Unreachable`; every negative verdict is exact-certified.
  - **G5.5** `[REGRESS]`/`[ORACLE]` **Float/exact reconciliation.** The exact marking-equation decider agrees with the prior `f64` LP wherever the LP was correct and corrects every near-boundary disagreement (cross-checked vs. ILP / brute force); coverage is non-decreasing.
- **Exit.** The §1 design defect is resolved; S1 holds with exact checkers; the silent negative-path hole is closed.

---

## M6 — Cluster quotient and Rank Theorem  *(F4 / B2; certifies the second stub)*

- **Goal.** The cluster quotient (cheapest keystone) and the certified well-formedness decision.
- **Depends.** M5 (rank), M2 (certified verdict).
- **Epic.** **B2** (union-find clusters → `c`) · resolves the `is_covered_by_s_components` half of A2 with a certificate · **B8** (NUPN `unit_safe`, where the forest is preserved).
- **Delivers.** Union-find clusters → `c`; `well_formed ⇔ rank(C) == c − 1`; the certified replacement for the `is_covered_by_s_components` decision path; the NUPN forest as a `Partition` source carrying its `unit_safe` certificate.
- **Gates.**
  - **G6.1** `[PROP]` **Cluster = flow-components.** The partition equals the connected components of the place-transition coupling (vs. reference).
  - **G6.2** `[ORACLE]` **Rank Theorem agreement.** `rank == c − 1` agrees with state-space (live and bounded) on free-choice corpus nets.
  - **G6.3** `[REGRESS]` The ledger's `is_covered_by_s_components` entry is replaced by a certified verdict; no previously-decided net changes answer except the known-incorrect `false`.
  - **G6.4** `[UNIT]` A `unit_safe` NUPN input emits a checked safety certificate; the forest survives conversion.
- **Exit.** The structural decidability result is certified; the second stub is resolved.

---

## M7 — Semiflows, invariants, and the closure family  *(F2 / B1, B7)*

- **Goal.** The `Invariants` layer, split along its tractability seam; consolidate the siphon/trap engines.
- **Depends.** M5.
- **Epic.** **B1** (certificate vs. coverage split) · **B7** (the `Closure` family).
- **Delivers.** `compute_invariants`; the *single separating* P-invariant attached to a negative verdict on the fast path (exact-checked before return); minimal-semiflow coverage predicates (Colom–Silva) computed lazily and **capped**; a `Closure` trait (`maximal_siphon_in`/`maximal_trap_in`, duality = an incidence-direction flip) deduping the shrinking loops, with the worst-case-exponential minimal-siphon enumeration capped or scoped with a logged bound.
- **Gates.**
  - **G7.1** `[PROP]` **Basis spans kernel.** Each P-invariant satisfies `yᵀC == 0`, each T-invariant `Cx == 0`, exactly; the separating invariant passes `y·(m'−m₀) ≠ 0`.
  - **G7.2** `[PROP]` **Minimality.** No returned semiflow's support strictly contains another's, and none admits a smaller-support non-negative generator.
  - **G7.3** `[ORACLE]` **Coverage implies property.** Positive-S-invariant coverage ⇒ conservative and bounded; positive-T-invariant coverage ⇒ consistent; both vs. state-space ground truth (within the cap).
  - **G7.4** `[LINT]` No unexplained `#[expect(unused)]` engines; the exponential enumeration is capped or scoped with a logged bound; CHC behaviour unchanged.
- **Exit.** The invariant layer matches theory on the corpus; the closures are consolidated and bounded.

---

## M8 — Free-choice and T-net structural deciders  *(F4 / B3, B4, B5, B6)*

- **Goal.** The polynomial structural deciders for the free-choice and T-net frontier — the generators that raise `f_struct` above the M0 floor.
- **Depends.** M3 (registry), M6 (clusters), M7 (invariants).
- **Epic.** **B3** (S-component decomposition, exact FC bounds — certifies the FC-boundedness half of A2) · **B4** (T-net bounds, circuit-based liveness) · **B5** (Rank/cluster theorem for simultaneous L+B) · **B6** (FC reachability via marking equation + unmarked-trap check).
- **Delivers.** S-component decomposition replacing the A2 FC stub with a certificate and exact per-place bounds; circuit-based T-net liveness and exact bounds without state-space exploration; the combined FC L+B certificate (positive S/T-invariants ∧ `rank C = c − 1` ∧ every proper siphon marked); `LiveBoundedFreeChoiceMarkingEquationWithTrapCheck` produced (the commented-out dispatch arm restored).
- **Gates.**
  - **G8.1** `[ORACLE]` Live FC nets covered by S-components decide in polynomial time with a checkable certificate; the A2 FC-boundedness regression passes via the efficient path.
  - **G8.2** `[ORACLE]` The documented T-net examples decide structurally with checkable bound/liveness certificates; circuit enumeration is restricted to circuits containing the place of interest.
  - **G8.3** `[ORACLE]` The documented FC examples decide simultaneous L+B; the combined certificate checks. Live and bounded FC systems decide reachability via the integer marking-equation + trap-check certificate.
  - **G8.4** `[MEASURE]` `f_struct` is re-measured against the M0 floor; the delta this milestone contributes is reported.
- **Exit.** The free-choice / T-net frontier is certified and polynomial; the coverage number has moved, measurably.

---

## M9 — The two new class-agnostic deciders  *(F5 / B10, B11)*

- **Goal.** Widen coverage at the general-net and ω-frontier with two deciders the ratified plan adds.
- **Depends.** M3 (registry), M5 (exact LA / Farkas), M7 (siphon/trap closures).
- **Epic.** **B10** (continuous/fluid relaxation, class-agnostic `ProveNo`) · **B11** (general-net deadlock-free siphon certificate).
- **Delivers.** A `ProveNo` continuous-reachability/coverability decider (Fraca–Haddad PTIME), the apex of the LP→ILP cascade, deciding *general, unbounded* instances at the ω-frontier where `reachability.rs` returns `Inconclusive`; witness = the Farkas/place-invariant `y` or the maximal firing set + blocking empty siphon, checked by a dot product resp. a polynomial firing-set fixpoint recompute. CHC's `Ok` arm exposed as a certifying *deadlock-free* verdict for general nets (the converse — unmarked siphon as a deadlock witness — explicitly excluded as unsound in general).
- **Gates.**
  - **G9.1** `[PROP]`/`[ORACLE]` **Continuous soundness.** Zero soundness violations against the oracle; a net where the state-equation LP passes but continuous reachability fails returns `Unreachable`.
  - **G9.2** `[ORACLE]` **Deadlock-free soundness.** A general (non-FC) net with every minimal siphon holding a marked trap yields a checked deadlock-free verdict without exploration.
  - **G9.3** `[MEASURE]` A measured fraction of currently-abstained `ReachabilityCardinality`/`UpperBounds` (via B10) and general-net `ReachabilityDeadlock` (via B11) instances is converted to sound verdicts — the falsifier is a corpus table.
- **Exit.** Two class-agnostic deciders widen coverage with checked certificates.

---

## M10 — Order abstraction and backward coverability  *(F7 / B9; second phase)*

- **Goal.** Abstract the order; convert the blanket ω-frontier `Inconclusive` into a backward-coverability refinement.
- **Depends.** M1 (verdict). (Independent of the M5–M9 algebra path.)
- **Epic.** **B9** (the near-term WSTS lift) · **E1** (honest degradation for general nets).
- **Delivers.** `WellQuasiOrder` (with the test-enforced wqo obligation) + `Ideal<D>` with `join`; `Omega` generalized to `Ideal<ℕ>`; the Abdulla-style backward-coverability loop carrying a partial over-approximation certificate; `UniqueSortedSlice` as the finite-basis representation.
- **Gates.**
  - **G10.1** `[REGRESS]` **Refactor equivalence.** The generalized engine over the ℕ-`Ideal` reproduces the current coverability/reachability graphs exactly (node/edge isomorphism); `Omega == Ideal<ℕ>` as a pure refactor; a second trivial WQO domain drives the explorer unchanged.
  - **G10.2** `[ORACLE]` **Backward = forward on bounded nets.** Backward coverability agrees with the forward graph on all bounded corpus nets.
  - **G10.3** `[REGRESS]` **Monotone coverage.** The frontier converts some prior `Inconclusive` to a refinement carrying an over-approximation certificate, and changes no previously-decided verdict; no general-net path fails silently.
- **Exit.** WSTS substrate and increased coverage, soundness unchanged. (The full WSTS zoo — H1 — and other WQO domains are the horizon, recorded not scheduled.)

---

## M11 — Observation crate and the soundness sentinel  *(F6 / D2, D3)*

- **Goal.** The differential measurement layer, the φ extractor, and the always-on soundness sentinel.
- **Depends.** M3 (deciders to measure), M2 (checked truth signal).
- **Epic.** **D2** (`petrivet-observe`, JSONL schema, φ) · **D3** (corpus driver, soundness sentinel, differential fitness). (**D4** per-decider fibres is the author's-call seam, conditional.)
- **Delivers.** `petrivet-observe` depending only on `petrivet` (CI-enforced via `cargo tree`); `Observation` and the differential `FitnessComparison` (versioned, run-context-tagged); `φ(N)` over the cached accessors plus the M6/M7/M8 structural coordinates and the NUPN unit-tree shape; the corpus driver folding `Observation` into `FitnessComparison`; the always-on soundness sentinel (every `Decided` row with a known oracle must agree).
- **Gates.**
  - **G11.1** `[LINT]` **Dependency direction.** The core's `Cargo.toml` never lists `observe`; the `cargo tree` lint passes.
  - **G11.2** `[PROP]` **Shift invariance.** Scaling all raw wall-times by any `k > 0` leaves every `FitnessComparison` ranking and log-ratio unchanged — performance assertions are origin-free and machine-portable.
  - **G11.3** `[ORACLE]` **Soundness sentinel.** Over the corpus, every accepted verdict agrees with the oracle (`value=None` → unknown, skipped); a deliberately broken decider is detected (the live regression for the M0 stub fixes).
- **Exit.** Self-labeling dataset and portable regression test in place; S1 is continuously enforced by the sentinel.

---

## M12 — The thesis and evaluation rig  *(Epic G; alongside M6–M11)*

- **Goal.** Promote the harness to thesis grade; commit the falsifiable claim; report the headline `f_struct` and `f`.
- **Depends.** M0 (floor), M2 (`f`), M11 (telemetry). Runs *alongside* the generator milestones, not after them.
- **Epic.** **G1** (commit the claim) · **G2** (evaluation design, the structural-tier ablation) · **G3** (versioned corpus) · **G4** (family-held-out, two-denominator protocol) · **G5** (reproducibility) · **G6** (certificate coverage = `f`) · **G7** (related work) · **G8** (write-up milestones to the 2026-11-02 deadline). **G9** (learned selection as a result) is **OUT** for the thesis window.
- **Delivers.** The committed falsifiable claim (*a polynomial structural certifying tier decides a large, characterizable fraction of queries with an independently checkable certificate and without state-space exploration; where it abstains, it abstains honestly, and the boundary is predictable from cheap structural features*) with its named falsifier and the structural-tier ablation as the primary internal baseline; a versioned, provenance-tracked corpus pinned to a manifest; the family-held-out, two-denominator coverage table counted in queries decided; one-command reproduction; certificate coverage and the independent-check pass rate (= `f`); positioning against the structural, certifying-algorithms, proof-logging, continuous-net, and reduction lineages.
- **Gates.**
  - **G12.1** `[MEASURE]` The two-denominator coverage table (`f_struct`) and the certificate coverage / check-pass rate (`f`) are produced by one run; every thesis number traces to a harness output.
  - **G12.2** `[REGRESS]`/`[ORACLE]` Family-held-out cross-validation and SBS/VBS are computed; correctness assertions still pass; results are origin-free and machine-portable.
  - **G12.3** `[LINT]` The thesis residue is deleted; the Introduction/Objectives state the falsifiable claim with its falsifier and named baseline; every baseline is cited and every complexity claim backed; the unfolding/symbolic absence is stated as an explicit boundary.
- **Exit.** The headline numbers exist and are reproducible; the claim is committed and falsifiable; the write-up schedule ends at 2026-11-02. *MCC ranking is not a goal; the unit of evidence is a characterization plus a construction.*

---

## The gated sequel and the horizon (recorded; sequenced after the spine)

These are dependency-gated by the spine and the checker, so a wrong choice costs time, never correctness. They are recorded with their gates but are not part of the thesis-critical path.

### S-Reductions — certified reductions  *(Epic F)*
- **Epic.** **F0** (mis-tag, done in M0) · **F1** (the `Reduction { applicable, apply, lift }` trait and the lifting-firewall robustness test) · **F2** (first reductions: implicit-place removal reusing the B1 Farkas dual).
- **Depends.** M2 (C1/C4 checkers), M5 (Farkas), M3 (registry).
- **Gate (the firewall test).** `[PROP]` An identity reduction round-trips; a **deliberately wrong `lift` is caught by the unchanged original-net checker**; the trusted-base size is unchanged. Trusted lifts are restricted to *existential* witnesses (firing sequences) until the compositional checker-completeness obligation is discharged. A lifted certificate checks against the *original* net, so the whole reduction library lives outside the TCB.

### S-Ladder — the learned selection sequel  *(Epic D5–D8)*
- **Epic.** **D6** (cooperative cancellation / budget — the Rung-2 prerequisite the registry deferred) · **D5** (Rung 1: empirical hardness ranker, SATzilla-lineage, cost-sensitive regret) · **D7** (Rung 2: adaptive sequential policy) · **D8** (Rung 3: planner over certified reductions).
- **Depends.** M2 (C2 gate), M3 (D1), M11 (D3 harness), and a **measured SBS→VBS gap**.
- **Gate (the honesty ledger).** `[MEASURE]` The SBS→VBS gap on the corpus is reported *first*; the ranker is built only if the gap exceeds a threshold (else the hand-ordered cascade is the honest answer and the ML is dead weight). `[PROP]` If built, every verdict is still certificate-checked; selection changes only *which* proofs are attempted, never *what* is accepted; the sentinel stays green. **MCC ranking is OUT.**

### S-Horizon — generality and the factorization residuals  *(Epic H)*
- **Epic.** **H1** (the WSTS zoo, once B9/M10 abstracts the order) · **H2a** (Φ_bound, *scheduled first* among the residuals) · **H2b** (Φ_inv, the novel Rank-Theorem link).
- **Depends.** M10 (H1); M5/M6/M7 (the residuals).
- **The dissolution, restated.** The scalar Φ_PN, the boolean-verdict Φ, and the necessity claim are **dissolved**. What remains are two computable, monotone, theorem-backed-zero, per-property residuals whose deliverable is their **measurement over the corpus**, not their metaphysics.
- **Gate.** `[ORACLE]`/`[MEASURE]` Φ_bound: monotonicity proven; the zero-set verified on `class.rs` fixtures (live bounded FC nets, strongly-connected T-nets); a Φ_bound>0 fixture constructed; the **distribution measured over the MCC NUPN corpus, indexed by `NetClass`** — the actual deliverable; the minimizing cut emitted as an F3-checkable witness. Φ_inv: integer-valued, basis-free; the Rank-Theorem link (to `c`) established with the interface correction explicit; the cut emitted. `[LINT]` No stochastic or IIT apparatus is introduced.

---

## Cross-cutting quality  *(Epic X)*

- **X1 — Property-based testing against state-space ground truth.** For bounded fixtures, compare every decider's certificate-backed answer against exhaustive state-space construction — the oracle for the generators, the checkers, and the reductions. `[PROP]` A suite across generated nets within a size envelope. (Complements M2's checking gate.)
- **X2 — Resolve the `literature.rs` blueprint by building it.** Each dangling link (`structural::*`, `Invariants`, `SComponent`, `crate::model::*`, broken at line 409) is resolved by building the module, not demoted to prose. `[LINT]` `cargo doc` builds without broken intra-doc warnings. (Tracks M1/M5–M8.)
- **X3 — Lint and documentation build clean across the workspace.** `[LINT]` CI enforces clean `clippy`/`cargo doc` across all crates, including the non-default members once M1 restores the wasm build.
- **X4 — φ feature artifact and sufficiency test.** φ(N) as a versioned artifact in `petrivet-observe`, with the NUPN unit-forest parser (observe-side, no core touch) and a measurable sufficiency criterion (mutual information with the hardness label). `[MEASURE]` A sufficiency report; B-derived coordinates wired in as M6–M8 land. (Tracks M11.)

---

## Critical path

```
M0 ──┬── M1 ── M2 ── M3 ───────────────────────────┐   (defects+floor → contract → checkers → registry)
     │                    │                         │
     └── M4 ── M5 ──┬── M6 ─┬── M8 ── M9 ───────────┤   (exact LA → quotient/invariants → FC/T-net → new deciders)
                    └── M7 ─┘                        │
                                                     ├── M11 ── M12   (observe → the rig → f_struct, f)
     M10 ── (order abstraction, second phase) ───────┘

   sequel/horizon (gated, off the thesis-critical path):
     S-Reductions (Epic F)   ◄── M2, M5, M3
     S-Ladder (Epic D5–D8)   ◄── M2, M3, M11, a measured SBS→VBS gap
     S-Horizon (Epic H)      ◄── M10 (H1); M5/M6/M7 (Φ_bound, Φ_inv)
```

M2 (the checkers) and M5 (the exact linear algebra) are the two load-bearing milestones — the signature contribution and the soundness precondition. The remaining milestones compose under a constant guarantee: analytical capability and measured coverage increase across milestones while soundness, enforced by the standing invariants S1–S4 and the original-net checking invariant, remains constant. The thesis is the two numbers — `f_struct` (coverage) and `f` (certifying fraction) — not the soundness theorem.

---

## Divergences from the in-progress milestone numbering, and how they were reconciled

This document previously sequenced M0–M11 with a different ordering. Reconciling to the ratified [`BACKLOG.md`](../../BACKLOG.md) required four substantive changes; each is recorded here so the history is legible.

1. **The checkers were promoted ahead of the algebra.** The old order ran M2/M3 (exact arithmetic, exact LA) before the checkers (folded into M1/M4). The ratified inversion makes the *checker-and-format* the signature contribution and sequences it (Epic C / new M2) **before** the structural generators and the exact-LA-dependent algebraic certificates that feed it. The exact core (M4/M5) remains a soundness precondition but is no longer the headline.
2. **The decider registry moved earlier.** The old M8 placed the registry after the algebraic deciders; the ratified plan (D1) places it **before** Epic B so generators are born as `Decider`s. It is now M3.
3. **New scope was folded in.** PNML fidelity (A7), the float-`Unreachable` audit (B1a), the two new deciders (B10/B11), the siphon/trap consolidation (B7), the certificate format and frontier map (C6/C7), certified reductions (Epic F), and the full thesis rig (Epic G with the `f_struct` floor G4a) were thin or absent in the old numbering. They are now M0, M2, M5, M7, M9, M12, and the sequel section. None contradicts the prior design; each strengthens the same trust boundary.
4. **The scalar Φ was dissolved; standing invariants renamed.** The old M11 built a single `Φ_PN`; it is replaced by the two measured residuals (H2a/H2b) in S-Horizon. The standing invariants, formerly A1–A4, are renamed S1–S4 to avoid collision with the ratified Epic A.

The result is one plan, not two: the F/M view here and the A–H view in [`BACKLOG.md`](../../BACKLOG.md) describe the same dependency-ordered build, with the same falsifiable gates and the same invariant discipline.

---

*Derived from the [foundational design](foundational-design.md) and reconciled to the ratified [`BACKLOG.md`](../../BACKLOG.md). Gates are stated as invariants: a milestone is complete when its invariants are proven and the standing invariants remain green. Every milestone carries a falsifiable gate; the trust boundary and the original-net checking invariant hold constant across all of them.*
