//! The static topology of a Petri net.

pub mod boundedness;
pub mod siphon_trap;
pub mod invariant;

use crate::core::mapping::DenseMapping;
use crate::core::net::{DenseNet, IdxArc, IdxNode};
use crate::prelude::{Marking, NetBuilder, NetClass, PetriNet};
use petgraph::Graph;
use std::num::NonZeroU32;

/// A *place* is one of the two types of nodes in a Petri net,
/// often represented visually by a circle.
///
/// Places can hold tokens, and the distribution of tokens across
/// places is called a *marking* of the net.
///
/// The set of places `S` defines the *place set* of a [`Net`],
/// which is one of the three components of the net structure.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Place(pub(crate) NonZeroU32);

/// A *transition* is one of the two types of nodes in a Petri net,
/// often represented visually by a rectangle or bar.
///
/// Transitions provide the dynamic behavior of a Petri net.
/// A transition is *enabled* in a given [`Marking`] if all of its preset places have
/// at least one token. An enabled transition can *fire*, consuming one token from each
/// of its preset places and producing one token on each of its postset places,
/// resulting in a new marking.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Transition(pub(crate) NonZeroU32);

/// A directed edge from a [`Place`] to a [`Transition`] or vice versa.
///
/// The set of arcs `F` defines the *flow relation* of a [`Net`],
/// which defines how places and transitions are connected.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Arc {
    PlaceToTransition(Place, Transition),
    TransitionToPlace(Transition, Place),
}

/// A *node* in [`Net`] is either a [`Place`] or a [`Transition`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Place(Place),
    Transition(Transition),
}

impl From<Place> for Node {
    fn from(p: Place) -> Self {
        Node::Place(p)
    }
}

impl From<Transition> for Node {
    fn from(t: Transition) -> Self {
        Node::Transition(t)
    }
}

/// The structure of a [`PetriNet`], independent of any
/// initial marking or dynamic behavior.
///
/// A `Net` is formally defined as a three-tuple `(S, T, F)`, where
/// - `S` is a finite, nonempty set of [`Places`](Place),
/// - `T` is a finite, nonempty set of [`Transitions`](Transition) disjoint from `S`,
/// - `F` is a set containing elements from (S × T) ∪ (T × S), called the *flow relation*,
///   which defines directed [`Arcs`](Arc) between places and transitions.
#[derive(Debug, Clone)]
pub struct Net {
    /// Inner net structure, optimized for efficient analysis algorithms.
    #[allow(clippy::struct_field_names)]
    pub(crate) dense_net: DenseNet,
    /// The bipartite graph structure of the net.
    pub(crate) graph: Graph<IdxNode, ()>,

    /// Monotonic counter used when converting this [`Net`] back to a [`builder::NetBuilder`]
    /// so new nodes continue to receive unused ids. Ids are never reused for removed nodes.
    pub(crate) next_place_id: NonZeroU32,
    /// Monotonic counter used when converting this [`Net`] back to a [`builder::NetBuilder`]
    /// so new nodes continue to receive unused ids. Ids are never reused for removed nodes.
    pub(crate) next_transition_id: NonZeroU32,
    /// Bidirectional mapping between public handles and dense ranks for this snapshot.
    pub(crate) mapping: std::sync::Arc<DenseMapping>,

    /// The annotations on the net.
    /// Boxed so that it only adds a single pointer's worth of overhead to the Net struct.
    #[cfg(feature = "pnml")]
    pub labels: Option<Box<crate::pnml::labels::NetLabels>>,

    /// The visual properties of the net.
    /// Boxed so that it only adds a single pointer's worth of overhead to the Net struct.
    #[cfg(feature = "pnml")]
    pub graphics: Option<Box<crate::pnml::graphics::PnmlGraphics>>,
}

#[cfg(feature = "pnml")]
impl Net {
    // todo: temporary solution, work this into NetBuilder ideally!
    pub(crate) fn set_labels(&mut self, labels: crate::pnml::labels::NetLabels) {
        self.labels = Some(Box::new(labels));
    }
    pub(crate) fn set_graphics(&mut self, graphics: crate::pnml::graphics::PnmlGraphics) {
        self.graphics = Some(Box::new(graphics));
    }
}

impl Net {
    /// Creates a new net builder for constructing a net.
    #[must_use]
    pub fn builder() -> NetBuilder {
        NetBuilder::new()
    }

    /// Creates a [`PetriNet`] by combining a reference to this net with the given marking.
    pub fn with_initial_marking(
        &self,
        initial_marking: impl Into<Marking<u32>>,
    ) -> PetriNet<&Self> {
        PetriNet::new(self, initial_marking)
    }

    /// Returns the structural class of this net.
    #[must_use]
    pub const fn class(&self) -> NetClass {
        self.dense_net.class
    }

    /// Returns true if the net is strongly connected.
    #[must_use]
    pub const fn is_strongly_connected(&self) -> bool {
        self.dense_net.is_strongly_connected
    }

    /// Iterator over all places.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.mapping.places()
    }

    /// Iterator over all transitions.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.mapping.transitions()
    }

    /// Number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> u32 {
        self.mapping.place_count()
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> u32 {
        self.mapping.transition_count()
    }

    /// Number of nodes in the net (places + transitions).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.dense_net.node_count()
    }

    /// Number of arcs in the net.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        self.dense_net.arc_count()
    }

    /// Iterator over all nodes (places then transitions) as [`Node`].
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        Iterator::chain(
            self.places().map(Node::Place),
            self.transitions().map(Node::Transition),
        )
    }

    /// Iterates over all [`Transition`]s in the *preset* `(p•)` of the provided [`Place`].
    ///
    /// The preset of a place `p` is the set of transitions `t` such that
    /// there is an arc from the transition to the place (`t → p`).
    ///
    /// If the provided place does not exist in the net, this method returns an empty iterator.
    pub fn place_preset(&self, place: &Place) -> impl Iterator<Item = Transition> + '_ {
        self.mapping.place_idx(*place).into_iter().flat_map(|idx| {
            self.dense_net.preset_p[idx]
                .iter()
                .map(|&t_idx| self.mapping.transition(t_idx))
        })
    }

    /// Iterates over all [`Transition`]s in the *postset* `(p•)` of the provided [`Place`] =.
    ///
    /// The postset of a place `p` is the set of transitions `t` such that
    /// there is an arc from the place to the transition (`p → t`).
    ///
    /// If the provided place does not exist in the net, this method returns an empty iterator.
    pub fn place_postset(&self, place: &Place) -> impl Iterator<Item = Transition> + '_ {
        self.mapping.place_idx(*place).into_iter().flat_map(|idx| {
            self.dense_net.postset_p[idx]
                .iter()
                .map(|&t_idx| self.mapping.transition(t_idx))
        })
    }

    /// Iterates over all [`Place`]s in the *preset* `(•t)` of the provided [`Transition`].
    ///
    /// The preset of a transition `t` is the set of places `p` such that
    /// there is an arc from the place to the transition (`p → t`).
    ///
    /// If the provided transition does not exist in the net, this method returns an empty iterator.
    pub fn transition_preset(&self, transition: &Transition) -> impl Iterator<Item = Place> + '_ {
        self.mapping
            .transition_idx(*transition)
            .into_iter()
            .flat_map(|idx| {
                self.dense_net.preset_t[idx]
                    .iter()
                    .map(|&p_idx| self.mapping.place(p_idx))
            })
    }

    /// Iterates over all [`Place`]s in the *postset* `(t•)` of the provided [`Transition`].
    ///
    /// The postset of a transition `t` is the set of places `p` such that
    /// there is an arc from the transition to the place (`t → p`).
    ///
    /// If the provided transition does not exist in the net, this method returns an empty iterator.
    pub fn transition_postset(&self, transition: &Transition) -> impl Iterator<Item = Place> + '_ {
        self.mapping
            .transition_idx(*transition)
            .into_iter()
            .flat_map(|idx| {
                self.dense_net.postset_t[idx]
                    .iter()
                    .map(|&p_idx| self.mapping.place(p_idx))
            })
    }

    /// Iterates over all [`Arc`]s in the net in unspecified order.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.dense_net.arc_indices().map(|idx_arc| match idx_arc {
            IdxArc::PlaceToTransition(p_idx, t_idx) => {
                let place = self.mapping.place(p_idx);
                let transition = self.mapping.transition(t_idx);
                Arc::PlaceToTransition(place, transition)
            }
            IdxArc::TransitionToPlace(t_idx, p_idx) => {
                let transition = self.mapping.transition(t_idx);
                let place = self.mapping.place(p_idx);
                Arc::TransitionToPlace(transition, place)
            }
        })
    }

    /// Returns whether every place in this net belongs to an S-component.
    #[must_use]
    pub fn is_covered_by_s_components(&self) -> bool {
        // todo
        false
    }

    /// Returns true if the given marking enables no transitions in this net
    #[must_use]
    pub fn is_deadlock(&self, marking: Marking<u32>) -> bool {
        let idx_marking = self.mapping.decode(marking);
        self.dense_net.is_deadlock(&idx_marking)
    }
}

impl AsRef<Net> for Net {
    fn as_ref(&self) -> &Net {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::net::Net;

    fn example_net() -> Net {
        let mut net = Net::builder();
        let [p0, p1] = net.add_places();
        let [t0, t1] = net.add_transitions();
        net.add_arcs((p0, t0, p1, t1, p0));
        net.build().expect("valid net")
    }

    #[test]
    fn test_n_places() {
        let net = example_net();
        assert_eq!(net.place_count(), 2);
        assert_eq!(net.place_count(), net.places().count() as u32);
    }

    #[test]
    fn test_n_transitions() {
        let net = example_net();
        assert_eq!(net.transition_count(), 2);
        assert_eq!(net.transition_count(), net.transitions().count() as u32);
    }

    #[test]
    fn test_n_nodes() {
        let net = example_net();
        assert_eq!(net.node_count(), 4);
        assert_eq!(net.node_count(), net.nodes().count());
    }

    #[test]
    fn test_n_arcs() {
        let net = example_net();
        assert_eq!(net.arc_count(), 4);
        assert_eq!(net.arc_count(), net.arcs().count());
    }
}
