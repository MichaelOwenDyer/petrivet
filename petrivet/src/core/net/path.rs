use crate::core::net::{IdxNode, PlaceIdx, TransitionIdx};
use crate::net::Net;
use crate::system::PetriNet;
use graph_cycles::Cycles;
use petgraph::algo::tarjan_scc;
use petgraph::visit::NodeFiltered;
use std::ops::Deref;
use tap::TryConv;

/// A path through the directed bipartite graph of a Petri net.
///
/// This is an alternating list of [`Places`](Place) and [`Transitions`](Transition)
/// with implied arcs between them. It can start and end on either type of node.
///
/// Implementation note: a Path has no knowledge of the Petri net it belongs to,
/// so it is just a list of nodes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct IdxPath {
    /// Nodes in the path.
    nodes: Vec<IdxNode>,
}

impl IdxPath {
    /// Returns an iterator over all [`Places`](Place) in the path.
    pub fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
        self.nodes.iter().filter_map(|node| match node {
            IdxNode::Place(p_idx) => Some(*p_idx),
            IdxNode::Transition(_) => None,
        })
    }

    /// Returns an iterator over all [`Transitions`](Transition) in the path.
    pub fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
        self.nodes.iter().filter_map(|node| match node {
            IdxNode::Transition(t_idx) => Some(*t_idx),
            IdxNode::Place(_) => None,
        })
    }
}

impl Deref for IdxPath {
    type Target = [IdxNode];

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl TryFrom<Vec<IdxNode>> for IdxPath {
    type Error = ();

    fn try_from(nodes: Vec<IdxNode>) -> Result<Self, Self::Error> {
        nodes
            .array_windows::<2>()
            .try_for_each(|neighbors| match neighbors {
                [IdxNode::Place(_), IdxNode::Transition(_)]
                | [IdxNode::Transition(_), IdxNode::Place(_)] => Ok(()),
                _ => Err(()),
            })?;
        Ok(Self { nodes })
    }
}

/// A [`IdxPath`] of nodes whose end leads into its beginning, forming a cycle.
///
/// The path is guaranteed to be nonempty with even length.
/// The arc between the last node and the first node must maintain the
/// alternating sequence of places and transitions.
///
/// Note: it is not currently guaranteed that the cycle is simple
/// (that it does not contain repeated nodes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdxCircuit {
    /// Nodes in the circuit.
    /// Guaranteed to have positive even length.
    path: IdxPath,
}

impl TryFrom<IdxPath> for IdxCircuit {
    type Error = ();

    fn try_from(path: IdxPath) -> Result<Self, Self::Error> {
        if path.is_empty() {
            return Err(()); // A circuit cannot be empty
        }
        if !path.len().is_multiple_of(2) {
            return Err(()); // A circuit must have an even number of nodes to maintain alternation.
        }
        Ok(Self { path })
    }
}

impl Deref for IdxCircuit {
    type Target = IdxPath;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Net {
    /// Returns an iterator over all circuits in the net.
    pub(crate) fn circuits(&self) -> impl Iterator<Item = IdxCircuit> + '_ {
        self.dense_net.graph.cycles().into_iter().map(|cycle| {
            cycle
                .into_iter()
                .map(|node_index| {
                    *self
                        .dense_net
                        .graph
                        .node_weight(node_index)
                        .expect("the cycle should only contain valid node indices")
                })
                .collect::<Vec<_>>()
                .try_conv::<IdxPath>()
                .expect("crate graph_cycles should return a path with alternating node types")
                .try_conv::<IdxCircuit>()
                .expect("crate graph_cycles should return a cycle with even length")
        })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns true if there exists a directed circuit in which every place has zero tokens.
    ///
    /// An unmarked circuit is a nontrivial SCC of the subgraph of the net induced by the unmarked
    /// places and all transitions.
    pub fn has_unmarked_circuit(&self) -> bool {
        let unmarked_subgraph = NodeFiltered::from_fn(&self.dense_net.graph, |node| {
            match self.dense_net.graph[node] {
                IdxNode::Place(p_idx) => self.marking[p_idx] == 0,
                IdxNode::Transition(_) => true,
            }
        });
        tarjan_scc(&unmarked_subgraph)
            .into_iter()
            .any(|scc| scc.len() > 1)
    }
}
