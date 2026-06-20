# Foundational Components for the Future `petrivet`
### Required domains, algebra, and trust architecture

> Status: design document. Companion to the [essays](../essays/README.md) and the [implementation backlog](foundations-backlog.md). It specifies the components that do not yet exist in the codebase but are prerequisites for the analyses described in the vision documents. Statements about current code are verifiable against the cited file/line references; proposed components are identified as proposed. The Petri-net theory itself is Michael's.

---

## 1. A soundness defect in the current LP-based deciders

The marking-equation deciders described elsewhere as "exact" are implemented over floating-point arithmetic. [`find_marking_equation_rational_solution`](../../petrivet/src/core/analysis/semi_decision.rs) returns `Box<[f64]>`; the integer variant rounds via `v.round() as u32`; the structural-boundedness witness is an `f64` weight vector. Each is assembled from `f64::from(incidence.get(p,t))` and solved by `microlp`'s floating-point simplex.

This has a direct consequence for soundness. A floating-point feasibility result near a constraint boundary can be incorrect, and a rounded firing-count vector does not constitute a proof. Exact arithmetic is therefore not only the prerequisite for new algebraic results (invariants, rank, Farkas duals), as the essays frame it; it is also the prerequisite for the *existing* algebraic deciders to produce checkable certificates. The design therefore has two primary dependencies, not one: exact arithmetic (so that witnesses can be checked) and the certificate calculus (so that checking is the only trusted operation).

---

## 2. Scope and definitions

The intended system can be summarized as a pipeline of five functions: deciders produce candidate witnesses; checkers validate witnesses and thereby establish verdicts; a selection policy orders decider execution; an observation layer measures execution cost; and compositional analysis quantifies the degree to which a net fails to decompose. Each function depends on a domain the repository does not yet contain.

A "foundation" in this document is the set of required object types, each with the algebraic laws it must satisfy, together with one architectural decision: the boundary of the trusted computing base (TCB).

The recurring mathematical structures across the system are four: partial orders, ideal completions, closure operators, and quotients. The current code implements each of them by hand for specific cases. The foundation provides a named, exact, certificate-bearing implementation of each.

The required components are enumerated as F1–F8, grouped into three layers (production, validation, routing/observation), one transversal component (the order abstraction), and one dependent component (compositional analysis).

---

## 3. Trust architecture

The first decision is the boundary of the trusted computing base. Soundness will depend only on a small, enumerable set of components; all other components — deciders, the selection policy, the observation layer, future reductions, and any learned model — may contain errors without affecting correctness.

```
TRUSTED COMPUTING BASE  (small, audited, ideally formally verified)
  • exact-arithmetic kernel        (F1)  — all algebraic certificates depend on it
  • certificate checkers           (F3)  — check(): replay / dot-product / closure re-test
  • engine primitives              (existing) — fire / is_enabled / the product order

OUTSIDE THE TCB  (errors cost execution time, not correctness)
  • deciders, the selection policy and driver   (F5)
  • the observation crate                        (F6)
  • future certified reductions and any learned model   (Tier 3)
```

The deliverable of the validation layer is therefore an interface boundary, not a feature. If `Verdict::Proven` can be constructed only by passing a certificate through its checker, the policy-independence of soundness (§5, F3) is a type invariant rather than a runtime property. The enforceable rule: no component in the TCB may depend on any component outside it. A CI lint asserts the dependency direction.

---

## 4. Production layer

### F1 · Exact arithmetic and linear algebra (primary dependency)

**Function.** A Petri net's linear theory is defined over ℤ and its field of fractions ℚ. Checkable algebraic certificates cannot be produced over `f64`. F1 supplies an exact scalar type and exact matrix operations.

**Types.**
- `Rational`: a normalized exact rational with an exact zero-test. Its representation and overflow policy is the first engineering decision of the program: `i64/i64` normalized is fast but overflows on large MCC instances; the alternatives are `i128` with overflow detection and promotion, or a bignum dependency (`num-rational`, currently absent from the dependency tree). This decision gates F1 and affects the cost of every downstream proof.
- `Matrix<S>` / `Vector<S>` over `S ∈ {Rational, i64}`, built on the existing dense [`IncidenceMatrix`](../../petrivet/src/core/analysis/incidence.rs) storage (currently `new`/`get` only).

**Algebra and laws.** Field axioms for `Rational`; canonical normalization; exact equality. On matrices: `rank`, `kernel` (right null-space basis), `left_kernel` (kernel of the transpose), `solve` (a particular solution plus the null-space), and `farkas_certificate`: on an infeasible system `Cx = b, x ≥ 0`, return a dual `y` with `yᵀC ≥ 0` and `yᵀb < 0`. Required law: every returned object is verifiable by one exact dot product.

**Enables.**
1. Replacement of the floating-point marking-equation and boundedness deciders with exact versions, so they can emit certificates (resolves §1).
2. `rank(C)`, which with the cluster count from F4 gives the Rank Theorem (well-formed ⇔ `rank = c − 1`).
3. Extraction of the Farkas dual currently discarded at [`reachability.rs`](../../petrivet/src/api/system/reachability.rs) line 177, giving unreachability results a P-invariant certificate.
4. Null-space bases, which are the semiflows of F2.
5. The Esparza–Melzer refinement loop (marking equation plus trap/siphon constraints), which strengthens the order-insensitive over-approximation.

**Integration point.** Replace the `f64::from(...)` LP assembly in `semi_decision.rs` with exact solves; retain `good_lp`/`microlp` only as an inexact filter that never constructs `Proven`/`Refuted`.

### F2 · Semiflows and invariants

**Function.** The conservation laws of a net are the kernels of `C` and `Cᵀ`; the canonical generators are the minimal non-negative ones.

**Types.** `PInvariant` (a place weighting `y` with `yᵀC = 0`), `TInvariant` (`Cx = 0`), and the minimal semiflow generators (Colom–Silva / Martínez–Silva). These are the `Invariants` type and `compute_invariants` function that [`literature.rs`](../../petrivet/src/literature.rs) references but that do not exist.

**Algebra and laws.** A semiflow basis spans the kernel; minimality means no non-negative generator has strictly smaller support. Coverage predicates: `is_covered_by_s_invariants` (implies conservative and bounded), `is_covered_by_t_invariants` (implies consistent). Each invariant is verifiable in O(arcs) by one exact multiplication.

**Enables.** Conservativeness and consistency as certificates; structural boundedness as a checkable P-subinvariant rather than an `f64` vector; the soundness witness for implicit-place removal (Rung 3); and the interface-correction term in the F8 composition rule.

---

## 5. Validation layer

### F3 · Certificate and verdict calculus

**Function.** A verdict is a checkable witness rather than a boolean; "undecided" is a distinct value, not an error; and trust is confined to the checkers.

**Types.**
- `Verdict<P, N> = Proven(P) | Refuted(N) | Inconclusive`: three-valued, so that `Inconclusive` is distinguishable from `Refuted`. This type removes the L0 defect at [`liveness.rs`](../../petrivet/src/api/system/liveness.rs), where an unbounded net's transitions and a genuinely dead transition both receive `L0`.
- `trait Certificate { type Claim; fn check(&self, net, query) -> bool; }`. A firing sequence is checked by replay against `fire`/`is_enabled`; a Parikh vector by recomputing `m₀ + C·σ`; a P-invariant by one exact dot product (F1); a siphon/trap pair by re-evaluating its closure conditions.
- `fn accept<C: Certificate>(c, …) -> Verdict`: the only public constructor of `Proven`/`Refuted`. This function is the TCB in code.
- The `crate::model` module referenced by `literature.rs`, which is the location of the per-property certificate types.

**Algebra and laws.** Soundness law: `check() == true ⇒ Claim holds`; this is the only property trust depends on. Policy independence: the value of an accepted verdict is invariant under which decider produced the candidate and in what order — the soundness theorem stated as a structural property rather than an empirical one.

**Enables.** The two `Some(false)` stubs (the marked-graph liveness arm in `liveness.rs`, and [`is_covered_by_s_components`](../../petrivet/src/api/net/mod.rs), which returns a hardcoded `false`) must either carry a witness or return `Inconclusive`; the type does not admit an unwitnessed positive verdict. Offline auditability of all results. Safety of the Tier-3 selection layer: a learned scheduler can change only which proofs are attempted, never whether an invalid one is accepted.

---

## 6. Routing and observation layer

### F4 · Quotient, sub-net, and composition

**Function.** The recurring operation is the quotient (collapsing a coupling into equivalence classes) and its inverse, the induced sub-net; composition is the adjoint operation.

**Types.** `Partition` over places ∪ transitions (union-find over the existing sorted preset/postset slices) with the cluster equivalence (transitive closure of the flow relation), yielding the cluster count `c`. `SubNet`, the net induced by a unit or component, itself a `DenseNet`, so that existing analyses apply recursively. `Interface`, the shared boundary places. `compose`, with a property-specific operator `⊗` (verdict join for booleans, direct sum plus an F2 interface correction for invariant spaces, maximum for place bounds).

**Algebra and laws.** Partitions form a refinement lattice; the cluster quotient is the finest one the Rank Theorem requires. The parsed-but-unused NUPN unit forest ([`nupn.rs`](../../petrivet/src/api/pnml/nupn.rs)) becomes a `Partition` source; cutting at any antichain yields the partition lattice that F8 minimizes over. Law for `⊗`: associative, with `compose(parts) = whole` exactly when the property factors over the cut.

**Enables.** The cluster count for the Rank Theorem (and the certified replacement for the `is_covered_by_s_components` stub); the S/T-component decompositions (`s_components`, `is_covered_by_s_components`); certified reductions; and the partition lattice for F8.

### F5 · Decider, policy, and driver

**Function.** Make the execution schedule a parameter. Represent each decider as a value with metadata, order them by a policy, and gate every acceptance on a certificate.

**Types.** `trait Decider { fn polarity() -> Polarity; /* ProveYes | ProveNo | Exact */ fn cost_class() -> CostClass; fn domain(NetClass) -> bool; fn run(net, query, budget) -> Outcome<Verdict>; }`; a `Policy` whose default reproduces the current hand-coded order; a `Driver` that selects admissible deciders by domain, orders them by the policy, gates on the certificate, and returns the first `Proven`/`Refuted` or else `Inconclusive`. Associated type: `Budget`/`Cancellation`, a cooperative cancellation token threaded into the exploration loop and the solver calls. This is absent today (the only early exit is the ω short-circuit) and is a prerequisite for both Rung 2 and bounded measurement.

**Enables.** The cascade as data; the learned selection layer; parallel racing of deciders; anytime results.

### F6 · Measurement and cost (downstream only)

**Function.** Execution cost has no canonical origin (machine speed is an additive shift in log-cost); only differences are comparable across machines. The layer observes and does not act.

**Types.** `Cost` in an affine space (raw fibers tagged with run context; derived differential invariants — rankings and log-ratios). `Observation` and `FitnessComparison`. `Features` φ(N), assembled from the cached accessors (`NetClass`, strong connectivity, counts) plus the F2 invariant dimensions, F4 component and cluster counts, and the NUPN unit-tree shape. A separate crate whose dependency arrow points only at the core (CI-enforced). Specified in the [self-measurement harness plan](../self-measurement-harness-plan.md).

**Enables.** The self-labeling training set; machine-portable regression tests; a soundness monitor that detects trusted-but-incorrect deciders.

---

## 7. Transversal component

### F7 · Order abstraction (second phase)

**Function.** The engine is generic over the fiber (`TokenOps`) but not over the order; the order is the one un-abstracted element (the `merge_ordering` fold in [`marking.rs`](../../petrivet/src/core/marking.rs)). F7 abstracts it.

**Types.** `trait WellQuasiOrder`; a generalization of `Omega` to `Ideal<D>` (the ideal completion of an arbitrary WQO; `Omega` is the ℕ instance). ω-acceleration is restated as "join with the limit of the dominating chain."

**Enables.** The engine generalizes to a well-structured transition system (WSTS) framework (lossy channel systems, broadcast protocols, etc.); and, in the near term, the unbounded-net frontier — backward coverability over upward-closed sets, converting the current blanket `Inconclusive` into a refinement loop ([`UniqueSortedSlice`](../../petrivet/src/core/unique_sorted_slice.rs) is the natural finite-basis representation). This is independent of the F1–F5 path but increases coverage (fewer `Inconclusive` results, hence more training signal), so it is a second phase rather than future work.

---

## 8. Dependent component

### F8 · Compositional analysis (Φ_PN)

Depends on F1 (the invariant-space `⊗` and interface correction), F3 (the verdict distance δ), F4 (the partition lattice), and F5 (per-unit analysis cheap enough to minimize over a lattice). `Φ_PN = min over the partition lattice of distance(whole verdict, ⊗ of part verdicts)`. `Φ = 0` iff the property factors over some cut; `Φ > 0` indicates an irreducible property, with the minimizing cut as the witness. Scope constraint: this is the factorization-residual quantity only; no stochastic or integrated-information-theory apparatus is introduced, because the qualitative analyzer lacks the stochastic semantics that apparatus requires.

---

## 9. Dependency graph

```
        F1 exact LA ──────────────┬───────────────┐
         │  (root, no deps)       │               │
         ▼                        ▼               ▼
        F2 invariants        F4 quotient /     (Farkas dual →
         │                   subnet / compose   F3 negative certs)
         │                        │               │
         ▼                        ▼               ▼
   ┌──────────────  F3 certificate / verdict calculus  ─────────┐
   │  (Verdict + Certificate + accept gate are independent and  │
   │   come first; algebraic certificates require F1; subsumes  │
   │   the two Some(false) fixes and the L0 repair)             │
   └───────────────────────────┬───────────────────────────────┘
                               ▼
                       F5 decider / driver / budget
                               ▼
                       F6 observation  (separate crate)

   F7 WQO / Ideal  ── independent, second phase ──►  (coverage)
   F8 Φ_PN  ◄── requires F1 + F3 + F4 + F5  (dependent, last)
```

There are two roots: F1 (exact linear algebra) and F3 (the certificate calculus). Both are prerequisites; the remaining components compose from them.

---

## 10. Sequencing and cross-cutting decisions

**Implementation order** (detailed, with gates, in the [backlog](foundations-backlog.md)):

1. **F3 first.** The `Verdict`/`Certificate`/`accept` scaffolding plus `check()` for the already-structured witnesses (firing sequence, Parikh, siphon/trap, ω-marking). This removes the L0 defect and forces the two `Some(false)` stubs to be honest. It requires no new numerical code.
2. **F1 second.** Resolves the floating-point defect (making F3's algebraic certificates valid), then enables F2, rank, and Farkas duals. Self-contained classical mathematics; the only significant decision is the rational representation.
3. **F2 and F4.** The structural layer required for the decidability results (invariants, the cluster quotient, the Rank Theorem, S/T-components). These implement the names currently dangling in `literature.rs`.
4. **F5.** Represent the cascade as data; add the budget/cancellation token.
5. **F6, F7, F8.** The observation crate, the WSTS/unbounded-net work, and compositional analysis, ordered by priority.

**Decisions required before implementing F1/F3:**
- *Exact-arithmetic representation*: `i128` with promotion vs. a bignum dependency. Gates F1; affects every downstream proof's cost.
- *Certificate location*: witnesses are already name-based at the API boundary (`FiringSequence(Box<[Transition]>)`), so `check()` maps back through the existing `Mapping`; the `crate::model` module is their location.
- *TCB lint*: a CI assertion that the trusted set imports nothing from the producer/observer set, making the boundary enforced rather than documented.

Summary: F1 and F3 are the two missing roots; F2/F4 add the structural algebra on F1; F5 routes over F3; F6/F7/F8 are the second phase. Each component carries its own algebra and its own checkable law, and the trust architecture confines the Tier-3 components to the region outside the TCB.

---

*Methodology: derived from a line-by-line reading of the codebase on `docs/vision` — the state-space engine, the class-gated cascades, the floating-point LP layer, the siphon/trap closures, and the `literature.rs` citation index — and designed from first principles, using the [essays](../essays/README.md) as input but not as constraints. Every statement about current code is checked against a file/line reference; every proposed type is identified as proposed.*
