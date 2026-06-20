# The Checkable Frontier
## A certificate format and a single checker family for behavioural Petri-net verification — and a map of where compact proofs are forbidden

> Status: vision essay. Exploratory writing (Daniel Dyer, with Claude), built on Michael's tool. The core Petri-net theory is Michael's; the framing and the map are proposed here. This essay owns the **certificate format**, the one load-bearing checker decision, and the **per-property × polarity hardness map** — the heart of the project's signature contribution. The broader inversion is stated once in [README.md](README.md). Companion to [`soundness-as-a-free-variable.md`](soundness-as-a-free-variable.md), which proves this apparatus makes a learned scheduler safe, and [`the-coverage-claim.md`](the-coverage-claim.md), which measures how much of the corpus it reaches.

---

## 1. The missing artifact

The SAT, SMT, and ILP communities settled a question that Petri-net model checking has left open: *what does a model checker hand you when it says "no"?* In SAT the answer is a **DRAT** proof — a sequence of clause additions and deletions that a small, separately written checker replays against the original CNF, deciding UNSAT without trusting the solver that produced it. **LRAT/GRAT** add the hints that make checking linear and the checker verifiable; **VeriPB** extends the discipline to cutting-plane reasoning; SMT solvers emit proof terms an external kernel re-derives. The shared invariant: *the prover is untrusted; a small checker re-establishes the claim against the original problem; the proof is a serialized artifact that crosses tool boundaries.* (The lineage is detailed in §6.)

Behavioural Petri-net verification has no such artifact. A reachability checker reports `true`; a liveness checker reports `L4`; a boundedness checker reports a bound — and the caller either re-runs a second tool and compares verdicts, or trusts the first. There is no DRAT for "this marking is reachable," no LRAT for "this place is bounded by 3," no shared format a competing tool could emit and a neutral referee could check. The Model Checking Contest, the field's central crucible, adjudicates by *consensus of tools* precisely because no tool produces a checkable proof: the oracle file encodes `?` when the tools disagree and no human has resolved it ([`oracle.rs`](../../mcc-tests/src/oracle.rs)).

The claim of this essay is that `petrivet` is one `serde` derive and one naming convention away from supplying the missing artifact, and that doing so *systematically, per property and per polarity, behind one checker family* is the unoccupied ground. The proof objects already exist. The analyses do not return booleans; they return owned, serializable evidence:

- reachability returns a `ReachabilityProof` — a `FiringSequence(Box<[Transition]>)`, a Parikh vector `HashMap<Transition, u32>`, or a token-conservation scalar — or an `UnreachabilityProof` naming the theorem that refuted it ([`reachability.rs`](../../petrivet/src/api/system/reachability.rs):42, :65);
- coverability returns a `CoverabilityProof { firing_sequence, covering_marking }` where the covering marking may carry ω ([`coverability.rs`](../../petrivet/src/api/system/coverability.rs):46);
- boundedness returns per-place `Boundedness` with a `PositivePlaceSubvariant(y)` weight vector or a coverability-graph witness ([`boundedness.rs`](../../petrivet/src/api/system/boundedness.rs):46);
- liveness returns per-transition levels with a `LivenessMethod` recording *how* — `Circuit`, `StateMachine`, `MarkedGraph`, `FreeChoice { commoner_hack_result }`, `ReachabilityGraph` ([`liveness.rs`](../../petrivet/src/api/system/liveness.rs):60);
- the Commoner–Hack criterion returns `Result<Box<[SiphonTrapPair]>, UnmarkedSiphonTrapPair>` — every siphon with its marked trap on success, the one starving siphon on failure ([`chc.rs`](../../petrivet/src/api/system/chc.rs):40).

These are not logs. They are witnesses whose shape is dictated by the theorem that produced them — the proof-carrying stance of [Principle 4](principles.md). What is missing is three things: a *format* that serializes them tool-agnostically; a *checker* that re-establishes the property against the original net; and a *map* of exactly where such a checker can be near-linear, where it must be polynomial, and where complexity theory forbids it being compact at all. This essay supplies all three as claims, each with its falsifier.

---

## 2. The format

> **Claim.** The certificate is a five-tuple `Cert = (net_id, query, polarity, witness, theorem_id)`, anchored to PNML place/transition *names* rather than internal indices, and it already exists as borrow-free serializable data — the format is a `serde` derive and a name-anchoring convention, not new theory.
>
> **Falsifier.** A witness variant that cannot round-trip through serialization without losing information the checker needs; or a witness that cannot be expressed over names because it depends on an internal index that has no PNML counterpart. (Two such witnesses exist today; §5 names them.)

The five fields are exactly the information a neutral checker requires and no more:

- **`net_id`** — a content hash or manifest reference identifying the *original* net the certificate is about. Trust is anchored to a specific `(N, M₀)`; a certificate is meaningless without it.
- **`query`** — the target the verdict concerns: a marking for reachability/coverability, a place for a bound, a transition for liveness, unit for a query-free property. The query is not optional: a firing-sequence witness is vacuous without the marking it claims to reach. (This is why the [foundational design](../foundations/foundational-design.md) makes `query` a required argument of `check`, and the [backlog](../../BACKLOG.md) A1 makes a query-free property pass a unit `Query`.)
- **`polarity`** — `ProveYes | ProveNo | Exact`. Reachability-positive and reachability-negative are *different proof systems* with different checkers and different complexities; the polarity tells the checker which obligation it is discharging. This field is latent today in the split proof enums (`ReachabilityProof` vs. `UnreachabilityProof`, `CoverabilityProof` vs. `NonCoverabilityProof`) and in the prove-NO-only LP filters.
- **`witness`** — the payload: a firing word, a Parikh vector, a place-weighting `y`, a siphon/trap cover, an ω-marking with its pumping lasso. This is the data already owned by the proof enums.
- **`theorem_id`** — the named result that licenses the inference from witness to verdict. The enums already carry this: `LiveStateMachineTokenConservation`, `LiveMarkedGraphMarkingEquationIntegerSolution`, `MarkingEquationNoRationalSolution`. The `theorem_id` is what lets a checker dispatch to the right re-establishment procedure, and what makes the certificate *self-describing* across tools.

The single non-obvious design rule is **anchoring to names, not indices**. Internally a net is a `DenseNet` with `u32` place and transition indices assigned at build time; the witnesses are stored over those indices and translated back through the `Mapping` at the API boundary (`FiringSequence(Box<[Transition]>)` is already a sequence of public `Transition` handles). But indices are a private, build-order-dependent encoding. A certificate that says "fire transition 7" is checkable only by the tool that assigned 7. A certificate that says "fire transition `release_lock`" — the PNML `<transition id="release_lock">` — is checkable by *any* tool that parsed the same PNML file. Name-anchoring is what converts an intra-tool witness into an interoperable artifact, and it is the difference between a `serde` derive that serializes private state and a *format*. This is the C6 item of the [backlog](../../BACKLOG.md): the proof objects are "owned, serializable, borrow-free — the format is one `serde` derive and one net-anchoring convention away."

The honest scope: round-trip-and-check *within* the tool is achievable now. Cross-tool adoption — another model checker emitting `Cert` and `petrivet` checking it, or vice versa — is a *position*, not a deliverable. The format is designed to make that possible; whether the field adopts it is outside the thesis. What the thesis can show is the property that *makes* adoption possible: a hand-authored certificate, or one produced by a different internal procedure, checks identically against the original net (backlog C6 acceptance criterion).

---

## 3. The trust surface and the one load-bearing decision

> **Claim.** The checker has signature `check(net, m0, query) -> bool`, re-establishes the property against the **original** net, and shares no code with the generators beyond primitive net access (`fire`, `is_enabled`, `get`). This **original-net invariant is the single load-bearing decision** of the whole architecture: it is what holds the trusted base constant under certified reductions, and it is what makes a wrong reduction `lift` cost time rather than correctness.
>
> **Falsifier.** A checker that calls solver or graph machinery shared with a generator (so a generator bug could be replicated in its own checker); or a checker that validates against a *reduced* residual net rather than the original (so a wrong reduction could pass a too-weak check). Either collapses the trust boundary.

The reason the original-net invariant is load-bearing, and not merely tidy, is the reduction calculus. A certified reduction (Epic F of the [backlog](../../BACKLOG.md); [`the-sequel.md`](the-sequel.md)) transforms a net into a smaller residual, decides the residual, and *lifts* the residual certificate back to a verdict on the original. The `lift` is the hard part — it is "the real work" of any reduction library, and it is exactly where a subtle bug hides. The architecture's defence is brutally simple: **the lifted certificate is checked against the original net by the unchanged checker.** If the `lift` is wrong, the re-padded witness fails to replay on the original `(N, M₀)`, the checker rejects, and the search backtracks. A wrong `lift` wastes the time the reduction took; it cannot produce a wrong answer. This is the property that places the entire reduction library — and any learned planner over it — *outside* the trusted computing base.

That defence works only if the checker is *literally* re-establishing the property on the original net, using nothing the generator used to produce the witness. Three consequences follow, and each is a discipline the code must hold:

1. **No shared code with generators.** A firing-sequence checker replays `fire`/`is_enabled` from `M₀` and reads the final marking; it must not call the state-space explorer that *found* the sequence. A Farkas checker computes one exact dot product `y·(m′−m₀)` and confirms `y·C = 0`; it must not call the LP solver that *found* `y`. If a checker reused the generator's machinery, a generator bug would be faithfully reproduced in its own checker — the certificate would check, and be wrong. The replay/dot-product checkers "invoke no solver/graph machinery" (backlog C1 acceptance criterion).

2. **The checker is the entire trusted base.** Everything else — deciders, the selection policy, reductions, any learned model — may contain errors without affecting correctness, because `Verdict::Proven` can be constructed *only* by passing a witness through its checker (the [foundational design](../foundations/foundational-design.md) §3 makes this a type invariant: `accept` is the only public constructor). The trusted base is `{the C1 checkers} ∪ {whatever deciders still return a bare boolean}`, and the figure of merit is the **certifying fraction `f`** — the share of accepted verdicts that carry a checked certificate. The GRAT discipline applies verbatim: shrink the checker, then verify *it*; the generators stay fast and untrusted.

3. **In-band checking makes the guarantee operational, not aspirational.** A verified-decision entry point runs the checker *before returning the verdict on the decision path*, not merely in tests (backlog C4). For ordinary deciders this is a redundant safety net; for *lifted* certificates it is mandatory, and it is the single line — `if certificate.check(net, m0, query)` — that makes a buggy reduction cost time and never correctness.

The contrast with the rest of the verification-ML landscape is exact and is developed in [`soundness-as-a-free-variable.md`](soundness-as-a-free-variable.md): MuZero learns the dynamics model *inside* the trust boundary and its correctness is empirical; `petrivet` keeps every learner outside, and correctness is structural, because the checker re-derives the fact from scratch.

---

## 4. The map of the checkable frontier

This is the heart of the contribution. Certificate strength is *sharply non-uniform*: some verdicts carry a witness a child could check in one pass; some need polynomial work; and some are, by the structure of the problem, *forbidden* a compact checkable certificate. The deliverable is not a tool that emits proofs — many tools emit *something*. The deliverable is the **boundary**: a per-property × polarity table that says, for each verdict shape, what the cheapest sound checker costs, with the complexity-theoretic justification where the cost is super-polynomial. To my knowledge no behavioural model checker has drawn this map.

A note on what "checkable in time *t*" means here, and why it is the honest measure. The cost that matters is the cost of the *checker*, not the *generator*. Enumerating all siphons of a net is worst-case exponential; *checking* an exhibited siphon/trap cover is linear. The asymmetry is the entire point of certifying algorithms: the prover may sweat, the verifier must not. So the table below grades each cell by **checker** complexity, and flags separately where the *generator* is expensive (which affects coverage, not trust).

### 4.1 The table

| Property | Polarity | Witness | Checker | Checker cost | Status in code |
|---|---|---|---|---|---|
| Reachability | ProveYes | firing word `σ` | replay `fire` from `M₀`, confirm final = target | `O(\|σ\|·d)` | present (`FiringSequence`) |
| Reachability | ProveYes (S-net) | token sum | one scalar comparison + class check | `O(\|P\|+\|T\|)` | present (`LiveStateMachineTokenConservation`) |
| Reachability | ProveYes (live T-net) | Parikh vector `x` | recompute `M₀ + C·x`, confirm = target | `O(arcs)` | present (`LiveMarkedGraph…IntegerSolution`) |
| Reachability | ProveNo (rational) | Farkas / P-semiflow `y` | confirm `y·C = 0` ∧ `y·(target−M₀) ≠ 0`, exact | `O(arcs)` | **witness discarded** (`…NoRationalSolution`) |
| Coverability | ProveYes (bounded) | firing word | replay, confirm covers target | `O(\|σ\|·d)` | present |
| Coverability | ProveYes (unbounded) | ω-marking **+ pumping lasso** | replay prefix to lasso head, confirm cycle strictly pumps the ω-places | `O((\|σ\|+\|loop\|)·d)` | **lasso missing** (§5) |
| Coverability | ProveNo | Farkas `y` | one exact dot product | `O(arcs)` | witness discarded |
| Boundedness | ProveYes (structural) | place invariant `y > 0`, `yᵀC ≤ 0` | confirm sign and `yᵀC ≤ 0`, derive `⌊(y·M₀)/y[p]⌋` | `O(arcs)` | present as `f64`; needs exact `y` |
| Boundedness | ProveYes (k-safety) | place invariant or unit-tree | dot product / NUPN `unit_safe` check | `O(arcs)` | invariant absent; NUPN parsed-but-dropped |
| Unboundedness | ProveNo | Karp–Miller self-covering lasso | replay, confirm the loop is enabled and strictly increases a place | `O((\|σ\|+\|loop\|)·d)` | derivable from coverability graph |
| Deadlock | exists (ProveYes) | dead marking + firing word to it | replay to marking, confirm no transition enabled | `O(\|σ\|·d + arcs)` | reachable (`Deadlocks`) |
| Deadlock-freedom | ProveYes (general net) | siphon cover, each with a marked trap | confirm each set is a siphon and holds a marked trap | `O(pairs·arcs)` | CHC `Ok` arm, **discarded outside FC** |
| Liveness | ProveYes (free-choice) | siphon/trap cover (CHC) | confirm every proper siphon's exhibited trap is marked | `O(pairs·arcs)`, *checking* the cover | present (`FreeChoice`) |
| Liveness | ProveYes (T-net) | every circuit marked | confirm each exhibited circuit holds a token | `O(circuits·\|circuit\|)` | **circuit tokens missing** (§5) |
| Liveness | — (general, non-FC) | **none known compact** | — | **the wall** | honest `Inconclusive` |
| Reachability | ProveNo (integer-only) | **cutting-plane / VeriPB derivation** | replay the derivation, super-polynomial in the worst case | **no single Farkas dual** | `…NoIntegerSolution` (payload-free) |

Read the table as three bands.

### 4.2 Near-linear-checkable (the certifying core)

Everything in the first band shares the certifying-algorithms shape: a witness whose validity is one replay or one dot product against the original net.

- **Positive reachability and coverability** carry a *firing word*. The checker replays `fire`/`is_enabled` from `M₀` — Principle 4's `firing-sequence-checks-by-replay` — and reads the final marking. The state-space explorer is untrusted; the checker re-walks the sequence it produced. Linear in the word length times the branching arity `d` of the touched transitions.
- **Negative reachability and coverability (rational)** carry a *Farkas dual* / P-semiflow `y`: a place-weighting with `y·C = 0` and `y·(target − M₀) ≠ 0`. This is the conservation law that *explains* the impossibility — the discarded dual of [`principles.md`](principles.md) §2.2.a. Checking it is one exact dot product. The witness is computed today and thrown away at `reachability.rs:177` and `coverability.rs:124`; emitting it converts every LP-refuted "no" into a near-linear-checkable verdict symmetric to the positive ones. **The exactness caveat is load-bearing:** the dual must be checked over ℚ, because a *floating* dual is not a proof and a spurious floating "infeasible" is a silent false `Unreachable` (backlog B1a) — the one negative-path hazard the firewall does not catch, since there is no positive object to re-check.
- **Structural boundedness and k-safety** carry a *place invariant* `y > 0` with `yᵀC ≤ 0`; the per-place bound `⌊(y·M₀)/y[p]⌋` follows. The checker confirms the sign condition and the dot product. The same shape covers NUPN `unit_safe`: a parsed-but-currently-discarded one-token-per-unit invariant that is a free safety certificate (backlog B8).
- **Unboundedness** carries a *Karp–Miller self-covering lasso*: a firing word reaching a marking `M`, then a loop returning to a marking `≥ M` that strictly increases some place. The checker replays both and confirms the strict increase — the finite witness standing in for the infinite fact (Principle 2).
- **Deadlock existence** carries a reachable dead marking and the word to it; the checker replays and confirms no transition is enabled there.

These cells are where the contribution is *already real* in the witness shapes, modulo the two repairs of §5 and the exact-arithmetic substrate (backlog B0). The certifying fraction `f` is, concretely, how much of the corpus lands in this band.

### 4.3 Polynomial (the free-choice island)

**Free-choice liveness** is the subtle and important case. Liveness of a free-choice system is decided by the Commoner–Hack criterion: every proper siphon contains a trap marked under `M₀` (Murata Thm 12; Primer Thm 5.17). The witness is the `Box<[SiphonTrapPair]>` already returned by [`commoner_hack_criterion`](../../petrivet/src/api/system/chc.rs). Here is the asymmetry that makes the cell honest:

> *Enumerating all siphons is worst-case exponential. Checking an exhibited siphon/trap cover is linear per pair.*

The generator may pay an exponential price to find the cover (and the [backlog](../../BACKLOG.md) B7 flags exactly this as the real scaling risk on the CHC path). But the *checker* receives the cover as data and confirms two local facts per place set: that it is a siphon (closed under presets), and that the trap it contains is marked. That is the whole content of "polynomial in the table": the verifier is linear, the prover is not, and the certificate is what transports the verdict across that gap. This is also why deadlock-*freedom* for *general* nets earns a cell — CHC's `Ok` arm is a sound sufficient condition for deadlock-freedom on any net (the converse fails and is excluded), and that certifying value is discarded today outside free-choice (backlog B11).

### 4.4 The wall (where complexity forbids a compact certificate)

Two cells are marked **the wall**, and they are the most important entries in the table because they are *negative results stated as claims*.

> **Claim (general liveness).** General (non-free-choice) liveness has no known compact, near-linearly-checkable certificate. Outside the free-choice class the siphon/trap characterization is only sufficient, not equivalent (asymmetric-choice nets already break the iff), and no structural witness is known that an independent checker can replay in polynomial time to *certify* liveness on an arbitrary net.
>
> **Falsifier.** Exhibit, for some net class strictly beyond free-choice, a polynomially-checkable liveness certificate. *This is the standout open target of the whole programme* — a positive resolution would extend the certifying core past its current boundary and is the genuinely hard, high-novelty item (backlog C7).

> **Claim (integer-only infeasibility).** Integer-only reachability infeasibility has no single Farkas dual. When the rational marking equation is feasible but the *integer* one is not, the honest witness is not a place-weighting but a **cutting-plane derivation** — VeriPB-shaped, a sequence of integer-rounding inferences — and its length is worst-case super-polynomial.
>
> **Falsifier.** Produce a single dot-product-checkable dual that certifies integer-infeasibility for the rational-feasible case. Linear-programming duality forbids it: the rational dual certifies only the rational refutation; the integer gap is exactly the content a single dual cannot express. This is the boundary at which the certificate *changes kind* — from a one-line dual to a many-line derivation — and it is the same boundary VeriPB was built to cross in the pseudo-Boolean world.

The payload-free `MarkingEquationNoIntegerSolution` variant ([`reachability.rs`](../../petrivet/src/api/system/reachability.rs):75) sits precisely on this wall: it records *that* the ILP was infeasible without a witness, because the witness it would need is the cutting-plane derivation, not a dual. Honesty here is to state the boundary, not to fabricate a compact witness that the mathematics does not provide. The verdict remains sound — it is produced by a trusted ILP filter — but it sits in the trusted base rather than the certifying fraction, and the table says so.

The discipline the table enforces is the [claim-honesty method](principles.md) §4.3 applied to *certificates*: a cell is either near-linear, or polynomial-with-an-exponential-generator, or on the wall — and each label states which, with its justification. The map's value is that it draws the line precisely.

---

## 5. Two currently-incomplete witnesses

The map is not all aspiration; most of it is implemented witness shapes. But two emitted witnesses are *incomplete* in a way the checker would catch, and naming them is the §2 falsifier discharged honestly.

> **The coverability ω-witness lacks its pumping lasso.** `CoverabilityProof` carries a `firing_sequence` and a `covering_marking` that may contain ω ([`coverability.rs`](../../petrivet/src/api/system/coverability.rs):46). For a *bounded* cover this is complete: replay the sequence, confirm the reached marking covers the target. But when the covering marking carries ω, the firing sequence reaches a node of the *coverability graph*, not a reachable marking — and ω asserts "this place can exceed any finite threshold." A checker cannot confirm that from the prefix alone; it needs the **pumping cycle**: the loop, enabled at the lasso head, that strictly increases the ω-places, so the checker can replay prefix-then-loop and witness the unbounded growth directly. The Karp–Miller construction *has* this lasso internally (it is how ω is introduced); the witness must surface it (backlog C7).

> **`LivenessMethod::MarkedGraph{}` carries no circuit-token data.** The marked-graph liveness method is an empty struct with a `// todo: return circuits and their token counts` ([`liveness.rs`](../../petrivet/src/api/system/liveness.rs):77). The theorem is "live iff every circuit is marked"; the checkable witness is *the circuits and their token counts*, so a checker can confirm each circuit holds a token. Without that payload the verdict is a bare boolean — trusted, not certified — and it sits in the trusted base rather than the certifying fraction. (This is distinct from, but adjacent to, the A2 `Some(false)` stub in the *efficient* marked-graph liveness path, which the [soundness essay](soundness-as-a-free-variable.md) treats as the near-term north star.)

Both are small repairs that move a cell from "trusted" to "certifying," and both are exactly the kind of gap the in-band checker (§3) would expose: a certificate that cannot replay is a certificate the format will not accept.

---

## 6. Positioning: what is new and what is not

> **Claim.** The *systematic per-property × polarity treatment, behind one checker family, for a behavioural model checker* is the unoccupied ground. The existence of certificates for individual Petri-net properties is not new, and the essay does not claim it.

The honest positioning has three reference classes.

**Proof logging (DRAT/LRAT/GRAT, VeriPB, SMT proofs).** This is the direct lineage and the model the contribution imitates. DRAT/LRAT/GRAT gave SAT a re-checkable refutation format and a small verified checker (Lammich, 2017). VeriPB extended it to cutting-plane reasoning — and the §4.4 integer-infeasibility wall is *precisely* the boundary VeriPB exists to cross, which is why the table names a "VeriPB-shaped derivation" rather than a dual. The contribution is the *transplant* of this discipline into behavioural Petri-net verification, where it does not yet exist, plus the observation that the boundary between the cheap-dual band and the cutting-plane band falls at the rational/integer gap — the same place it falls in ILP.

**Certifying algorithms (McConnell–Mehlhorn–Näher–Schweitzer, 2011).** The general theory of a program that emits a witness an independent checker verifies. Individual Petri-net properties have certifying procedures in this sense — a firing sequence certifies reachability, an invariant certifies boundedness — and that is *not* this essay's novelty. What the certifying-algorithms literature does not supply is the *map*: the per-property × polarity boundary that says where, across the whole behavioural-property surface of one model, a compact checker exists and where complexity forbids it.

**FastForward and certifying model checkers.** FastForward (Blondin–Haase–Offtermatt, *TACAS* 2021) uses Petri over-approximations as A* distance oracles and returns the witnessing firing sequence — sound-on-success for *one* property. The contribution generalizes the sound-on-success guarantee from one property to the whole property surface, behind one format and one checker family, and *marks the cells where the guarantee cannot be made compact*.

So the claim is narrow and defensible: not "Petri-net verdicts can carry certificates" (known), but "here is the checkable frontier of behavioural Petri-net verification — one format, one checker family, the per-property × polarity cost of checking, and the two walls where complexity forbids a compact certificate." The boundary itself is the deliverable.

---

## 7. The contribution is proportional to two measured numbers

The strength of this essay's claim is not rhetorical; it is two fractions, and they are measured, not asserted.

The **certifying fraction `f`** is the share of accepted verdicts that carry a checked certificate — the size of the certifying core relative to the trusted base. A contribution that certifies 5% of verdicts and trusts the rest is a curiosity; one that certifies most of a real corpus is a result. `f` is reported over the MCC P/T corpus and the trusted base is required non-increasing in CI (backlog A6/C5/G6).

The **structural-coverage fraction `f_struct`** is the share of *queries decided by the structural tier without state-space exploration* — how much of the corpus lands in the polynomial bands of §4 at all. The two numbers compose: `f_struct` says how much the cheap, checkable tier reaches; `f` says how much of what it reaches carries a checked proof. Their product is the operative size of the checkable frontier on real instances. The measurement is the subject of [`the-coverage-claim.md`](the-coverage-claim.md), and it is the thesis's committed primary claim — the unit of evidence is *a characterization plus a construction*, not a benchmark ranking.

The relationship to the rest of the corpus closes cleanly. This essay is the prerequisite for [`soundness-as-a-free-variable.md`](soundness-as-a-free-variable.md): there is no policy-independence theorem without the checker the theorem is about, and the §3 in-band check is the line of code that proof relies on. [`the-factorization-residual.md`](the-factorization-residual.md) measures what *cannot* be coarse-grained; the present essay measures what *can* be checked. [`latent-architecture.md`](latent-architecture.md) reads the evidence/decision structures all of this is built from.

The boundary, restated as the deliverable: *here is the checkable frontier of behavioural Petri-net verification — the format that carries a verdict across tools, the one checker that re-establishes it against the original net, the per-property × polarity cost of checking, and the two walls (general liveness, integer-only infeasibility) where complexity theory forbids a compact certificate.* The format is one `serde` derive away; the checker is one trait; the map is the thesis. A checkable liveness certificate for a single class strictly beyond free-choice is the standout open target — and the table is what makes it precise enough to attack.

---

### References (curated)

- Lammich. *Efficient Verified (UN)SAT Certificate Checking* (GRAT). CADE 2017 / JAR 2019. Heule, Hunt, Wetzler. *DRAT/LRAT.* (clausal proof logging.)
- Bogaerts, Gocht, McCreesh, Nordström. *VeriPB / Certified pseudo-Boolean and cutting-plane reasoning.* (2022–).
- McConnell, Mehlhorn, Näher, Schweitzer. *Certifying algorithms.* Computer Science Review 5(2) (2011).
- Blondin, Haase, Offtermatt. *Directed Reachability for Infinite-State Systems* (FastForward). TACAS 2021.
- Murata. *Petri Nets: Properties, Analysis and Applications.* Proc. IEEE 77(4) (1989). Desel, Esparza. *Free Choice Petri Nets.* Cambridge, 1995. Best, Devillers. Commoner, Hack (siphon/trap liveness).
- Czerwiński, Orlikowski; Leroux, Schmitz. (the Ackermannian / EXPSPACE complexity frontier underwriting the wall.)
