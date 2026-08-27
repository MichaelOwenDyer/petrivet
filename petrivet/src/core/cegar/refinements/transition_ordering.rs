use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::SmtSolver;
use crate::core::marking::IdxMarking;
use crate::core::net::{DenseNet, IdxNode, PlaceIdx, TransitionIdx};
use ahash::{HashMap, HashMapExt};
use petgraph::Graph;
use petgraph::algo::tarjan_scc;
use petgraph::graph::NodeIndex;
use petgraph::prelude::EdgeRef;

/// Checks for `(transition, place)` pairs where `transition` consumes more of `place` than
/// `m0` provides, and that dependency is part of a cycle in the graph of all such dependencies.
///
/// Such dependencies allow for solutions to the marking equation that are not realizable because
/// they would require a transition to "borrow" tokens from an insufficiently marked place,
/// in order to enable another transition to replace those tokens.
/// The SMT solver does not know that the original transition was not enabled since it does not
/// understand transition firing order.
///
/// We resolve this by enforcing that the SMT solver assign a partial order to the involved
/// transitions, such that for each of these `(transition, place)` pairs, at least one transition
/// that produces into `place` must fire before `transition` can fire.
pub struct TransitionOrderingRule;

impl TransitionOrderingRule {
    /// Constructs a graph which has an edge `place → transition` whenever
    /// `consume(transition, place) > m0(place)`, and an edge `transition → place` whenever
    /// `transition` produces into `place`.
    ///
    /// Reports back all `place -> transition` edges that are part of a nontrivial strongly
    /// connected component of this graph, which are the only ones that can lead to unrealizable
    /// "ouroboros" cycles.
    pub fn check(
        net: &DenseNet,
        m0: &IdxMarking<u32>,
    ) -> Option<TransitionOrderingRefinement> {
        let mut graph: Graph<IdxNode, ()> = Graph::new();
        let mut node_index: HashMap<IdxNode, NodeIndex> = HashMap::new();

        // place -> transition: transition needs more tokens from place than m0 provides.
        for t_idx in net.transition_indices() {
            for &p_idx in &net.preset_t[t_idx] {
                if u32::from(net.incidence_matrix.get_consume(t_idx, p_idx)) > m0[p_idx] {
                    let p_node = *node_index.entry(IdxNode::Place(p_idx))
                        .or_insert_with(|| graph.add_node(IdxNode::Place(p_idx)));
                    let t_node = *node_index.entry(IdxNode::Transition(t_idx))
                        .or_insert_with(|| graph.add_node(IdxNode::Transition(t_idx)));
                    graph.add_edge(p_node, t_node, ());
                }
            }
        }

        // transition -> place: transition produces into place (only meaningful for a place already
        // known to be in the graph, i.e. already insufficiently marked for someone).
        for t_idx in net.transition_indices() {
            for &p_idx in &net.postset_t[t_idx] {
                if let Some(&p_node) = node_index.get(&IdxNode::Place(p_idx))
                    && net.incidence_matrix.get_effect(t_idx, p_idx) > 0 {
                    let t_node = *node_index.entry(IdxNode::Transition(t_idx))
                        .or_insert_with(|| graph.add_node(IdxNode::Transition(t_idx)));
                    graph.add_edge(t_node, p_node, ());
                }
            }
        }

        if graph.node_count() == 0 {
            return None;
        }

        let scc_id: HashMap<NodeIndex, usize> = tarjan_scc(&graph)
            .into_iter()
            .enumerate()
            .flat_map(|(i, members)| members.into_iter().map(move |n| (n, i)))
            .collect();

        let mut cyclic_dependencies = Vec::new();
        for edge in graph.edge_references() {
            let src = edge.source();
            let dst = edge.target();
            // Only place -> transition edges within the same SCC are relevant to the refinement.
            // The transition -> place edges exist purely to let the SCC computation find the cycle back.
            if scc_id[&src] == scc_id[&dst]
                && let IdxNode::Place(p_idx) = graph[src]
                && let IdxNode::Transition(t_idx) = graph[dst] {
                cyclic_dependencies.push(InitiallyDisabledTransition { t_idx, p_idx });
            }
        }
        (!cyclic_dependencies.is_empty()).then_some(TransitionOrderingRefinement { cyclic_dependencies })
    }
}

/// A `(transition, place)` pair where `transition` consumes more of `place` than `m0` provides,
/// and that dependency is part of a cycle in the graph of all such dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitiallyDisabledTransition {
    /// The transition that needs more tokens from `place` than `m0` provides.
    t_idx: TransitionIdx,
    /// At least one feeder of this place must fire before `transition` can fire.
    p_idx: PlaceIdx,
}

/// A refinement that informs the SMT solver of all initially disabled transitions which are part
/// of a cycle in the dependency graph, and enforces a partial ordering on the disabled transitions
/// and the feeder transitions of the insufficiently marked places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionOrderingRefinement {
    cyclic_dependencies: Vec<InitiallyDisabledTransition>,
}

impl TransitionOrderingRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        problem: &CegarProblem,
        transition_terms: &[S::Int],
        callback: Option<&dyn Fn(IdxLemma)>
    ) {
        let mut order_terms = HashMap::new();

        for InitiallyDisabledTransition { t_idx, p_idx } in self.cyclic_dependencies {
            let feeders: Vec<TransitionIdx> = problem.net.preset_p[p_idx]
                .iter()
                .copied()
                .filter(|&feeder| feeder != t_idx && problem.net.incidence_matrix.get_effect(feeder, p_idx) > 0)
                .collect();

            if feeders.is_empty() {
                // No transition can ever add more tokens to `p`, so `t` can never fire at all.
                let zero = solver.mk_int(0);
                let t_dead = solver.eq(&transition_terms[t_idx], &zero);
                let lemma = IdxLemma::TransitionOrdering { t_idx, p_idx, feeders };
                if let Some(callback) = callback {
                    callback(lemma.clone());
                }
                solver.assert_tracked(&t_dead, lemma);
                return;
            }

            let zero = solver.mk_int(0);
            let t_fires = solver.gt(&transition_terms[t_idx], &zero);
            let some_feeder_fires_first = {
                let mut feeder_conditions = Vec::with_capacity(feeders.len());
                let t_order = order_terms
                    .entry(t_idx)
                    .or_insert_with(|| solver.mk_int_var(&format!("o{t_idx}")))
                    .clone();
                for &feeder in &feeders {
                    let feeder_order = order_terms
                        .entry(feeder)
                        .or_insert_with(|| solver.mk_int_var(&format!("o{feeder}")))
                        .clone();
                    let feeder_fires = solver.gt(&transition_terms[feeder], &zero);
                    let feeder_fires_first = solver.lt(&feeder_order, &t_order);
                    feeder_conditions.push(solver.and([feeder_fires, feeder_fires_first]));
                }
                solver.or(feeder_conditions)
            };
            let implication = solver.implies(&t_fires, &some_feeder_fires_first);
            let lemma = IdxLemma::TransitionOrdering { t_idx, p_idx, feeders };
            if let Some(callback) = callback {
                callback(lemma.clone());
            }
            solver.assert_tracked(&implication, lemma);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::prelude::NetBuilder;

    #[test]
    fn finds_the_genuine_cycle_and_the_escape_route() {
        let mut b = NetBuilder::new();
        let [s1, s2, s3] = b.add_places();
        let [t1, t2, u, u_prime] = b.add_transitions();
        b.add_arcs((s2, t1, s1));
        b.add_arcs((s1, t2, s2));
        b.add_arcs((s3, u, s2));
        b.add_arcs((s2, u_prime, s3));
        let net = b.build().unwrap();
        let m0 = net.mapping.decode(Marking::from([(s3, 1)]));

        let refinement = TransitionOrderingRule::check(
            &net.dense_net,
            &m0,
        ).expect("cycle should be detected");

        assert_eq!(refinement.cyclic_dependencies.len(), 2, "exactly two dependencies should be reported");
        assert!(refinement.cyclic_dependencies.contains(&InitiallyDisabledTransition {
            t_idx: net.mapping.transition_idx(t1).expect("transition in built net"),
            p_idx: net.mapping.place_idx(s2).expect("place in built net")
        }));
        assert!(refinement.cyclic_dependencies.contains(&InitiallyDisabledTransition {
            t_idx: net.mapping.transition_idx(t2).expect("transition in built net"),
            p_idx: net.mapping.place_idx(s1).expect("place in built net")
        }));
    }

    /// If the "escape route" place starts sufficiently marked for *everyone* that needs it, no
    /// cycle survives at all - the whole thing degenerates to an acyclic producer chain that
    /// `GuidedExplorer` can already order on its own, so nothing should be reported.
    #[test]
    fn no_requirements_when_nothing_is_actually_cyclic() {
        let mut b = NetBuilder::new();
        let [p, q] = b.add_places();
        let t = b.add_transition();
        b.add_arcs((p, t, q));
        let net = b.build().unwrap();
        let m0 = Marking::from([(p, 1)]); // p already sufficiently marked for t
        let refinement = TransitionOrderingRule::check(&net.dense_net, &net.mapping.decode(m0));
        assert!(refinement.is_none(), "no place is insufficiently marked, so nothing qualifies");
    }
}
