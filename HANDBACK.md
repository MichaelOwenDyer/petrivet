# HANDBACK — petrivet foundation (Workflow 1: M0–M2 + M4/M5)

*For Michael. The foundation is laid to the working doctrine; the flag is not captured. This
records what was built, what is **proven** (with the test names), the runway left to the
sprint, and the premise-level flags for your ratification. Everything is **local** on branch
`foundation-brickwork` — no push, no PR.*

> **Scope note — this is a partial handback.** The HANDOFF envisioned one pass (M0–M5 + the B2
> keystone). The work was split into two workflows; **this handback covers Workflow 1: M0, M1,
> M2, M4, M5** — plus the post-handback A6 polarity-coherence gate. **M3 (the decider registry)
> and the B2 keystone (cluster quotient) are PENDING — Workflow 2.** The critical path is honoured:
> M3 depends on M2, and the M4/M5 footings are independent of M3, so Workflow 2 (M3 + B2) slots in
> cleanly on top of this branch. See §"Runway" for what M3/B2 inherit, and
> **[`docs/foundations/m3-b2-handoff.md`](docs/foundations/m3-b2-handoff.md) — the complete Workflow 2
> brief** (state, gates, the ready seams, the hard-won lessons, the boundary, and the
> decompose-for-Michael deliverable).

> **Soundness remediation since this handback (read first).** After this handback an independent
> adversarial review found and fixed a set of soundness holes the M2/M5 cut left open — two
> false-`Proven` checkers (`ParikhVectorCert`, `SiphonTrapCoverCert`) and three `f64` verdict
> paths (`Uncoverable`, `Unreachable`, positive `bounded`), plus the arithmetic minors. The
> consolidated record — what was unsound, the sound fix, the oracle-verified counterexamples and
> regression test names, and the **theory choices for you to ratify** — is in
> [**`docs/foundations/m2-soundness-remediation.md`**](docs/foundations/m2-soundness-remediation.md)
> (discernment in [`foundational-design.md`](docs/foundations/foundational-design.md) §4.4–§4.6′).
> **This supersedes §4 flag 2 below:** `ParikhVectorCert` no longer uses the class fence described
> there (the fence was *unsound* — necessary ≠ sufficient without liveness); it now accepts by
> *realizing* `σ` as an actual firing sequence, sound on any class. The remediation commits
> (`3bb2a38` → `3a1d442`) sit atop this handback's commits; the gate is re-verified green (lib
> 217/0, `checker_invariants` 9/0, `firewall_probe` 2/0, clippy histogram byte-identical to
> baseline).

Result-strength labels below: **proven** = a gate test passes; **observed** = measured/run;
**inferred** = reasoned, not yet executed end-to-end.

---

## 1. What was built, per milestone

The foundation is built **bottom-up to the doctrine**: every change is *fix + the regression
test that would have caught it + the rationale kept in `docs/foundations/foundational-design.md`*
(doctrine #8). The design doc now carries the implementation record in §1.2 (M0), §F3″ (M1),
§F3‴ (M2), §4.2 (M4), §4.3 (M5).

### M0 — soundness defects fixed; trust boundary's first abstentions; floor measured *(landed before this workflow; commits `56ab387`, `4d3aa55`)*
The two `Some(false)` stubs demoted to honest abstention (A2); the deadlock-freedom fabricated
negative removed; A5 type-distinct inconclusive-vs-dead; A7/E4/E7 PNML fidelity → hard
`PnmlConversionError`; B1a falsifier; F0 mis-tag; the `f_struct` **floor** (`mcc-tests`,
**0.167** in-scope / **0.133** all-corpus, the *starting line*). Two engine-correctness
defects were uncovered and fixed closing the green gate (inert `fire_unchecked`; scrambled
petgraph mirror).

### M1 — the verdict/certificate contract *(landed before this workflow; commits `486a219`, `9bd0c89`, `6bc8ef8`)*
`crate::model`: `Verdict<P,N>` with a type-distinct `Inconclusive(Abstention)`; the
`Certificate { check(net, m0, query) }` trait; `accept` as the sole public constructor of
`Proven`/`Refuted`. The firewall is a **type invariant** — decided variants carry
private-field `Proof`/`Refutation` wrappers, so an un-checked verdict is un-name-constructible
even from a downstream crate (three `compile_fail` doctests prove it). `literature.rs:409`
resolved; a first-class `serde` feature.

### M2 — per-certificate checkers, the ledger, the format, the frontier *(this workflow; commit `05a259e`)* — **the signature substrate**
- **C1 — four `Certificate` checkers** (`api/model/checkers.rs`), each re-validating against the
  **original** net using only primitive access (`try_fire`, transition pre/post sets, marking
  sums) — sharing no code with any generator:
  - `FiringSequenceCert` (positive reachability/coverability by replay, near-linear);
  - `ParikhVectorCert` (marking-equation integer solution recomputed over ℤ; **re-derives the
    sufficiency class** and rejects a Parikh witness on a class where the equation is only
    necessary);
  - `TokenConservationCert` (negative reachability for state machines; re-derives `|•t|=|t•|=1`
    directly, not from a class tag);
  - `SiphonTrapCoverCert` (free-choice liveness / general deadlock-freedom; re-verifies each
    exhibited siphon-with-marked-trap; polynomial in the *exhibited* cover).
- **A4's checker** (added at M5, see below): `BoundednessSubinvariantCert`.
- **C5 + A6 — the trusted-base ledger** (`api/model/ledger.rs`): every fast decider tagged with
  **polarity** (the A6 surfacing) and certifying-vs-bare-boolean; reports **`f = 0.571`**
  (4 certifying / 3 bare-boolean / 7 total — *structural* `f`, not the corpus-weighted empirical
  `f` of M12) and pins the bare-boolean trusted base **non-increasing**.
- **C6 — the interchange format** (`api/model/format.rs`, serde-gated): the canonical
  `Cert<W> = {net_id, query, polarity, witness, theorem_id}` over name-anchored witnesses;
  round-trips, and a hand-authored (foreign-tool) certificate deserializes.
- **C7 — the checkable-frontier map** (`api/model/frontier.rs`): the per-(property × polarity)
  table with checker complexities and the explicit **wall** (general liveness; integer-only
  infeasibility → cutting-plane).

### M4 — the exact-rational scalar (`Rational`, B0 scalar half) *(this workflow; commit `8185179`)*
`core/analysis/rational.rs`: `Rational` over **`i128` with overflow *detection*** — every op is
`checked_*` returning `Result`, **never a wrap** (no `impl Add`/`Mul`, so a caller must thread the
`Result` and an overflow forces an honest abstention). Normalized → `Eq`/`Ord` are
representation-independent; ordering is **exact at every magnitude** (a cross-multiply fast path
with an overflow-free continued-fraction fallback — no `f64` ever decides an order). The
representation decision (i128, not i64, not bignum) is recorded in §4.2 with its falsifier.

### M5 — exact linear algebra; the B1a closure; the A4 checker *(this workflow; commit `5e54fa0`)* — **load-bearing**
- **The exact matrix layer** (`core/analysis/exact_matrix.rs`): `Matrix` over `Rational` with
  `rank`, `right_kernel` (T-semiflows), `left_kernel` (P-semiflows), `solve`, and
  `farkas_certificate` (the **exact** separating P-invariant on an infeasible system). Exact RREF
  over `Rational`; the fraction-free **Bareiss schedule** is the deferred *cost* optimization
  (the gate is *exact*, which `Rational` delivers).
- **B1a — the silent false-`Unreachable` hole, closed.** The f64 LP is demoted to an inexact
  *filter*; `analyze_reachability` returns `Unreachable` only when the exact guard
  (`marking_equation_exact`, `b ∈ col(C)` over ℚ) confirms infeasibility, else escalates — so a
  spurious f64 infeasibility at a degenerate vertex can no longer become a false `Unreachable`.
- **A4 — structural boundedness as an exact, checkable P-subinvariant**: `BoundednessSubinvariantCert`
  re-verifies `y > 0` and `yᵀ·C ≤ 0` over ℚ exactly.

---

## 2. What is PROVEN (gates, with test names)

A milestone gate is proven when its test passes against the original net (S1) and the standing
invariants hold. Full suite for this workflow: **`petrivet` lib 181/0**, **`checker_invariants`
9/0**, **`firewall_probe` 2/2**, **`mcc-tests` structural_floor 1/1**, the M1 firewall doctests
4/4.

### M2 gates
| Gate | Status | Proof (tests) |
|---|---|---|
| `[PROP]` every accepted verdict's `check` is `true` and matches brute force | **proven** | `checkers::every_checker_promotes_a_valid_candidate_through_accept`; `checker_invariants::{firing_sequence,token_conservation,siphon_trap_cover}_checks_and_agrees_with_ground_truth` |
| `[PROP]` original-net independence: a *different generator's* (or lifted) witness for the same `(net,query,verdict)` validates identically | **proven** | `checker_invariants::independent_witnesses_for_the_same_verdict_validate_identically`; `..::a_witness_is_rejected_against_a_different_net`; `..::a_witness_is_rejected_against_a_different_query` |
| `[LINT]` CI fails if an emitted certificate fails its checker; the trusted base is reported and **non-increasing** | **proven** (in-tree form) | `ledger::trusted_base_is_reported_and_non_increasing`; `checkers::every_checker_rejects_an_invalid_candidate_at_the_gate` |
| `[UNIT]` format round-trip | **proven** | `format::tests::{firing_word,parikh,siphon_trap_cover}_cert_round_trips`; `..::a_hand_authored_certificate_deserializes` |
| `[MEASURE]` the per-property × polarity frontier table with checker complexities and the stated wall | **proven** (table pinned) | `frontier::frontier_map_matches_committed_baseline` |
| `[LINT]` `petrivet-wasm` compiles + CI build (**A3b**) | **NOT met — deferred** | capability-gated to M5/M6 (witness redesign) + A6 enums; the hard boundary forbids building it now. See §4. |

### M4 gates
| Gate | Status | Proof (tests) |
|---|---|---|
| `[PROP]` field axioms; `a + (−a) == 0` exactly | **proven** | `rational::{additive_identity_and_inverse, multiplicative_identity_and_inverse, commutativity, associativity, distributivity, subtraction_is_add_of_negation}` |
| `[PROP]` value-equality independent of representation | **proven** | `rational::equality_is_representation_independent`; `..::ordering_is_exact_and_consistent_with_subtraction`; `..::ordering_is_exact_even_when_the_cross_product_overflows` |
| `[PROP]` overflow detected, never silently wrapped | **proven** | `rational::{overflow_is_detected_not_wrapped, division_by_zero_and_zero_denominator_fail, i128_min_is_refused_by_new}` |

### M5 gates
| Gate | Status | Proof (tests) |
|---|---|---|
| `[PROP]` rank–nullity; `C·k == 0` exactly; Farkas duality exact | **proven** | `exact_matrix::{rank_nullity_holds, kernel_vectors_are_annihilated_exactly, left_kernel_vectors_annihilate_columns_exactly, farkas_certificate_is_exact_on_infeasible_and_absent_on_feasible, solve_returns_exact_particular_solution}` |
| `[ORACLE]` the ill-conditioned feasible-at-a-degenerate-vertex net is *not* reported `Unreachable` | **proven** | `reachability::tests::b1a_feasible_target_at_degenerate_vertex_not_reported_unreachable` (M0 falsifier, now backed by the exact guard); `exact_matrix::marking_equation_exact_guard_separates_infeasible_from_feasible` |
| `[REGRESS]/[ORACLE]` exact agrees with the prior f64 LP where correct; coverage non-decreasing | **observed** | full lib suite 181/0 + `mcc-tests` floor reproduce — no decisiveness regression (S2). The exhaustive corpus diff is the M12 rig's job, not this foundation. |

### Standing invariants (this workflow)
- **S1** — every accepted `Proven`/`Refuted` `check`s against the original net (the checkers are
  written to that contract; `checker_invariants` cross-checks each verdict against the public
  `is_*` ground truth). **proven.**
- **S2** — no decisiveness regression: lib 181/0 and the mcc floor reproduce; the only verdict-path
  change (B1a) preserves every green test. **observed.**
- **S3** — core never imports the observer set; the bare-boolean trusted base is reported and does
  not grow (pinned at 3 by `ledger`). **proven** (in-tree form; the CI `cargo tree` lint is M11).
- **S4** — clean build; `cargo clippy -p petrivet --all-targets` warning histogram **byte-identical**
  to the pre-M2 baseline (zero new warnings — measured by moving the new files aside and diffing);
  full suite passes. **proven.**

---

## 3. The runway left toward M3 / B2 and the sprint

The foundation is built so that **adding a generator is writing one `Decider` and its
certificate — nothing structural left to invent.** Concretely, Workflow 2 and the sprint inherit:

- **The verdict/certificate contract** (M1) with the firewall enforced *by type*.
- **Four original-net checkers + the A4 boundedness checker** (M2/M5): a new generator's verdict
  is validated by an *existing* checker the moment its witness has one of these shapes (firing
  word, Parikh vector, token conservation, siphon/trap cover, positive sub-invariant).
- **The exact-rational core** (M4/M5) with a stable API: `Rational` (checked, overflow-detecting)
  and `Matrix` (`rank`/`kernel`/`left_kernel`/`solve`/`farkas_certificate`). The B2 cluster
  quotient and the Rank Theorem (`rank C = c − 1`) have their `rank` already; B1 invariants have
  `left_kernel`; B3/B4 bounds have the exact incidence and the Farkas dual.
- **The trusted-base ledger and the frontier map** (C5/C7): each new decider gets a ledger row
  (flip `BareBoolean → Certifying` when it routes through a checker) and a frontier cell.

**PENDING — Workflow 2 (explicitly not built here):**
- **M3 — the decider registry (D1).** The cascade is *still hardcoded* `match self.class()` in the
  `api/system/*` analyses; no `Decider` trait exists yet. M3 wraps it behind a `Decider` trait
  (polarity/cost/`admissible`) + a `Policy` whose default reproduces today's cascade exactly, with
  empty typed slots so B's generators are *born* as `Decider`s. M3 depends on M2 (done).
- **B2 — the cluster quotient keystone.** Not built. The union-find over preset/postset slices
  yielding the cluster count `c` is the cheapest, highest-leverage foundation; it unlocks the Rank
  Theorem (against the `rank` already built) and S/T-component decomposition. Build the keystone;
  leave the deciders it unlocks (B3–B6) for the sprint.

**C4 (verify-on-return) is partially deferred.** M2 built the checkers and proved they compose
with `accept`; **routing every public `analyze_*`/`is_*` through `accept` + a C1 checker at
runtime** is best done *with* the M3 registry (so the gate lives in one place, not scattered
across every analysis method). The ledger names which deciders each checker would cover when C4
lands. Until then, the firewall holds at the *type* level and in tests; the live analyses still
return their legacy result types.

**A3b / `petrivet-wasm`** stays deferred (the wasm crate targets a post-capability API: method
enums from A6/M2 and the `ReachabilityProof` witness redesign from M5/M6). Building it to force a
compile is the over-reach the boundary forbids. It is *armed* (the polarity type exists) and
*closed* once the witness shapes land.

---

## 4. Premise-level flags for your ratification (doctrine #5)

I changed **no** ratified essay. These are flags, not edits — for you to ratify, refine, or
reject. None blocks the foundation.

1. **`f` is two distinct numbers, and the foundation reports only the cheaper one.** The ledger's
   `f = 0.571` is the **structural** certifying fraction (certifying deciders ÷ all deciders). The
   thesis figure of merit (C5/G6) is the **empirical** `f` — the share of *accepted corpus
   verdicts* carrying a checked certificate, weighted by how often each decider fires. They can
   diverge sharply (a rarely-firing bare-boolean decider barely dents the empirical `f`). The
   docs use a single symbol `f`; consider naming the two (`f_struct-cert` vs `f_emp`?) so the
   thesis claim is unambiguous. *Flag only — I did not rename anything in the corpus.*

2. **The marking-equation Parikh checker is sound only on a sufficiency class, and the checker
   enforces that itself.** `ParikhVectorCert` rejects a Parikh witness on any class where the state
   equation is merely *necessary* (e.g. `General`), accepting only on `MarkedGraph`/`FreeChoice`/
   `Circuit`. This is the correct soundness fence, but it means the certificate's validity is
   *class-conditional* — worth an explicit line in the checkable-frontier chapter (the C7 table
   already carries the theorem ids). If you intend a Parikh certificate that is class-agnostic, it
   needs the accompanying trap-check witness (B6), which is out of this foundation's scope.

3. **B1a is closed for the *equality* (sign-free) infeasibility; the *signed* sub-case escalates
   rather than certifies.** The exact guard certifies `m'−m₀ ∉ col(C)` (sufficient for
   unreachability). When the system is col-space-feasible but the f64 LP claimed infeasible (a
   *sign-only* infeasibility), the path now **escalates to ILP/state-space** instead of returning
   the cheap `Unreachable`. This is sound and preserves the final boolean, but it trades a cheap
   verdict for an exhaustive one in that sub-case. A fully-exact *signed* certificate (an exact LP
   / the integer Farkas of the cutting-plane wall) is the C7 "wall" item — confirm you are content
   to leave the signed sub-case as honest escalation for now.

4. **The Bareiss *schedule* is deferred; the exact *result* is not.** The exact linear algebra is
   correct over `Rational` today; the fraction-free Bareiss elimination order (which keeps integer
   intermediates Hadamard-bounded) is the *performance* follow-on. If a corpus net overflows the
   `i128` kernel, the op returns `Overflowed` and the decider abstains honestly — no wrong answer,
   but a lost verdict. Whether to pre-empt that with Bareiss (or the bignum promotion the
   `checked_*` API is shaped for) is a *measured* call I have left open, per doctrine #6.

5. **Pre-existing breakage, untouched and flagged for transparency** (not introduced by this
   workflow; verified identical on the M1 baseline): (a) **20 doctest failures** — crate-root
   re-export drift (`petrivet::Net`/`Marking`/`PetriNet` unresolved at the root; the doctests use
   `petrivet::Net`, but `lib.rs` re-exports only *modules*). An X2/X3 cleanup, one re-export line.
   The M1 firewall doctests pass. (b) **2 `pnml_integration` failures** — missing nested MCC
   archive fixtures (`tests/fixtures/Champagne/PT/…`, `…/CopsAndRobbers/PT/…`); the `fixtures/`
   dir holds flat files only, so these are absent *data* artifacts, not code. (c) **`examples/playground.rs`**
   fails to compile (E0308, API drift) — untouched. None of these is in M0–M5's remit; each is a
   small, separable cleanup.

---

## 5. Delivery

All work is **local** on `foundation-brickwork` (commits `05a259e` M2, `8185179` M4, `5e54fa0`
M5, atop the M0/M1 commits). **No push, no PR** — the delivery sequence is Daniel's: you receive
the premise (the docs on `main`) and ratify it *first*; then this branch (where the foundation
heads), and decide when to sprint. The chisel is yours.
