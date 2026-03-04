//! Literature index: key results from Petri net theory referenced in this library.
//!
//! This module contains no code. It documents the theoretical foundations that
//! the library's analysis algorithms are based on, organized by source.
//!
//! # Sources
//!
//! - **Murata 1989**: T. Murata, "Petri Nets: Properties, Analysis and
//!   Applications," *Proceedings of the IEEE*, vol. 77, no. 4, pp. 541–580, 1989.
//! - **Primer**: E. Best and R. Devillers, *Petri Net Primer*, Springer, 2024.
//!
//! # Murata 1989
//!
//! ## Theorem 4 — Liveness of S-nets (state machines)
//!
//! > A state machine (N, M₀) is live iff N is strongly connected and M₀
//! > has at least one token.
//!
//! Used in: [`SNetLivenessEvidence`],
//! [`System::analyze_liveness`](crate::System::analyze_liveness).
//!
//! ## Theorem 5 — Safety of S-nets (state machines)
//!
//! > A state machine (N, M₀) is safe iff M₀ has at most one token.
//! > A live state machine (N, M₀) is safe iff M₀ has exactly one token.
//!
//! Applicable to safety/1-boundedness analysis of S-nets.
//! Not yet exploited as a shortcut in `analyze_boundedness`.
//!
//! Cited in: [`SNetLivenessEvidence`].
//!
//! ## Theorem 7 — Liveness of T-nets (marked graphs)
//!
//! > A marked graph (G, M₀) is live iff M₀ places at least one token on
//! > each directed circuit in G.
//!
//! Used in: [`TNetLivenessEvidence`],
//! [`System::analyze_liveness`](crate::System::analyze_liveness).
//!
//! ## Theorem 8 — Place bounds in T-nets (marked graphs)
//!
//! > The maximum number of tokens that a place can have in a marked graph
//! > (G, M₀) is equal to the minimum number of tokens placed by M₀ on a
//! > directed circuit containing that place.
//!
//! Provides exact place bounds for T-nets without state-space exploration.
//! Not yet exploited as a shortcut in `analyze_boundedness` (requires circuit
//! enumeration).
//!
//! Cited in: [`TNetLivenessEvidence`].
//!
//! ## Theorem 9 — Safety of T-nets (marked graphs)
//!
//! > A live marked graph (G, M₀) is safe iff every place belongs to a
//! > directed circuit C with M₀(C) = 1.
//!
//! Not yet exploited: could provide an O(circuit-count) safety check for
//! T-nets in `analyze_boundedness` (requires circuit enumeration).
//!
//! Cited in: [`TNetLivenessEvidence`].
//!
//! ## Theorem 12 — Commoner/Hack Criterion
//!
//! > A free-choice net (N, M₀) is live iff every proper siphon of N
//! > contains a trap that is marked under M₀.
//!
//! For general nets, the condition is sufficient for deadlock-freedom but
//! not necessary.
//!
//! Used in: [`commoner_hack_criterion`](structural::commoner_hack_criterion),
//! [`CommonerHackCriterionResult`].
//!
//! ## Theorem 13 — Safety of live free-choice nets
//!
//! > A live free-choice net (N, M₀) is safe iff N is covered by strongly
//! > connected SM-components (S-components) each of which has exactly one
//! > token at M₀.
//!
//! Not yet exploited: combined with S-component analysis, could provide a
//! polynomial safety check for live FC nets in `analyze_boundedness`.
//!
//! Cited in: [`SComponent`].
//!
//! ## Theorem 14 — S-component coverage of live safe free-choice nets
//!
//! > Let (N, M₀) be a live and safe free-choice net. Then every place
//! > belongs to a strongly connected S-component of N.
//!
//! Together with Theorem 13, this shows that live safe FC nets decompose
//! fully into S-components.
//!
//! Cited in: [`SComponent`],
//! [`is_covered_by_s_components`](structural::is_covered_by_s_components).
//!
//! ## Theorem 15 — Liveness of asymmetric-choice nets
//!
//! > An asymmetric-choice net (N, M₀) is live if (but not only if) every
//! > siphon in N contains a marked trap.
//!
//! This is a sufficient (but not necessary) condition for liveness, unlike
//! the equivalence in Theorem 12 for free-choice nets.
//!
//! Used in: [`commoner_hack_criterion`](structural::commoner_hack_criterion).
//!
//! ## Theorem 21 — Reachability in S-nets
//!
//! > For S-nets, the marking equation M₀ + N·x = M' has a rational
//! > solution iff M' is reachable from M₀.
//!
//! The rational (LP) relaxation is exact for S-nets due to total
//! unimodularity of the incidence matrix.
//!
//! Used in: [`System::analyze_reachability`](crate::System::analyze_reachability).
//!
//! ## Theorem 22 — Reachability in T-nets
//!
//! > For T-nets, M' is reachable from M₀ iff there exists a non-negative
//! > integer solution x to M₀ + N·x = M' such that no token-free directed
//! > circuit exists in the subnet induced by the support of x.
//!
//! Used in: [`System::analyze_reachability`](crate::System::analyze_reachability).
//!
//! ## Theorem 26 — Circuit token invariance in T-nets
//!
//! > In a marked graph, the total number of tokens on each directed
//! > circuit is constant under all firings.
//!
//! This invariant is the foundation for the L0-or-L4 dichotomy in T-nets:
//! if a circuit has zero tokens initially, its transitions can never fire;
//! if it has at least one token, its transitions are live.
//!
//! Used in: [`TNetLivenessEvidence`].
//!
//! ## §IV-B — Incidence matrix and state equation
//!
//! > M' = M₀ + N · x, where N is the incidence matrix and x is the
//! > firing count vector.
//!
//! Murata uses the convention where N is |T|×|P| (transposed relative to
//! our convention). This library follows the Primer convention: N is
//! |P|×|T|.
//!
//! Used in: [`IncidenceMatrix`],
//! [`find_marking_equation_rational_solution`](semi_decision::find_marking_equation_rational_solution).
//!
//! ## Table 5 — Structural boundedness
//!
//! > A net is structurally bounded iff there exists y > 0 such that
//! > yᵀ · N ≤ 0 (every place is covered by a positive place subvariant).
//!
//! Used in: [`find_positive_place_subvariant`](semi_decision::find_positive_place_subvariant),
//! [`is_structurally_bounded`](semi_decision::is_structurally_bounded).
//!
//! ## Definition 5.1 — Liveness levels L0–L4
//!
//! > L0 (dead): t never fires. L1: t fires at least once in some sequence.
//! > L2: for any k, t fires ≥k times in some sequence. L3: t fires
//! > infinitely often in some infinite sequence. L4 (live): from every
//! > reachable marking, t can eventually fire.
//!
//! Used in: [`LivenessLevel`].
//!
//! ## §V-C — Liveness via reachability graph SCCs
//!
//! > On a bounded reachability graph, transition t is L4 iff it labels an
//! > edge in every terminal SCC; L3 (≡L2) iff it labels an edge in some
//! > non-trivial SCC; L1 iff it labels any edge; L0 otherwise.
//!
//! Used in: [`ReachabilityGraph::liveness_levels`](crate::ReachabilityGraph::liveness_levels).
//!
//! ## §VI-C — S-components and T-components
//!
//! > A net covered by S-components is conservative and bounded.
//! > A net covered by T-components is consistent and repetitive.
//!
//! Used in: [`s_components`](structural::s_components),
//! [`t_components`](structural::t_components).
//!
//! ## Theorem 30 — Conservativeness
//!
//! > A Petri net N is (partially) conservative iff there exists a vector y
//! > of positive (non-negative) integers such that Ay = 0, y ≠ 0.
//!
//! Conservativeness means the weighted token sum Mᵀy is constant under
//! all reachable markings. Full conservativeness (y > 0) is equivalent
//! to coverage by non-negative S-invariants. Could expose an explicit
//! `is_conservative()` convenience method.
//!
//! Used in: [`Invariants::is_covered_by_s_invariants`],
//! [`compute_invariants`](structural::compute_invariants).
//!
//! ## Theorem 32 — Consistency
//!
//! > A Petri net N is (partially) consistent iff there exists a vector x
//! > of positive (non-negative) integers such that Aᵀx = 0, x ≠ 0.
//!
//! Consistency means there exists a marking and a firing sequence returning
//! to that marking in which every transition fires at least once.
//! Full consistency (x > 0) is equivalent to coverage by non-negative
//! T-invariants. Could expose an explicit `is_consistent()` convenience
//! method.
//!
//! Used in: [`Invariants::is_covered_by_t_invariants`],
//! [`compute_invariants`](structural::compute_invariants).
//!
//! ## §VII — Invariant analysis
//!
//! > S-invariants (place invariants): vectors y with yᵀ · N = 0.
//! > T-invariants (transition invariants): vectors x with N · x = 0.
//!
//! Used in: [`compute_invariants`](structural::compute_invariants).
//!
//! # Petri Net Primer (Best & Devillers)
//!
//! ## Provision 4.5 — No empty Petri nets
//!
//! > If C has no rows, or no columns, or both, linear algebra cannot
//! > reasonably be expected to work, but the nets are then easy to
//! > analyse. Therefore, we will assume that there is at least one
//! > transition and at least one place in the nets we consider.
//!
//! Used in: [`BuildError::Empty`](crate::net::builder::BuildError::Empty).
//!
//! ## Definition 4.1 — Incidence matrix
//!
//! > N is a |P|×|T| integer matrix with N\[s,t\] = W(t,s) − W(s,t).
//!
//! Used in: [`IncidenceMatrix`].
//!
//! ## Proposition 4.3 — State equation
//!
//! > If M' is reachable from M₀, then M₀ + N·σ̄ = M' where σ̄ is the
//! > Parikh vector of the firing sequence.
//!
//! Used in: [`find_marking_equation_rational_solution`](semi_decision::find_marking_equation_rational_solution).
//!
//! ## Proposition 4.12 — Structural boundedness via LP
//!
//! > A net is structurally bounded iff there exists y > 0 with yᵀN ≤ 0.
//!
//! Used in: [`find_positive_place_subvariant`](semi_decision::find_positive_place_subvariant).
//!
//! ## Theorem 5.17 — Commoner/Hack Criterion (CHC)
//!
//! > A well-formed free-choice system is live iff every proper siphon
//! > contains a marked trap.
//!
//! Used in: [`commoner_hack_criterion`](structural::commoner_hack_criterion).
//!
//! ## Corollary 5.30 — Liveness of S-systems
//!
//! > A plain S-system is live iff it is covered by cycles and every
//! > strongly connected component carries at least one token under M₀.
//!
//! §5.6 Case 2 further establishes that for weakly (not strongly)
//! connected S-systems, the CHC can never be satisfied because non-final
//! SCCs form siphons without traps.
//!
//! Used in: [`SNetLivenessEvidence`].
//!
//! ## Theorem 5.31 — Liveness and realisability in T-systems
//!
//! > A plain T-system is live iff all places s satisfy •s ≠ ∅ and all
//! > elementary cycles carry at least one token under M₀.
//!
//! Used in: [`TNetLivenessEvidence`].
//!
//! ## Theorem 5.22 — S-component coverage implies conservativeness
//!
//! > If a net N is covered by (strongly connected) S-components, then N is
//! > conservative (i.e., bounded under every initial marking).
//!
//! Used in: [`s_components`](structural::s_components).
//!
//! ## Theorem 5.23 — T-component coverage implies consistency
//!
//! > If a net N is covered by (strongly connected) T-components, then N is
//! > consistent.
//!
//! Used in: [`t_components`](structural::t_components).
//!
//! ## Theorem 5.34 — Boundedness criterion for live free-choice systems
//!
//! > Let (N, M₀) be a live FC-system. A place s is m-bounded iff there
//! > exists a strongly connected S-component (S₁, T₁, F₁) with s ∈ S₁
//! > and M₀(S₁) ≤ m. The system is bounded iff it is covered by strongly
//! > connected S-components.
//!
//! Not yet exploited: would make `analyze_boundedness` polynomial for live
//! FC nets by computing exact per-place bounds from S-component token sums.
//!
//! Cited in: [`SComponent`],
//! [`is_covered_by_s_components`](structural::is_covered_by_s_components).
//!
//! ## Proposition 5.39 — Boundedness criterion for live S-systems
//!
//! > In a live S-system, place s is m-bounded where m = M₀(S₁) and
//! > (S₁, T₁, F₁) is the unique strongly connected S-component containing
//! > s. Moreover, there exists a reachable marking with s carrying m tokens.
//!
//! Not yet exploited: would give exact per-place bounds for live S-nets
//! without state-space exploration.
//!
//! Cited in: [`SNetLivenessEvidence`].
//!
//! ## Algorithm 6.19 — Maximal siphon/trap in a subset
//!
//! Computing the maximal siphon in a place set X ⊆ S:
//!
//! ```text
//! Input:  N = (S, T, F),  X ⊆ S
//! Output: maximal siphon D ⊆ X
//!
//! D := X
//! while ∃ s ∈ D, t ∈ •s : t ∉ D• →
//!     choose such an s
//!     D := D \ {s}
//! end while
//! ```
//!
//! The dual algorithm for traps replaces the condition with
//! `∃ s ∈ D, t ∈ s• : t ∉ •D`.
//!
//! Termination is guaranteed because D shrinks on each iteration.
//! The result is the unique maximal siphon (or trap) contained in X.
//!
//! Used in: [`minimal_siphons`](structural::minimal_siphons),
//! [`minimal_traps`](structural::minimal_traps).
//!
//! ## Definition 5.9 — S-components and T-components
//!
//! > An S-component is a strongly connected subnet where every transition
//! > has exactly one input and one output place in the component.
//! > A T-component is a strongly connected subnet where every place has
//! > exactly one input and one output transition in the component.
//!
//! Used in: [`s_components`](structural::s_components),
//! [`t_components`](structural::t_components).

use crate::analysis::model::{CommonerHackCriterionResult, SNetLivenessEvidence, TNetLivenessEvidence};
use crate::analysis::{semi_decision, structural};
use crate::analysis::structural::{IncidenceMatrix, Invariants, SComponent};
use crate::LivenessLevel;