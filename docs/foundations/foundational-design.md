# Foundational Design for the Future `petrivet`
### The trust architecture: certificates, checkers, and the exact-rational core

> Status: design document (specification-grade). The bridge between the [essays](../essays/README.md) — which argue the vision — and the [implementation backlog](foundations-backlog.md), which sequences the build. It specifies the components that do not yet exist in the codebase but are prerequisites for the analyses the vision describes, and it fixes the shapes (`Verdict`, `Certificate`, `Decider`) the rest of the program is written against. Statements about current code are verifiable against the cited file/line references; proposed components are identified as proposed. The Petri-net theory itself is Michael's.
>
> This document is **coherent with the ratified engineering plan** in [`BACKLOG.md`](../../BACKLOG.md) (epics A–H, X). Where this document names a component F-number and the backlog names an epic letter, the [crosswalk in §11](#11-crosswalk-the-fm-numbering-against-the-ah-epics) reconciles them. The two documents share one direction, one set of invariants, and no contradictions.

---

## 0. The ratified inversion (what this document is built on)

The vision essays lead with a *theorem* — that soundness is independent of the selection policy — and a *scalar*, Φ_PN, that measures how far a property fails to factor. The ratified plan **inverts the emphasis**, and this design document is written from the inverted position:

- **The certificate-and-checker is the stone.** The signature technical contribution is an interoperable, machine-checkable certificate for every property verdict, re-validated by a small external checker that *is* the entire trusted base. (Epic C; here F3 + the original-net checking invariant of §5.)
- **The falsifiable headline is empirical.** The claim that earns or loses the thesis is a measured coverage number: *on the real MCC P/T corpus, a polynomial structural certificate decides a large, characterizable fraction of queries without state-space exploration; where it abstains, it abstains honestly* (`f_struct`, Epic G). It is not a theorem.
- **The soundness firewall is the enabling property, not the headline.** "Soundness is independent of the selection policy" is, as a theorem, a one-line corollary of certifying-algorithms (McConnell–Mehlhorn–Näher) composed with algorithm-selection (Rice; SATzilla). Its non-trivial content is a *precondition the code must first discharge*: every fast decider must be certifying. The two `Some(false)` stubs (§1) and the silent PNML corruptions (§1.1) violate that precondition today. The figure of merit is the **certifying fraction `f`** — the share of accepted verdicts carrying a checked certificate — measured and required non-increasing in its trusted-base complement.
- **Learned selection is the SATzilla-style sequel** (Epic D ladder), gated behind the checker so a mis-selection costs time, never correctness. **MCC ranking is OUT** as a thesis goal: the contest is the crucible (honest abstention under an oracle cross-check) and the label source (the certificate is the training label), not a leaderboard.
- **The scalar Φ_PN is dissolved.** The single net-level scalar, the boolean-verdict Φ (which is just assume-guarantee reasoning), and the "needs all four roots" necessity claim do not survive examination. What survives are **two computable, monotone, theorem-backed-zero, per-property factorization residuals — Φ_bound and Φ_inv** — whose deliverable is their *measurement over the corpus*, not their metaphysics (§8). IIT is absent from the repository and is not introduced.

The remainder specifies the architecture that the inversion requires.

---

## 1. A soundness defect in the current LP-based deciders

The marking-equation deciders described elsewhere as "exact" are implemented over floating-point arithmetic. [`find_marking_equation_rational_solution`](../../petrivet/src/core/analysis/semi_decision.rs) returns `Box<[f64]>`; the integer variant rounds via `v.round() as u32`; the structural-boundedness witness is an `f64` weight vector. Each is assembled from `f64::from(incidence.get(p,t))` and solved by `microlp`'s floating-point simplex.

This has a direct consequence for soundness. A floating-point feasibility result near a constraint boundary can be incorrect, and a rounded firing-count vector does not constitute a proof. Exact arithmetic is therefore not only the prerequisite for new algebraic results (invariants, rank, Farkas duals), as the essays frame it; it is the prerequisite for the *existing* algebraic deciders to produce checkable certificates at all. The design therefore has two primary dependencies, not one: **exact arithmetic** (so that witnesses can be checked) and the **certificate calculus** (so that checking is the only trusted operation). This finding is correct and central; it survives unchanged into the ratified plan as the soundness precondition of the whole construction.

The defect has a worse, quieter relative. The negative verdicts `Unreachable`/`Uncoverable` rest on `microlp` *failing* to find a rational solution ([`reachability.rs`](../../petrivet/src/api/system/reachability.rs) line 177). A spurious floating "infeasible" on a genuinely feasible rational system yields a silent **false `Unreachable`** — and unlike the two stubs of §3, the firewall cannot catch it: there is no positive object to check on the negative path, so the wrong answer carries no certificate to reject it. This is the floating-`Unreachable`-verdict audit (backlog B1a / M3); it is A2-priority and data-dependent, hence invisible to any single example.

### 1.1 Input fidelity at the PNML boundary is a soundness concern

Before any decider runs, the converter can already have changed the net. Initial markings above `u32::MAX` are silently clamped ([`convert.rs`](../../petrivet/src/api/pnml/convert.rs) line 287); weighted arcs are silently linearised to weight-1 (`convert.rs` lines 262–267). Each produces a *structurally different net* than the input — wrong-net-in, confident-answer-out — contrary to the converter's own stated discipline (`convert.rs` line 37). These rank, in priority, with the two stubs: a certificate that checks against the wrong net is sound about nothing. The fix is to make both cases hard `PnmlConversionError`s (or explicit, recorded caveats), so import never fabricates a different net. (Backlog A7/E4/E7; milestone M0's defect ledger and M1's regression set.)

---

## 2. Scope and definitions

The intended system is a pipeline of five functions: **deciders** produce candidate witnesses; **checkers** validate witnesses against the original net and thereby establish verdicts; a **selection policy** orders decider execution; an **observation layer** measures execution cost; and **compositional analysis** quantifies the degree to which a net fails to decompose. Each function depends on a domain the repository does not yet contain.

A "foundation" in this document is a set of required object types, each with the algebraic laws it must satisfy, together with one architectural decision: the boundary of the trusted computing base (TCB), and the invariant that holds it constant under everything the deciders and reductions do (§3, §5).

The recurring mathematical structures across the system are four — partial orders, ideal completions, closure operators, and quotients. The current code implements each by hand for specific cases. The foundation provides a named, exact, certificate-bearing implementation of each.

The required components are enumerated **F1–F8**, grouped into three layers (production, validation, routing/observation), one transversal component (the order abstraction), and one dependent component (compositional analysis). The backlog sequences them as milestones M0–M11 and reconciles them to the ratified epics A–H; the crosswalk is §11.

---

## 3. Trust architecture

The first decision is the boundary of the trusted computing base. Soundness depends only on a small, enumerable set of components; everything else — deciders, the selection policy, the observation layer, future reductions, any learned model — may contain errors without affecting correctness.

```
TRUSTED COMPUTING BASE  (small, audited, measured, ideally formally verified)
  • certificate checkers           (F3)  — check(net, m0, query) against the ORIGINAL net
  • exact-arithmetic kernel        (F1)  — the dot-product / rank / Farkas the checkers rely on
  • engine primitives              (existing) — fire / is_enabled / the product order
  • the remaining bare-boolean deciders  — those not yet certifying (A6); the residue to shrink

OUTSIDE THE TCB  (errors cost execution time, not correctness)
  • deciders that emit a checked certificate     (F5; Epic B generators)
  • the selection policy and driver              (F5)
  • the observation crate                        (F6)
  • future certified reductions and any learned model   (Epics F, D-ladder)
```

The deliverable of the validation layer is an **interface boundary**, not a feature. If `Verdict::Proven`/`Refuted` can be constructed only by passing a certificate through its checker, the policy-independence of soundness (§5) is a type invariant rather than a runtime hope. Two enforceable rules carry it:

1. **No TCB component depends on any component outside it.** A CI lint asserts the dependency direction (`petrivet` never imports `petrivet-observe`; the producer/observer set is downstream-only).
2. **The trusted base is measured and non-increasing.** The TCB is exactly `{F3 checkers} ∪ {remaining bare-boolean deciders}`. Its bare-boolean complement is the residue A6 enumerates; CI reports its size and forbids it to grow. The certifying fraction `f` is its dual figure of merit — the share of accepted verdicts that *do* carry a checked certificate — and is reported over the corpus.

This is the GRAT discipline (Lammich): an unverified, possibly buggy *generator*; a small, eventually formally verified *checker*; trust confined to the latter. Petrivet adopts it wholesale and makes its trusted base a *measured, monotone quantity* rather than a claim.

---

## 4. Production layer

### F1 · Exact arithmetic and linear algebra (primary dependency)  — *Epic B0/B1a*

**Function.** A Petri net's linear theory is defined over ℤ and its field of fractions ℚ. Checkable algebraic certificates cannot be produced over `f64`. F1 supplies an exact scalar type and exact matrix operations — fraction-free (Bareiss) at the core, so that intermediate magnitudes stay polynomial.

**Types.**
- `Rational`: a normalized exact rational with an exact zero-test. Its representation and overflow policy is the first engineering decision of the program: `i64/i64` normalized is fast but overflows on large MCC instances; the alternatives are `i128` with overflow detection and promotion, or a bignum dependency (`num-rational`, currently absent from the dependency tree). This decision gates F1 and affects the cost of every downstream proof.
- `Matrix<S>` / `Vector<S>` over `S ∈ {Rational, i64}`, built on the existing dense [`IncidenceMatrix`](../../petrivet/src/core/analysis/incidence.rs) storage (currently `new`/`get` only).

**Algebra and laws.** Field axioms for `Rational`; canonical normalization; exact equality. On matrices, via a **fraction-free Bareiss elimination**: `rank`, `kernel` (right null-space basis — the T-semiflows), `left_kernel` (kernel of the transpose — the P-semiflows), `solve` (a particular solution plus the null-space), and `farkas_certificate`: on an infeasible system `Cx = b, x ≥ 0`, return an *exact* dual `y` with `yᵀC ≥ 0` and `yᵀb < 0`. **Required law: every returned object is verifiable by one exact dot product.** Smith/Hermite normal form is deferred to a scoped sub-task (B0b) used only by integer-marking refinement and minimal-semiflow extraction; SNF is the wrong default cost.

**Enables.**
1. Replacement of the floating-point marking-equation and boundedness deciders with exact versions, so they can emit certificates (resolves §1).
2. `rank(C)`, which with the cluster count from F4 gives the Rank Theorem (well-formed ⇔ `rank = c − 1`).
3. Exact extraction of the Farkas dual currently computed and discarded at `reachability.rs` line 177 — a P-invariant certificate for unreachability, re-checked by one dot product. The exact recheck is also what closes the silent false-`Unreachable` hole (§1; B1a): a negative verdict returns only after `y·C = 0 ∧ y·(m'−m₀) ≠ 0` holds in exact arithmetic, or after null-space membership of `m'−m₀` in `ker(Cᵀ)` is decided over ℚ.
4. Null-space bases, which are the semiflows of F2.
5. The Esparza–Melzer refinement loop (marking equation plus trap/siphon constraints), which strengthens the order-insensitive over-approximation.

**Integration point.** Replace the `f64::from(...)` LP assembly in `semi_decision.rs` with exact solves; retain `good_lp`/`microlp` only as an *inexact filter that never constructs `Proven`/`Refuted`* — it may suggest, never decide.

### F2 · Semiflows and invariants  — *Epic B1*

**Function.** The conservation laws of a net are the kernels of `C` and `Cᵀ`; the canonical generators are the minimal non-negative ones.

**Types.** `PInvariant` (a place weighting `y` with `yᵀC = 0`), `TInvariant` (`Cx = 0`), and the minimal semiflow generators (Colom–Silva / Martínez–Silva). These are the `Invariants` type and `compute_invariants` function that [`literature.rs`](../../petrivet/src/literature.rs) references but that do not exist.

**Algebra and laws.** The work splits along a sharp tractability seam the backlog makes explicit:
- **The certificate (polynomial, fast path).** A *single separating* invariant attached to a negative verdict. The emitted invariant must be exact-rational and pass `y·C = 0 ∧ y·(m'−m₀) ≠ 0` in exact arithmetic *before the verdict returns*. One dot product checks it; it stays on the fast path.
- **The coverage (worst-case exponential, off the fast path).** `is_covered_by_s_invariants` / `is_covered_by_t_invariants` via minimal-semiflow generators is worst-case exponential; it is computed lazily and capped. S-invariant coverage implies conservative and bounded; T-invariant coverage implies consistent; each is verifiable in O(arcs) by one exact multiplication.

**Enables.** Conservativeness and consistency as certificates; structural boundedness as a checkable P-subinvariant rather than an `f64` vector (the polynomial *prove-bounded* certificate on general nets — backlog A4 — emitting `PositivePlaceSubvariant(y)` with the derived per-place bound `⌊(y·M₀)/y[p]⌋` and falling back to the coverability graph only when the LP does not decide); the soundness witness for implicit-place removal (Epic F); and the interface-correction term in Φ_inv (§8).

---

## 5. Validation layer

### F3 · Certificate and verdict calculus, checked against the original net  — *Epic A1 + Epic C1*

**Function.** A verdict is a checkable witness rather than a boolean; "undecided" is a distinct value, not an error; and trust is confined to the checkers — each of which re-establishes the property *against the original net*.

**Types.**
- `Verdict<P, N> = Proven(P) | Refuted(N) | Inconclusive`: three-valued, so that `Inconclusive` is type-distinct from `Refuted`. This type removes the L0 defect at [`liveness.rs`](../../petrivet/src/api/system/liveness.rs), where an unbounded net's transitions and a genuinely dead transition both receive `L0` — "unknown" and "provably non-live" become distinguishable.
- `trait Certificate { fn check(&self, net, m0, query) -> bool; }`. **The `query` argument is required**: a firing-sequence witness is meaningless without the target it claims to reach; a query-free property passes a unit `Query`. A firing sequence is checked by replay against `fire`/`is_enabled`; a Parikh vector by recomputing `m₀ + C·σ`; a P-invariant by one exact dot product (F1); a siphon/trap pair by re-evaluating its closure conditions. Certificate payloads are **owned and serializable** — they are the training label and the external-checker input (C6).
- `fn accept<C: Certificate>(c, net, m0, query) -> Verdict`: the only public constructor of `Proven`/`Refuted`. This function is the TCB in code.
- The `crate::model` module referenced by `literature.rs` (broken at line 409), which is the location of the per-property certificate types and the `Verdict`. Consolidating it resolves the five heterogeneous result shapes across `api/system/*` and unblocks `petrivet-wasm` (A3).

**The original-net checking invariant (the single most load-bearing decision).** Each checker must re-establish the property against the **original** `(net, m0, query)`, assuming nothing about which decider or reduction produced the witness, and sharing no code with the generators beyond primitive net access (`fire`, `is_enabled`, exact dot product). This is the line that:

- keeps the **trusted base constant under reduction-lifting** — a certificate produced on a reduced residual is `lift`ed and re-checked against the *original* net by the unchanged checker, so a buggy reduction or a buggy `lift` costs time, never correctness (Epic F);
- makes the format **tool-agnostic** — a certificate from a *different* generator (or, in principle, another tool) for the same `(net, query, verdict)` validates identically;
- makes `f` and the trusted-base size **meaningful** — the checkers are the only code whose correctness is load-bearing, and they are few, small, and replay-only (no solver, no graph machinery on the dot-product/replay paths).

**Algebra and laws.** Soundness law: `check(net, m0, query) == true ⇒ the claimed verdict holds for (net, m0, query)`; this is the only property trust depends on. Policy independence: the value of an accepted verdict is invariant under which decider produced the candidate and in what order — the soundness theorem stated as a structural property rather than an empirical one, and (per the inversion) the *enabling* property, not the headline.

**Enables.** The two `Some(false)` stubs — the marked-graph liveness arm in `liveness.rs` and [`is_covered_by_s_components`](../../petrivet/src/api/net/mod.rs) line 270, which returns a hardcoded `false` consumed for live FC nets at `boundedness.rs` line 67 — must either carry a witness or return `Inconclusive`/`None`; the type does not admit an unwitnessed positive verdict. Offline auditability of every result. Safety of the learned selection layer: a learned scheduler can change only which proofs are attempted, never whether an invalid one is accepted.

### F3′ · The certificate format and the checkable frontier  — *Epic C6/C7*

Two deliverables ride on F3 and are specified here because they are part of the trust architecture, not the algebra.

**An interoperable certificate format (C6).** Petri-net model checking lacks the DRAT/LRAT/GRAT/VeriPB analogue the SAT/ILP world has. Petrivet's proof objects are already owned, serializable, and borrow-free — the format is one `serde` derive and one net-anchoring convention away. Anchor certificates to PNML place/transition **names** (not internal indices): `Cert = (net_id, query, polarity, witness, theorem_id)`. The F3 checkers consume it; a hand-authored certificate for a verdict produced by a different procedure checks identically against the original net. Cross-tool adoption is recorded as a position, not a built claim.

**The map of the checkable frontier (C7).** Certificate strength is sharply non-uniform, and mapping it is a thesis deliverable (the per-property × polarity table of §6):

- *Near-linear-checkable:* positive reachability/coverability (firing words); LP-refuted unreachability/uncoverability (Farkas P-semiflows — one dot product); structural boundedness (place invariants); unboundedness (Karp–Miller self-covering lassos); k-safety; deadlock existence.
- *Polynomial:* free-choice liveness (an *exhibited* siphon/trap cover — checking the cover is linear even though enumerating all siphons is hard).
- *The wall:* general (non-free-choice) liveness has no known compact checkable certificate; *integer-only* infeasibility has no single Farkas dual (its honest witness is a cutting-plane / VeriPB-shaped derivation, worst-case super-polynomial).

Two emitted witnesses are currently incomplete and must be enriched before they check: the coverability ω-witness lacks the **pumping cycle** (lasso) the checker needs, and `LivenessMethod::MarkedGraph{}` carries no **circuit-token** data. A checkable liveness certificate for one class strictly beyond free-choice is the standout open, high-novelty target.

---

## 6. Routing and observation layer

### F4 · Quotient, sub-net, and composition  — *Epic B2 (quotient), B3 (decomposition)*

**Function.** The recurring operation is the quotient (collapsing a coupling into equivalence classes) and its inverse, the induced sub-net; composition is the adjoint operation.

**Types.** `Partition` over places ∪ transitions (union-find over the existing sorted preset/postset `UniqueSortedSlice`s) with the cluster equivalence (transitive closure of the flow relation), yielding the cluster count `c`. `SubNet`, the net induced by a unit or component, itself a `DenseNet`, so existing analyses apply recursively. `Interface`, the shared boundary places. `compose`, with a property-specific operator `⊗` (verdict join for booleans, direct sum plus an F2 interface correction for invariant spaces, maximum for place bounds).

**Algebra and laws.** Partitions form a refinement lattice; the cluster quotient is the finest one the Rank Theorem requires. The cheapest keystone in the algebraic core: one union-find yields the partition near-linearly and unlocks *both* the Rank-Theorem count `c` (checked against `rank(C) = c − 1` once F1 lands) and S-/T-component extraction. The parsed-but-unused NUPN unit forest ([`nupn.rs`](../../petrivet/src/api/pnml/nupn.rs)) becomes a `Partition` source — and carries its own free `unit_safe` (one-token-per-unit) safety certificate (backlog B8), if the forest is preserved through conversion rather than flattened. Law for `⊗`: associative, with `compose(parts) = whole` exactly when the property factors over the cut.

**Enables.** The cluster count for the Rank Theorem (and the certified replacement for the `is_covered_by_s_components` stub via S-component decomposition with exact free-choice bounds); the S/T-component decompositions; certified reductions (Epic F); and the partition lattice that Φ_bound and Φ_inv minimize over (§8).

### F5 · Decider, policy, and driver  — *Epic D1; the registry precedes Epic B*

**Function.** Make the execution schedule a parameter. Represent each decider as a value with metadata, order them by a policy, gate every acceptance on a certificate. The registry is sequenced **before** the structural generators (Epic B): B's generators are most cleanly *born* as `Decider` impls (and `Reduction` witnesses), so the registry refactor should precede them.

**Types.** `trait Decider { fn polarity() -> Polarity; /* ProveYes | ProveNo | Exact */ fn cost_class() -> CostClass; fn admissible(NetClass) -> bool; fn run(net, query, budget) -> Outcome<Verdict>; }`; a `Policy` whose default reproduces the current hand-coded `match self.class()` cascade exactly; a `Driver` that selects admissible deciders by domain, orders them by the policy, gates on the certificate, and returns the first `Proven`/`Refuted` or else `Inconclusive`. Associated type: `Budget`/`Cancellation`, a cooperative cancellation token threaded into the exploration loop and the solver calls — absent today (the only early exit is the ω short-circuit and process-level `catch_unwind`), and a prerequisite for both the adaptive policy (Epic D-ladder) and bounded per-decider measurement.

Polarity is not new metadata invented here: it is latent in the split proof enums and the prove-NO-only LP filters, and the registry surfaces it (backlog A6). It is what lets the two new deciders below declare themselves cleanly.

**Two new deciders the registry admits (backlog B10, B11).**
- **Continuous (fluid) relaxation — a class-agnostic `ProveNo` decider (B10).** The continuous relaxation (markings in ℝ≥0, fractional firing) is a sound over-approximation of discrete reachability/coverability, and continuous reachability/coverability/boundedness are **PTIME** (Fraca–Haddad 2015). It is the natural apex of the LP→ILP cascade already in `semi_decision.rs`, strictly tighter than the state-equation LP, and uniquely **class-agnostic**: it can decide *general, unbounded* instances at the ω-frontier where `reachability.rs` returns `Inconclusive` today. Witness: the Farkas/place-invariant `y` (F2) for the algebraic refutation, or the maximal firing set plus blocking empty siphon for the firing-set refutation. Checker (original-net): a dot product, resp. a polynomial firing-set fixpoint recompute.
- **General-net deadlock-*free* siphon certificate (B11).** The CHC engine ([`siphon_trap.rs`](../../petrivet/src/core/analysis/siphon_trap.rs) line 370) is, for *general* nets, a sound *sufficient* condition for deadlock-freedom (the `Ok` arm: every minimal siphon contains a marked trap). The dispatch discards this certifying value outside free-choice. Exposing it yields a certifying deadlock-free verdict for general nets; the checker confirms each exhibited place set is a siphon containing a marked trap — linear per pair, the generator bearing the enumeration cost. (The *converse* — an unmarked siphon as a reachable-deadlock witness — is **not** sound in general and is explicitly excluded.)

**Enables.** The cascade as data; the learned selection layer; parallel racing of deciders; anytime results.

### F6 · Measurement and cost (downstream only)  — *Epic D2/D3; G the rig*

**Function.** Execution cost has no canonical origin (machine speed is an additive shift in log-cost); only differences are comparable across machines. The layer observes and does not act.

**Types.** `Cost` in an affine space (raw fibers tagged with run context; derived differential invariants — rankings and log-ratios). `Observation` and the differential `FitnessComparison`. `Features` φ(N), assembled from the cached accessors (`NetClass`, strong connectivity, counts) plus the F2 invariant dimensions, F4 component and cluster counts, and the NUPN unit-tree shape. A separate crate (`petrivet-observe`) whose dependency arrow points only at the core (CI-enforced via `cargo tree`). Specified in the [self-measurement harness plan](../self-measurement-harness-plan.md).

**Enables.** The self-labeling training set; machine-portable regression tests; an always-on **soundness sentinel** that detects trusted-but-incorrect deciders (every `Decided` row with a known oracle must agree — the live regression for the §3 stub fixes). This sentinel, the corpus driver, and the differential no-regression test are the substrate the thesis rig (Epic G) promotes to thesis grade, and the source of the headline `f_struct`.

---

## 7. Transversal component

### F7 · Order abstraction (second phase)  — *Epic B9; far horizon H1*

**Function.** The engine is generic over the fiber (`TokenOps`) but not over the order; the order is the one un-abstracted element (the hand-rolled `impl<T:Ord> PartialOrd for IdxMarking<T>` and the `merge_ordering` fold in [`marking.rs`](../../petrivet/src/core/marking.rs)). `Omega` is the ideal completion of ℕ and the coordinatewise ω-promotion is the *ideal join*, neither captured by `TokenOps`. F7 abstracts it.

**Types.** `trait WellQuasiOrder` (with the test-enforced wqo obligation that licenses termination); a generalization of `Omega` to `Ideal<D>` (the ideal completion of an arbitrary WQO, with a `join`-based acceleration; `Omega` is the ℕ instance). ω-acceleration is restated as "join with the limit of the dominating chain."

**Enables.** The *near-term* payoff is not the WSTS zoo (that is the far horizon, H1) but converting the blanket `Inconclusive` at the ω-frontier into an Abdulla-style **backward-coverability refinement** carrying a partial over-approximation certificate (backlog E1; [`UniqueSortedSlice`](../../petrivet/src/core/unique_sorted_slice.rs) is the natural finite-basis representation). Because it lowers the `Inconclusive` rate it both widens coverage and increases training signal; it is a second phase, not future work. The full WSTS reuse — lossy channel systems, ν-nets, broadcast protocols, BVASS — is recorded as a horizon.

---

## 8. Dependent component

### F8 · Compositional analysis — the two factorization residuals  — *Epic H (dissolved to H2a/H2b)*

The scalar `Φ_PN = min over the partition lattice of distance(whole verdict, ⊗ of part verdicts)` is **dissolved** (§0). The single net-level scalar, the boolean-verdict version (assume-guarantee reasoning, a mature field), and the necessity claim do not survive. What remains are two *per-property* residuals, each computable, monotone, theorem-backed-zero, and non-vacuous; the **deliverable is their measurement over the corpus**, indexed by `NetClass`, not the metaphysics.

- **Φ_bound — the boundedness-factorization residual (schedule first; backlog H2a).** `Φ_bound = min over cuts (NUPN units / S-component cover) of Σ_p (b⊗(p) − b(N)_p)`, where `b⊗(p)` is the in-block structural bound with interface transitions made free. Monotone (`b⊗ ≥ b`), so well-signed; **provably 0** on live bounded free-choice nets (Hack) and strongly-connected T-nets (the circuit theorem); **> 0** (indeed ∞) on nets bounded only through cross-block synchronization. The minimizing cut is an F3-checkable witness.
- **Φ_inv — the invariant rank-defect residual (the novel Rank-Theorem link; backlog H2b).** `Φ_inv = min over cuts of dim ker(Cᵀ) − dim(⊕ block-local invariants, interface-corrected)` — the count of conservation laws no single block can see. Integer, basis-free, non-negative (with the explicit interface correction zeroing interface-coupled coordinates). Its link to the Rank Theorem (`rank C = c − 1`, hence to the cluster count `c` of F4) is the genuinely novel object, pending a full literature check against Kronecker / compositional methods.

Scope constraint: these are the factorization-residual quantities only; no stochastic or integrated-information-theory apparatus is introduced. The qualitative analyzer lacks the stochastic semantics that apparatus requires, and IIT is absent from the repository.

---

## 9. Dependency graph

```
        F1 exact LA ──────────────┬───────────────┐
         │  (root; Bareiss core)   │               │
         ▼                        ▼               ▼
        F2 invariants        F4 quotient /     (Farkas dual →
         │                   subnet / compose   F3 negative certs;
         │                        │             B1a exact recheck)
         ▼                        ▼               │
   ┌──────────────  F3 certificate / verdict calculus  ─────────┐
   │  Verdict + Certificate(check against ORIGINAL net) + accept │
   │  gate are independent and come first; algebraic certificates│
   │  require F1; subsumes the two Some(false) fixes, the L0     │
   │  repair, the format (C6) and the frontier map (C7)          │
   └───────────────────────────┬───────────────────────────────┘
                               ▼
                       F5 decider / driver / budget  (registry BEFORE Epic B;
                               │                       admits B10 continuous,
                               ▼                       B11 deadlock-free)
                       F6 observation  (separate crate) → G rig → f_struct

   F7 WQO / Ideal  ── independent, second phase ──►  (coverage)
   F8 Φ_bound, Φ_inv  ◄── requires F1 + F3 + F4  (dependent, measured, last)
```

There are two roots: F1 (exact linear algebra) and F3 (the certificate calculus). Both are prerequisites; the remaining components compose from them. The certificate calculus is independent of the numerics and lands first; the numerics make its algebraic certificates valid.

---

## 10. Sequencing and cross-cutting decisions

**Implementation order** (detailed, with falsifiable gates, in the [backlog](foundations-backlog.md)):

0. **Fix the soundness defects, and measure the floor.** The two `Some(false)` stubs demoted to abstention; the silent PNML corruptions made hard errors; the floating-`Unreachable` audit; *and, in parallel,* the structural-coverage floor `f_struct` measured **now**, before building anything — it is the cheapest decisive experiment and the baseline Epic B is judged against. (Backlog A2/A5/A7, B1a; G4a.)
1. **F3 first.** The `Verdict`/`Certificate`/`accept` scaffolding with the `query` argument and the original-net invariant, plus `check()` for the already-structured witnesses (firing sequence, Parikh, siphon/trap, ω-marking). Removes the L0 defect and forces the two stubs honest. Requires no new numerical code. Then the certifying audit (`f`, the trusted-base ledger) and the wasm unblock.
2. **The checkers and the format.** Per-certificate checkers (original-net), in-band checking (verify-on-return), the trusted-base ledger and `f`, the interchange format, the checkable-frontier map. *This is the signature contribution; it precedes the generators that feed it.*
3. **F1 second among the algebra.** Resolves the floating-point defect (validating F3's algebraic certificates), then enables F2, rank, and Farkas duals. Self-contained classical mathematics; the only significant decision is the rational representation.
4. **F4 / F2 structural layer.** The cluster quotient (cheapest keystone), invariants, the Rank Theorem, S/T-components — the names dangling in `literature.rs`, and the two new deciders (B10, B11).
5. **F5 routing**, with the budget/cancellation token; **F6 observation** and the **Epic G rig alongside**; then **certified reductions (Epic F)** and the **learned ladder (Epic D5–D8)** as the gated sequel; **F7 / WSTS reuse** and the **Φ residuals** as the horizon.

**Decisions required before implementing F1/F3:**
- *Exact-arithmetic representation*: `i128` with promotion vs. a bignum dependency. Gates F1; affects every downstream proof's cost.
- *Certificate location and anchoring*: witnesses are already name-based at the API boundary (`FiringSequence(Box<[Transition]>)`), so `check()` maps back through the existing `Mapping`; the `crate::model` module is their location, and PNML names (not indices) are the cross-tool anchor.
- *TCB lint*: a CI assertion that the trusted set imports nothing from the producer/observer set, making the boundary enforced rather than documented; and the bare-boolean trusted-base set reported and **non-increasing**.

---

## 11. Crosswalk: the F/M numbering against the A–H epics

This document's F1–F8 components and the backlog's M0–M11 milestones predate the ratified [`BACKLOG.md`](../../BACKLOG.md). They are reconciled — not replaced — by the following crosswalk. No third, conflicting plan is introduced; where the two diverged, the ratified epic governs and the foundations text above has been brought into line.

| Foundations (F / M) | Ratified epic (BACKLOG.md) | Notes on reconciliation |
|---|---|---|
| F3 `Verdict`/`Certificate`/`accept` | **A1** (contract) + **C1** (checkers) | Split: A1 lands the *types*; C1 lands the *checkers*. The `query` argument and the original-net invariant are made explicit (were implicit in F3). |
| the two `Some(false)` fixes; L0 repair | **A2** (north star), **A5** | Were folded into F3/M1; promoted here to the standalone near-term north star. |
| PNML fidelity (not in original F-set) | **A7 / E4 / E7** | *Added*: the silent import corruptions are a soundness sibling of A2, absent from the original foundations text. |
| F1 exact LA | **B0** (+ **B0b** SNF) | "Fraction-free (Bareiss)" made explicit; the float-`Unreachable` audit broken out as **B1a**. |
| F1 Farkas-dual recheck | **B1a** | *Added* as a distinct, A2-priority audit of the negative path. |
| F2 invariants | **B1** (certificate vs. coverage split) | The single-separating-invariant certificate (fast) is split from the exponential coverage (capped). |
| structural-boundedness witness | **A4** | Reframed from "replace the f64 vector" to a polynomial general-net *prove-bounded* decider emitting `PositivePlaceSubvariant`. |
| F4 quotient / cluster count | **B2** | The cheapest keystone; precedes decomposition. |
| F4 S/T-components, sub-net | **B3** (+ **B4** T-nets, **B5** Rank Thm, **B6** FC reachability) | F4's decomposition fans out into B3–B6. |
| F4 NUPN partition | **B8** | The `unit_safe` certificate is surfaced; the forest preserved through conversion. |
| (siphon/trap engines) | **B7** (`Closure` family) | *Added*: consolidate the engines; cap the exponential enumeration. |
| (new deciders) | **B10** continuous, **B11** deadlock-free | *Added* by the ratified plan; admitted by the F5 registry. |
| F5 decider/driver/budget | **D1** (+ **D6** cancellation) | Sequenced *before* Epic B (was after, in M8). Cancellation broken out as D6. |
| F6 observation | **D2 / D3 / D4** | The soundness sentinel is D3. |
| (the rig) | **Epic G** (G1–G8, **G4a** floor) | *Added/sharpened*: the thesis rig and the headline `f_struct`; G4a is runnable now. |
| F7 WQO / Ideal | **B9** (near-term) + **H1** (horizon) | Split: B9 is the backward-coverability payoff; H1 the WSTS zoo. |
| F8 Φ_PN (scalar) | **dissolved** → **H2a** Φ_bound, **H2b** Φ_inv | The scalar is gone; two measured per-property residuals remain. |
| (reductions) | **Epic F** (F0–F2) | *Added*: the Rung-3 apparatus, outside the TCB by the original-net check. |
| (learned ladder) | **D5–D8** | The SATzilla-style sequel; gated behind the checker. MCC ranking is **OUT**. |
| standing invariants A1–A4 (M-doc) | **standing invariants** (backlog) | Renamed to avoid collision with Epic A; carried verbatim as gates. |

**Where the two plans diverged, and how they were reconciled.** Three substantive divergences:

1. **Registry placement.** The original M-sequence put the `Decider` registry (F5/M8) *after* the algebraic deciders. The ratified plan moves it (D1) *before* Epic B, so the generators are born as `Decider`s rather than retrofitted. The text above (F5, §10) now reflects the earlier placement.
2. **The scalar Φ.** The original F8/M11 built a single scalar `Φ_PN`. The ratified plan dissolves it to two measured residuals. F8 and §8 are rewritten accordingly; the metaphysical framing is removed.
3. **Scope additions.** PNML fidelity (A7), the float-`Unreachable` audit (B1a), the two new deciders (B10/B11), the siphon/trap consolidation (B7), the certificate format and frontier map (C6/C7), the reductions (Epic F), and the thesis rig (Epic G with the `f_struct` floor) are all present in the ratified plan and were thin or absent in the original foundations text. They are folded in above; none contradicts the original design, and each strengthens the same trust boundary.

---

*Methodology: derived from a line-by-line reading of the codebase — the state-space engine, the class-gated cascades, the floating-point LP layer, the siphon/trap closures, and the `literature.rs` citation index — and reconciled to the ratified [`BACKLOG.md`](../../BACKLOG.md). Every statement about current code is checked against a file/line reference; every proposed type is identified as proposed. The companion [implementation backlog](foundations-backlog.md) sequences this design into gated milestones.*
