//! Structured analysis results with evidence.
//!
//! Each `analyze_*` method on [`System`](crate::system::System) returns a result
//! struct with two parts:
//!
//! 1. **Uniform fields**: always valid regardless of which method was used.
//!    For example, `BoundednessAnalysis.place_bounds` always contains per-place
//!    bounds (tight or upper-estimate depending on method).
//!
//! 2. **Evidence enum**: describes *how* the answer was obtained and carries
//!    method-specific artifacts. Marked `#[non_exhaustive]` so new analytical
//!    methods can be added without breaking downstream code.
//!
//! This design allows users to write code that is agnostic to the specific
//! analysis method used, while still providing access to method-specific
//! details when needed.

use crate::marking::Marking;
use crate::marking::OmegaMarking;
use crate::net::{Place, Transition};
use crate::{Omega, PlaceMap, TransitionMap};
use std::collections::HashSet;

pub type Siphon = HashSet<Place>;
pub type Trap = HashSet<Place>;

/// Result of the Commoner/Hack criterion check.
///
/// For free-choice nets, this criterion is both necessary and sufficient for
/// liveness: a free-choice system (N, M₀) is live if and only if every proper
/// siphon of N contains a trap that is marked under M₀.
///
/// For general nets, the condition is sufficient for deadlock-freedom but
/// not necessary: if every siphon contains a marked trap, the net is
/// deadlock-free, but the converse does not hold.
///
/// References:
/// - [Murata 1989, Theorem 12](crate::literature#theorem-12--commonerhack-criterion)
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc)
#[derive(Debug, Clone)]
pub struct CommonerHackCriterionResult {
    /// Each minimal siphon paired with the maximal trap contained within it.
    /// If the trap is empty, no marked trap was found in that siphon.
    pub siphon_trap_pairs: Box<[SiphonTrapPair]>,
}

impl CommonerHackCriterionResult {
    /// Whether the Commoner/Hack criterion holds: every siphon contains a marked trap.
    #[must_use]
    pub fn is_satisfied(&self) -> bool {
        self.siphon_trap_pairs.iter().all(|pair| pair.trap_is_marked)
    }
}

/// A minimal siphon and the maximal trap found within it,
/// and whether that trap is marked.
#[derive(Debug, Clone)]
pub struct SiphonTrapPair {
    /// The minimal siphon (a set of places D with •D ⊆ D•).
    pub siphon: Siphon,
    /// The maximal trap contained in this siphon (a set of places Q with Q• ⊆ •Q).
    /// Empty if no trap was found.
    pub trap: Trap,
    /// Whether at least one place in the trap is marked in the reference marking.
    pub trap_is_marked: bool,
}

/// Result of boundedness analysis.
///
/// `place_bounds` is always populated. When proved via the structural LP,
/// bounds are derived upper estimates (potentially loose). When proved via
/// the coverability graph, bounds are exact.
#[derive(Debug, Clone)]
pub struct BoundednessAnalysis {
    /// Per-place bounds, indexed by place index.
    pub place_bounds: PlaceMap<Omega>,
    /// How the result was obtained.
    pub method: BoundednessAnalysisMethod,
}

impl BoundednessAnalysis {
    /// Returns the bound of the system as a whole: the maximum over all places.
    #[must_use]
    pub fn system_bound(&self) -> Omega {
        self.place_bounds.values().max().copied().unwrap_or_default()
    }

    /// Returns the bound for a specific place.
    /// 
    /// # Panics
    /// 
    /// Panics if the provided place is not found in the net.
    #[must_use]
    pub fn place_bound(&self, p: Place) -> Omega {
        self.place_bounds[p]
    }

    /// Per-place bounds in dense index order.
    #[must_use]
    pub fn place_bounds_dense(&self) -> Vec<Omega> {
        self.place_bounds.values().copied().collect()
    }
}

/// Evidence for a boundedness result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BoundednessAnalysisMethod {
    /// Structural LP found a positive vector y with yᵀN ≤ 0.
    /// Bounds are derived as M\[p\] ≤ ⌊(y·M₀) / y\[p\]⌋: valid but
    /// potentially loose.
    PositivePlaceSubvariant(PlaceMap<f64>),
    /// Full coverability graph explored. Bounds are exact.
    CoverabilityGraph,
}

/// Liveness level of a transition from a given initial marking, following Murata 1989 §V-C.
///
/// The levels form a strict hierarchy: L4 ⊂ L3 ⊂ L2 ⊂ L1, and L0 means
/// the transition is dead (not even L1).
///
/// For **bounded** nets, L2 and L3 coincide: if a transition can fire any positive
/// integer k number of times, the finite state space forces a cycle, making it
/// possible to fire infinitely often. We still distinguish them in the enum for
/// theoretical completeness, but bounded-net analysis reports L3 when both
/// L2 and L3 hold.
///
/// References:
/// - [Murata 1989, Definition 5.1](crate::literature#definition-51--liveness-levels-l0l4)
/// - Petri Net Primer, §5.4 (liveness)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LivenessLevel {
    /// Dead: the transition never fires in any firing sequence from the initial marking.
    L0,
    /// Potentially firable: there exists a firing sequence from the initial marking
    /// where the transition fires at least once.
    L1,
    /// For any positive integer `k`, there exists a firing sequence from the initial marking
    /// where the transition fires at least `k` times (but not necessarily infinitely often!).
    L2,
    /// Potentially infinitely fireable: there exists a firing sequence from the initial marking
    /// where the transition fires infinitely many times.
    L3,
    /// Live: the transition is L1-live from every marking reachable from the initial marking
    /// (can always become enabled again).
    L4,
}

impl LivenessLevel {
    /// Whether this is L0 (dead).
    #[must_use]
    pub fn is_dead(self) -> bool {
        self == Self::L0
    }

    /// Whether this is L4 (live).
    #[must_use]
    pub fn is_live(self) -> bool {
        self == Self::L4
    }
}

/// Result of liveness analysis.
///
/// `levels` is always populated with one entry per transition. When proved
/// via Commoner's theorem (free-choice nets), all transitions are L4.
/// When proved via SCC analysis on the reachability graph, levels are
/// individually computed.
#[derive(Debug, Clone)]
pub struct LivenessAnalysis {
    /// Per-transition liveness levels.
    pub levels: TransitionMap<LivenessLevel>,
    /// How the result was obtained.
    pub method: LivenessMethod,
}

impl LivenessAnalysis {
    /// The overall liveness level of the net (minimum over all transitions).
    #[must_use]
    pub fn net_level(&self) -> LivenessLevel {
        self.levels
            .values()
            .copied()
            .min()
            .unwrap_or(LivenessLevel::L4)
    }

    /// Liveness level of a specific transition.
    #[must_use]
    pub fn transition_level(&self, t: Transition) -> LivenessLevel {
        self.levels[t]
    }

    /// Per-transition liveness levels in dense index order.
    #[must_use]
    pub fn levels_dense(&self) -> Vec<LivenessLevel> {
        self.levels.values().copied().collect()
    }
}

/// Evidence for liveness analysis of an S-net.
///
/// In an S-net, each transition has exactly one input and one output place.
/// The "place graph" (places as nodes, transitions as directed edges) determines
/// liveness levels via its SCC decomposition:
///
/// - **Sink SCC, marked**: transitions on internal cycles are **L4** (tokens
///   can never leave; can always be routed to fire any internal transition).
/// - **Non-sink SCC, marked**: internal transitions are **L3** (tokens *can*
///   stay cycling forever, but *can also* escape via outgoing transitions,
///   so not L4). See Primer §5.6 Case 2: CHC fails for non-final SCCs.
/// - **Inter-SCC transitions**: at most **L1** (each token passes through at
///   most once; total tokens conserved in S-nets).
/// - **Unreachable**: **L0**.
///
/// References:
/// - [Murata 1989, Theorem 4](crate::literature#theorem-4--liveness-of-s-nets-state-machines) (SC liveness)
/// - [Murata 1989, Theorem 5](crate::literature#theorem-5--safety-of-s-nets-state-machines) (safety via token count)
/// - [Primer, Corollary 5.30](crate::literature#corollary-530--liveness-of-s-systems)
/// - [Primer, Proposition 5.39](crate::literature#proposition-539--boundedness-criterion-for-live-s-systems) (per-place bounds)
#[derive(Debug, Clone)]
pub struct SNetLivenessEvidence {
    /// The SCCs of the place graph, in topological order (sources first).
    pub components: Box<[SNetComponent]>,
}

/// A strongly connected component in the place graph of an S-net.
#[derive(Debug, Clone)]
pub struct SNetComponent {
    /// Places in this SCC.
    pub places: Box<[Place]>,
    /// Transitions internal to this SCC (both endpoints in the same SCC).
    pub transitions: Box<[Transition]>,
    /// Total token count on places in this SCC under M₀.
    pub token_sum: u32,
    /// Whether this SCC has no outgoing transitions to other SCCs.
    pub is_sink: bool,
}

/// Evidence for liveness analysis of a T-net (marked graph).
///
/// In a T-net, each place has exactly one input and one output transition.
/// A fundamental invariant: **the token count on every directed circuit is
/// constant under all firings** (each transition on the circuit removes one
/// token from its input place and adds one to its output place; external
/// transitions cannot touch circuit places).
///
/// Consequence: every transition in a T-net is either **L0** or **L4** — no
/// intermediate liveness levels are possible.
///
/// A transition t is L4 iff every directed circuit containing t is marked
/// AND all predecessor transitions (in the SCC DAG of the transition graph)
/// are L4.
///
/// References:
/// - [Murata 1989, Theorem 7](crate::literature#theorem-7--liveness-of-t-nets-marked-graphs) (SC liveness)
/// - [Murata 1989, Theorem 8](crate::literature#theorem-8--place-bounds-in-t-nets-marked-graphs) (exact place bounds via circuit token counts)
/// - [Murata 1989, Theorem 9](crate::literature#theorem-9--safety-of-t-nets-marked-graphs) (safety iff every circuit carries 1 token)
/// - [Murata 1989, Theorem 26](crate::literature#theorem-26--circuit-token-invariance-in-t-nets) (circuit token invariance)
/// - [Primer, Theorem 5.31](crate::literature#theorem-531--liveness-and-realisability-in-t-systems)
#[derive(Debug, Clone)]
pub struct TNetLivenessEvidence {
    /// The SCCs of the transition graph, in topological order (sources first).
    /// Each SCC is live (all transitions L4) iff all internal circuits are
    /// marked AND all predecessor SCCs are live.
    pub components: Box<[TNetComponent]>,
}

/// A strongly connected component in the transition graph of a T-net.
#[derive(Debug, Clone)]
pub struct TNetComponent {
    /// Transitions in this SCC.
    pub transitions: Box<[Transition]>,
    /// Places internal to this SCC (both endpoint transitions in the same SCC).
    pub places: Box<[Place]>,
    /// Whether all directed circuits within this SCC carry at least one token
    /// under M₀. (Trivially true for acyclic/singleton SCCs.)
    pub all_circuits_marked: bool,
    /// Whether all predecessor SCCs in the DAG are live.
    /// Combined with `all_circuits_marked`, determines if transitions here are L4.
    pub predecessors_live: bool,
}

/// Evidence for a liveness result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum LivenessMethod {
    /// S-net SCC analysis.
    ///
    /// Per-transition levels derived from the SCC decomposition of the place
    /// graph and the token distribution across components.
    ///
    /// References: [Murata 1989 Theorem 4](crate::literature#theorem-4--liveness-of-s-nets-state-machines), [Primer Corollary 5.30](crate::literature#corollary-530--liveness-of-s-systems).
    SNet(SNetLivenessEvidence),
    /// T-net circuit analysis.
    ///
    /// Per-transition levels derived from the SCC decomposition of the
    /// transition graph. Every transition is either L0 or L4 due to the
    /// circuit token invariance property.
    ///
    /// References: [Murata 1989 Theorems 7 & 26](crate::literature#theorem-7--liveness-of-t-nets-marked-graphs), [Primer Theorem 5.31](crate::literature#theorem-531--liveness-and-realisability-in-t-systems).
    TNet(TNetLivenessEvidence),
    /// Commoner's theorem applied (free-choice net). All transitions are L4.
    ///
    /// Reference: [Primer Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc), [Murata 1989 Theorem 12](crate::literature#theorem-12--commonerhack-criterion).
    FreeChoice(CommonerHackCriterionResult),
    /// Strongly-connected component analysis on the full reachability graph (bounded net).
    ReachabilityGraphSCC,
    /// Current algorithms could not decide (unbounded general net).
    Inconclusive,
}

/// A reachable deadlock marking with a firing sequence to reach it.
#[derive(Debug, Clone)]
pub struct Deadlock {
    /// A firing sequence from M₀.
    pub firing_sequence: Box<[Transition]>,
    /// The marking reached at the end of the firing sequence where no transitions are enabled.
    pub marking: Marking,
}

impl Deadlock {
    /// The firing sequence as dense transition indices (`u32`), for FFI and other callers that
    /// cannot name [`Transition`].
    #[must_use]
    pub fn firing_sequence_indices(&self) -> Vec<u32> {
        firing_sequence_to_indices(&self.firing_sequence)
    }
}

/// Evidence for a deadlock-freedom result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DeadlockAnalysisMethod {
    /// Commoner/Hack criterion guarantees deadlock-freedom based on the initial marking.
    CommonerTheorem(CommonerHackCriterionResult),
    /// State space was fully explored and no deadlocks were found.
    Exploration,
    /// The net is an unbounded general net where the reachability graph is infinite;
    /// this library's current algorithms cannot decide deadlock-freedom here.
    Inconclusive,
}

/// Result of deadlock-freedom analysis.
///
/// `deadlocks` is always a valid list: empty if the system is deadlock-free,
/// populated with witnesses if deadlocks exist. When the structural
/// (siphon/trap) check proves freedom, no exploration is needed and
/// `deadlocks` is empty. When exploration is required, all reachable
/// deadlocks are returned with firing sequences.
#[derive(Debug, Clone)]
pub struct DeadlockAnalysis {
    /// All reachable deadlock markings with witness firing sequences.
    /// Empty if deadlock-free.
    pub deadlocks: Box<[Deadlock]>,
    /// How the result was obtained.
    pub evidence: DeadlockAnalysisMethod,
}

impl DeadlockAnalysis {
    /// Whether the system is deadlock-free.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.deadlocks.is_empty()
    }
}

/// Result of reachability analysis.
///
/// Three possible outcomes:
/// - `Reachable`: the target is definitely reachable, with a witness.
/// - `Unreachable`: the target is definitely unreachable, with a proof.
/// - `Inconclusive`: current algorithms could not decide (e.g., unbounded
///   general net where LP/ILP filters pass but full exploration is infinite).
#[derive(Debug, Clone)]
pub enum ReachabilityResult {
    /// The target marking is reachable from M₀.
    Reachable(ReachabilityProof),
    /// The target marking is definitely not reachable from M₀.
    Unreachable(UnreachabilityProof),
    /// Current algorithms could not decide.
    Inconclusive,
}

impl ReachabilityResult {
    /// Whether the target is definitely reachable.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable(_))
    }

    /// Whether the target is definitely unreachable.
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable(_))
    }

    /// Whether the analysis was inconclusive.
    #[must_use]
    pub fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive)
    }
}

pub type FiringSequence = Box<[Transition]>;

/// Converts a firing sequence of dense [`Transition`] handles to a vector of
/// raw `u32` indices. Useful for external crates that cannot name [`Transition`].
#[must_use]
pub fn firing_sequence_to_indices(seq: &[Transition]) -> Vec<u32> {
    seq.iter().map(|t| t.index()).collect()
}

/// Witness that a marking is reachable: a firing sequence from M₀ to the target.
#[derive(Debug, Clone)]
pub struct ReachabilityWitness {
    /// The firing sequence from M₀ to the target marking.
    pub firing_sequence: FiringSequence,
}

pub type ParikhVector<T> = TransitionMap<T>;

#[derive(Debug, Clone)]
pub enum ReachabilityProof {
    StronglyConnectedSNetTokenConservation {
        marking_sum: u32,
    },
    SNetMarkingEquationRationalSolution(ParikhVector<f64>),
    TNetMarkingEquationIntegerSolution(ParikhVector<u32>),
    FiringSequence(FiringSequence),
}

impl ReachabilityProof {
    /// If this is a `FiringSequence` proof, returns the sequence as dense
    /// transition indices. Returns `None` for structural proofs.
    #[must_use]
    pub fn firing_sequence_indices(&self) -> Option<Vec<u32>> {
        match self {
            Self::FiringSequence(seq) => Some(firing_sequence_to_indices(seq)),
            _ => None,
        }
    }

    /// Token-conservation marking sum (only for `StronglyConnectedSNetTokenConservation`).
    #[must_use]
    pub fn marking_sum(&self) -> Option<u32> {
        match self {
            Self::StronglyConnectedSNetTokenConservation { marking_sum } => Some(*marking_sum),
            _ => None,
        }
    }
}

/// Proof that a marking is unreachable.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UnreachabilityProof {
    /// The net is an S-net and the target marking has a different
    /// token sum than the initial marking.
    SNetTokenConservationViolation {
        initial_marking_sum: u32,
        target_marking_sum: u32,
    },
    /// The LP marking equation (rational relaxation) is infeasible.
    /// Some S-invariant is violated.
    MarkingEquationNoRationalSolution,
    /// The ILP marking equation (integer) is infeasible.
    /// Stronger than LP: no integer firing count vector exists.
    MarkingEquationNoIntegerSolution,
    /// Full state space explored; target not found.
    ExhaustiveSearch,
}

impl From<ReachabilityProof> for ReachabilityResult {
    fn from(value: ReachabilityProof) -> Self {
        ReachabilityResult::Reachable(value)
    }
}

impl From<UnreachabilityProof> for ReachabilityResult {
    fn from(value: UnreachabilityProof) -> Self {
        ReachabilityResult::Unreachable(value)
    }
}

#[derive(Debug, Clone)]
pub enum CoverabilityResult {
    /// The target marking is coverable from M₀.
    Coverable(CoverabilityProof),
    /// The target marking is not coverable from M₀.
    Uncoverable(NonCoverabilityProof),
}

impl CoverabilityResult {
    /// Whether the target is coverable.
    #[must_use]
    pub fn is_coverable(&self) -> bool {
        matches!(self, Self::Coverable(_))
    }

    /// Whether the target is not coverable.
    #[must_use]
    pub fn is_uncoverable(&self) -> bool {
        matches!(self, Self::Uncoverable(_))
    }
}

/// Proof that a marking is coverable.
///
/// For bounded nets, the coverability graph contains only finite markings, so the
/// returned `covering_marking` is a reachable marking.
///
/// For unbounded nets, the coverability graph may contain ω-markings. An ω-marking
/// that covers the target is still a valid proof of coverability, but it may not be
/// a reachable marking itself. Instead, it represents the existence of reachable
/// markings that can exceed any finite threshold on its ω-places.
///
/// A node of the coverability graph covers the target.
///
/// The witness firing sequence reaches a node in the coverability graph. The
/// node marking may contain ω.
///
/// todo: good idea to always include firing sequence? Maybe some callers don't need it
///  and it is not necessarily cheap to construct. But how to provide ergonomic access
///  otherwise? Maybe a 'witness' method for lazy computation? But I guess without the
///  firing sequence it is not so much proof as just a statement of existence.
///
/// todo: instead of pub fields, provide read-only accessor methods and no public constructors
///  so that the struct is type-system proof.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CoverabilityProof {
    /// A firing sequence from M₀ to a node whose marking covers the target.
    pub firing_sequence: FiringSequence,
    /// The node marking M″ with M″ ≥ target (may contain ω).
    pub covering_marking: OmegaMarking,
}

impl CoverabilityProof {
    /// The firing sequence as dense transition indices.
    #[must_use]
    pub fn firing_sequence_indices(&self) -> Vec<u32> {
        firing_sequence_to_indices(&self.firing_sequence)
    }
}

/// Various methods to demonstrate that a marking is not coverable
/// in a given system.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NonCoverabilityProof {
    /// The LP marking equation (rational relaxation) is infeasible.
    /// Some S-invariant is violated.
    MarkingEquationNoRationalSolution,
    /// The ILP marking equation (integer) is infeasible.
    /// Stronger than LP: no integer firing count vector exists.
    MarkingEquationNoIntegerSolution,
    /// Full coverability graph explored; target not covered.
    ExhaustiveSearch,
}

/// If we have proof of coverability, the target is coverable.
impl From<CoverabilityProof> for CoverabilityResult {
    fn from(value: CoverabilityProof) -> Self {
        CoverabilityResult::Coverable(value)
    }
}

/// If we have proof of non-coverability, the target is not coverable.
impl From<NonCoverabilityProof> for CoverabilityResult {
    fn from(value: NonCoverabilityProof) -> Self {
        CoverabilityResult::Uncoverable(value)
    }
}