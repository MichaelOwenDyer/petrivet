use crate::core::net::{DenseNet, IdxArc, IdxNode, PlaceIdx, TransitionIdx};
use crate::system::PetriNet;
use graph_cycles::Cycles;
use std::ops::Deref;
use tap::TryConv;
use crate::net::Net;

/// A path through the directed bipartite graph of a Petri net.
///
/// This is an alternating list of [`Places`](Place) and [`Transitions`](Transition)
/// with implied arcs between them. It can start and end on either type of node.
///
/// Implementation note: a Path has no knowledge of the Petri net it belongs to,
/// so it is just a list of nodes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct IdxPath {
    /// Nodes in the path.
    nodes: Vec<IdxNode>,
}

impl IdxPath {
    /// Returns an iterator over all [`Places`](Place) in the path.
    pub fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                IdxNode::Place(p_idx) => Some(*p_idx),
                _ => None,
            })
    }

    /// Returns an iterator over all [`Transitions`](Transition) in the path.
    pub fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                IdxNode::Transition(t_idx) => Some(*t_idx),
                _ => None,
            })
    }

    /// Returns an iterator over all [`Arcs`](Arc) in the path.
    pub fn idx_arcs(&self) -> impl Iterator<Item = IdxArc> + '_ {
        self.nodes
            .array_windows::<2>()
            .map(|neighbors| match neighbors {
                [IdxNode::Place(p_idx), IdxNode::Transition(t_idx)] => {
                    IdxArc::PlaceToTransition(*p_idx, *t_idx)
                }
                [IdxNode::Transition(t_idx), IdxNode::Place(p_idx)] => {
                    IdxArc::TransitionToPlace(*t_idx, *p_idx)
                }
                _ => unreachable!("the path should maintain alternating nodes"),
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
        nodes.array_windows::<2>().try_for_each(|neighbors| match neighbors {
            [IdxNode::Place(_), IdxNode::Transition(_)] => Ok(()),
            [IdxNode::Transition(_), IdxNode::Place(_)] => Ok(()),
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
pub(crate) struct IdxCircuit {
    /// Nodes in the circuit.
    /// Guaranteed to have positive even length.
    path: IdxPath,
}

impl IdxCircuit {
    /// Returns an iterator over all [`Arcs`](Arc) implied by the circuit,
    /// including the arc between the last and the first node.
    pub fn idx_arcs(&self) -> impl Iterator<Item = IdxArc> {
        let last_arc = match (self.last(), self.first()) {
            (Some(IdxNode::Place(p_idx)), Some(IdxNode::Transition(t_idx))) => {
                IdxArc::PlaceToTransition(*p_idx, *t_idx)
            }
            (Some(IdxNode::Transition(t_idx)), Some(IdxNode::Place(p_idx))) => {
                IdxArc::TransitionToPlace(*t_idx, *p_idx)
            }
            _ => unreachable!("the circuit should maintain alternating nodes"),
        };
        std::iter::chain(
            self.path.idx_arcs(),
            std::iter::once(last_arc),
        )
    }
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
        self.graph
            .cycles()
            .into_iter()
            .map(|cycle| {
                cycle.into_iter()
                    .map(|node_index| {
                        *self.graph
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
    /// Returns an iterator over all unmarked circuits in the Petri net.
    pub(crate) fn unmarked_circuits(&self) -> impl Iterator<Item = IdxCircuit> {
        self.circuits().filter(|circuit| {
            circuit.place_indices().all(|p_idx| self.marking[p_idx] == 0)
        })
    }

    /// Returns true if there exists a directed circuit in which every place has zero tokens.
    ///
    /// In a strongly connected marked graph, such a circuit implies the net is not live
    /// (no transition on the circuit can ever fire).
    ///
    /// Efficiency: runs a DFS in `O(n + m)`.
    pub fn has_unmarked_circuit(&self) -> bool {
        let n = self.dense_net.place_count();
        let is_zero: Vec<bool> = self.dense_net
            .place_indices()
            .map(|p| self.marking[p] == 0)
            .collect();

        if is_zero.iter().all(|&z| !z) {
            return false;
        }

        let mut visited = vec![false; n];
        let mut in_stack = vec![false; n];

        for start in self.dense_net.place_indices() {
            if is_zero[start] && !visited[start]
                && dfs_zero_circuit(start, &is_zero, &self.dense_net, &mut visited, &mut in_stack)
            {
                return true;
            }
        }
        false
    }
}

/// Performs a depth-first search through the nodes of the net starting from place `p`,
/// trying to find a circuit of zero-token places. `is_zero` is a precomputed vector
/// indicating which places have zero tokens. `visited` and `in_stack` are used to
/// track the DFS state and detect cycles.
fn dfs_zero_circuit(
    p: usize,
    is_zero: &[bool],
    dense_net: &DenseNet,
    visited: &mut Vec<bool>,
    in_circuit: &mut Vec<bool>,
) -> bool {
    visited[p] = true;
    in_circuit[p] = true;
    for &t in &dense_net.postset_p[p] {
        for &next_p in &dense_net.postset_t[t] {
            if !is_zero[next_p] { continue; }
            if in_circuit[next_p] { return true; }
            if !visited[next_p]
                && dfs_zero_circuit(next_p, is_zero, dense_net, visited, in_circuit)
            {
                return true;
            }
        }
    }
    in_circuit[p] = false;
    false
}