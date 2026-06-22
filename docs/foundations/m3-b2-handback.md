# HANDBACK — petrivet foundation (Workflow 2: M3 + B2)

*For Michael. The second workflow of the foundation: the **M3 decider registry** and the **B2
cluster-quotient keystone**, built on the Workflow-1 base (M0–M2 + M4/M5 + A6). This records what was
built, what is **proven** (with test names), the runway left, and the premise-level flags for your
ratification. Delivered as the development PR (stacked on the docs PR).*

> **What this advances in your library** (mapped to your README features):
> - **Structural analysis without state-space exploration** — the B2 cluster keystone supplies the
>   missing definition for the `c` your `class.rs` Rank-Theorem doc already names; the decider seam
>   widens your "System too complex for state-space exploration?" feature.
> - **Intelligent per-class dispatch** — M3 turns "petrivet intelligently chooses the most efficient
>   algorithm for your net's structure" into a first-class, reorderable seam, **purely additive**
>   (your five cascade methods are untouched and serve as the regression oracle).
> - **Correctness** — the exact-arithmetic guard and the bug fixes (incl. two false-`Proven` holes
>   and the `m₀`-deadlock) protect the answers your analyses return; your fast `f64` path is kept as a
>   *suggester*, with an exact guard added so a near-boundary float can't mint a wrong verdict.
>
> The *thesis-contribution framing* below (the certificate-and-checker as the contribution, the
> certifying fraction `f`) is a **proposal you own** — accept, refine, or reject. The library work
> stands either way.

> **Read the Workflow-1 handback first** ([`HANDBACK.md`](../../HANDBACK.md)) for the base this
> stacks on, and the brief this discharges ([`m3-b2-handoff.md`](m3-b2-handoff.md)). The discernment
> behind each piece is in [`foundational-design.md`](foundational-design.md) **§4.8 (B2)** and
> **§4.9 (M3)**; the authoritative gates are `BACKLOG.md` (M3, and the B2 portion of M6).

Result-strength labels: **proven** = a gate test passes; **observed** = measured/run; **inferred**
= reasoned, not executed end-to-end.

---

## 1. What was built

### B2 — the cluster-quotient keystone *(commit `24f914f`; design §4.8)*
A new self-contained module `core/analysis/cluster.rs`:
- **`clusters(&DenseNet) -> ClusterPartition`** — a flat union-find (path-halving + union-by-rank)
  over the node universe `places ∪ transitions` (transitions offset by `place_count`, since the two
  dense index spaces overlap at 0), driven by the **consumption** relation only (`p ∈ •t`). The
  count `c` is the number of distinct roots over the whole node set, singletons included.
- **`rank_cluster_relation(&DenseNet)`** — pairs `c` with the exact M5 incidence rank
  (`incidence_over_rationals(net).rank()`); `holds()` tests `rank + 1 == c` (the overflow-safe form
  of `rank == c − 1`).

The cluster **definition is authored here** — `class.rs:301` names `c` in the Rank Theorem but
defines no cluster and carries no example. The consumption-arc (Desel–Esparza) choice is load-bearing
and was *validated, not assumed*: the all-arc alternative gives a different `c` under which
`rank == c − 1` fails. **B2 registers no decider** (per the boundary); the S/T-component deciders it
unlocks (B3–B6) are left for the sprint.

### M3 — the decider registry (D1) *(commits `f47a451` scaffolding+reachability, `c93140b` the four remaining drivers; design §4.9)*
The cascade made a value. `api/model/registry.rs` holds the property-agnostic abstraction:
`trait Decide<P,N>` (name / polarity / cost_class / admissible / run), `trait Policy` + `DefaultPolicy`
(keeps the admissible deciders in **registration = cascade order**; does *not* re-sort by cost), a
`Driver` (first decided wins, else `Inconclusive`), `CostClass`, and `Budget` (the D6 cancellation
seam, `unbounded()` by default so the cascade is reproduced exactly). `BareBooleanCert` lets a
trusted-base decider route uniformly through `accept` without inventing a certificate it does not
have (its `check` returns `true`; the A6 polarity gate still applies; the ledger keeps counting it
bare-boolean, so `f` is not inflated).

`api/model/registry/{reachability,coverability,boundedness,liveness,deadlock_freedom}.rs` hold the
five per-property decider lists. Each is **a few cheap structural deciders + one exhaustive decider
that delegates to the public `analyze_*`/`is_*`/`try_*` cascade method** — so exact reproduction is
*structural*, not re-implemented (the five cascade files are untouched and remain the `[REGRESS]`
oracle). The cert-gated cheap deciders self-correct (a wrong proposal fails its `check` → `Inconclusive`
→ the cascade decides); the trusted ones rest on a named theorem and carry `BareBooleanCert`. Where
the cascade already emits a re-checkable witness the registry routes through the **real** C1 checker:
firing-word positives → `FiringSequenceCert`, the S-net refuter → `TokenConservationCert`, and
**free-choice liveness + general deadlock-freedom thread the Commoner–Hack cover into
`SiphonTrapCoverCert{Live}`/`{DeadlockFree}`** (the certifying showcase, wired not deferred).

M3 is **purely additive**: the drivers run alongside the cascade; the public `is_*`/`analyze_*`
methods are not yet delegated to them (see Runway).

---

## 2. What is PROVEN (gates, with test names)

Full suite (workflow-2 tip): **`petrivet` lib 233/0** (232 at M3 + the §4.10 soundness-fix
regression), doctests **25/0**, `checker_invariants` **9/0**, `firewall_probe` **2/0**,
**`mcc-tests` `cascade_baseline` 1/1**. Build clean (only the 2
pre-existing `idx_arcs` warnings); `cargo clippy -p petrivet --all-targets` warning histogram
**byte-identical to the foundation-code baseline** (verified by a stash diff; the new files are
clippy-clean under pedantic + nursery).

### B2 gates (M6 keystone portion)
| Gate | Status | Proof (tests, in `cluster.rs`) |
|---|---|---|
| `[PROP]` cluster == flow-components | **proven** | `clusters_match_independent_flood_components` (union-find vs an independent BFS flood over the same graph) |
| `[ORACLE]` `rank == c − 1` agrees with state-space on well-formed nets | **proven** | `well_formed_nets_satisfy_rank_equals_clusters_minus_one` (oracle-confirmed live ∧ bounded ⇒ `rank == c−1`); `cluster_count_and_rank_match_hand_computed_table` (the hand-computed `(c, rank)` table — the cluster-definition validator) |
| the relation is **non-trivial** (not vacuously true) | **proven** | `rank_cluster_relation_is_discriminating` (a connected net with `rank ≠ c−1`) |

### M3 gates (D1)
| Gate | Status | Proof (tests) |
|---|---|---|
| `[REGRESS]` the default-policy driver returns identical verdicts to the cascade across the corpus | **proven** | `mcc-tests cascade_baseline::default_drivers_reproduce_the_cascade_over_the_corpus` (all 5 drivers vs the cascade over the in-tree fixtures; identity reach/cover on every fixture, the query-free properties full-cascade on the small fixtures) |
| `[REGRESS]` (unit) per property, the driver pole equals the cascade on built nets | **proven** | `registry::{reachability,coverability,boundedness,liveness,deadlock_freedom}::*::driver_reproduces_cascade_pole` (incl. the three-valued liveness mapping: live → Proven, non-live → Refuted, unbounded → Inconclusive) |
| `[PROP]` over random admissible orderings the accepted verdict is invariant (policy-independence) | **proven** | `registry::*::accepted_pole_invariant_over_*orderings` (exhaustive Heap's-algorithm permutation of the admissible deciders, the house no-`proptest` convention) |

### Standing invariants
- **S1** — every accepted `Proven`/`Refuted` routes through `accept` (the cert-gated checkers
  re-validate against the original net; the cascade verdicts the drivers reproduce already satisfy
  S1 from Workflow 1). **proven.**
- **S2** — no decisiveness regression: the corpus `[REGRESS]` + unit `[REGRESS]` show the drivers
  decide exactly what the cascade decides. **proven.**
- **S3** — the trusted base did not grow: the registry uses `BareBooleanCert` for the bare rows and
  the **ledger is untouched** (`ledger::trusted_base_is_reported_and_non_increasing` still pins 3
  bare / 4 certifying, `f = 4/7`). **proven.**
- **S4** — clean build; clippy histogram byte-identical to baseline; full suite passes. **proven.**

### Independent adversarial verification
*Per doctrine #1 (a milestone's author cannot verify it — M2 once shipped false-`Proven` holes that
passed their own tests), an independent multi-agent review built **running** counterexamples against
the state-space oracle across four dimensions, each in an isolated worktree. **Three came back fully
clean; one found a real soundness hole, now fixed.***

- **B2 cluster/rank — sound.** A verifier independently recomputed `c` (a BFS flood, distinct from the
  module's union-find) and the incidence rank (a fraction-free **Bareiss** integer elimination, distinct
  from the library's `Rational` RREF, self-checked against 8 known-rank matrices) and confirmed agreement
  on every net; pinned the consumption-arc definition by building nets where the all-arc partition gives
  a different `c` (the implementation matched the consumption value); confirmed the necessary direction
  and the non-trivial discriminator. No divergence, no hole.
- **Reachability / coverability / boundedness — sound.** 200+ adversarial cases (sweeps over a 5-net,
  multi-class corpus × generated targets): the driver pole equalled the cascade pole *and* the
  hand-derived true answer on every case, **including the cardinal-sin directions** — a non-SC S-net with
  an equal-sum unreachable target (no false Proven), the §4.6 asymmetric-choice-CHC unbounded net (no
  false `bounded`), and the `u32` token-sum overflow (no false reachable).
- **Trust boundary + S2/S3 — intact.** The ledger is unmoved (`trusted_base_size()==3`,
  `certifying_count()==4`, `f==4/7`); all 15 `BareBooleanCert` pairings are polarity-coherent (audited
  and proven by a decisiveness battery — every driver *decides* whatever the cascade decides, never a
  spurious `Inconclusive`); the firewall compile-fail doctests hold; the registry adds no public
  `Proven`/`Refuted` constructor.
- **Liveness / deadlock-freedom (CHC) — one hole found and FIXED.** The `FreeChoiceCHCLive` double fence
  (the decider is class-gated to free-choice *and* `SiphonTrapCoverCert{Live}` re-checks `is_free_choice()`
  and re-derives the universal) held — no false `Proven(live)` was mintable, including on the BLOCKER-2
  non-FC-CHC-non-live net. But the verifier found a **false `Proven(DeadlockFree)`**: when `m₀` itself is
  a total deadlock (the empty-marked cycle), the state-space explorer's `search` never tested the seed
  marking, so `is_deadlock_free()` fabricated a `true` and the driver minted `Proven(DeadlockFree)`. This
  is a **cascade (M0-era) bug** the driver faithfully reproduced (S2 held). **Fixed** at the source —
  `deadlocks()` now tests the seed first, exactly once — with the oracle-backed regression
  `deadlock_freedom::tests::initial_marking_deadlock_is_detected` and the driver-level pin in the M3
  registry test. Full record + the flag for you: **§4.10**.

*Net: the registry/certificate architecture verified sound on every dimension; the one hole was a
pre-M3 explorer bug surfaced by M3's verification and corrected. The four agents' full adversarial test
suites (B2 cluster/rank, the structural drivers, the trust-boundary battery) ran green in their
worktrees and are preserved in the workflow transcript for permanent integration if you want them in CI.*

---

## 3. The runway left

The registry is built so a future structural generator **slots in as one `Decide` impl at its
cascade position with its certificate — no change to the trait, policy, or driver**. Concretely:

- **C4 / per-row certifying completion.** The registry is the first place deciders route through
  `accept` at runtime. FC-liveness and deadlock-freedom CHC verdicts are now *genuinely certifying*.
  The remaining structural verdicts carry `BareBooleanCert` (an honest under-claim — the ledger is
  unchanged): the strongly-connected S-net reachability (realize the firing word), boundedness (emit
  the exact `BoundednessSubinvariantCert` sub-invariant — the A4/B3 completion), and the
  ω-coverability positive (the pumping-cycle witness — C7). Each flips its ledger row to `Certifying`
  when its generator emits the witness.
- **Public-API delegation.** The drivers run *alongside* the cascade. Delegating the public
  `is_*`/`analyze_*` methods to the driver (so the registry is the single decision path) is a later,
  separately-gated step — kept out of M3 so the `[REGRESS]` equivalence is the ground truth before
  any rewiring.
- **B2 consumers (the sprint).** The cluster partition + `c` feed the S/T-component deciders (B3–B6)
  and the full Rank-Theorem characterization (the positive S/T-invariants + the proper-siphon
  condition close the gap from the *necessary* relation to the `⇔`). Each is a new `Decide` impl in
  the right per-property list.
- **The learned policy (Epic D).** `cost_class` is carried as metadata but the default policy
  ignores it; ordering by cost is the SATzilla-style sequel, gated on a measured SBS→VBS gap.

---

## 4. Premise-level flags for your ratification (doctrine #5)

I changed **no** ratified essay. These are flags, not edits.

1. **B2's `rank == c − 1` is gated as NECESSARY, not the equivalence the BACKLOG item states as
   shorthand.** The Rank Theorem characterizes a *well-formed* (live ∧ bounded) free-choice system by
   **four** conditions — positive S-invariant, positive T-invariant, `rank = c − 1`, every proper
   siphon marked — so `rank = c − 1` is one necessary condition, not `⇔`. Hand-built witnesses pin
   both directions: an unbounded FC net satisfies `rank = c − 1` yet is not well-formed (the converse
   fails); a self-loop net has `rank ≠ c − 1` (the relation is non-trivial). The keystone soundly
   establishes `c` and the necessary direction; the full `⇔` needs the S/T-invariant + siphon
   conditions — exactly the deciders B2 deliberately does not build. **Confirm** the intended keystone
   claim is the count + the necessary relation (the defensible reading), with the `⇔` deferred to the
   milestone that lands those deciders.

2. **Certifying coverage is partial *by design*, and the under-claim is honest.** FC-liveness and
   deadlock-freedom are genuinely certifying at runtime; reachability's strongly-connected S-net,
   boundedness, and the ω-coverability positive route through `BareBooleanCert`. This does **not**
   inflate `f` (the ledger is unchanged and still pins 4/7). It is the conservative choice — a
   `BareBooleanCert` claims nothing it cannot check. **Confirm** you want the per-row C4 completions
   scheduled with their generators (§3) rather than forced now.

3. **The registry granularity follows the ledger, not a maximal per-arm explosion.** Each property
   lifts the ledger's rows (a structural positive, a refuter where one exists, the certifying CHC
   decider) plus the exhaustive fallback. This keeps the registry legible and gives clean slots; a
   finer per-arm decomposition is available if the sprint wants separately-orderable structural arms.

4. **The corpus `[REGRESS]` samples by size to stay bounded.** Driver↔cascade equivalence is
   structural-by-construction (the exhaustive decider delegates to the cascade), so the gate compares
   identity reachability/coverability on every fixture and the query-free properties full-cascade only
   on the small fixtures (≤ 33 nodes) — large/state-explosive MCC nets (e.g. `philo`) would only
   re-pay the cascade's own exponential cost with no extra assurance. The state-space equivalence is
   additionally pinned by the per-property unit `[REGRESS]` tests. **Confirm** this sampling is
   acceptable, or schedule a budgeted full-corpus pass once the `Budget` seam is wired into the
   cascade (the D6 cancellation work).

5. **Pre-existing breakage, untouched and verified identical to the baseline** (not introduced here):
   the `examples/playground.rs` `E0308` (API drift) and the `pnml_integration`/`library_correctness`
   missing-fixture failures. Each is a small, separable cleanup outside M3/B2's remit.

---

## 5. Delivery

All work is **local** on `workflow-2` (off `foundation-code`): `24f914f` (B2), `f47a451`
(M3 scaffolding + reachability), `c93140b` (M3 the four remaining drivers + corpus `[REGRESS]`).
**No push, no PR** — the delivery sequence is Daniel's. The commits are decomposed to be read as a
narrated argument: B2 the keystone, then M3 as the abstraction-plus-reference, then the rest applied
uniformly. The chisel is yours.
