//! WASM bindings for the petrivet library.
//!
//! The main entry point is [`WasmSystem`], which wraps a `System<Rc<Net>>`
//! and exposes simulation, analysis, and structural queries to JavaScript.
//!
//! # Constructing a system
//!
//! The primary way to construct a [`WasmSystem`] from a browser is to
//! pass a PNML XML string to [`WasmSystem::parse_pnml`]:
//!
//! ```js
//! const xml = await file.text();
//! const sys = WasmSystem.parsePnml(xml);
//! const structure = sys.netStructure();
//! ```

use std::collections::HashMap;
use std::rc::Rc;

use petrivet::analysis::model::{
    BoundednessAnalysisMethod, CoverabilityResult, DeadlockAnalysisMethod, LivenessMethod,
    NonCoverabilityProof, ReachabilityProof, ReachabilityResult, UnreachabilityProof,
};
use petrivet::labeled::NetLabels;
use petrivet::marking::{Marking, Omega};
use petrivet::net::{Arc as PetriArc, Net, PlaceKey, TransitionKey};
use petrivet::net::builder::{NetBuilder, BuilderArc};
use petrivet::net::class::NetClass;
use petrivet::pnml::PnmlDocument;
use petrivet::pnml::convert::PnmlGraphics;
use petrivet::system::System;
use wasm_bindgen::prelude::*;

mod types;
use types::*;

/// A Petri net system (net + marking) exposed to JavaScript.
///
/// Owns the net via `Rc<Net>` so that `reset()` can cheaply reinstate the
/// initial marking without cloning the topology. The `labels` and `graphics`
/// fields are populated when the system is constructed from a PNML document.
///
/// `place_keys` and `transition_keys` map dense indices (0..n) to the
/// corresponding [`PlaceKey`]/[`TransitionKey`] handles used by the petrivet API.
#[wasm_bindgen]
pub struct WasmSystem {
    system: System<Rc<Net>>,
    initial_marking: Marking,
    labels: Option<NetLabels>,
    graphics: Option<PnmlGraphics>,
    place_keys: Vec<PlaceKey>,
    transition_keys: Vec<TransitionKey>,
}

#[wasm_bindgen]
impl WasmSystem {
    /// Parse a PNML XML string and construct a system from the first P/T net
    /// found in the document.
    ///
    /// Position and label data from the PNML file are preserved and exposed
    /// via [`net_structure`](Self::net_structure).
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if the XML is malformed, the document
    /// contains no P/T net, or the net topology is invalid.
    #[wasm_bindgen(js_name = parsePnml)]
    pub fn parse_pnml(xml: &str) -> Result<WasmSystem, JsError> {
        let doc = PnmlDocument::from_xml(xml)
            .map_err(|e| JsError::new(&e.to_string()))?;

        let (system, labels, graphics) = doc
            .nets
            .iter()
            .find_map(|net| net.to_pt_system().ok())
            .ok_or_else(|| JsError::new("no P/T net found in PNML document"))?;

        let initial_marking = system.current_marking().clone();
        let place_keys: Vec<PlaceKey> = system.net().place_keys().collect();
        let transition_keys: Vec<TransitionKey> = system.net().transition_keys().collect();
        let (net, marking) = system.into_parts();
        let system = System::new(Rc::new(net), marking);

        Ok(WasmSystem {
            system,
            initial_marking,
            labels: Some(labels),
            graphics: Some(graphics),
            place_keys,
            transition_keys,
        })
    }
}

impl WasmSystem {
    fn from_parts(
        system: System<Rc<Net>>,
        initial_marking: Marking,
        labels: Option<NetLabels>,
        graphics: Option<PnmlGraphics>,
    ) -> Self {
        let place_keys: Vec<PlaceKey> = system.net().as_ref().place_keys().collect();
        let transition_keys: Vec<TransitionKey> = system.net().as_ref().transition_keys().collect();
        Self { system, initial_marking, labels, graphics, place_keys, transition_keys }
    }
}

#[wasm_bindgen]
impl WasmSystem {
    /// Convert this system into a `WasmNetBuilder` seeded with the current net
    /// topology, names, PNML positions, and initial marking.
    ///
    /// Use the returned builder to edit the net, then call `builder.build()` to
    /// get a new `WasmSystem` with the updated topology.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> WasmNetBuilder {
        let net = self.system.net().as_ref();
        let builder = NetBuilder::from(net.clone());

        let mut place_names: HashMap<u32, String> = HashMap::new();
        let mut transition_names: HashMap<u32, String> = HashMap::new();
        let mut place_positions: HashMap<u32, (f64, f64)> = HashMap::new();
        let mut transition_positions: HashMap<u32, (f64, f64)> = HashMap::new();

        let n_places = net.place_count();
        let n_transitions = net.transition_count();

        if let Some(labels) = &self.labels {
            for i in 0..n_places as usize {
                if let Some(name) = labels.place_name_at(i) {
                    place_names.insert(i as u32, name.to_string());
                }
            }
            for i in 0..n_transitions as usize {
                if let Some(name) = labels.transition_name_at(i) {
                    transition_names.insert(i as u32, name.to_string());
                }
            }
        }
        if let Some(g) = &self.graphics {
            for i in 0..n_places as usize {
                if let Some(pos) = g.place_position_at(i) {
                    place_positions.insert(i as u32, (pos.x, pos.y));
                }
            }
            for i in 0..n_transitions as usize {
                if let Some(pos) = g.transition_position_at(i) {
                    transition_positions.insert(i as u32, (pos.x, pos.y));
                }
            }
        }

        let initial_tokens: HashMap<u32, u32> = self.initial_marking
            .iter()
            .enumerate()
            .filter(|(_, t)| **t > 0)
            .map(|(i, t)| (i as u32, *t))
            .collect();

        // Build the key maps: `From<Net>` preserves the net’s keys; new ids continue after the
        // largest existing id. Dense index `i` maps to `sorted_ids[i]`.
        let sorted_place_ids: Vec<PlaceKey> = builder.place_keys().collect();
        let sorted_trans_ids: Vec<TransitionKey> = builder.transition_keys().collect();

        let place_key_map: HashMap<u32, PlaceKey> = sorted_place_ids
            .iter()
            .enumerate()
            .map(|(i, &pk)| (i as u32, pk))
            .collect();
        let transition_key_map: HashMap<u32, TransitionKey> = sorted_trans_ids
            .iter()
            .enumerate()
            .map(|(i, &tk)| (i as u32, tk))
            .collect();

        let next_place_id = n_places as u32;
        let next_transition_id = n_transitions as u32;

        WasmNetBuilder {
            builder,
            net_name: self.labels.as_ref().and_then(|l| l.net_name()).map(str::to_string),
            place_names,
            transition_names,
            place_positions,
            transition_positions,
            initial_tokens,
            place_key_map,
            transition_key_map,
            next_place_id,
            next_transition_id,
        }
    }
}

#[wasm_bindgen]
impl WasmSystem {
    /// Returns topology, optional PNML positions, and labels in one object.
    ///
    /// Call this once on load and cache the result on the JS side; none of
    /// these values change during simulation.
    #[wasm_bindgen(js_name = netStructure)]
    pub fn net_structure(&self) -> WasmNetStructure {
        let net = self.system.net().as_ref();
        let n_places = net.place_count();
        let n_transitions = net.transition_count();

        // Build a reverse lookup from PlaceKey/TransitionKey to dense index.
        let pk_to_dense: HashMap<PlaceKey, u32> = self.place_keys
            .iter()
            .enumerate()
            .map(|(i, &pk)| (pk, i as u32))
            .collect();
        let tk_to_dense: HashMap<TransitionKey, u32> = self.transition_keys
            .iter()
            .enumerate()
            .map(|(i, &tk)| (tk, i as u32))
            .collect();

        let pt_arcs = net
            .arcs()
            .filter_map(|arc| match arc {
                PetriArc::PlaceToTransition(p, t) => Some(WasmArc {
                    source: pk_to_dense[&p],
                    target: tk_to_dense[&t],
                }),
                PetriArc::TransitionToPlace(..) => None,
            })
            .collect();

        let tp_arcs = net
            .arcs()
            .filter_map(|arc| match arc {
                PetriArc::TransitionToPlace(t, p) => Some(WasmArc {
                    source: tk_to_dense[&t],
                    target: pk_to_dense[&p],
                }),
                PetriArc::PlaceToTransition(..) => None,
            })
            .collect();

        let place_positions = (0..n_places as usize)
            .map(|i| {
                self.graphics
                    .as_ref()
                    .and_then(|g| g.place_position_at(i))
                    .map(|pos| WasmPosition { x: pos.x, y: pos.y })
            })
            .collect();

        let transition_positions = (0..n_transitions as usize)
            .map(|i| {
                self.graphics
                    .as_ref()
                    .and_then(|g| g.transition_position_at(i))
                    .map(|pos| WasmPosition { x: pos.x, y: pos.y })
            })
            .collect();

        let place_names = (0..n_places as usize)
            .map(|i| {
                self.labels
                    .as_ref()
                    .and_then(|l| l.place_name_at(i))
                    .map(str::to_string)
            })
            .collect();

        let transition_names = (0..n_transitions as usize)
            .map(|i| {
                self.labels
                    .as_ref()
                    .and_then(|l| l.transition_name_at(i))
                    .map(str::to_string)
            })
            .collect();

        let net_name = self
            .labels
            .as_ref()
            .and_then(|l| l.net_name())
            .map(str::to_string);

        let net_class = match net.class() {
            NetClass::Circuit => WasmNetClass::Circuit,
            NetClass::SNet => WasmNetClass::SNet,
            NetClass::TNet => WasmNetClass::TNet,
            NetClass::FreeChoice => WasmNetClass::FreeChoice,
            NetClass::AsymmetricChoice => WasmNetClass::AsymmetricChoice,
            NetClass::Unrestricted => WasmNetClass::Unrestricted,
        };

        WasmNetStructure {
            place_count: n_places as u32,
            transition_count: n_transitions as u32,
            pt_arcs,
            tp_arcs,
            place_positions,
            transition_positions,
            place_names,
            transition_names,
            net_name,
            net_class,
        }
    }
}

#[wasm_bindgen]
impl WasmSystem {
    /// The current token counts, one entry per place (indexed by place index).
    #[wasm_bindgen(js_name = currentMarking)]
    pub fn current_marking(&self) -> Vec<u32> {
        self.system.current_marking().iter().copied().collect()
    }

    /// Indices of transitions that are currently enabled.
    #[wasm_bindgen(js_name = enabledTransitions)]
    pub fn enabled_transitions(&self) -> Vec<u32> {
        let enabled_keys = self.system.enabled_transitions();
        let tk_to_dense: HashMap<TransitionKey, u32> = self.transition_keys
            .iter()
            .enumerate()
            .map(|(i, &tk)| (tk, i as u32))
            .collect();
        enabled_keys
            .iter()
            .map(|tk| tk_to_dense[tk])
            .collect()
    }

    /// Whether no transitions are enabled (the system is deadlocked).
    #[wasm_bindgen(js_name = isDeadlocked)]
    pub fn is_deadlocked(&self) -> bool {
        self.system.is_deadlocked()
    }

    /// Attempt to fire the transition at `transition_index`.
    ///
    /// Returns `true` if the transition was enabled and has been fired,
    /// `false` if it was not enabled (marking is unchanged).
    #[wasm_bindgen]
    pub fn fire(&mut self, transition_index: u32) -> bool {
        let tk = self.transition_keys[transition_index as usize];
        self.system.try_fire(tk).is_ok()
    }

    /// Reset the marking to the initial marking the system was constructed with.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        let net = Rc::clone(self.system.net());
        self.system = System::new(net, self.initial_marking.clone());
    }
}

#[wasm_bindgen]
impl WasmSystem {
    /// Whether the system is bounded under the current initial marking.
    #[wasm_bindgen(js_name = isBounded)]
    pub fn is_bounded(&self) -> bool {
        self.system.is_bounded()
    }

    /// Whether the system is live (L4) under the current initial marking.
    #[wasm_bindgen(js_name = isLive)]
    pub fn is_live(&self) -> bool {
        self.system.is_live()
    }

    /// Whether the system is deadlock-free under the current initial marking.
    #[wasm_bindgen(js_name = isDeadlockFree)]
    pub fn is_deadlock_free(&self) -> bool {
        self.system.is_deadlock_free()
    }
}

#[wasm_bindgen]
impl WasmSystem {
    /// Full boundedness analysis with per-place bounds and proof method.
    #[wasm_bindgen(js_name = analyzeBoundedness)]
    pub fn analyze_boundedness(&self) -> WasmBoundednessAnalysis {
        let boundedness = self.system.analyze_boundedness();

        let is_bounded = boundedness.system_bound().is_finite();

        let place_bounds = boundedness
            .place_bounds_dense()
            .into_iter()
            .map(omega_to_wasm)
            .collect();

        let method = match boundedness.method {
            BoundednessAnalysisMethod::PositivePlaceSubvariant(..) => {
                WasmBoundednessMethod::PositivePlaceSubvariant
            }
            BoundednessAnalysisMethod::CoverabilityGraph => WasmBoundednessMethod::CoverabilityGraph,
            _ => WasmBoundednessMethod::CoverabilityGraph,
        };

        WasmBoundednessAnalysis { is_bounded, place_bounds, method }
    }

    /// Full liveness analysis with per-transition levels and proof method.
    #[wasm_bindgen(js_name = analyzeLiveness)]
    pub fn analyze_liveness(&self) -> WasmLivenessAnalysis {
        let result = self.system.analyze_liveness();

        let net_level = liveness_level_to_wasm(result.net_level());

        let levels = result
            .levels_dense()
            .into_iter()
            .map(liveness_level_to_wasm)
            .collect();

        let method = match result.method {
            LivenessMethod::SNet(_) => WasmLivenessMethod::SNet,
            LivenessMethod::TNet(_) => WasmLivenessMethod::TNet,
            LivenessMethod::FreeChoice(_) => WasmLivenessMethod::FreeChoice,
            LivenessMethod::ReachabilityGraphSCC => WasmLivenessMethod::ReachabilityGraphSCC,
            LivenessMethod::Inconclusive => WasmLivenessMethod::Inconclusive,
            _ => WasmLivenessMethod::Inconclusive,
        };

        WasmLivenessAnalysis { net_level, levels, method }
    }

    /// Full deadlock-freedom analysis with reachable deadlock witnesses.
    #[wasm_bindgen(js_name = analyzeDeadlockFreedom)]
    pub fn analyze_deadlock_freedom(&self) -> WasmDeadlockAnalysis {
        let result = self.system.analyze_deadlock_freedom();

        let is_deadlock_free = result.is_deadlock_free();

        let deadlocks = result
            .deadlocks
            .iter()
            .map(|d| WasmDeadlock {
                marking: d.marking.iter().copied().collect(),
                firing_sequence: d.firing_sequence_indices(),
            })
            .collect();

        let method = match result.evidence {
            DeadlockAnalysisMethod::CommonerTheorem(_) => WasmDeadlockMethod::CommonerTheorem,
            DeadlockAnalysisMethod::Exploration => WasmDeadlockMethod::Exploration,
            DeadlockAnalysisMethod::Inconclusive => WasmDeadlockMethod::Inconclusive,
            _ => WasmDeadlockMethod::Inconclusive,
        };

        WasmDeadlockAnalysis { is_deadlock_free, deadlocks, method }
    }

    /// Analyze whether `target` (a token count per place) is reachable from
    /// the initial marking.
    ///
    /// `target` must have the same length as the number of places in the net;
    /// passing the wrong length will panic.
    #[wasm_bindgen(js_name = analyzeReachability)]
    pub fn analyze_reachability(&self, target: Vec<u32>) -> WasmReachabilityResult {
        let target = Marking::from(target);
        match self.system.analyze_reachability(&target) {
            ReachabilityResult::Reachable(proof) => {
                let firing_sequence = proof.firing_sequence_indices().unwrap_or_default();
                let wasm_proof = match &proof {
                    ReachabilityProof::FiringSequence(..) => {
                        WasmReachabilityProof::FiringSequence
                    }
                    ReachabilityProof::StronglyConnectedSNetTokenConservation {
                        marking_sum,
                    } => WasmReachabilityProof::SNetTokenConservation { marking_sum: *marking_sum },
                    ReachabilityProof::SNetMarkingEquationRationalSolution(..) => {
                        WasmReachabilityProof::SNetMarkingEquation
                    }
                    ReachabilityProof::TNetMarkingEquationIntegerSolution(..) => {
                        WasmReachabilityProof::TNetMarkingEquation
                    }
                };
                WasmReachabilityResult::Reachable { firing_sequence, proof: wasm_proof }
            }
            ReachabilityResult::Unreachable(proof) => {
                let wasm_proof = match proof {
                    UnreachabilityProof::SNetTokenConservationViolation {
                        initial_marking_sum,
                        target_marking_sum,
                    } => WasmUnreachabilityProof::SNetTokenConservationViolation {
                        initial_marking_sum,
                        target_marking_sum,
                    },
                    UnreachabilityProof::MarkingEquationNoRationalSolution => {
                        WasmUnreachabilityProof::MarkingEquationNoRationalSolution
                    }
                    UnreachabilityProof::MarkingEquationNoIntegerSolution => {
                        WasmUnreachabilityProof::MarkingEquationNoIntegerSolution
                    }
                    UnreachabilityProof::ExhaustiveSearch => {
                        WasmUnreachabilityProof::ExhaustiveSearch
                    }
                    _ => WasmUnreachabilityProof::ExhaustiveSearch,
                };
                WasmReachabilityResult::Unreachable { proof: wasm_proof }
            }
            ReachabilityResult::Inconclusive => WasmReachabilityResult::Inconclusive,
        }
    }

    /// Analyze whether `target` is coverable: whether there exists a reachable
    /// marking `M` such that `M(p) >= target(p)` for every place `p`.
    ///
    /// `target` must have the same length as the number of places in the net.
    #[wasm_bindgen(js_name = analyzeCoverability)]
    pub fn analyze_coverability(&self, target: Vec<u32>) -> WasmCoverabilityResult {
        let target = Marking::from(target);
        match self.system.analyze_coverability(&target) {
            CoverabilityResult::Coverable(proof) => {
                let firing_sequence = proof.firing_sequence_indices();
                let covering_marking =
                    proof.covering_marking.iter().copied().map(omega_to_wasm).collect();
                WasmCoverabilityResult::Coverable { firing_sequence, covering_marking }
            }
            CoverabilityResult::Uncoverable(proof) => {
                let wasm_proof = match proof {
                    NonCoverabilityProof::MarkingEquationNoRationalSolution => {
                        WasmNonCoverabilityProof::MarkingEquationNoRationalSolution
                    }
                    NonCoverabilityProof::MarkingEquationNoIntegerSolution => {
                        WasmNonCoverabilityProof::MarkingEquationNoIntegerSolution
                    }
                    NonCoverabilityProof::ExhaustiveSearch => {
                        WasmNonCoverabilityProof::ExhaustiveSearch
                    }
                    _ => WasmNonCoverabilityProof::ExhaustiveSearch,
                };
                WasmCoverabilityResult::Uncoverable { proof: wasm_proof }
            }
        }
    }
}

#[wasm_bindgen]
impl WasmSystem {
    /// Export the net topology as a Graphviz DOT string.
    ///
    /// Useful for interoperating with external tools or rendering a read-only
    /// view via `@viz-js/viz`. Places are circles, transitions are rectangles.
    /// Labels from the PNML document are used when available.
    #[wasm_bindgen(js_name = toDot)]
    pub fn to_dot(&self) -> String {
        let net = self.system.net().as_ref();
        let mut out = String::new();
        let name = self
            .labels
            .as_ref()
            .and_then(|l| l.net_name())
            .unwrap_or("petri_net");
        out.push_str(&format!(
            "digraph {} {{\n  rankdir=LR;\n  node [fontname=\"sans-serif\"];\n",
            dot_id(name)
        ));

        for (i, _pk) in self.place_keys.iter().enumerate() {
            let label = self
                .labels
                .as_ref()
                .and_then(|l| l.place_name_at(i))
                .unwrap_or("");
            let display = if label.is_empty() {
                format!("p{i}")
            } else {
                label.to_string()
            };
            out.push_str(&format!(
                "  p{i} [shape=circle label={}];\n",
                dot_id(&display)
            ));
        }

        for (i, _tk) in self.transition_keys.iter().enumerate() {
            let label = self
                .labels
                .as_ref()
                .and_then(|l| l.transition_name_at(i))
                .unwrap_or("");
            let display = if label.is_empty() {
                format!("t{i}")
            } else {
                label.to_string()
            };
            out.push_str(&format!(
                "  t{i} [shape=box label={}];\n",
                dot_id(&display)
            ));
        }

        let pk_to_dense: HashMap<PlaceKey, usize> = self.place_keys
            .iter()
            .enumerate()
            .map(|(i, &pk)| (pk, i))
            .collect();
        let tk_to_dense: HashMap<TransitionKey, usize> = self.transition_keys
            .iter()
            .enumerate()
            .map(|(i, &tk)| (tk, i))
            .collect();

        for arc in net.arcs() {
            match arc {
                PetriArc::PlaceToTransition(p, t) => {
                    let pi = pk_to_dense[&p];
                    let ti = tk_to_dense[&t];
                    out.push_str(&format!("  p{pi} -> t{ti};\n"));
                }
                PetriArc::TransitionToPlace(t, p) => {
                    let ti = tk_to_dense[&t];
                    let pi = pk_to_dense[&p];
                    out.push_str(&format!("  t{ti} -> p{pi};\n"));
                }
            }
        }

        out.push('}');
        out
    }
}

// ---------------------------------------------------------------------------
// WasmNetBuilder
// ---------------------------------------------------------------------------

/// A mutable Petri net builder exposed to JavaScript.
///
/// Create one with `new WasmNetBuilder()` or via `WasmSystem.toBuilder()`.
/// Edit the net by adding/removing places, transitions, and arcs, setting
/// names, positions, and initial token counts.
/// Call `build()` to produce a `WasmSystem` ready for simulation and analysis.
///
/// Node IDs returned by `addPlace` / `addTransition` are stable u32 IDs that
/// never change during the lifetime of the builder and are never reused
/// after a node is removed. The `structure()` snapshot uses these same IDs so
/// that Cytoscape element IDs remain stable during editing.
#[wasm_bindgen]
pub struct WasmNetBuilder {
    builder: NetBuilder,
    net_name: Option<String>,
    place_names: HashMap<u32, String>,
    transition_names: HashMap<u32, String>,
    place_positions: HashMap<u32, (f64, f64)>,
    transition_positions: HashMap<u32, (f64, f64)>,
    initial_tokens: HashMap<u32, u32>,
    /// Maps JS u32 IDs to the actual PlaceKey handles in the builder.
    place_key_map: HashMap<u32, PlaceKey>,
    /// Maps JS u32 IDs to the actual TransitionKey handles in the builder.
    transition_key_map: HashMap<u32, TransitionKey>,
    /// Monotonic counter for next place JS ID.
    next_place_id: u32,
    /// Monotonic counter for next transition JS ID.
    next_transition_id: u32,
}

#[wasm_bindgen]
impl WasmNetBuilder {
    /// Creates an empty builder.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmNetBuilder {
        WasmNetBuilder {
            builder: NetBuilder::new(),
            net_name: None,
            place_names: HashMap::new(),
            transition_names: HashMap::new(),
            place_positions: HashMap::new(),
            transition_positions: HashMap::new(),
            initial_tokens: HashMap::new(),
            place_key_map: HashMap::new(),
            transition_key_map: HashMap::new(),
            next_place_id: 0,
            next_transition_id: 0,
        }
    }

    // ---- Nodes ----

    /// Add a place at position `(x, y)` with an optional name.
    /// Returns the stable builder ID for the new place.
    #[wasm_bindgen(js_name = addPlace)]
    pub fn add_place(&mut self, x: f64, y: f64, name: Option<String>) -> u32 {
        let pk = self.builder.add_place();
        let id = self.next_place_id;
        self.next_place_id += 1;
        self.place_key_map.insert(id, pk);
        self.place_positions.insert(id, (x, y));
        if let Some(n) = name {
            self.place_names.insert(id, n);
        }
        id
    }

    /// Add a transition at position `(x, y)` with an optional name.
    /// Returns the stable builder ID for the new transition.
    #[wasm_bindgen(js_name = addTransition)]
    pub fn add_transition(&mut self, x: f64, y: f64, name: Option<String>) -> u32 {
        let tk = self.builder.add_transition();
        let id = self.next_transition_id;
        self.next_transition_id += 1;
        self.transition_key_map.insert(id, tk);
        self.transition_positions.insert(id, (x, y));
        if let Some(n) = name {
            self.transition_names.insert(id, n);
        }
        id
    }

    /// Remove a place and all its incident arcs.
    /// Returns `true` if the place existed and was removed.
    #[wasm_bindgen(js_name = removePlace)]
    pub fn remove_place(&mut self, place_id: u32) -> bool {
        let Some(&pk) = self.place_key_map.get(&place_id) else {
            return false;
        };
        if self.builder.remove_place(pk) {
            self.place_key_map.remove(&place_id);
            self.place_names.remove(&place_id);
            self.place_positions.remove(&place_id);
            self.initial_tokens.remove(&place_id);
            true
        } else {
            false
        }
    }

    /// Remove a transition and all its incident arcs.
    /// Returns `true` if the transition existed and was removed.
    #[wasm_bindgen(js_name = removeTransition)]
    pub fn remove_transition(&mut self, transition_id: u32) -> bool {
        let Some(&tk) = self.transition_key_map.get(&transition_id) else {
            return false;
        };
        if self.builder.remove_transition(tk) {
            self.transition_key_map.remove(&transition_id);
            self.transition_names.remove(&transition_id);
            self.transition_positions.remove(&transition_id);
            true
        } else {
            false
        }
    }

    // ---- Arcs ----

    /// Add a place→transition arc. Returns `true` if the arc was newly added.
    #[wasm_bindgen(js_name = addArcPT)]
    pub fn add_arc_pt(&mut self, place_id: u32, transition_id: u32) -> bool {
        let Some(&pk) = self.place_key_map.get(&place_id) else { return false };
        let Some(&tk) = self.transition_key_map.get(&transition_id) else { return false };
        self.builder.add_arc((pk, tk))
    }

    /// Add a transition→place arc. Returns `true` if the arc was newly added.
    #[wasm_bindgen(js_name = addArcTP)]
    pub fn add_arc_tp(&mut self, transition_id: u32, place_id: u32) -> bool {
        let Some(&tk) = self.transition_key_map.get(&transition_id) else { return false };
        let Some(&pk) = self.place_key_map.get(&place_id) else { return false };
        self.builder.add_arc((tk, pk))
    }

    /// Remove a place→transition arc. Returns `true` if it was present.
    #[wasm_bindgen(js_name = removeArcPT)]
    pub fn remove_arc_pt(&mut self, place_id: u32, transition_id: u32) -> bool {
        let Some(&pk) = self.place_key_map.get(&place_id) else { return false };
        let Some(&tk) = self.transition_key_map.get(&transition_id) else { return false };
        self.builder.remove_arc((pk, tk))
    }

    /// Remove a transition→place arc. Returns `true` if it was present.
    #[wasm_bindgen(js_name = removeArcTP)]
    pub fn remove_arc_tp(&mut self, transition_id: u32, place_id: u32) -> bool {
        let Some(&tk) = self.transition_key_map.get(&transition_id) else { return false };
        let Some(&pk) = self.place_key_map.get(&place_id) else { return false };
        self.builder.remove_arc((tk, pk))
    }

    // ---- Labels & positions ----

    #[wasm_bindgen(js_name = setNetName)]
    pub fn set_net_name(&mut self, name: String) {
        self.net_name = Some(name);
    }

    #[wasm_bindgen(js_name = setPlaceName)]
    pub fn set_place_name(&mut self, place_id: u32, name: String) {
        self.place_names.insert(place_id, name);
    }

    #[wasm_bindgen(js_name = setTransitionName)]
    pub fn set_transition_name(&mut self, transition_id: u32, name: String) {
        self.transition_names.insert(transition_id, name);
    }

    #[wasm_bindgen(js_name = setPlacePosition)]
    pub fn set_place_position(&mut self, place_id: u32, x: f64, y: f64) {
        self.place_positions.insert(place_id, (x, y));
    }

    #[wasm_bindgen(js_name = setTransitionPosition)]
    pub fn set_transition_position(&mut self, transition_id: u32, x: f64, y: f64) {
        self.transition_positions.insert(transition_id, (x, y));
    }

    /// Set the initial token count for a place.
    #[wasm_bindgen(js_name = setInitialTokens)]
    pub fn set_initial_tokens(&mut self, place_id: u32, tokens: u32) {
        if tokens == 0 {
            self.initial_tokens.remove(&place_id);
        } else {
            self.initial_tokens.insert(place_id, tokens);
        }
    }

    // ---- Queries ----

    /// Number of active places.
    #[wasm_bindgen(js_name = placeCount)]
    pub fn place_count(&self) -> u32 {
        self.builder.place_count() as u32
    }

    /// Number of active transitions.
    #[wasm_bindgen(js_name = transitionCount)]
    pub fn transition_count(&self) -> u32 {
        self.builder.transition_count() as u32
    }

    /// Snapshot of the builder's current structure, using stable builder IDs.
    ///
    /// Suitable for rendering the net being edited in Cytoscape.js — use the
    /// `id` field of each node as the Cytoscape element ID.
    pub fn structure(&self) -> WasmBuilderStructure {
        // Build reverse maps: PlaceKey → JS ID, TransitionKey → JS ID.
        let pk_to_js: HashMap<PlaceKey, u32> = self.place_key_map
            .iter()
            .map(|(&id, &pk)| (pk, id))
            .collect();
        let tk_to_js: HashMap<TransitionKey, u32> = self.transition_key_map
            .iter()
            .map(|(&id, &tk)| (tk, id))
            .collect();

        let sorted_places: Vec<PlaceKey> = self.builder.place_keys().collect();
        let sorted_transitions: Vec<TransitionKey> = self.builder.transition_keys().collect();

        let places: Vec<WasmBuilderPlace> = sorted_places
            .iter()
            .map(|pk| {
                let id = pk_to_js[pk];
                let (x, y) = self.place_positions.get(&id).copied().unwrap_or((0.0, 0.0));
                WasmBuilderPlace {
                    id,
                    name: self.place_names.get(&id).cloned(),
                    x,
                    y,
                    initial_tokens: self.initial_tokens.get(&id).copied().unwrap_or(0),
                }
            })
            .collect();

        let transitions: Vec<WasmBuilderTransition> = sorted_transitions
            .iter()
            .map(|tk| {
                let id = tk_to_js[tk];
                let (x, y) = self.transition_positions.get(&id).copied().unwrap_or((0.0, 0.0));
                WasmBuilderTransition {
                    id,
                    name: self.transition_names.get(&id).cloned(),
                    x,
                    y,
                }
            })
            .collect();

        let mut pt_arcs: Vec<WasmBuilderArc> = Vec::new();
        let mut tp_arcs: Vec<WasmBuilderArc> = Vec::new();
        for arc in self.builder.arcs() {
            match arc {
                BuilderArc::PlaceToTransition(p, t) => {
                    pt_arcs.push(WasmBuilderArc {
                        source_id: pk_to_js[&p],
                        target_id: tk_to_js[&t],
                    });
                }
                BuilderArc::TransitionToPlace(t, p) => {
                    tp_arcs.push(WasmBuilderArc {
                        source_id: tk_to_js[&t],
                        target_id: pk_to_js[&p],
                    });
                }
            }
        }

        WasmBuilderStructure {
            places,
            transitions,
            pt_arcs,
            tp_arcs,
            net_name: self.net_name.clone(),
        }
    }

    // ---- Build ----

    /// Validate the current net and produce a `WasmSystem` ready for
    /// simulation and analysis.
    ///
    /// Active nodes are compacted into dense `0..n` indices. Compact index `i`
    /// corresponds to the node whose builder ID is `place_ids[i]` /
    /// `transition_ids[i]` (sorted ascending).
    ///
    /// # Errors
    ///
    /// Returns a JS error string if the net is empty (no places or transitions)
    /// or disconnected (isolated nodes with no arcs).
    pub fn build(&self) -> Result<WasmSystem, JsValue> {
        let sorted_place_ids: Vec<PlaceKey> = self.builder.place_keys().collect();
        let sorted_trans_ids: Vec<TransitionKey> = self.builder.transition_keys().collect();

        // Reverse map: PlaceKey → JS ID.
        let pk_to_js: HashMap<PlaceKey, u32> = self.place_key_map
            .iter()
            .map(|(&id, &pk)| (pk, id))
            .collect();
        let tk_to_js: HashMap<TransitionKey, u32> = self.transition_key_map
            .iter()
            .map(|(&id, &tk)| (tk, id))
            .collect();

        let marking_vec: Vec<u32> = sorted_place_ids
            .iter()
            .map(|pk| {
                let js_id = pk_to_js[pk];
                self.initial_tokens.get(&js_id).copied().unwrap_or(0)
            })
            .collect();

        let net = self.builder.clone().build()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut labels = NetLabels::with_capacity(
            net.place_count() as usize,
            net.transition_count() as usize,
        );
        for (compact_idx, pk) in sorted_place_ids.iter().enumerate() {
            let js_id = pk_to_js[pk];
            if let Some(name) = self.place_names.get(&js_id) {
                labels.set_place_name_at(compact_idx, name);
            }
        }
        for (compact_idx, tk) in sorted_trans_ids.iter().enumerate() {
            let js_id = tk_to_js[tk];
            if let Some(name) = self.transition_names.get(&js_id) {
                labels.set_transition_name_at(compact_idx, name);
            }
        }
        if let Some(name) = &self.net_name {
            labels.set_net_name(name);
        }

        // PnmlGraphics is not constructible from external crates (fields use
        // pub(crate) types). Skip graphics for builder-created systems.
        let graphics = None;

        let initial_marking = Marking::from(marking_vec.clone());
        let system = System::new(Rc::new(net), marking_vec);

        Ok(WasmSystem::from_parts(system, initial_marking, Some(labels), graphics))
    }
}

// ---------------------------------------------------------------------------

/// Produce a quoted DOT identifier, escaping internal double-quotes.
fn dot_id(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn omega_to_wasm(omega: Omega) -> WasmOmega {
    match omega {
        Omega::Finite(n) => WasmOmega::Finite(n),
        Omega::Unbounded => WasmOmega::Unbounded,
    }
}

fn liveness_level_to_wasm(
    level: petrivet::analysis::model::LivenessLevel,
) -> WasmLivenessLevel {
    use petrivet::analysis::model::LivenessLevel;
    match level {
        LivenessLevel::L0 => WasmLivenessLevel::L0,
        LivenessLevel::L1 => WasmLivenessLevel::L1,
        LivenessLevel::L2 => WasmLivenessLevel::L2,
        LivenessLevel::L3 => WasmLivenessLevel::L3,
        LivenessLevel::L4 => WasmLivenessLevel::L4,
    }
}
