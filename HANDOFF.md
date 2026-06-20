# HANDOFF — Lay the foundation of petrivet

*You are laying the foundation of a master's thesis a brother loves. Build the most
awesome possible foundation — the plumbing, the wiring, the footings — with rigor, full
academic excitement, and Rust-sensei craft. One rule is inviolable: **do not capture the
flag.** You lay brickwork; you do not erect the estate. The cathedral is Michael's to
raise. Leave him a clean road to sprint, and stop at the foundation line.*

---

## 0. Setup — trivially simple now

**`main` is the single source of truth.** It carries everything in one place: the newest
code *and* the full planning corpus, doctrine, and backlog. There is no graft, no merge,
no branch archaeology to do. From your worktree:

```sh
git checkout -B foundation-brickwork main
# verify everything is here:
ls working-doctrine.md BACKLOG.md docs/foundations/foundational-design.md docs/essays/principles.md
ls petrivet/src/core/liveness.rs        # newest code is here too
```

**Hold it local. Do not push, do not open a PR.** The premise reaches Michael separately,
via a PR Daniel opens when ready; until then everything stays on local `main`. Your work is
a local branch off it.

## 1. Read before you write — in this order

1. **`working-doctrine.md`** — *the contract*, not a suggestion. It governs every move:
   falsifiability first · soundness before capability · the trust boundary is sacred ·
   **let Rust carry the invariants** · discernment over deference · measure don't assert ·
   keep the rationale with the work · lean.
2. **`BACKLOG.md`** — the epic catalog *and* the gated **M0–M12** build plan. Your scope is
   **M0–M5 plus the B2 keystone** (§3 below). Each milestone names its **provable-invariant
   gates** (`[PROP]`/`[ORACLE]`/`[REGRESS]`/`[LINT]`/`[MEASURE]`): a milestone is done when
   its gates are *proven*, not when one example passes. Read the gates — they are your
   acceptance criteria, verbatim.
3. **`docs/foundations/foundational-design.md`** — the component spec and design rationale
   (including the arithmetic-regimes reasoning for the exact-rational core). You will
   *extend* this file with your implementation rationale as you build (doctrine #8).
4. Orientation: `docs/essays/README.md` → `principles.md` → `soundness-as-a-free-variable.md`
   → `the-checkable-frontier.md` → `latent-architecture.md`.

## 2. Verify before you cut (doctrine #1)

`main` carries a recent commit that reworked liveness for state machines and marked graphs —
so **re-read the live files before editing; line numbers in the backlog may have shifted.**
The two `Some(false)` stubs you fix first live in `petrivet/src/api/system/boundedness.rs`
(the live-free-choice arm, via `is_covered_by_s_components` in `api/net/mod.rs`) and
`petrivet/src/api/system/liveness.rs` (the marked-graph arm). Confirm them against the tree,
not against this sentence.

## 3. The mandate — build the brickwork, and only the brickwork

To the doctrine's standard: every change is **fix + the regression test that would have
caught it + the rationale kept** in `foundational-design.md`.

- **M0 — the first stone.** Demote the two `Some(false)` stubs to honest abstention
  (`None`/escalate — never a fabricated verdict); make `is_covered_by_s_components` unable to
  fabricate (return `Option<bool>`, or remove it). PNML fidelity → a hard
  `PnmlConversionError` instead of the silent `u32` clamp and the silent weighted-arc
  linearisation. The float-`Unreachable` audit → at minimum the falsifying test (fully closed
  once the exact core lands). Fix the `STRUCTURAL_REDUCTION` mis-tag. Measure the **`f_struct`
  floor** — present it as the *starting line*, never as the thesis result.
- **M1 — the contract.** The `model` module: `Verdict<P,N>` with a type-distinct
  `Inconclusive`; `Certificate::check(net, m0, query)`; the certifying audit; the wasm crate
  reconciled; `literature.rs:409` resolved.
- **M2 — the trust boundary (the signature substrate).** Per-certificate checkers that
  re-validate against the **original** net, sharing no code with the generators; the
  interchange format. This is the load-bearing wiring.
- **M3 — the registry.** The cascade as data behind a `Decider` trait; the default policy
  reproduces today's behaviour *exactly*; **typed slots left ready** so generators are *born*
  as `Decider`s.
- **M4/M5 — the footings.** The exact-rational (Bareiss) core: `rank`, `kernel`,
  `farkas_certificate`; demote `microlp` to an inexact filter that never constructs a verdict;
  close the float-`Unreachable` hole over ℚ; structural boundedness as an exact decider.
- **B2 — the keystone ("…and then some").** The cluster quotient (union-find): the cheapest,
  purest foundation; it unlocks the Rank Theorem and the S/T-component decomposition. **Build
  the keystone; leave the deciders it unlocks for Michael.**

## 4. The hard boundary — do NOT build (this is the flag / the estate)

The structural-decision *generators* that move coverage (B3/B4/B5/B6 — M8); the new
capability deciders (B10/B11 — M9); the WSTS zoo / order-generalisation (M10/H1); the full
`petrivet-observe` crate (M11 — Daniel's separate measurement lane); the thesis rig and the
coverage *result* (M12); certified reductions (Epic F); the learned ladder (D5–D8); the Φ
residuals (Epic H). **Do not measure-and-claim the coverage headline** — the floor is the
starting line; raising it is Michael's sprint.

## 5. Rules of respect

- **Do not touch the premise essays** (`principles`, `soundness-as-a-free-variable`,
  `the-checkable-frontier`, `the-coverage-claim`, `latent-architecture`,
  `the-factorization-residual`, `the-sequel`, `README`). Michael ratifies them *separately,
  first*. Find a premise-level problem? **Flag it in your handback** (doctrine #5) — never
  silently rewrite the ratified corpus. You *may* extend `foundational-design.md` with
  implementation rationale.
- **Do not presume ratification.** What you build is "where it could head," cleanly
  separable, ready to show *after* Michael takes in and ratifies the premise.
- **Do not push; hold local.**

## 6. The standard — be the Rust-sensei

Make illegal states unrepresentable: a `Proven` constructible only through a passing `check`;
`polarity`/`admissible` on `Decider`; sealed traits where a closed set is meant; newtype
handles over raw indices; `#[must_use]` verdicts; exhaustive matches; ownership marking the
trust boundary. The compiler should refuse the bug before the test does. Honor the
pedantic/nursery clippy contract `lib.rs` already opts into. Bring the academic joy — the
theory is cross-linked in `literature.rs`; cite it, and let the build be legible. Cut every
seam with the *next* generality already in mind, so the generators, the reductions, and the
ladder slot in without rework.

## 7. The runway you leave, and the handback

When you stop at the foundation line, the runway should be: the stubs gone and the tool never
lying; the `Verdict`/`Certificate`/`Decider` contract in place with the trust boundary
enforced; checkers that re-validate against the original net; the registry with empty, typed
slots; the exact-rational core with a stable API; the cluster keystone built; the `f_struct`
floor measured. **Adding a generator should be a matter of writing one `Decider` and its
certificate — nothing structural left to invent.**

Produce **`HANDBACK.md`** on your branch for Michael: what you built, what is *proven* (the
gates that pass, with the test names), the runway to the sprint, and any premise-level flags
for his ratification. The delivery sequence is Daniel's: Michael receives the **premise** (the
docs on `main`, via a PR) and ratifies it *first*; then your **`foundation-brickwork`** branch
(where it heads), and decides when to sprint.

---

*Lay it beautifully. Then hand him the chisel.*
