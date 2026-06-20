# Baseline `--lib` test fixes (branch `fix-add-arcs-wiring`)

Report for Michael. Six unit tests were failing on `cargo test -p petrivet --lib`
at the committed baseline (`f3356bc`). The reported symptom — firing `t0` in a
two-place cycle built with `add_arcs((p0, t0, p1, t1, p0))` left the token at
`Place(1)` instead of `Place(2)` — was initially attributed to the `add_arcs`
chained-tuple macro or to `Marking` index resolution. Neither held. The macro is
correct (see the falsifier below); the failures had three distinct root causes.

All six now pass (`111 passed; 0 failed`). The change set is three commits:

| Commit | What |
|---|---|
| `Fix fire_unchecked discarding its token-delta` | the firing plumbing bug |
| `Fix scrambled node identity in the built petgraph` | the graph-construction bug |
| `Correct two liveness/reachability test expectations` | two tests whose setup contradicted their intent |

## Root cause 1 — `fire_unchecked` was a no-op (code bug)

`PetriNet::fire_unchecked` computed the post-fire token counts with
`checked_sub(1)` / `checked_add(1)` but **discarded the results** — they were
never written back into the marking, so firing changed nothing.

```rust
// before (no-op):
self.marking[p_idx].checked_sub(1).expect("...");
// after:
self.marking[p_idx] = self.marking[p_idx].checked_sub(1).expect("...");
```

Introduced by the `929c2f5` impl-block reorganization. Fixes `basic_firing`
and `into_parts`.

**Falsifier for the macro hypothesis.** `add_arcs_chain_matches_explicit_pairs`
builds the cycle via the chained tuple and via explicit `add_arc` pairs and
asserts the two arc sets are identical. They are — the macro wiring is correct,
which is what localized the defect to firing. `fire_unchecked_applies_token_delta`
is the direct regression, built with explicit `add_arc` to isolate it from the
helper.

## Root cause 2 — scrambled node identity in the built petgraph (code bug)

`NetBuilder::build` constructed the `Graph<IdxNode, ()>` node arrays by iterating
`place_to_index.values()` / `transition_to_index.values()`. `HashMap` iteration
order is arbitrary, so the node at position `i` was *labeled* with the dense index
of some other node, while the edge wiring and every reader index
`p_indices[i]` / `t_indices[i]` by dense index. Topology and labels were both
scrambled relative to the dense net.

The dense-net analyses were unaffected (they use the correctly-ordered
`preset_t`/`postset_t`/… slices). Only the **graph-based** structural results were
corrupted, because they recover dense indices from the graph:

- `Net::circuits` → `graph.cycles()` then `graph.node_weight(..)`;
- `liveness_via_state_machine_marked_sccs` → `condensation(self.graph)` then reads
  the SCC member labels.

This single bug caused three failures:

- `s_net_non_sc_mixed_levels` (t1 = L1, should be L3),
- `t_net_dead_predecessor_propagates` (t3 = L0, should be L1),
- the source-transition half of `t_net_source_transition_l4` (t_src = L0, should be L4).

**The optimized liveness algorithms (`fdcf7fb`) were correct** — they were being
fed a corrupted graph. The fix builds the node arrays in dense-index order so
position, label, and edge wiring agree.

## Root cause 3 — two tests whose setup contradicted their intent (test bugs)

Verified against the firing semantics and the exact `classify` definitions in
`core/class.rs`, then corrected to match the stated intent.

- **`general_net_reachability_fallback`** asserted `NetClass::General`, but the net
  it built is genuinely free-choice (`p1• = p2• = {t2}`; `•t0 = •t1 = {p0}`, which
  satisfies `is_free_choice`). `classify` is correct; the test never exercised the
  general-net fallback it is named for. Added one arc (`p1 → t1`) to create
  symmetric confusion (`p0• = {t0,t1}`, `p1• = {t1,t2}`, sharing only `t1`, neither
  contained in the other), making the net genuinely General. Reachability of
  `[(p1,1)]` still holds via `t0` (fires on `p0` alone).

- **`t_net_source_transition_l4`** initialized the empty marking, leaving the
  downstream circuit `t0 → p0 → t1 → p1 → t0` unmarked. A source transition feeds
  `p_src` but cannot revive an empty circuit (`t0` also needs `p1`, `t1` needs
  `p0` — mutually blocked), so `t0`/`t1` are genuinely dead (L0), contradicting the
  all-L4 expectation and the test's own doc comment ("downstream L4 if all circuits
  are marked"). Marked the circuit with `[(p0,1)]`; the source then keeps it live
  and all transitions are L4.

## Verification

- `cargo test -p petrivet --lib`: 111 passed, 0 failed (was 103 passed, 6 failed).
- `cargo clippy -p petrivet --all-targets`: no new warnings at any edited line
  (the crate's `pedantic`/`nursery` contract has pre-existing baseline warnings
  elsewhere, unchanged).

## Out of scope (pre-existing, not addressed here)

- `cargo test -p petrivet` (the full suite, incl. doctests) still fails to compile
  the doctests because of the broken `#[cfg(doc)] use crate::model` at
  `literature.rs:409`. This is the known A1/M1 blueprint dangling link, untouched
  by this branch, and is why the request was scoped to `--lib`.
