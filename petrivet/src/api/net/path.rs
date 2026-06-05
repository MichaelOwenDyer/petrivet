use crate::net::{Arc, Net, Node, Place, Transition};
use crate::system::PetriNet;
use graph_cycles::Cycles;
use std::ops::Deref;
use tap::TryConv;
use crate::core::net::DenseNet;

/// A path through the directed bipartite graph of a Petri net.
///
/// This is an alternating list of [`Places`](Place) and [`Transitions`](Transition)
/// with implied arcs between them. It can start and end on either type of node.
///
/// Implementation note: a Path has no knowledge of the Petri net it belongs to,
/// so it is just a list of nodes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Path {
    /// Nodes in the path.
    member_nodes: Vec<Node>,
}

impl Path {
    /// Creates a new empty path.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a new node to the end of the path,
    /// ensuring the alternating sequence of places and transitions is maintained.
    ///
    /// If the last node is a place, the new node must be a transition, and vice versa.
    pub fn push(&mut self, node: Node) -> Result<(), ()> {
        if let Some(last_node) = self.member_nodes.last() {
            match (last_node, node) {
                (Node::Place(_), Node::Transition(_)) | (Node::Transition(_), Node::Place(_)) => {
                    self.member_nodes.push(node);
                    Ok(())
                }
                _ => Err(()),
            }
        } else {
            // If the path is empty, we can start with any node.
            self.member_nodes.push(node);
            Ok(())
        }
    }

    /// Pops the last node from the path, returning it if the path is not empty.
    pub fn pop(&mut self) -> Option<Node> {
        self.member_nodes.pop()
    }

    /// Returns an iterator over all [`Places`](Place) in the path.
    pub fn places(&self) -> impl Iterator<Item = Place> {
        self.member_nodes
            .iter()
            .filter_map(|node| match node {
                Node::Place(place) => Some(*place),
                _ => None,
            })
    }

    /// Returns an iterator over all [`Transitions`](Transition) in the path.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> {
        self.member_nodes
            .iter()
            .filter_map(|node| match node {
                Node::Transition(transition) => Some(*transition),
                _ => None,
            })
    }

    /// Returns an iterator over all [`Arcs`](Arc) implied by the path.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> {
        self.member_nodes
            .array_windows::<2>()
            .map(|neighbors| match neighbors {
                [Node::Place(p), Node::Transition(t)] => {
                    Arc::PlaceToTransition(*p, *t)
                }
                [Node::Transition(t), Node::Place(p)] => {
                    Arc::TransitionToPlace(*t, *p)
                }
                _ => unreachable!("the path should maintain alternating nodes"),
            })
    }
}

impl Deref for Path {
    type Target = [Node];

    fn deref(&self) -> &Self::Target {
        &self.member_nodes
    }
}

impl TryFrom<Vec<Node>> for Path {
    type Error = ();

    fn try_from(nodes: Vec<Node>) -> Result<Self, Self::Error> {
        nodes.array_windows::<2>().try_for_each(|neighbors| match neighbors {
            [Node::Place(_), Node::Transition(_)] | [Node::Transition(_), Node::Place(_)] => Ok(()),
            _ => Err(()),
        })?;
        Ok(Self { member_nodes: nodes })
    }
}

/// A [`Path`] of nodes whose end leads into its beginning, forming a cycle.
///
/// The path is guaranteed to be nonempty with even length.
/// The arc between the last node and the first node must maintain the
/// alternating sequence of places and transitions.
///
/// Note: it is not currently guaranteed that the cycle is simple
/// (that it does not contain repeated nodes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Circuit {
    /// Nodes in the circuit.
    /// Guaranteed to have positive even length.
    path: Path,
}

impl Circuit {
    /// Returns an iterator over all [`Arcs`](Arc) implied by the circuit,
    /// including the arc between the last and the first node.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> {
        let last_arc = match (self.last(), self.first()) {
            (Some(Node::Place(p)), Some(Node::Transition(t))) => {
                Arc::PlaceToTransition(*p, *t)
            },
            (Some(Node::Transition(t)), Some(Node::Place(p))) => {
                Arc::TransitionToPlace(*t, *p)
            },
            _ => unreachable!("the circuit should maintain alternating nodes"),
        };
        std::iter::chain(
            self.path.arcs(),
            std::iter::once(last_arc),
        )
    }
}

impl TryFrom<Path> for Circuit {
    type Error = ();

    fn try_from(path: Path) -> Result<Self, Self::Error> {
        if path.is_empty() {
            return Err(()); // A circuit cannot be empty
        }
        if !path.len().is_multiple_of(2) {
            return Err(()); // A circuit must have an even number of nodes to maintain alternation.
        }
        Ok(Self { path })
    }
}

impl Deref for Circuit {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Net {
    /// Returns an iterator over all circuits in the net.
    pub fn circuits(&self) -> impl Iterator<Item = Circuit> {
        self.graph()
            .cycles()
            .into_iter()
            .map(|cycle| {
                cycle.into_iter()
                    .map(|node_index| {
                        *self.graph()
                            .node_weight(node_index)
                            .expect("the cycle should only contain valid node indices")
                    })
                    .collect::<Vec<_>>()
                    .try_conv::<Path>()
                    .expect("crate graph_cycles should return a path with alternating node types")
                    .try_conv::<Circuit>()
                    .expect("crate graph_cycles should return a path which ")
            })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns true if there exists a directed circuit in which every place has zero tokens.
    ///
    /// In a strongly connected marked graph, such a circuit implies the net is not live
    /// (no transition on the circuit can ever fire).
    ///
    /// Efficiency: runs a DFS in `O(n + m)`.
    pub fn has_unmarked_circuit(&self) -> bool {
        fn dfs_zero_circuit(
            p: usize,
            is_zero: &[bool],
            dense_net: &DenseNet,
            visited: &mut Vec<bool>,
            in_stack: &mut Vec<bool>,
        ) -> bool {
            visited[p] = true;
            in_stack[p] = true;
            for &t in &dense_net.postset_p[p] {
                for &next_p in &dense_net.postset_t[t] {
                    if !is_zero[next_p] { continue; }
                    if in_stack[next_p] { return true; }
                    if !visited[next_p]
                        && dfs_zero_circuit(next_p, is_zero, dense_net, visited, in_stack)
                    {
                        return true;
                    }
                }
            }
            in_stack[p] = false;
            false
        }

        let n = self.dense_net.place_count();
        let is_zero: Vec<bool> = (0..n).map(|p| self.marking[p] == 0).collect();

        if is_zero.iter().all(|&z| !z) {
            return false;
        }

        let mut visited = vec![false; n];
        let mut in_stack = vec![false; n];

        for start in 0..n {
            if is_zero[start] && !visited[start]
                && dfs_zero_circuit(start, &is_zero, &self.dense_net, &mut visited, &mut in_stack)
            {
                return true;
            }
        }
        false
    }
}