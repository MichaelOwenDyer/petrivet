//! The SCC computation behind liveness must not recurse over the reachability graph.
//!
//! A net that moves a token at a time from `p0` to `p1` has a reachability graph that is one long chain.
//! A recursive SCC search overflows the stack on it, which aborts the process (the test thread has 2 MiB).

#![cfg(feature = "pnml")]

use petrivet::state_space::ExplorationOrder;
use petrivet::system::PetriNet;

#[test]
fn liveness_of_a_long_chain_state_space() {
    let tokens = 200_000;
    let pnml = format!(
        r#"<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
  <net id="n" type="http://www.pnml.org/version-2009/grammar/ptnet"><page id="pg">
    <place id="p0"><initialMarking><text>{tokens}</text></initialMarking></place>
    <place id="p1"/>
    <transition id="t0"/>
    <arc id="a0" source="p0" target="t0"/>
    <arc id="a1" source="t0" target="p1"/>
  </page></net>
</pnml>"#
    );
    let sys = PetriNet::from_pnml(&pnml).unwrap();
    let graph = sys.explore_reachability_graph(ExplorationOrder::BreadthFirst).build_graph_until(None).unwrap();
    assert_eq!(graph.marking_count(), tokens + 1);
    assert!(graph.transition_liveness().is_quasi_live());
    assert!(!graph.transition_liveness().is_live());
}
