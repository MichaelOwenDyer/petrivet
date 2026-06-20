# HANDOFF — Workflow 2: M3 (decider registry) + B2 (cluster keystone)

*For the next agent thread and its workflow. The foundation (M0–M2 + M4/M5,
remediated, with the A6 polarity gate) is laid, green, and verified. Your job is
two well-scoped milestones — **M3** (the decider registry) and the **B2** cluster
keystone — on a base built so that "adding a generator is writing one `Decider`
and its certificate, nothing structural left to invent." This brief plus
`working-doctrine.md` and `BACKLOG.md` is everything you need.*

---

## 0. The base you work from (two stacked branches, local only)

- **`foundation-docs`** — the docs standalone branch: the vision corpus, the
  working doctrine, the backlog, `foundational-design.md` (with every build
  record), the M2 remediation report, the baseline-test and doctest reports, and
  **this handoff**. Code is at the `f3356bc` baseline here — it is the *clarified
  planning/design base*.
- **`foundation-code`** — the code branch, **stacked on `foundation-docs`**: the
  full foundation implementation (M0–M2 + M4/M5 + remediation + A6). **This is
  your working branch.** `git diff foundation-docs..foundation-code` is the entire
  foundation code delta; stack your M3 + B2 commits on top of it.

Everything is **local**. Do **not** push, do **not** open a PR. The delivery
sequence is Daniel's: Michael ratifies the premise (the docs) first, then takes
the code branch.

**Green gate (must hold at every step):** `cargo build -p petrivet` and
`cargo clippy -p petrivet --all-targets` clean with **no new warnings** (the crate
opts into `clippy::pedantic + nursery + cargo` in `lib.rs`; honor it); `cargo test
-p petrivet --lib --doc` and the integration tests (`checker_invariants`,
`firewall_probe`) pass. Current baseline: lib **218/0**, doctests **25/0**,
`checker_invariants` 9/0, `firewall_probe` 2/0; the only build warnings are the 2
pre-existing `idx_arcs` dead-code ones.

---

## 1. What is already done (so you don't rebuild it)

- **M0** — soundness defects fixed: the two `Some(false)` stubs → honest
  abstention (`is_covered_by_s_components` is `Option<bool>` returning `None`; the
  marked-graph liveness arm is a real computation); PNML fidelity → hard
  `PnmlConversionError` (`>u32::MAX` markings, non-unit-weight arcs); the
  `STRUCTURAL_REDUCTION` mis-tag removed; the `f_struct` **floor** measured
  (`mcc-tests`). Two engine bugs were also found and fixed here: the inert
  `fire_unchecked` (it discarded its token delta) and a scrambled petgraph mirror
  in `build()` (`HashMap`-order node labels corrupted `circuits()`/SCC liveness).
- **M1** — the `model` module (`api/model/`): `Verdict<P,N>` with a type-distinct
  `Inconclusive(Abstention)` and **private `Proof`/`Refutation` payload fields**
  (an un-checked decided verdict is *un-name-constructible* even downstream — three
  `compile_fail` doctests prove it; `Verdict` is `Serialize` but **not**
  `Deserialize`). `accept` is the **sole** public constructor of `Proven`/`Refuted`.
- **M4/M5** — the exact-rational core (`core/analysis/`): `Rational` over `i128`
  with **overflow detection, never wrap** (`checked_*` returning `Result`); `Matrix`
  with `rank`, `right_kernel`, `left_kernel`, `solve`, `farkas_certificate` (exact);
  B1a closed (negative `Unreachable`/`Uncoverable` re-derived over ℚ, microlp demoted
  to a filter); A4 boundedness certificate.
- **M2 + remediation + A6** — six `Certificate` checkers in `api/model/checkers.rs`,
  each re-establishing its property against the **original** net (sharing no code
  with generators beyond primitive access); the trusted-base **ledger**
  (`api/model/ledger.rs`, reports `f`); the C6 serde **format**; the C7 **frontier**.
  Two cardinal-sin holes found by an independent oracle review were remediated
  (`ParikhVectorCert` now **realizes by replay**, sound on any class;
  `SiphonTrapCoverCert` re-derives the Commoner–Hack **universal** and carries a
  typed `SiphonTrapClaim { Live, DeadlockFree }` with `Live` class-gated to
  free-choice). The three legacy `f64` verdict paths were closed over ℚ. **A6**: the
  `accept` gate now enforces **polarity coherence** (`Certificate::polarity()` must
  match the candidate's pole, else `Inconclusive`).

Full narrative + rationale: `foundational-design.md` (§1–§10, with the §4.x build
records and §4.7 for A6) and `m2-soundness-remediation.md`.

---

## 2. What you build — the gates (verbatim from `BACKLOG.md`)

### M3 — Decider registry (item D1). Depends: M2 (done).
> **Items:** D1 (registry with polarity/cost/`admissible`; a `Policy` whose default
> reproduces today's cascade **exactly**).
> **Gates:** `[REGRESS]` the default-policy driver returns identical verdicts to the
> current cascade across the corpus · `[PROP]` over random admissible orderings the
> accepted verdict is invariant (the soundness theorem, tested — the *enabling*
> property, not the headline).
> *B's generators are born as `Decider`s.*

The shape (from `foundational-design.md` §F5): `trait Decider { fn polarity() ->
Polarity; fn cost_class() -> CostClass; fn admissible(NetClass) -> bool; fn run(net,
query, budget) -> Outcome<Verdict>; }`; a `Policy` whose default reproduces the
current hand-coded `match self.class()` cascade **exactly**; a `Driver` that selects
admissible deciders, orders them by the policy, gates on the certificate, and returns
the first `Proven`/`Refuted` or else `Inconclusive`. Leave **empty, typed slots** so
B's future generators slot in as `Decider`s with no structural change.

### B2 — the cluster-quotient keystone (the "…and then some"). Depends: M5 (rank), M2.
> **Items:** B2 (union-find clusters → `c`; `well_formed ⇔ rank(C) == c−1`).
> **Gates:** `[PROP]` cluster = flow-components · `[ORACLE]` `rank == c−1` agrees with
> state-space on FC nets.

**Scope it to the keystone only:** the union-find over the preset/postset slices →
the cluster count `c`, and the Rank-Theorem relation `rank(C) == c−1` as a tested
invariant (you have exact `rank` from M5). **Do NOT** build the S/T-component
*deciders* it unlocks (B3–B6) — leave the estate for Michael (§5).

---

## 3. Why this is now ridiculously easy (the seams are ready)

- **The registry's data model already exists.** `api/model/ledger.rs::trusted_base()`
  already enumerates every fast `is_efficiently_*` decider with its **polarity**
  (`ProveYes`/`ProveNo`/`Exact`) and **kind** (`Certifying`/`BareBoolean`). M3 is
  largely "make the ledger executable": lift each enumerated decider into a `Decider`
  impl and a `Policy` that fires them in the current order.
- **The trust boundary is done and enforced.** A new decider does not invent any
  soundness machinery: it proposes a `Decided` candidate and routes it through
  `accept(candidate, &cert, net, m0, query)`. `accept` gates on the certificate's
  `check` against the original net **and** on polarity coherence (A6). So wiring the
  cascade through `accept` (C4, verify-on-return) is now safe — a wrong-signed or
  unchecked verdict cannot escape.
- **The checkers are done.** When a decider's witness is one of the existing shapes
  (firing sequence, realized Parikh vector, token conservation, siphon/trap cover,
  positive sub-invariant), the matching checker validates it — no new checker needed
  for M3.
- **B2's inputs are ready.** `rank` is in `core/analysis/exact_matrix.rs` (exact);
  the partition is a near-linear union-find over the stored
  `DenseNet.{preset_t,postset_t,preset_p,postset_p}` (`UniqueSortedSlice` over dense
  indices) in `core/net/`. `core/class.rs` states the Rank Theorem (`rank C = c−1`).

---

## 4. The map (where to work)

- **The cascade M3 wraps:** `api/system/{reachability,coverability,boundedness,
  liveness,deadlock_freedom}.rs` — each `is_efficiently_*`/`analyze_*` dispatches on
  `match self.class()`. The **default `Policy` must reproduce this exactly** (S2: no
  decisiveness regression — verify against the committed baseline).
- **The model layer:** `api/model/{verdict,query,certificate,accept,checkers,ledger,
  format,frontier,mod}.rs`. `Decider`/`Policy`/`CostClass` are new types — `model` (or
  a new `model::registry`) is their natural home; `Polarity` already lives in
  `api/model/frontier.rs`.
- **The exact core (for B2):** `core/analysis/{exact_matrix,rational,incidence}.rs`.
- **B2 inputs:** `core/net/` (`DenseNet`, the preset/postset slices),
  `core/unique_sorted_slice.rs`, `core/class.rs`.
- **The oracle (for verification):** the public `is_reachable`/`is_live`/
  `is_deadlock_free`/`is_bounded` + the `mcc-tests` harness (state-space ground truth).

---

## 5. The boundary — what NOT to build (do not capture the flag)

Lay the brickwork; leave the estate for Michael.

- **B2 keystone only** — the quotient + `c` + the `rank == c−1` invariant. **Not** the
  S/T-component deciders (B3–B6), **not** the FC/T-net structural deciders, **not** the
  class-agnostic deciders (B10/B11), **not** the WSTS zoo, **not** the full
  `petrivet-observe` crate, **not** the thesis rig / coverage *result* (the `f_struct`
  floor is the starting line, never a claimed headline), **not** certified reductions
  (Epic F), **not** the learned ladder (D5–D8), **not** the Φ residuals (Epic H).
- **Do not re-litigate the open theory ratifications** — they are Michael's calls.
  They are: T1–T8 in `m2-soundness-remediation.md` §4 (realization completeness; the
  `SiphonTrapClaim` split + empty-siphon convention; the exact-closure lean points;
  the A6 single-pole model), and the vending-machine S-net correction flagged in
  `doctest-modernization.md`. Respect them; if you find a *new* premise-level issue,
  **flag it in your handback** — never silently rewrite the ratified essays
  (`docs/essays/*` are off-limits).

---

## 6. Hard-won lessons (the rules that keep this sound)

These are paid for in this thread's mistakes — follow them and your work stays sound.

1. **Adversarially verify against the state-space oracle — always.** M2's checkers
   passed their *own* tests yet shipped two cardinal-sin holes (false `Proven`); only
   an independent review that *built running counterexamples against the oracle* caught
   them. Tests written by the same agent that wrote the code are **not** independent
   verification. For each decider/certificate, build the counterexample and confirm the
   verdict agrees with `is_reachable`/`is_live`/… on small bounded nets.
2. **A class tag is never a sufficiency proof.** The Parikh hole was exactly
   "`NetClass` membership ⇒ the theorem applies" — but the marking equation is only
   *necessary* without liveness/boundedness/trap preconditions. Re-derive the property
   (or realize the witness) from primitives; do not trust the classifier as a stand-in
   for a theorem's hypotheses.
3. **No `f64` on any verdict path.** Floating point may *suggest* (a filter), never
   *decide*. The exact `Rational`/`Matrix` kernel is there; route verdicts through it.
4. **A new `Decider`/`Certificate` declares its polarity and routes through `accept`.**
   The A6 gate refuses a wrong-signed pairing. `Certificate::polarity()` has no default
   — declare `ProveYes`/`ProveNo` consciously (a single witness is one pole, never
   `Exact`).
5. **Let Rust carry the invariants.** Make illegal states unrepresentable (the
   firewall is the model: private `Proof`/`Refutation`, `accept` the sole door,
   `SiphonTrapClaim` making the polarity conflation unrepresentable). Prefer a compile
   error to a test.
6. **Every change = fix + the regression test that would have caught it + the
   rationale kept in `foundational-design.md`** (doctrine #8). A milestone is done when
   its gates are *proven* (a `[PROP]`/`[ORACLE]`/`[REGRESS]` test passes), not when one
   example passes.
7. **The default policy reproduces today's cascade EXACTLY** (S2). The registry is a
   refactor of *how* deciders are dispatched, not *what* they decide; prove verdict
   equality against the committed baseline before adding any ordering freedom.

---

## 7. The deliverable form for Michael (the final step of this workflow)

When M3 + B2 are complete and green, **decompose the work so Michael can carefully
follow it and its rationale.** Recommended: a clean, legible sequence of small,
single-purpose commits (or an annotated walkthrough), each with the *why* — the
finding, the design choice, the rejected alternative, the gate it discharges. Keep
the two-branch shape: docs deltas land on (or rebase onto) `foundation-docs`; code
lands stacked on `foundation-code`. The goal is that Michael can read the change as a
narrated argument, not a wall of diff. Produce a `HANDBACK` for this workflow the way
the foundation did: what was built, what is *proven* (with test names), the runway
left, and any premise-level flags.

---

## 8. Standing invariants (hold at every milestone — `BACKLOG.md`)

- **S1** — every accepted `Proven`/`Refuted` `check`s against the *original* net and
  agrees with the oracle where it exists.
- **S2** — no decisiveness regression vs the committed baseline (only an explicit
  `Inconclusive → Decided` or a known-stub correction is allowed).
- **S3** — the core never imports the observer set; the bare-boolean trusted-base set
  (the ledger) is reported and never grows.
- **S4** — clean build: `cargo build`/`clippy` (pedantic + nursery) and the full suite
  pass.

*Read `working-doctrine.md` first — it is the contract. Then `BACKLOG.md` (M3, and the
B2 portion of M6) for the authoritative gates. Then build it beautifully, and leave
the estate for Michael.*
