//! Regression tests for refutations that rested on an increment constraint (Wimmel & Wolf 2011).
//!
//! Increment constraints steer the search towards a realizable firing count vector but are not facts
//! about the net. Both targets below are reachable (found by LoLA and by exhaustive exploration), and
//! the CEGAR analysis used to prove them unreachable. It must now answer "reachable" or give up.

#![cfg(feature = "pnml")]

use petrivet::prelude::*;
use petrivet::system::PetriNet;
use petrivet::system::reachability::{CoverabilityResult, ReachabilityResult};
use std::collections::HashMap;

fn load(path: &str, target: &[u32]) -> (PetriNet<petrivet::net::Net>, Marking<u32>) {
    let text = std::fs::read_to_string(format!("{}/tests/fixtures/cegar/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let sys = PetriNet::from_pnml(&text).unwrap();
    let labels = sys.labels.as_deref().unwrap();
    let by_id: HashMap<String, _> = sys.places().map(|p| (labels.place_id(p).unwrap().to_string(), p)).collect();
    let target = target.iter().enumerate().map(|(i, n)| (by_id[&format!("p{i}")], *n)).collect();
    (sys, target)
}

#[test]
fn reachable_target_is_not_refuted() {
    let (sys, target) = load("increment_unsound_reach.pnml", &[1, 0, 0, 0, 3]);
    let result = sys.analyze_reachability(target, None);
    assert!(!result.is_unreachable(), "a reachable target was refuted: {result:?}");
    assert!(matches!(result, ReachabilityResult::Reachable { .. } | ReachabilityResult::Inconclusive));
}

#[test]
fn coverable_target_is_not_refuted() {
    let (sys, target) = load("increment_unsound_cover.pnml", &[1, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    let result = sys.analyze_coverability(target, None);
    assert!(!matches!(result, CoverabilityResult::Uncoverable { .. }), "a coverable target was refuted: {result:?}");
}
