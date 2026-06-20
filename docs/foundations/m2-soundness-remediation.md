# M2 soundness remediation — what was unsound, the sound fix, and the theory choices to ratify

*For Michael. This is the consolidated remediation record for the **foundation brickwork** passes
on branch `foundation-brickwork` (commits `3bb2a38` → `3a1d442`, atop the M0–M5 baseline `32eb583`).
An independent adversarial review built running counterexamples against the library's own state-space
oracle (`is_reachable`/`is_live`/`is_deadlock_free`/`is_coverable`/`is_bounded`) and found a set of
soundness holes the M2/M5 cut left open. Each was reproduced as a failing oracle cross-check, fixed,
and pinned by a regression test. This document is the lean index; the per-fix discernment lives in
[`foundational-design.md`](foundational-design.md) §4.4–§4.6′ (kept with the work, doctrine #8).*

Result-strength labels: **proven** = a regression test passes against the original net; **observed**
= measured/run; **argued** = reasoned, not mechanically discharged. The cardinal sin throughout is a
fabricated `Verdict::Proven`/`Refuted` — a verdict not backed by a `check` that actually re-establishes
the property. Abstention (a `check` returning `false` / `Inconclusive`) is honest.

---

## 1. What was unsound (the seven holes)

Two of these mint a **false `Proven`** (the cardinal sin); the rest rest a verdict on `f64` or an
unchecked integer accumulator. All were verified at the source against the oracle.

| # | Site | Defect | Direction |
|---|---|---|---|
| **B1** | `ParikhVectorCert::check` (checkers.rs) | accepted a balanced marking-equation `σ` as positive reachability on a **class fence** alone; the equation is *necessary, not sufficient* (needs liveness). Plus an **unchecked-`i64`** displacement accumulator that wraps in release. | **false `Proven`** (unsafe) |
| **B2** | `SiphonTrapCoverCert::check` (checkers.rs) | validated only the **exhibited** `(siphon, trap)` pairs, never the Commoner–Hack **universal**; and the one check backed *both* free-choice liveness and general deadlock-freedom (polarity-ambiguous). | **false `Proven`** (cardinal) |
| **M3** | `analyze_coverability` (coverability.rs) | negative `Uncoverable` rested on the `f64` microlp LP/ILP in both arms. | false `Uncoverable` |
| **M4** | `analyze_reachability` ILP arm (reachability.rs) | negative `Unreachable` from `f64` branch-and-bound, no exact guard. Also: the **live-marked-graph** arm short-circuited *before* the exact guard, resting both directions on the `f64` ILP. | false `Unreachable` |
| **M5** | `is_bounded()` / structural-boundedness predicates (boundedness.rs, core/net) | positive `bounded` rested on the `f64` `find_positive_place_subvariant`; the exact `BoundednessSubinvariantCert` was dead code. A separate AC-class arm asserted a **non-existent** "CHC ⇒ bounded" theorem. | false `bounded` (cardinal, AC arm) |
| **m-i** | `Rational::from_int` (rational.rs) | bypassed `new`'s `i128::MIN` guard. | unrepresentable value |
| **m-ii** | `Marking::total_tokens` / decider verdict paths | summed `u32` (wraps in release). Widened in the checker but **not** in the deciders, whose verdict paths read the wrapping public `u32` sum. | wrap on verdict path |

---

## 2. The sound fix applied to each

The contract held throughout: every `check` re-establishes the property against the **original**
`(net, m0, query)`, sharing no code with any generator **beyond primitive net access** — `try_fire`/
`is_enabled`, pre/post sets, marking, exact dot product, and (C1 exception) siphon/trap + minimal-siphon
enumeration. Re-using the net's *liveness/boundedness decision machinery* was treated as off-limits;
where a theorem precondition could not be re-derived from primitives, the certificate was re-grounded on
a property that **can** be, or the checker abstains. No verdict path rests on `f64`; exact arithmetic
never silently wraps (`checked_*` / `Rational`; overflow ⇒ abstain).

- **B1 — acceptance by *realization*, not by equation.** `ParikhVectorCert::check` now greedily fires
  `σ` as an actual firing sequence (`try_fire`/`is_enabled`) and accepts iff the reached `u32` marking
  **equals** the target. A realized `σ` *is* a firing sequence, so it certifies reachability on **any**
  class with nothing left to trust — no class tag, no liveness assumption, no `f64`, and no `i64`
  accumulator (the wrapping `displacement_on` is gone; the check compares actual markings). Capability
  is preserved on the theorem-backed (persistent) classes and *extended* to general nets where `σ`
  genuinely realizes. (§4.4 BLOCKER 1.)

- **B2 — re-derive the *universal*, and type the polarity.** `check` re-enumerates the original net's
  minimal siphons (`minimal_siphons`) and, for each, computes the maximal trap inside it
  (`maximal_trap_in`), confirming it is non-empty and marked under `M₀` — re-establishing the
  Commoner–Hack universal from primitives rather than trusting the cover to be complete. A typed
  `SiphonTrapClaim { Live, DeadlockFree }` carries which verdict the cert backs: `Live` is class-gated
  to free-choice (`NetClass::is_free_choice`, a structural pre/post-cardinality recomputation — not
  liveness machinery), `DeadlockFree` is accepted on any class. The dangerous conflation "CHC passed
  on a general net, therefore live" is now **unrepresentable** (doctrine #4). The exhibited cover is
  advisory: a malformed pair can only force abstention, never a false accept. (§4.4 BLOCKER 2, two
  holes.)

- **M3 — exact covering invariant.** `analyze_coverability` decides `Uncoverable` only via
  `covering_invariant_exact` (an exact ℚ P-sub-invariant scanned over the P-semiflow basis ± ), else
  it escalates to the Karp–Miller (integer/ω) coverability graph. The `f64` LP survives only as an
  inexact *filter*. (§4.4 MAJOR 3.)

- **M4 — exact reachability guard + realized live-MG arm.** `analyze_reachability` decides `Unreachable`
  only via `marking_equation_exact` (the exact ℚ Farkas dual `b ∉ col(C)`), else the exact state space.
  The live-marked-graph arm no longer short-circuits onto the `f64` ILP: it uses the ILP only as a
  *suggestion*, **realizes** `σ` into a replay-checkable `FiringSequence`, and on failure falls *through*
  (no early `return`) to the exact guard and the exact state space. (§4.4 MAJOR 4; §4.5 RESIDUAL 1.)

- **M5 — route the positive answer through the exact certificate; delete the false AC theorem.**
  `is_bounded()`'s positive structural answer routes through `BoundednessSubinvariantCert` (exact `y > 0`,
  `yᵀ·C ≤ 0` over ℚ), falling back to the exact coverability graph on validation failure. The public
  `Net::is_structurally_bounded` / `is_place_structurally_bounded` predicates were lifted off the bare
  `f64` LP onto exact certs (`BoundednessSubinvariantCert` / the new `PlaceBoundednessSubinvariantCert`),
  with the `f64` LP demoted to a suggester. The unsound `AsymmetricChoice ⇒ commoner_hack ⇒ bounded`
  arm — which asserted a theorem that does not exist — is removed; AC nets abstain to the exact path.
  The non-strongly-connected marked-graph arm, which asserted `Unbounded` from a structural-class fact
  it did not re-derive, now keeps only the sound positive direction and abstains to the exact graph
  otherwise. (§4.4 MAJOR 5; §4.6.)

- **m-i — `Rational::from_int` guarded.** Now `assert!(n != i128::MIN, …)`, upholding the same
  `i128::MIN` guard `new` enforces (no caller passes `i128::MIN`; the assert documents the contract).
  (§4.4 minor i.)

- **m-ii — `u64` accumulators on every token-sum verdict site.** `TokenConservationCert` sums in `u64`
  (`total_tokens_u64`); the decider verdict paths that read the public `u32` sum
  (`is_efficiently_reachable`, `is_efficiently_live`, the boundedness/safety deciders) route through
  non-wrapping `u64` accumulators (`wide_sum`/`total_tokens_wide`). (§4.4 minor ii; §4.5 RESIDUAL 2.)

**Exact-lens confirmation pass (§4.6′):** a dedicated re-review traced every verdict-producing path of
`is_coverable`/`is_reachable`/`is_bounded` and confirmed — positively — that none rests on `f64`. The
only `f64` reachable from a verdict path is suggestion/audit only (a rounded or near-boundary suggestion
can only *fail* an exact check, never mint a verdict). Three near-boundary/degenerate counterexamples pin
this.

---

## 3. Oracle-verified counterexamples + regression test names (all **proven**)

Every fix is pinned by a test that (a) reproduces the counterexample and asserts the checker/decider now
rejects/abstains, and (b) asserts a genuinely-valid witness is still accepted. All names below pass on
the current tree; the rejection tests were verified to **fail on the pre-fix code** by reverting.

| Hole | Counterexample (oracle-confirmed) | Rejection / abstention test | Capability (still-accepts) test |
|---|---|---|---|
| **B1** | non-live but strongly-connected marked graph; `{pd:1}+C·(t0,t1)={pc:1}` balances yet `{pc:1}` is `Unreachable` | `parikh_rejects_nonlive_marked_graph_unreachable_target`; `parikh_rejects_an_unrealizable_balanced_witness` | `parikh_accepts_a_realizable_witness_on_a_general_net` |
| **B2** (incomplete cover) | non-live net with one marked pair `{p0,p1}` whose cover omits unmarked offender `{q0,q1}`; `is_live == false` | `siphon_trap_cover_rejects_an_incomplete_cover` | `siphon_trap_cover_accepts_when_the_universal_holds` |
| **B2** (polarity) | `General` 4-place net, bounded + deadlock-free + every minimal siphon marked-trap-covered, yet `is_live == false` | `siphon_trap_cover_rejects_liveness_on_a_non_free_choice_net` | `siphon_trap_cover_accepts_deadlock_freedom_on_a_free_choice_net` |
| **M3** | two producers into a shared place + an unbounded pump; coverable target at a degenerate vertex | `adversarial_degenerate_coverable_target_not_reported_uncoverable` | (same test asserts `Coverable`) |
| **M4** (general arm) | rationally-feasible target with an integer-only gap | `adversarial_rationally_feasible_target_decided_by_state_space_not_f64`; `b1a_feasible_target_at_degenerate_vertex_not_reported_unreachable` | (decided by exact state space, both directions) |
| **M4** (live-MG arm) | live marked graph; reachable vs unreachable targets | `live_marked_graph_arm_is_sound_in_both_directions` | (asserts reachable ⇒ replayable `FiringSequence`) |
| **M5** (AC arm) | `{pa,pb,punb}`, `t_pump:{pa}→{pa,punb}`, `t1:{pa,pb}→{pa,pb}`; `class==AsymmetricChoice`, CHC holds, yet `Unbounded` and `punb` coverable to 1000 | `ac_chc_net_that_is_unbounded_is_not_reported_bounded` | `bounded_ac_net_still_decided_bounded_via_fallback` |
| **M5** (f64 predicates) | near-boundary `yᵀ·C` an exact net rejects | `net_is_structurally_bounded_rests_on_exact_certificate`; `net_is_place_structurally_bounded_rests_on_exact_certificate`; `place_boundedness_subinvariant_rejects_on_an_unbounded_place` | `is_place_bounded_on_general_net_uses_exact_path` |
| **M5** (non-SC MG) | a connected non-SC T-net | `non_sc_marked_graph_decided_by_exact_graph` | `sc_marked_graph_keeps_efficient_bounded_verdict` |
| **m-i** | `Rational::from_int(i128::MIN)` | `from_int_min_is_guarded` | `from_int_admits_every_representable_integer` |
| **m-ii** | (overflow pins on the wide accumulators) | `…overflow…` / `…wide…` pins (§4.5) | full lib suite reproduces decisiveness |
| **exact-lens** | near-boundary unbounded net | `adversarial_near_boundary_unbounded_net_not_reported_bounded` | (oracle confirms pump coverable) |

**Green gate (S4), current HEAD `3a1d442` (verified this pass):** `cargo build -p petrivet` clean (only
the 2 pre-existing `idx_arcs` dead-code warnings); `cargo clippy -p petrivet --all-targets` warning
histogram **byte-identical** to the pre-brickwork baseline `32eb583` (30 lib / 36 lib-test / 1 bench —
zero new, verified by a clean-checkout worktree diff this pass); `cargo test -p petrivet --lib`
**217/0**, `--test checker_invariants` **9/0**, `--test firewall_probe` **2/0**. Pre-existing-and-unchanged
(left per the brief, not masked): the ~20 doctest re-export-drift failures, the 2 `pnml_integration`
missing-fixture failures, the `mcc-tests` `library_correctness` missing-`oracle/models` failure, and the
`examples/playground.rs` compile error — all confirmed identical on the baseline.

---

## 4. Theory choices to ratify (the `flag_for_michael` items) — **separated from the fixes above**

These are *premise-level* choices the fixes rest on. The Petri-net theory is yours; the fixes are
sound **given** these readings, but the readings themselves are for you to ratify, refine, or correct.
None blocks the foundation. Each is recorded with its discernment in `foundational-design.md` §4.4–§4.6.

**T1 — B1 realization completeness (the persistence argument).** Acceptance-by-realization is strictly
sound on any class. The *capability* claim — that greedy maximal realization is **complete** wherever
the marking equation is realizable on the classes the certificate targets — rests on **persistence**:
marked graphs are persistent; free-choice is cluster-persistent, so a greedy maximal strategy realizes
any realizable `σ` by the diamond property. **Ratify the persistence/completeness argument**, or scope
where a full realizability search (or a liveness-precondition carried as witness data) is wanted. A
non-persistent net where greedy stalls on a *realizable* `σ` would be a **capability** (not soundness)
regression there, handled by honest abstention.

**T2 — B2 verdict-meaning split (the one genuine design choice).** Two sound options:
- **(recommended)** carry `SiphonTrapClaim { Live, DeadlockFree }` and class-gate `Live` to free-choice
  — preserves the FC-liveness capability the live decider already mints, while making the polarity
  conflation unrepresentable; **or**
- demote the cert to deadlock-freedom-only universally (simpler, strictly weaker, safe fallback).

The recommended form rests on two standard reductions being the intended reading of **Murata Thm 12 /
Primer 5.17**: (i) proper-siphon universal ⇔ minimal-siphon universal; (ii) maximal-trap-marked ⇔
∃ marked trap. And on the **empty-minimal-siphon convention**: a net with no proper siphon **vacuously
satisfies** the universal — ratify *accept* (vacuous-truth, textbook-correct, recommended) vs a
conservative *abstain*. Confirm also that `minimal_siphons` / `maximal_trap_in` count as the C1-blessed
primitive (read as **yes**) while `commoner_hack_criterion` (the decision) stays off-limits.

**T3 — B2 cost relocation (a frontier-map correction, not a footnote).** Re-deriving the universal is
**exponential-in-|P|** worst case, and that cost now sits **inside the trusted accept path** — there is
no compact local certificate that a place-set list is the *complete* minimal-siphon set. The C7
free-choice-liveness cell must read **"exponential-but-exact (no compact completeness certificate
exists)"**, not "polynomial in the exhibited cover." Ratify **exact-now / budget-guard-later** (a
cancellation-token guard returning `false`/Inconclusive when over budget is the deferred D1 follow-on,
not yet wired). The `f_struct` floor measurement is unaffected (G4a already excludes CHC).

**T4 — the polarity-coherence gate on `accept` (the deferred minor) — NOW IMPLEMENTED.** The principled
cross-cutting fix is an A6 `Polarity` on the `Certificate` surface so `accept` refuses a cert whose pole
is incoherent with the candidate verdict's polarity. This is **no longer deferred**: it landed
post-remediation (`foundational-design.md` §4.7). `Certificate::polarity() -> Polarity` is now required
(each witness declares `ProveYes`/`ProveNo` — the six checkers agree with the `ledger.rs` column), and
`accept` promotes only when `cert.polarity()` agrees with the candidate's pole (an `Exact` cert matches
either), collapsing a mismatch to `Inconclusive(NotApplicable)` ahead of `check`. The `SiphonTrapClaim`
field still closes the liveness-vs-deadlock-freedom reading at the C1 layer; the gate adds boundary-level
defence in depth, so a future M3 decider cannot wire a passing checker to a wrong-signed candidate.
Pinned by `accept_enforces_polarity_coherence` and the corrected
`accept_is_check_gated_for_all_polarity_outcome_combinations`. **To ratify:** confirm that a certificate
is single-pole (`ProveYes`/`ProveNo`, never `Exact`) is the right model, and that an `Exact` cert (none
exist yet) being compatible with either candidate pole is acceptable.

**T5 — M3 covering-guard completeness (a lean point).** The exact covering guard scans only the
P-semiflow basis and its negation; a separating sub-invariant that is a non-trivial non-negative
*combination* escalates to the (sound) coverability graph rather than being caught on the fast path.
Confirm this lean point, or schedule the exact covering-LP (Farkas-for-inequalities / covering-LP
duality) as B-epic work.

**T6 — M5 rationalization & abstain-to-`false`.** The `f64`→`Rational` suggestion is rationalized by
rounding and a few small scalings; a valid exact sub-invariant with an *unusual denominator* could be
missed and fall back (to the graph for `is_bounded`; to `false` for the marking-free structural
predicates, which have no graph fallback). This is a **capability** miss, never a soundness hole.
Confirm the lean rationalization, or schedule an exact structural-boundedness LP. Also confirm
**removing the AC boundedness fast-path** (there is no CHC ⇒ bounded theorem for AC nets) and the
**non-SC marked-graph abstention** (the structural-iff `Unbounded` verdict is almost certainly sound but
is asserted from a class fact we do not re-derive; we abstain to the exact graph rather than assert it).

**T7 — exact-closure escalation behavior (the cross-cutting posture).** Every exact guard's failure mode
is **escalate, never fabricate**: an `i128` kernel overflow returns `Overflowed`/`None` and the caller
falls to the exact state-space/coverability graph; a near-boundary `f64` suggestion that fails its exact
re-check abstains. The deferred performance pre-emption (fraction-free **Bareiss** schedule, or bignum
promotion the `checked_*` API is shaped for) is a *measured* call left open per doctrine #6 — confirm
honest escalation is acceptable for now.

**T8 — disposition of the now-dead FC positive proof type.** `LiveBoundedFreeChoiceMarkingEquationWith‐
TrapCheck` (and the `MarkingEquationNoIntegerSolution` / `LiveMarkedGraphMarkingEquationIntegerSolution`
proof variants) have no producer after the realization rewrite. Recommendation: **KEEP** as doctrine-#7
blueprint markers (the genuinely-novel sound-from-primitives FC positive path composes bounded-S-component
cover + CHC-marked-trap cover + no-unmarked-trap-on-U), vs **delete-now** for leanness (doctrine #9).
Your call.

---

## 5. Net standing after remediation

- **Two false-`Proven` checkers made sound:** `ParikhVectorCert` (realization, any class) and
  `SiphonTrapCoverCert` (re-derived universal + typed polarity fence). Zero live-capability loss (the
  FC positive proof type had no producer).
- **All verdict paths are `f64`-free:** `Uncoverable`, `Unreachable` (both arms incl. live-MG), and
  positive `bounded`/structural-boundedness route through the exact ℚ kernel, escalating to the exact
  state-space/coverability graph on failure. The only surviving `f64` is suggestion/audit, provably off
  every verdict path.
- **Arithmetic discipline closed on the verdict path:** no unchecked `i64`/`u32` accumulator decides a
  verdict; `Rational::from_int` upholds the `i128::MIN` guard.
- **The trusted base did not silently grow.** Capability was restored *soundly* (frontier cells stay
  built; B2 adds a negative-Farkas checker cell), not by disabling checkers. Where full capability is
  not soundly achievable from primitives (the FC liveness precondition), the certificate re-grounds on a
  re-derivable property or abstains.

Delivery: **local** commits on `foundation-brickwork` (`3bb2a38` → `3a1d442`). **No push, no PR.**
