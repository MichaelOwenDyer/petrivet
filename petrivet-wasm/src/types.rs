//! TypeScript-friendly types for the petrivet WASM API.
//!
//! Every type here derives [`Tsify`] so that `wasm-pack build` generates
//! a corresponding TypeScript declaration in the output `pkg/`. The
//! `into_wasm_abi` attribute on each type makes it directly returnable from
//! `#[wasm_bindgen]` functions without going through an untyped `JsValue`.

use serde::Serialize;
use tsify_next::Tsify;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub enum WasmNetClass {
    Circuit,
    SNet,
    TNet,
    FreeChoice,
    AsymmetricChoice,
    Unrestricted,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmPosition {
    pub x: f64,
    pub y: f64,
}

/// A directed arc, represented as a pair of dense indices.
///
/// For place-to-transition arcs, `source` is a place index and `target` is a
/// transition index. For transition-to-place arcs, the roles are reversed.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmArc {
    pub source: u32,
    pub target: u32,
}

/// Everything a renderer needs to draw the net: topology, optional PNML
/// positions, and human-readable labels.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmNetStructure {
    pub place_count: u32,
    pub transition_count: u32,
    /// Arcs from places to transitions (`source` = place index, `target` = transition index).
    pub pt_arcs: Vec<WasmArc>,
    /// Arcs from transitions to places (`source` = transition index, `target` = place index).
    pub tp_arcs: Vec<WasmArc>,
    /// PNML centre position for each place, or `null` if the source had no graphics.
    pub place_positions: Vec<Option<WasmPosition>>,
    /// PNML centre position for each transition, or `null` if the source had no graphics.
    pub transition_positions: Vec<Option<WasmPosition>>,
    pub place_names: Vec<Option<String>>,
    pub transition_names: Vec<Option<String>>,
    pub net_name: Option<String>,
    pub net_class: WasmNetClass,
}

/// A token count that is either a concrete non-negative integer or ω (unbounded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type", content = "value")]
pub enum WasmOmega {
    Finite(u32),
    Unbounded,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmBoundednessAnalysis {
    pub is_bounded: bool,
    /// Per-place bounds, indexed by place index.
    pub place_bounds: Vec<WasmOmega>,
    pub method: WasmBoundednessMethod,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type")]
pub enum WasmBoundednessMethod {
    /// Structural LP found a positive place subvariant; bounds may be loose.
    PositivePlaceSubvariant,
    /// Full coverability graph explored; bounds are exact.
    CoverabilityGraph,
}

/// Per-transition liveness level (Murata 1989 §V-C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub enum WasmLivenessLevel {
    /// Dead: never fires from the initial marking.
    L0,
    /// Potentially firable at least once.
    L1,
    /// Firable k times for any k (but not necessarily infinitely often).
    L2,
    /// There exists an infinite firing sequence containing this transition.
    L3,
    /// Live: always eventually fireable from any reachable marking.
    L4,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmLivenessAnalysis {
    /// Minimum liveness level across all transitions.
    pub net_level: WasmLivenessLevel,
    /// Per-transition liveness level, indexed by transition index.
    pub levels: Vec<WasmLivenessLevel>,
    pub method: WasmLivenessMethod,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type")]
pub enum WasmLivenessMethod {
    SNet,
    TNet,
    FreeChoice,
    ReachabilityGraphSCC,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmDeadlock {
    /// The deadlocked marking, indexed by place.
    pub marking: Vec<u32>,
    /// A firing sequence from M₀ that reaches this deadlock.
    pub firing_sequence: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmDeadlockAnalysis {
    pub is_deadlock_free: bool,
    /// All reachable deadlocks. Empty when `is_deadlock_free` is true.
    pub deadlocks: Vec<WasmDeadlock>,
    pub method: WasmDeadlockMethod,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type")]
pub enum WasmDeadlockMethod {
    CommonerTheorem,
    Exploration,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "status")]
pub enum WasmReachabilityResult {
    Reachable {
        /// A witness firing sequence from M₀ to the target (may be empty for
        /// structural proofs that do not produce an explicit path).
        firing_sequence: Vec<u32>,
        proof: WasmReachabilityProof,
    },
    Unreachable {
        proof: WasmUnreachabilityProof,
    },
    /// Current algorithms could not decide (unbounded general net).
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type")]
pub enum WasmReachabilityProof {
    FiringSequence,
    SNetTokenConservation { marking_sum: u32 },
    SNetMarkingEquation,
    TNetMarkingEquation,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type")]
pub enum WasmUnreachabilityProof {
    SNetTokenConservationViolation {
        initial_marking_sum: u32,
        target_marking_sum: u32,
    },
    MarkingEquationNoRationalSolution,
    MarkingEquationNoIntegerSolution,
    ExhaustiveSearch,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "status")]
pub enum WasmCoverabilityResult {
    Coverable {
        /// Firing sequence from M₀ to the covering node.
        firing_sequence: Vec<u32>,
        /// The node marking that covers the target (may contain ω).
        covering_marking: Vec<WasmOmega>,
    },
    Uncoverable {
        proof: WasmNonCoverabilityProof,
    },
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(tag = "type")]
pub enum WasmNonCoverabilityProof {
    MarkingEquationNoRationalSolution,
    MarkingEquationNoIntegerSolution,
    ExhaustiveSearch,
}

// ---------------------------------------------------------------------------
// Builder types
// ---------------------------------------------------------------------------

/// A place as seen by `WasmNetBuilder.structure()`.
///
/// The `id` field is the stable builder ID — it never changes while the
/// builder is alive and is never reused after the place is removed.
/// Arcs in `WasmBuilderStructure` reference nodes by this ID.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmBuilderPlace {
    /// Stable builder ID (use this as the Cytoscape node ID during editing).
    pub id: u32,
    pub name: Option<String>,
    /// X coordinate stored in the builder (may be 0 if not yet placed).
    pub x: f64,
    /// Y coordinate stored in the builder.
    pub y: f64,
    /// Initial token count for this place.
    pub initial_tokens: u32,
}

/// A transition as seen by `WasmNetBuilder.structure()`.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmBuilderTransition {
    pub id: u32,
    pub name: Option<String>,
    pub x: f64,
    pub y: f64,
}

/// A directed arc between two nodes, using builder IDs.
///
/// For `pt_arcs`: `source_id` is a place builder ID, `target_id` is a transition builder ID.
/// For `tp_arcs`: `source_id` is a transition builder ID, `target_id` is a place builder ID.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmBuilderArc {
    pub source_id: u32,
    pub target_id: u32,
}

/// Complete snapshot of a `WasmNetBuilder`'s current state.
///
/// All node references in arcs use stable builder IDs, not compact net indices.
/// Call `WasmNetBuilder.build()` to get a `WasmSystem` with dense 0-indexed nodes.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WasmBuilderStructure {
    pub places: Vec<WasmBuilderPlace>,
    pub transitions: Vec<WasmBuilderTransition>,
    /// Place-to-transition arcs (source = place id, target = transition id).
    pub pt_arcs: Vec<WasmBuilderArc>,
    /// Transition-to-place arcs (source = transition id, target = place id).
    pub tp_arcs: Vec<WasmBuilderArc>,
    pub net_name: Option<String>,
}
