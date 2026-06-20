# Doctest modernization (branch `fix-doctests`)

Report for Michael. `cargo test -p petrivet --doc` failed **20 of 25** doctests at
the foundation tip (`32eb583`). The trigger was the missing crate-root re-exports,
but because `--doc` aborts each doctest on its first error, that masked three
further layers of staleness. All 25 now pass; the 4 firewall doctests on
`api/model/verdict.rs` stayed green throughout. One commit: `959b1fb`.

## 1. Crate-root re-exports (the X2/X3 item)

`lib.rs` re-exported only modules, so `petrivet::Net`, `petrivet::Marking`,
`petrivet::PetriNet`, `petrivet::NetBuilder` did not resolve at the root. Added
`pub use crate::prelude::{ … }` at the crate root; the module paths
(`petrivet::net::Net`, `petrivet::system::PetriNet`, …) remain valid and equivalent.
No new clippy warnings (verified the root re-export adds none).

## 2. Wrong module paths in doctests

- `petrivet::net::system::PetriNet` → `petrivet::system::PetriNet`. `system` is a
  top-level module (`pub use api::system`), **not** a submodule of `net`. (system/mod.rs,
  state_space/coverability.rs, core/analysis/semi_decision.rs.)
- `petrivet::state_space::reachability::{…}` — the `reachability` submodule is private;
  its types are re-exported at `petrivet::state_space::*`. The import was unused, so removed.
- A root `use petrivet::{CoverabilityGraph, ExplorationOrder};` that the body never used — removed.

## 3. Stale-API bodies (revealed once the imports resolved)

These had drifted from the current API and had been masked by the import errors:

- `Marking` has no `From<[u32; N]>`; positional `PetriNet::new(net, [1, 0])` →
  `[(p0, 1)]` (only `From<[(Place, T); N]>` exists).
- Ambiguous `[(idle, 1)].into()` argument to `new` → drop the `.into()`.
- Explorer method renames: `iter()` → `explore_iter()`, `state_count()` → `marking_count()`.
- `add_arcs((p, t))` with a 2-tuple (`IntoArcs` is only implemented for ≥3 alternating
  nodes) → `add_arc((p, t))`. (state_space/reachability.rs and the class.rs
  AsymmetricChoice/General examples.)
- The removed method `choose_and_fire` → `enabled_transitions().next()` + `try_fire`.
- `remove_transition(p2)` was passed a `Place` → a transition (`t2`).
- The `semi_decision` example used the internal `IdxMarking` and called
  `analyze_reachability(&IdxMarking)` — but that method takes `&Marking<u32>`. Rewritten
  onto the public `is_reachable(&Marking)` surface.
- The three `no_run` fragments in the `PetriNet` struct doc referenced undefined locals
  (they still compile under `no_run`) → made runnable with hidden setup.

## 4. A semantic error worth your attention (`StateMachine` example, class.rs)

The state-machine illustration (the vending machine, "Murata Figure 4") had a single
`get_candy_for_15` transition serving **both** `bal_15` (exact 15¢) and `bal_20` (15¢ with
5¢ change). That gives it two input places and two output places, so the transition is not
a 1-1 bridge and **the net is not an S-net** — it classifies as `General`. This contradicted
both the example's own comment ("every transition is just a simple bridge between two
places") and its `assert!(class == NetClass::StateMachine)`.

I split it into `get_candy_for_15` (`bal_15 → bal_0`) and `get_candy_for_15_with_change`
(`bal_20 → bal_5`) so every transition is a 1-1 bridge and the net is a genuine state
machine. If Murata's figure intends the single transition, then the figure is not an S-net
and the example should illustrate a different class — flagging for your call; I took the
split as the reading that matches the example's stated intent.

## Verification

`cargo test -p petrivet --doc` 25/25; `--lib` 181/0; `cargo build` / `cargo clippy -p
petrivet --all-targets` clean, no new warnings. Branch `fix-doctests` is off `32eb583`
and touches only `lib.rs` plus doc comments — disjoint from the M2 remediation's files,
so it merges onto `foundation-brickwork` cleanly.
