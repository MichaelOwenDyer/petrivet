# Petrivet: Requirements & Architecture Plan

## 1. Vision & Scope

### 1.1 What is Petrivet?

Petrivet is a Rust library for modeling, simulating, and analyzing Petri nets. It aims to be:

- **Correct**: Grounded in formal Petri net theory, with analysis results you can trust.
- **Ergonomic**: Easy to pick up for engineers modeling systems and researchers studying net properties.
- **Performant**: Efficient data structures and algorithms, with room for future optimization.
- **Incremental**: A solid foundation for ordinary nets that can be extended to richer net classes.

### 1.2 Who is it for?

- **Engineers** modeling concurrent systems, workflows, protocols, or control systems.
- **Researchers** studying Petri net theory, structural properties, and decidability results.
- **Students** learning about Petri nets and formal methods.

### 1.3 What is explicitly OUT of scope (for now)?

These are interesting but deferred to avoid the breadth-first trap:

- Colored Petri Nets (CPNs)
- Temporal/modal logic (LTL, CTL)
- Const generic nets and `petrinet!` macro
- WASM support
- Visualization / graph layout
- Import/export (PNML, DOT) - useful but not foundational
- Algorithmic Petri Nets
- Parallel state space exploration
- Generic unsigned token types (u8/u16/u64 - just u32 or i32 for now)

These can be revisited once the foundation is solid.

---

## 2. Architecture Overview

### 2.1 Guiding Principles

1. **One way to do things**: There should be one obvious path for common tasks, not two parallel systems.
2. **Composition over monomorphization**: Extensions (guards, actions, metadata) are stored alongside the core net, not embedded in generic type parameters.
3. **Algorithms that work**: Every analysis method either returns a real answer or clearly says "not yet implemented." No `todo!()` panics in public APIs.
4. **Stable Rust**: No nightly features required.

### 2.2 Module Structure

```
petrivet/
├── src/
│   ├── lib.rs
│   ├── net/                    # Net structure (the graph)
│   │   ├── mod.rs              #   Net, Place, Transition, Arc
│   │   ├── builder.rs          #   NetBuilder
│   │   └── class.rs            #   Structural classification types
│   ├── marking.rs              # Marking, Omega, OmegaMarking, Tokens
│   ├── system.rs               # System<N> = net + initial marking
│   ├── simulation.rs           # Firing rules, enabled transitions, step execution
│   ├── analysis/               # All analysis lives here
│   │   ├── mod.rs              #   Analysis entry points and dispatch
│   │   ├── structural.rs       #   Invariants, siphons, traps, components
│   │   ├── state_space.rs      #   StateSpace graph, exploration iterators
│   │   ├── liveness.rs         #   Liveness analysis
│   │   ├── boundedness.rs      #   Boundedness analysis
│   │   └── reachability.rs     #   Reachability and coverability
│   └── extensions/             # Composition-based extensions
│       ├── mod.rs
│       ├── weighted.rs         #   Arc weights and place capacities
│       └── interpreted.rs      #   Guards, actions, timing (current DIPN)
```

### 2.3 Core Types

#### Net (the structure)

```rust
/// An ordinary Petri net N = (S, T, F).
/// All arc weights are implicitly 1. No capacities.
/// This is the foundational type - all other net classes build on or wrap it.
pub struct Net {
    n_places: usize,
    n_transitions: usize,
    /// preset[t] = input places of transition t
    preset: Box<[Box<[Place]>]>,
    /// postset[t] = output places of transition t
    postset: Box<[Box<[Place]>]>,
    /// preset_p[p] = transitions that produce into place p
    preset_p: Box<[Box<[Transition]>]>,
    /// postset_p[p] = transitions that consume from place p
    postset_p: Box<[Box<[Transition]>]>,
}
```

Key change from current code: **the incidence matrix and input/incidence markings are NOT stored in Net**. They are computed on demand or cached externally. The `Net` struct is purely the graph topology. This keeps it lean and avoids coupling structural data with behavioral data.

#### Place, Transition (identifiers)

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub(crate) usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition(pub(crate) usize);
```

Newtype wrappers around `usize`. The inner field is `pub(crate)` - users create them via the builder, not by hand. Display impls show `p0`, `p1`, `t0`, `t1`, etc.

#### Marking (the state)

```rust
/// A marking: token counts for each place, indexed by Place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Marking(Box<[u32]>);
```

Key change: **use `u32` for token counts**, not `Tokens(i32)`. Token counts are non-negative. Signed arithmetic is only needed internally when applying incidence vectors, and that can use `i64` intermediaries with checked subtraction. This eliminates the confusing `Tokens(i32)` type that made negative token counts representable in user-facing markings.

The `Omega<T>` enum for coverability markings remains as-is - it's well-designed.

#### System (net + marking, entry point for analysis)

```rust
/// A Petri net system: a net N paired with an initial marking M₀.
/// This is the primary entry point for behavioral analysis.
pub struct System<N = Net> {
    net: N,
    initial_marking: Marking,
}
```

This replaces both the current `PetriNet<'net>` and `System<N>`. One type, one purpose. The type parameter `N` defaults to `Net` for the simple case, but can be `SNet`, `TNet`, `Circuit`, or `FreeChoiceNet` when the user wants specialized analysis.

**Ergonomic usage (the default path):**

```rust
let net = builder.build()?;  // Returns ClassifiedNet (enum, but opaque to the user)
let system = System::new(net, (1, 0, 0));  // Accepts tuples via Into<Marking>

// Internally dispatches to the best algorithm for the detected class.
// The user never needs to think about classification.
println!("Bounded? {:?}", system.is_bounded());
println!("Live? {:?}", system.is_live());
```

The builder automatically classifies the net at build time. The returned type
carries the classification internally so that analysis methods can dispatch to
the most efficient algorithm without the user ever needing to know or care
about structural subclasses. This is the recommended path for most users.

**Power usage (specialized path for researchers):**

```rust
let net = builder.build()?;
// Researchers who know their net is an SNet can force the type for
// compile-time guarantees and to make the specialization explicit.
let system = System::new(SNet::try_from(net)?, marking);
println!("Live? {:?}", system.is_live());
```

### 2.4 Structural Classification

The builder classifies the net at build time. The result is an opaque
`ClassifiedNet` type (internally an enum) that carries both the net and its
structural class. Analysis methods on `System<ClassifiedNet>` dispatch to the
best algorithm automatically.

```rust
/// Opaque type returned by the builder. Internally dispatches to specialized impls.
pub struct ClassifiedNet { /* enum inside */ }

impl ClassifiedNet {
    /// Query the detected class (for display/logging, not required for analysis).
    pub fn class(&self) -> NetClass { ... }
    /// Access the underlying Net topology.
    pub fn net(&self) -> &Net { ... }
}

impl Net {
    pub fn is_s_net(&self) -> bool { ... }
    pub fn is_t_net(&self) -> bool { ... }
    pub fn is_free_choice(&self) -> bool { ... }
    // etc.
}
```

The newtype wrappers (`SNet(Net)`, `TNet(Net)`, `Circuit(Net)`, `FreeChoiceNet(Net)`)
remain for power users who want type-level guarantees. `TryFrom<Net>` and
`TryFrom<ClassifiedNet>` conversions are available.

The key design goal: **the default path never requires the user to understand
structural classification.** They just build a net and ask questions. The
library picks the best algorithm silently.

### 2.5 Extensions via Composition

Instead of embedding extension data into the net type via generics, extensions wrap the core net and store additional data in parallel structures:

```rust
/// A net with arc weights and optional place capacities.
pub struct WeightedNet {
    net: Net,
    /// Arc weights for place→transition arcs: weights_pt[t] parallels net.preset[t]
    weights_pt: Box<[Box<[u32]>]>,
    /// Arc weights for transition→place arcs: weights_tp[t] parallels net.postset[t]
    weights_tp: Box<[Box<[u32]>]>,
    /// Optional place capacities (None = unlimited)
    capacities: Option<Box<[Option<u32>]>>,
}

/// A data-interpreted net: core net + guards + actions on transitions.
pub struct InterpretedNet {
    net: Net,
    guards: Box<[Option<Box<dyn Fn() -> bool>>]>,
    actions: Box<[Option<Box<dyn Fn()>>]>,
    timings: Box<[TransitionTiming]>,
}
```

This means:
- The `dipn` module becomes `extensions::interpreted`, reusing the core `Net`, `Place`, `Transition` types.
- `WeightedNet` and `InterpretedNet` implement the same simulation traits as `Net`.
- Analysis algorithms that only need the topology can accept `&Net` (extracted via `AsRef<Net>`).

### 2.6 Simulation

Simulation is separated from analysis. A `Simulate` trait provides the firing rule:

```rust
pub trait Simulate {
    /// Returns an iterator over transitions enabled in the given marking.
    fn enabled_transitions<'a>(&'a self, marking: &'a Marking) -> impl Iterator<Item = Transition> + 'a;

    /// Fires a transition, mutating the marking. Returns Err if not enabled.
    fn fire(&self, marking: &mut Marking, transition: Transition) -> Result<(), FireError>;

    /// Checks if a specific transition is enabled.
    fn is_enabled(&self, marking: &Marking, transition: Transition) -> bool;
}
```

`Net`, `WeightedNet`, and `InterpretedNet` all implement `Simulate`. For `Net`, the firing rule is simple (subtract 1 from each input place, add 1 to each output place). For `WeightedNet`, it respects weights. For `InterpretedNet`, it additionally checks guards and runs actions.

### 2.7 State Space Exploration

The current `StateSpace` graph with petgraph + frontier + seen_nodes is a good design. It stays, but with cleanup:

- It lives in `analysis::state_space`.
- The exploration iterators (BFS, DFS) stay with their pluggable strategy pattern.
- `StateSpace` is parameterized by marking type (`Marking` for reachability, `OmegaMarking` for coverability).
- The omega-acceleration logic in the coverability iterator stays.

The `Findings` struct is removed. It tried to be a cumulative analysis cache but was half-implemented and coupled behavioral analysis to state space exploration prematurely. Analysis methods compute what they need when called.

---

## 3. Current Codebase Disposition

What to **keep** (good code, keep with minor refactoring):
- `NetBuilder` and its validation logic
- `Marking<T>` type and its `PartialOrd` implementation (covering relation)
- `Omega<T>` and `OmegaMarking`
- `StateSpace` graph structure
- Exploration iterators (BFS/DFS strategy pattern)
- Coverability iterator with omega-acceleration
- Reachability iterator
- Structural classification logic (`is_s_net`, `is_t_net`, `is_free_choice`)
- Newtype wrappers for net classes and their `TryFrom` impls
- The theoretical documentation in `class.rs` (it's excellent)
- Analysis traits (`LivenessAnalysis`, `BoundednessAnalysis`, `ReachabilityAnalysis`)
- Specialized impls for Circuit and SNet liveness/boundedness
- Siphon/trap `is_siphon`/`is_trap` verification methods
- DIPN simulation logic and tests (to be adapted, not rewritten)
- All existing tests

What to **change**:
- `Tokens(i32)` → `u32` for markings, signed math only internally
- `Net` struct: remove incidence matrix, input_markings, incidence_markings from stored fields
- Builder returns `Net` directly (classification available as method, not as return type)
- Merge `PetriNet<'net>` and `System<N>` into one `System<N>` type
- Move simulation (fire, enabled_transitions) out of `Net` into a `Simulate` trait
- DIPN: rewrite as `InterpretedNet` composing over core `Net` and types
- Remove `#![feature(step_trait)]` and let-chains; target stable Rust
- Reorganize modules per the proposed structure

What to **remove**:
- `StructureClass` enum (classification available as method on `Net`)
- `ClassifiedSystem` enum and its boilerplate dispatch impls
- `PetriNet<'net>` struct and `Findings` struct
- `StateSpaces` enum (Bounded/Unbounded) - premature
- The `Liveness` enum in `behavior/mod.rs` (duplicates analysis module)
- Duplicate type definitions between `dipn` and core

---

## 4. Phased Implementation Plan

### Phase 0: Cleanup & Unification (Foundation) - COMPLETE

**Goal**: One coherent codebase with no dead code, no `todo!()` panics, stable Rust.

Completed:
- [x] Remove `#![feature(step_trait)]` and any let-chain usage; ensure `cargo +stable build` works
- [x] Restructure modules: `net/` (Net, Place, Transition, builder, class), `marking.rs`, `system.rs`
- [x] Change `Tokens(i32)` to `u32` for `Marking<T>`; handle signed arithmetic via `apply_delta`
- [x] `Net` stores only topology (presets/postsets); no incidence matrix
- [x] Builder returns `ClassifiedNet`; classification is automatic and transparent
- [x] Unify into single `System<N>` with simulation (`choose_and_fire`, `try_fire`, `fire_any`)
- [x] `EnabledTransition` proof-token design with HRTB closure for zero-redundancy firing
- [x] `Marking<T>` generic over token type; `OmegaMarking` as type alias
- [x] Delete all legacy code (`structure/`, `behavior/`, `analysis/`, `dipn/`) and legacy examples
- [x] Remove unused dependencies (`ahash`, `num-traits`, `nalgebra`, `serde`, `serde_json`)
- [x] All 30 tests pass on stable, clippy clean, no `todo!()` in any public method
- [x] Mutex example demonstrating new simulation API

### Phase 1: State Space Exploration - COMPLETE

**Goal**: Fully working reachability and coverability graph construction with useful queries.

Completed:
- [x] Replace old `StateSpace` with shared `ExplorerCore<'a, T>` generic over token type
- [x] `ReachabilityGraph<'a>`: BFS/DFS exploration with user-driven termination via `explore_next()` / `iter()`
- [x] `CoverabilityGraph<'a>`: full Karp-Miller construction with omega-acceleration
- [x] `ExplorationOrder` (BFS/DFS) switchable mid-exploration
- [x] Frontier optimization: only seed transitions whose input places gained tokens, plus precomputed source transitions
- [x] `ExplorerCore` borrows `&'a Net` (no cloning); lifetime threads through CG and RG
- [x] Query methods: `is_reachable`, `path_to`, `is_coverable`, `is_bounded`, `place_bound`, `is_deadlock_free`, `deadlocks`, `contains`, `markings`, `state_count`, `edge_count`
- [x] `CoverabilityGraph::into_reachability_graph()` - near-zero-cost promotion for bounded nets
- [x] `ReachabilityGraph::from_coverability()` - O(n) conversion unwrapping `Omega::Finite(k)` → `k`
- [x] All 47 tests pass, clippy clean

Design decisions:
- No `ExplorationLimits` struct - the user controls termination externally via `explore_next()` or iterator combinators like `.take(n)`
- No `Inconclusive` result type - queries answer based on what has been explored so far; `is_fully_explored()` tells the user if the answer is definitive
- Graph stores `Marking<T>` directly as node weights (no wrapper struct)

### Phase 2: Structural Analysis & Behavioral Shortcuts

**Goal**: Verified structural analysis algorithms and semi-decision procedures that
serve as computational shortcuts for answering behavioral questions. High-level
`System` methods dispatch to the best available algorithm automatically.

Phase 2 is split into two sprints. Sprint 1 laid the foundation; Sprint 2 corrects
issues found during a literature verification pass and adds remaining features.

#### Sprint 1 - COMPLETE

Initial implementations of structural analysis and behavioral API. All code
compiles and passes tests, but a subsequent verification against the literature
(Murata 1989, Petri Net Primer, Lecture Notes) identified several correctness
issues and missing features addressed in Sprint 2.

Completed:
- [x] `IncidenceMatrix` struct and `Net::incidence_matrix()` computation
- [x] Integer null space via Bareiss algorithm (`analysis::math::integer_null_space`)
- [x] `Invariants` struct with S-invariant and T-invariant basis vectors
- [x] `compute_invariants(net)` using null space of incidence matrix
- [x] LP-based coverage check: `is_covered_by_s_invariants` / `is_covered_by_t_invariants`
- [x] `minimal_siphons(net)` and `minimal_traps(net)` (initial growing algorithm - buggy, replaced in Sprint 2)
- [x] `every_siphon_contains_marked_trap()` for Commoner's theorem
- [x] `check_marking_equation()` - LP relaxation of the state equation
- [x] `is_structurally_bounded(net)` and `is_place_structurally_bounded(net, place)` - LP formulations
- [x] `From<Net> for NetBuilder` and `From<ClassifiedNet> for NetBuilder`
- [x] High-level `System` methods: `is_bounded`, `is_dead`, `is_quasi_live`, `is_live`
- [x] `is_live` dispatches to Commoner's theorem for free-choice/S-net/T-net, otherwise exploration
- [x] Integration of `good_lp` with `microlp` backend (pure Rust, supports ILP via branch-and-bound)
- [x] `pub(crate) core()` accessors on `CoverabilityGraph` and `ReachabilityGraph`
- [x] `pub graph()` accessor on `ExplorerCore`

#### Sprint 2 - COMPLETE

Corrections from literature verification, new features, and closing optimizations.

**A. Corrections**

- [x] **A1. Incidence matrix convention switch.**
  Switched from `|T|×|P|` (Murata convention) to `|P|×|T|` (Primer convention)
  so the state equation reads `M' = M₀ + N·x` directly, without a transpose.
  Row index = place, column index = transition, so `N[p][t]` is the net token
  change at place `p` when transition `t` fires.
  Updated: `Net::incidence_matrix()`, `compute_invariants()`, `check_marking_equation()`,
  `is_structurally_bounded()`, `is_place_structurally_bounded()`, and all tests.

- [x] **A2. Fix siphon/trap enumeration.**
  Replaced Sprint 1 growing algorithm with two correct alternatives:
  - *Shrinking algorithm* (Primer, Algorithm 6.19): `maximal_siphon_in(net, subset)` and
    `maximal_trap_in(net, subset)` iteratively remove places that violate the property.
    Minimal enumeration via backtracking on top (`minimal_siphons`, `minimal_traps`).
  - *ILP enumeration*: `minimal_siphons_ilp`, `minimal_traps_ilp` with binary variables
    and no-good cuts for systematic completeness.
  - Improved `every_siphon_contains_marked_trap` to compute `maximal_trap_in` per siphon
    directly instead of requiring pre-enumerated traps. Now takes `&Net` instead of
    a trap slice. Updated `system.rs` caller accordingly.

- [x] **A3. Fix `is_live_by_exploration`.**
  Moved SCC-based liveness analysis to `ReachabilityGraph::liveness_levels()` and
  `ReachabilityGraph::is_live()`. Now correctly checks that `t` labels an edge in
  **every** terminal SCC (not just the first). All liveness levels computed in a
  single O(V+E) pass. `System::is_live_by_exploration` delegates to `rg.is_live()`.

- [x] **A4. Fix `is_dead` marking equation usage.**
  Added `check_covering_equation` to `semi_decision.rs` - uses `>=` constraints instead
  of `==` to check if any marking covering a threshold is reachable. Updated `is_dead`
  in `system.rs` to use this for the semi-decision step, correctly checking whether
  any enabling marking for the transition is LP-reachable.

**B. New Features**

- [x] **B1. ILP variant of marking equation.**
  Add `check_marking_equation_ilp()` alongside the existing LP relaxation.
  - LP infeasible → definitely unreachable (sound).
  - LP feasible, ILP infeasible → definitely unreachable (strictly stronger).
  - ILP feasible → possibly reachable (necessary but not sufficient; firing order
    may not exist even with an integer firing count vector).
  Both LP and ILP are necessary conditions for reachability, not sufficient.
  ILP is more expensive but can rule out cases LP cannot.
  (Murata 1989, §IV-B: "a nonnegative integer solution x must exist" is a
  necessary reachability condition.)

- [x] **B2. `LivenessLevel` enum and per-transition liveness analysis.**
  Implemented `LivenessLevel` enum in `system.rs` (Dead/L1/L2/L3/L4) with
  `PartialOrd`/`Ord` for level comparison. SCC-based decision for bounded nets on
  `ReachabilityGraph`:
  - L0: `t` labels no edge.
  - L1: `t` labels at least one edge.
  - L3 (≡L2 for bounded): `t` labels an edge in a non-trivial SCC.
  - L4: `t` labels an edge in every terminal SCC.
  High-level `System::liveness_levels()` builds CG → promotes to RG for SCC analysis,
  returns `Option<Box<[LivenessLevel]>>` (None if unbounded).
  Tests: cycle (all L4), deadlocked (all L0), absorbing branch (mixed L1/L3),
  mutex (all L4).

- [x] **B3. Document conservativeness.**
  S-invariant coverage implies *conservativeness*: the net has a positive S-invariant,
  meaning every firing preserves a weighted token sum. Conservativeness is strictly
  stronger than structural boundedness.
  Relationship: S-invariant coverage → conservativeness → structural boundedness.
  The structural boundedness LP (which finds any positive S-sub-invariant with
  `C·y ≤ 0`, `y > 0`) is a weaker but more direct check. Both belong in the library
  for different use cases. Add doc comments clarifying this hierarchy.

**C. Sprint 3: Class-Specific Reachability Shortcuts**

These use the structural class of the net to give *exact* reachability answers
where the general marking equation only gives necessary conditions.

- [x] **C1. S-net reachability via token conservation.**
  For S-nets, every transition moves exactly one token. The marking equation is
  both necessary and sufficient: `M'` is reachable from `M₀` iff the LP is
  feasible (i.e., all S-invariants are preserved). Polynomial time.
  Implemented as `is_reachable_s_net()` in `semi_decision.rs`.
  (Murata 1989, Theorem 21; Lautenbach & Thiagarajan 1979)

- [x] **C2. T-net reachability via exact marking equation.**
  For T-nets, every non-negative integer solution to `M' = M₀ + N·x`
  corresponds to a realizable firing sequence. The ILP is both necessary and
  sufficient. Implemented as `is_reachable_t_net()` in `semi_decision.rs`.
  (Murata 1989, Theorem 22)

- [x] **C3. `System::is_reachable()` high-level dispatcher.**
  Automatically selects the best algorithm based on net class:
  - S-nets → `is_reachable_s_net` (polynomial, exact)
  - T-nets → `is_reachable_t_net` (ILP, exact)
  - General → LP filter → ILP filter → state space exploration fallback

**D. Deferred Items (noted for future phases)**

These were planned in earlier phases but are out of scope for the current sprint.
They are recorded here so nothing is lost:

- Cache structural properties at build time (strong connectivity, possibly
  structural boundedness, S-invariant coverage) so they can be queried in O(1)
  instead of recomputed. Currently `NetClass` is cached but other properties
  like `is_strongly_connected()` rebuild a petgraph on every call.
- S-component and T-component detection (shelved - compiles and passes tests,
  but lower priority than class-specific shortcuts)
- Circuit (cycle) enumeration for T-net liveness
- Firing sequence bounds for bounded free-choice and T-nets
- Liveness-to-reachability reduction (wrapping a net to test liveness via reachability)
- L2 vs L3 distinction for unbounded nets (requires CG-based approximation)
- `analyze_*` richer API returning reasoning/witnesses beyond `bool`

**E. Testing and Quality**

- [x] Comprehensive tests for all corrections (A1–A4)
- [x] Tests for ILP marking equation (B1)
- [x] Tests for liveness levels on known nets (B2)
- [x] Tests for S-net and T-net reachability (C1–C2)
- [x] Tests for `System::is_reachable` dispatch (C3)
- [x] Run clippy and address all warnings
- [x] Update PLAN.md to reflect completed items

**F. Closing Optimizations**

- [x] **E1. `ReachabilityExplorer` / `ReachabilityGraph` type split.**
  Split the monolithic `ReachabilityGraph` into two types:
  - `ReachabilityExplorer<'a>` - incremental exploration handle for any net (bounded
    or not). Exposes `explore_next()`, `iter()`, `explore_all()`, basic queries.
  - `ReachabilityGraph<'a>` - fully explored, bounded graph (type-level proof of
    boundedness). Carries liveness and deadlock analysis methods.
  Construction: `ReachabilityGraph::build()` (convenience, non-terminating for
  unbounded nets), `TryFrom<ReachabilityExplorer>` (checks `is_fully_explored()`),
  `TryFrom<CoverabilityGraph>` (checks `is_bounded()`).

- [x] **E2. Batch liveness without `OnceCell`.**
  `ReachabilityGraph::liveness_levels()` computes all liveness levels in a single
  O(V+E) SCC pass and returns an owned `Box<[LivenessLevel]>`. No hidden caching;
  users store the result themselves. Convenience methods `liveness_level(t)` and
  `is_live()` call `liveness_levels()` internally.

- [x] **E3. Single-LP invariant coverage.**
  `is_covered_by_s_invariants` and `is_covered_by_t_invariants` now use a single
  LP per check instead of one LP per place/transition. Lambda variables are
  unrestricted in sign (necessary when the integer basis has negative entries).

- [x] **E4. Doc and code cleanup.**
  Fixed stale doc comments (removed "cached lazily" reference, corrected `N^T` →
  `N` for Primer convention). Removed redundant `is_s_net() || is_t_net()` check
  in `is_live()` (both are subclasses of free-choice). Fixed `is_covered` lambda
  bounds (was inadvertently restricted to `>= 0`; must be unrestricted for
  correctness with arbitrary integer basis vectors).

**Execution order**: A1 → A2 → A4 → A3+B2 → B1 → B3 → E → F1–F4 → C1–C3

**Exit criteria**: All structural analysis and semi-decision procedures are verified
against the literature. `system.is_live()`, `system.is_bounded()`, and
`liveness_levels()` return correct results for textbook nets. Siphon/trap enumeration
is complete (finds all minimal siphons/traps). Tests cover non-trivial nets.
`ReachabilityExplorer`/`ReachabilityGraph` type split provides clear bounded/unbounded
distinction. All analysis code clean under `clippy -D warnings`.

### Phase 4: Weighted and Capacity Nets

**Goal**: Support for the most common generalization of ordinary nets.

Tasks:
- [ ] `WeightedNet` struct with arc weights and optional place capacities
- [ ] `WeightedNetBuilder` (or extend `NetBuilder` with weight methods)
- [ ] `Simulate` implementation respecting weights and capacities
- [ ] Incidence matrix computation for weighted nets
- [ ] Adapt state space exploration for weighted firing rule
- [ ] E/C nets as a special case: `WeightedNet` where all capacities are 1 and all weights are 1
- [ ] Equivalence reduction: theorem that any bounded weighted net can be simulated by an ordinary net

**Exit criteria**: Can build, simulate, and analyze weighted nets. E/C nets work as a constrained case.

### Phase 5 and Beyond (Future)

Candidates for future phases, roughly ordered by value:
- Import/export (PNML at minimum, DOT for visualization)
- Inhibitor and reset arcs
- Colored Petri Nets
- Const generic nets and `petrinet!` macro
- Parallel state space exploration
- Generic token types (u8, u16, u64)
- Timed/stochastic nets

---

## 5. Design Decisions & Rationale

### 5.1 Why `u32` for tokens instead of generic unsigned types?

Generic token types (`Marking<T: Unsigned>`) add complexity to every function signature, every trait bound, every error message. The benefit (saving memory for small nets) is marginal compared to the ergonomic cost. `u32` handles up to ~4 billion tokens per place, which is sufficient for virtually all practical uses. If someone genuinely needs u64/u128, that can be added later as a feature flag or parallel type without affecting the core API.

### 5.2 Why not store the incidence matrix in Net?

The incidence matrix is derivable from the preset/postset structure. Storing it eagerly means:
- Every `Net` pays the memory cost whether or not anyone needs the matrix.
- The matrix must be kept in sync if the net structure ever changes.
- It couples structural representation with numerical analysis.

Instead, `Net` provides a method to compute it on demand. Analysis algorithms that need it can cache it locally.

### 5.3 Why composition over generics for extensions?

Consider two approaches to adding arc weights:

**Generic approach:**
```rust
struct Net<A: ArcData = ()> {
    arcs: Vec<(Place, Transition, A)>,
}
```

**Composition approach:**
```rust
struct WeightedNet {
    net: Net,
    weights: Vec<u32>,
}
```

The generic approach leads to monomorphization (every function is compiled N times for N arc types), makes error messages harder to read, and means the `Net` type itself changes shape depending on extensions. The composition approach keeps `Net` simple and fast, and extensions pay only their own cost. Since extension data (weights, guards) is rarely in the hot path of analysis algorithms that only need topology, the indirection cost is negligible.

### 5.4 Why `ClassifiedNet` instead of the current `StructureClass` / `ClassifiedSystem`?

The current design forces the user to interact with classification at every step:
```rust
let class: StructureClass = builder.build()?;
let system = ClassifiedSystem::new(class, marking);
system.is_live()  // dispatches through 5-arm match
```

Every new analysis method requires adding a 5-arm match in `ClassifiedSystem`.
The user must understand the classification system before they can ask a
simple question.

The new design hides classification behind an opaque type:
```rust
let net = builder.build()?;  // ClassifiedNet - user doesn't need to care
let system = System::new(net, (1, 0, 0));
system.is_live()  // internally dispatches to best algorithm
```

Classification still happens (at build time, once), and the library still uses
the best algorithm available. The difference is that this is invisible to users
who don't care. Power users who want type-level guarantees can still do:
```rust
let circuit = Circuit::try_from(net)?;
let system = System::new(circuit, marking);
system.is_live()  // statically dispatches to Circuit impl
```

The 5-arm match boilerplate moves inside `ClassifiedNet`'s trait impls,
where it belongs - library internals, not user-facing API.

### 5.5 EnabledTransition: Avoiding Redundant Enablement Checks

#### The Problem

The current DIPN simulation has a common pattern:

```rust
let enabled: Vec<_> = net.enabled_transitions(&marking).collect();
if let Some(&t) = enabled.first() {
    net.fire(&mut marking, t).unwrap();  // checks enablement AGAIN internally
}
```

`fire()` must re-check enablement because it accepts a bare `Transition` - the
caller could pass any transition, enabled or not. This means every fire
operation does the work twice.

#### The Fundamental Tension

We need three things simultaneously:
1. Let the user see which transitions are enabled (`&self`)
2. Let the user choose one
3. Fire it (`&mut self`) without re-checking

Steps 1 and 3 conflict under Rust's aliasing rules. Any proof token from step 1
borrows `&self`, which blocks `&mut self` in step 3. And if proof tokens can
outlive the state they were checked against, they become stale and unsafe.

#### Solution: Closure with Higher-Ranked Proof Token

The entire check-choose-fire cycle happens inside a closure called by the
system. A proof token type (`EnabledTransition<'a>`) ensures only transitions
from the enabled set can be fired. A higher-ranked lifetime bound (`for<'a>`)
prevents the token from escaping the closure. This is compile-time enforced -
not a runtime check.

```rust
use std::marker::PhantomData;

/// Proof that a transition was found enabled. Cannot be constructed by users
/// (private fields), cannot be copied or cloned, and cannot escape the
/// `choose_and_fire` closure (lifetime bound to an internal scope).
pub struct EnabledTransition<'a>(Transition, PhantomData<&'a ()>);

impl EnabledTransition<'_> {
    /// Inspect which transition this refers to.
    pub fn transition(&self) -> Transition { self.0 }
}

/// The set of transitions enabled in a specific marking.
/// Only exists inside the `choose_and_fire` closure.
pub struct EnabledSet<'a>(Vec<Transition>, PhantomData<&'a ()>);

impl<'a> EnabledSet<'a> {
    pub fn first(&self) -> Option<EnabledTransition<'a>> {
        self.0.first().map(|&t| EnabledTransition(t, PhantomData))
    }
    pub fn get(&self, index: usize) -> Option<EnabledTransition<'a>> {
        self.0.get(index).map(|&t| EnabledTransition(t, PhantomData))
    }
    pub fn iter(&self) -> impl Iterator<Item = EnabledTransition<'a>> + '_ {
        self.0.iter().map(|&t| EnabledTransition(t, PhantomData))
    }
    pub fn len(&self) -> usize { self.0.len() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}
```

The `System` method that ties it all together:

```rust
impl System {
    /// Compute the enabled set, let the caller choose one, and fire it.
    ///
    /// The closure receives the set of enabled transitions and returns a
    /// proof token for the chosen transition. The token cannot be fabricated
    /// (private fields), duplicated (not Copy/Clone), or stashed outside
    /// the closure (higher-ranked lifetime). This makes `fire` infallible
    /// with zero redundant enablement checks, enforced at compile time.
    ///
    /// Returns the fired transition, or None if the closure chose not to fire
    /// (or no transitions were enabled).
    pub fn choose_and_fire<F>(&mut self, choose: F) -> Option<Transition>
    where
        F: for<'a> FnOnce(EnabledSet<'a>) -> Option<EnabledTransition<'a>>
    {
        let enabled = self.compute_enabled();
        let set = EnabledSet(enabled, PhantomData);
        let chosen = choose(set)?;
        let t = chosen.0;
        self.fire_unchecked(t);
        Some(t)
    }
}
```

#### Why This Is Sound

The `for<'a>` bound (higher-ranked trait bound / HRTB) is the critical piece.
It means the closure must work for *any* lifetime `'a`, not a specific one.
This prevents stashing the proof token:

```rust
let mut stashed = None;
system.choose_and_fire(|set| {
    stashed = Some(set.first()?);
    //        ^^^^ COMPILE ERROR: cannot assign EnabledTransition<'a>
    //             (universally quantified) to stashed (has a fixed type
    //             with a specific lifetime)
    set.first()
});
```

And since `EnabledTransition` is not `Copy` or `Clone`, the user can't
duplicate it - they must pick exactly one and return it:

```rust
system.choose_and_fire(|set| {
    let chosen = set.first()?;
    let also = set.first()?;  // fine, but...
    drop(chosen);              // must give up one to return the other
    Some(also)
});
```

The only way to obtain an `EnabledTransition` is from `EnabledSet`, and the
only useful thing to do with it is return it. The API makes misuse a compile
error, not a runtime panic.

#### Usage Examples

```rust
// Pick the first enabled transition
system.choose_and_fire(|enabled| enabled.first());

// Pick a specific transition if it's enabled
system.choose_and_fire(|enabled| {
    enabled.iter().find(|et| et.transition() == t0)
});

// Pick based on external priority
system.choose_and_fire(|enabled| {
    priority_list.iter()
        .find_map(|&t| enabled.iter().find(|et| et.transition() == t))
});

// Random selection
system.choose_and_fire(|enabled| {
    if enabled.is_empty() { return None; }
    enabled.get(rng.gen_range(0..enabled.len()))
});
```

#### Complementary APIs

`choose_and_fire` is the zero-redundancy path for when you need to inspect the
enabled set before choosing. For simpler cases, provide direct methods:

```rust
impl System {
    /// Check-and-fire in one step. Best when you already know which transition
    /// you want to fire. No redundant check - single pass.
    pub fn try_fire(&mut self, transition: Transition) -> Result<(), FireError>;

    /// Fire any single enabled transition (useful for simulation loops).
    pub fn fire_any(&mut self) -> Option<Transition>;

    /// Read-only query: which transitions are currently enabled?
    /// Returns plain `Transition` values (no proof tokens).
    /// If you want to fire one of these, use `try_fire` or `choose_and_fire`.
    pub fn enabled_transitions(&self) -> Vec<Transition>;
}
```

The full API covers all patterns:

```rust
// Pattern 1: I know which transition I want - just try it
system.try_fire(t0)?;

// Pattern 2: I need to choose from the enabled set - no double-checking
system.choose_and_fire(|enabled| enabled.first());

// Pattern 3: Just fire something, I don't care which
system.fire_any();

// Pattern 4: I just want to look (read-only, no firing)
let enabled = system.enabled_transitions();
println!("Currently enabled: {:?}", enabled);
```

### 5.6 Why split simulation from analysis?

Simulation (firing transitions, stepping through execution) is useful on its own for engineers who just want to run their model. Analysis (liveness, boundedness, reachability) is a separate concern that builds on simulation. Keeping them separate means:
- Engineers can use `Simulate` without pulling in analysis machinery.
- The `Simulate` trait provides a clean extension point for `WeightedNet`, `InterpretedNet`, etc.
- Analysis algorithms can be generic over anything that implements `Simulate`.

---

## 6. Migration Checklist

This section tracks what needs to happen to go from the current codebase to the Phase 0 target.

### Types to rename/change:
- `structure::Index` → just use `usize` directly
- `behavior::Tokens(i32)` → `marking::Tokens` as `u32` (or just use `u32` in `Marking`)
- `structure::Place { index: usize }` → `net::Place(usize)` (tuple struct)
- `structure::Transition { index: usize }` → `net::Transition(usize)` (tuple struct)
- `behavior::Marking<T>` → `marking::Marking` (non-generic, uses `u32`)
- `behavior::Omega<T>` → `marking::Omega` (non-generic, wraps `u32`)

### Types to remove:
- `behavior::PetriNet<'net>`
- `behavior::Findings<'net>`
- `behavior::StateSpaces<'net>`
- `behavior::Liveness` enum
- `analysis::ClassifiedSystem` (replaced by `ClassifiedNet` + `System` dispatch)

### Types to refactor:
- `structure::class::StructureClass` → becomes the internal enum inside `ClassifiedNet`

### Types to add:
- `simulation::Simulate` trait
- `simulation::FireError`
- `extensions::interpreted::InterpretedNet` (replaces `dipn::Net`)
- `extensions::weighted::WeightedNet` (Phase 5)

### Modules to reorganize:
- `structure/` → `net/` (Place, Transition, Arc, Net, NetBuilder, classification)
- `behavior/marking.rs` → `marking.rs` (Marking, Omega, OmegaMarking)
- `behavior/mod.rs` → split between `system.rs`, `simulation.rs`, `analysis/state_space.rs`
- `dipn/` → `extensions/interpreted.rs`
- `analysis/` stays, with cleanup
