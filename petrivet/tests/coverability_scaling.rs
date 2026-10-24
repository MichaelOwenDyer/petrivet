//! The coverability graph of a bounded net must not cost much more than its reachability graph.
//!
//! Karp-Miller acceleration used to visit every graph ancestor of the source marking for every fired edge, which is
//! quadratic: on benchmark models of 8,000 to 16,000 markings the coverability graph took 36 to 51 s against 12 ms for
//! the reachability graph. It now walks the path of the exploration
//! tree, and visits all graph ancestors only once an ω has appeared, i.e. for unbounded nets.

#![cfg(feature = "pnml")]

use petrivet::state_space::ExplorationOrder;
use petrivet::system::PetriNet;
use std::time::{Duration, Instant};

/// A ring of three places, `tokens` tokens on `p0`: the markings are the ways to split the tokens over the places
/// (about `tokens^2 / 2` of them) and the graph is strongly connected, so nearly every marking is a graph ancestor of
/// every other one while the path of the exploration tree stays short.
fn ring(tokens: u32) -> PetriNet<petrivet::net::Net> {
    let pnml = format!(
        r#"<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
  <net id="n" type="http://www.pnml.org/version-2009/grammar/ptnet"><page id="pg">
    <place id="p0"><initialMarking><text>{tokens}</text></initialMarking></place>
    <place id="p1"/>
    <place id="p2"/>
    <transition id="t0"/><transition id="t1"/><transition id="t2"/>
    <arc id="a0" source="p0" target="t0"/><arc id="a1" source="t0" target="p1"/>
    <arc id="a2" source="p1" target="t1"/><arc id="a3" source="t1" target="p2"/>
    <arc id="a4" source="p2" target="t2"/><arc id="a5" source="t2" target="p0"/>
  </page></net>
</pnml>"#
    );
    PetriNet::from_pnml(&pnml).unwrap()
}

#[test]
fn coverability_graph_of_a_bounded_net_is_cheap_and_equals_the_reachability_graph() {
    let sys = ring(250); // 31,626 markings
    let reachability = sys.explore_reachability_graph(ExplorationOrder::BreadthFirst).build_graph_until(None).unwrap();
    assert_eq!(reachability.marking_count(), 251 * 252 / 2);

    let start = Instant::now();
    let coverability = sys.explore_coverability_graph(ExplorationOrder::BreadthFirst).build_graph_until(None).unwrap();
    // generous: the version that visits all graph ancestors per edge needs a multiple of this, this one well under a second
    assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());

    assert!(!coverability.has_omega_marking());
    assert_eq!(coverability.marking_count(), reachability.marking_count());
    assert_eq!(coverability.transition_count(), reachability.transition_count());
}

#[test]
fn an_unbounded_net_still_gets_its_omega() {
    // t0 consumes from p0 and returns the token, and adds one to p1 each time: p1 is unbounded
    let pnml = r#"<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
  <net id="n" type="http://www.pnml.org/version-2009/grammar/ptnet"><page id="pg">
    <place id="p0"><initialMarking><text>1</text></initialMarking></place>
    <place id="p1"/>
    <transition id="t0"/>
    <arc id="a0" source="p0" target="t0"/>
    <arc id="a1" source="t0" target="p0"/>
    <arc id="a2" source="t0" target="p1"/>
  </page></net>
</pnml>"#;
    let sys = PetriNet::from_pnml(pnml).unwrap();
    let graph = sys.explore_coverability_graph(ExplorationOrder::BreadthFirst).build_graph_until(None).unwrap();
    assert!(graph.has_omega_marking());
    assert!(!graph.is_bounded());
}
