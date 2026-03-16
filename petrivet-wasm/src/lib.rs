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

use std::rc::Rc;

use petrivet::analysis::model::{
    BoundednessAnalysisMethod, CoverabilityResult, DeadlockAnalysisMethod, LivenessMethod,
    NonCoverabilityProof, ReachabilityProof, ReachabilityResult, UnreachabilityProof,
};
use petrivet::labeled::NetLabels;
use petrivet::marking::{Marking, Omega};
use petrivet::net::{Arc as PetriArc, Net, Place, Transition};
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
#[wasm_bindgen]
pub struct WasmSystem {
    system: System<Rc<Net>>,
    initial_marking: Marking,
    labels: Option<NetLabels>,
    graphics: Option<PnmlGraphics>,
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
        let (net, marking) = system.into_parts();
        let system = System::new(Rc::new(net), marking);

        Ok(WasmSystem {
            system,
            initial_marking,
            labels: Some(labels),
            graphics: Some(graphics),
        })
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

        let pt_arcs = net
            .arcs()
            .filter_map(|arc| match arc {
                PetriArc::PlaceToTransition(p, t) => Some(WasmArc {
                    source: p.index() as u32,
                    target: t.index() as u32,
                }),
                PetriArc::TransitionToPlace(..) => None,
            })
            .collect();

        let tp_arcs = net
            .arcs()
            .filter_map(|arc| match arc {
                PetriArc::TransitionToPlace(t, p) => Some(WasmArc {
                    source: t.index() as u32,
                    target: p.index() as u32,
                }),
                PetriArc::PlaceToTransition(..) => None,
            })
            .collect();

        let place_positions = (0..net.place_count())
            .map(|i| {
                self.graphics
                    .as_ref()
                    .and_then(|g| g.place_graphics[i].as_ref())
                    .and_then(|g| g.position.as_ref())
                    .map(|pos| WasmPosition { x: pos.x, y: pos.y })
            })
            .collect();

        let transition_positions = (0..net.transition_count())
            .map(|i| {
                self.graphics
                    .as_ref()
                    .and_then(|g| g.transition_graphics[i].as_ref())
                    .and_then(|g| g.position.as_ref())
                    .map(|pos| WasmPosition { x: pos.x, y: pos.y })
            })
            .collect();

        let place_names = (0..net.place_count())
            .map(|i| {
                self.labels
                    .as_ref()
                    .and_then(|l| l.place_name(Place::from_index(i)))
                    .map(str::to_string)
            })
            .collect();

        let transition_names = (0..net.transition_count())
            .map(|i| {
                self.labels
                    .as_ref()
                    .and_then(|l| l.transition_name(Transition::from_index(i)))
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
            place_count: net.place_count() as u32,
            transition_count: net.transition_count() as u32,
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
        self.system
            .enabled_transitions()
            .iter()
            .map(|t| t.index() as u32)
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
        let t = Transition::from_index(transition_index as usize);
        self.system.try_fire(t).is_ok()
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

        let is_bounded = boundedness.of_system().is_finite();

        let place_bounds = boundedness
            .place_bounds
            .iter()
            .copied()
            .map(omega_to_wasm)
            .collect();

        let method = match boundedness.method {
            BoundednessAnalysisMethod::PositivePlaceSubvariant(_) => {
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
            .levels
            .iter()
            .copied()
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
                firing_sequence: d
                    .firing_sequence
                    .iter()
                    .map(|t| t.index() as u32)
                    .collect(),
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
                let (firing_sequence, wasm_proof) = match proof {
                    ReachabilityProof::FiringSequence(seq) => (
                        seq.iter().map(|t| t.index() as u32).collect(),
                        WasmReachabilityProof::FiringSequence,
                    ),
                    ReachabilityProof::StronglyConnectedSNetTokenConservation {
                        marking_sum,
                    } => (vec![], WasmReachabilityProof::SNetTokenConservation { marking_sum }),
                    ReachabilityProof::SNetMarkingEquationRationalSolution(_) => {
                        (vec![], WasmReachabilityProof::SNetMarkingEquation)
                    }
                    ReachabilityProof::TNetMarkingEquationIntegerSolution(_) => {
                        (vec![], WasmReachabilityProof::TNetMarkingEquation)
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
                let firing_sequence = proof
                    .firing_sequence
                    .iter()
                    .map(|t| t.index() as u32)
                    .collect();
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
