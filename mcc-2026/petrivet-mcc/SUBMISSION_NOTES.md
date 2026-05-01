# petrivet-mcc — MCC 2026 submission notes

This file records exactly what petrivet's MCC wrapper does for each examination
on contest day, and what we already know it will get wrong. Keeping this in
sync with `src/main.rs` makes it easier to identify whether a surprising MCC
verdict is "petrivet bug" or "documented limitation".

## How to test

### One PNML, one examination, on the host

```bash
mcc-2026/petrivet-mcc/scripts/run.sh <path/to/model.pnml> <Examination>

# examples
scripts/run.sh ../../petrivet/tests/fixtures/philo.pnml ReachabilityDeadlock
scripts/run.sh ../../petrivet/tests/fixtures/producer_consumer.pnml StateSpace
```

The script stages the PNML into a temporary BenchKit-shaped directory,
synthesises a matching `iscolored` file, sets the contest environment
variables (`BK_TOOL`, `BK_EXAMINATION`, `BK_INPUT`), and runs the binary
exactly the way `BenchKit_head.sh` would inside the VM. Override the
binary location with `PETRIVET_MCC_BIN`, override the input "name" the
tool sees (used to detect colored models via `*-COL-*`) with `BK_INPUT`.

### Library / wrapper unit tests

```bash
cargo test -p petrivet            # core library
cargo test -p petrivet-mcc        # wrapper (currently no tests)
cargo test                        # everything in the workspace
```

Four pre-existing failures in `petrivet::analysis` are listed under
"Known petrivet bugs" below; they are unrelated to the MCC submission
path.

### End-to-end inside the contest VM

```bash
cargo run -p mcc-orchestrator
```

This drives the full pipeline: build the binary, boot the supplied
`mcc2026.vmdk` overlay, install the binary and `BenchKit_head.sh`,
run a smoke test that asserts all four `STATE_SPACE` lines appear,
clean up the smoke fixture from the VM, power down, and flatten the
overlay into `artifacts/petrivet-2026.vmdk`. The orchestrator's host
dependencies (`qemu`, `ssh`, `scp`, `tar`) are pinned via
`mcc-2026/mcc-orchestrator/default.nix`; if `nix` is on PATH, missing
binaries are auto-installed via `nix shell nixpkgs#…`.

## What we participate in

| Examination               | Status               |
| ------------------------- | -------------------- |
| `StateSpace`              | Active               |
| `ReachabilityDeadlock`    | Active (stretch)     |
| `OneSafe`                 | Active (stretch)     |
| `QuasiLiveness`           | Active (stretch)     |
| `StableMarking`           | Active (stretch)     |
| `Liveness`                | Active (stretch)     |
| `UpperBounds`             | `DO NOT COMPETE`     |
| `ReachabilityFireability` | `DO NOT COMPETE`     |
| `ReachabilityCardinality` | `DO NOT COMPETE`     |
| `CTLFireability`          | `DO NOT COMPETE`     |
| `CTLCardinality`          | `DO NOT COMPETE`     |
| `LTLFireability`          | `DO NOT COMPETE`     |
| `LTLCardinality`          | `DO NOT COMPETE`     |

We bow out of every formula-based examination because petrivet does not yet
parse the MCC formula XML format, and adding that pipeline (atom evaluation,
CTL/LTL model checking, etc.) is out of scope for now.

## Common pre-flight gates

Every active examination shares the same early exits:

1. **Colored models.** If `iscolored` contains `TRUE`, or if neither file
   is present and the `BK_INPUT` name matches `*-COL-*`, we emit
   `CANNOT COMPUTE`. petrivet only handles P/T nets.
2. **Marker `unfinite`.** When present, the contest is telling us the
   reachability graph is infinite. We `CANNOT COMPUTE` for any examination
   that requires an explicit reachability graph (which is, today, all of
   them).
3. **Marker `large_marking`.** Empty file dropped by BenchKit when some
   reachable marking carries more than 2³² tokens in some place. petrivet
   stores token counts as `u32`, so we'd silently overflow before
   answering anything. We bail with `CANNOT COMPUTE` immediately.

## Reachability graph construction

All five global properties and `StateSpace` go through
`System::build_reachability_or_coverability` (added in this submission).
Internally that drives Karp–Miller exploration with the optimized
ancestor-only ω-acceleration test (see
[Primer, Algorithm 3.18](../../literature/petri%20net%20primer.pdf), versus
the lecture notes' weaker characterization). It short-circuits the moment
the first ω is introduced. So:

* For bounded nets we get the exact reachability graph and, for the
  global-property examinations, the exact answer.
* For unbounded nets we abandon with `CANNOT COMPUTE` instead of waiting
  for the full coverability graph (whose state count we couldn't legally
  report anyway under MCC's `STATES` rule).

## Per-examination caveats

### `StateSpace`

* All four lines (`STATES`, `TRANSITIONS`, `MAX_TOKEN_PER_MARKING`,
  `MAX_TOKEN_IN_PLACE`) are derived directly from the explicit reachability
  graph. No approximations.
* `STATES`/`TRANSITIONS` are emitted as `usize` (cast widening for
  display), token counts as `u32` — the same types the library uses
  internally. The Submission Manual marks `STATES` mandatory and only
  allows `-1` on the other three fields; we never need to use that
  escape hatch because the values are free once we have an RG.
* Because we go via the reachability graph, **unbounded nets always
  return `CANNOT COMPUTE`** even though the contest does award credit
  for partial info on the optional fields. We could in principle
  build the coverability graph and report `-1` for `STATES` while
  emitting real numbers for the bounded fields, but the manual is
  somewhat coy about whether `-1` is acceptable on the mandatory
  field; we play it safe.

### `ReachabilityDeadlock`

* Build RG via the short-circuiting coverability path; answer with
  `ReachabilityGraph::has_reachable_deadlock()`.
* Exact for bounded nets.
* **No structural pre-check.** Earlier iterations ran
  `commoner_hack_criterion` first, but minimal-siphon enumeration is
  exponential in the worst case, and a CHC failure is inconclusive —
  so we'd pay the LP cost *and* still build the RG. CHC pays off in
  *liveness* (where it's a sufficient condition we actually want to
  succeed), not deadlock detection.
* For unbounded nets we say `CANNOT COMPUTE`. Deadlock-freedom is
  decidable for all P/T nets via backward reachability (lecture
  notes §3.2.3, BACK2). petrivet does not implement BACK2 yet.

### `OneSafe`

* Three-tier dispatch (cheapest first):
  1. **Structural place bounds** via `find_positive_place_subvariant`
     (LP). If every place's derived bound is ≤ 1, answer TRUE without
     any state-space work.
  2. **Short-circuiting RG walk** when the net is structurally bounded:
     stop at the first reachable marking that puts ≥ 2 tokens in any
     place (FALSE witness).
  3. **Full RG fallback** for the remainder: `is_one_safe()` on the
     fully-built RG.
* For unbounded nets where step 1 fails, we say `CANNOT COMPUTE`. The
  coverability graph could answer this directly (FALSE iff any place
  is ω-marked or has a finite bound > 1), but we don't fall back to
  it today.

### `QuasiLiveness`

* Computed as "every transition has liveness level ≥ L1" — i.e. every
  transition appears on at least one edge of the reachability graph.
* The petrivet `liveness_levels` implementation walks Kosaraju SCCs of
  the reachability graph (Murata 1989, §V-C), which is exact for
  bounded nets.
* For unbounded nets we say `CANNOT COMPUTE`. As with the others,
  this is decidable in principle (the coverability graph captures
  all firable transitions), but we don't yet wire that path through.

### `StableMarking`

* Computed by scanning every reachable marking and tracking, per place,
  whether the token count has ever differed from the count seen at the
  first marking. If any place stays constant, we answer TRUE.
* Includes `x = 0` as a valid stable value, which is what the formula
  `∃p ∃x AG tokens-count(p) = x` allows.
* **Known weakness**: this is exact only for bounded nets. For
  unbounded nets we `CANNOT COMPUTE`. Note that on bounded nets the
  answer is essentially "does the net have a 1-place S-invariant of
  the form `1·p`" — petrivet's S-invariant analysis lives in
  `analysis/` but we don't use it here; we walk the explicit graph
  for simplicity and fewer surprises.

### `Liveness`

* Two-tier dispatch:
  1. **Free-choice + Commoner-Hack.** If the net is free-choice and CHC
     holds (every minimal siphon contains a marked trap), the net is
     L4-live by Murata Theorem 12. Polynomial-time, no exploration.
  2. **Full RG fallback** with `ReachabilityGraph::is_live()`, the
     SCC-based decision procedure (Murata §V-C).
* The CHC path is gated on `is_free_choice_net()` because, for non-FC
  nets, CHC is only sufficient — failure tells us nothing — and the LP
  cost can dominate.
* For unbounded nets we say `CANNOT COMPUTE`.
* The `is_live()` code path exercises the SCC-based decision procedure
  on the fully-built reachability graph. That code path passes its own
  unit tests; the failing analysis tests live elsewhere (specialized
  fast paths for T-net liveness from S-invariants).

## Known petrivet bugs that may surface

These are tests currently failing in the petrivet library at the time of
submission. They are pre-existing leftovers from a major refactor of the
public API (Place / Transition / Marking instead of internal
PlaceIdx / TransitionIdx / IdxMarking) and are not regressions caused by
the MCC submission code.

* `analysis::semi_decision::tests::t_net_reachability_positive` — the
  marking-equation integer-solution finder fails to certify a positive
  T-net reachability. Affects nothing on the MCC submission path
  because the binary always uses the explicit reachability graph.
* `analysis::tests::s_net_sc_marked_all_l4` — strongly-connected
  S-net liveness specialization is wrong on at least one marking.
  Same: not used by the binary; we go through the explicit RG.
* `analysis::tests::t_net_sc_all_circuits_marked_l4` — strongly-connected
  T-net liveness specialization. Same caveat.
* `analysis::tests::t_net_dead_predecessor_propagates` — non-SC T-net
  dead-predecessor propagation. Same caveat.

If the contest verdicts disagree with us specifically on `Liveness` for
a marked-graph instance, this is the most likely culprit. The other
four (deadlock / one-safe / quasi-liveness / stable-marking) only call
into the SCC-based bounded-RG code path, which is independently tested
and currently green.

## What we explicitly do not handle

* Colored Petri nets (we exit early on `iscolored=TRUE`).
* Inhibitor / reset / read arcs (PNML extensions). petrivet only
  models the standard P/T arcs.
* Nets whose state space exceeds available RAM during exploration —
  we don't detect this and would crash with an OOM rather than
  printing `CANNOT COMPUTE`. The contest's per-instance time limit
  (E-5.7 in the rules) means this is rarely reached, but a stress
  instance could hit it.
* `usize > i64::MAX` state counts (theoretical).
