//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

use crate::api::builder;
use crate::api::class::NetClass;
use crate::api::mapping::DenseMapping;
use crate::api::marking::Marking;
use crate::api::pnml::graphics::PnmlGraphics;
use crate::api::pnml::labels::NetLabels;
use crate::api::PetriNet;
use crate::core::net::{DenseNet, IdxArc};
use std::num::NonZeroU32;

/// A place in a net, often represented visually by a circle.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Place(pub(crate) NonZeroU32);

/// A transition in a net, often represented visually by a square / rectangle.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Transition(pub(crate) NonZeroU32);

/// An arc (edge) in a net, connecting a place to a transition or vice versa.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Arc {
    PlaceToTransition(Place, Transition),
    TransitionToPlace(Transition, Place),
}

/// A node in a net, which can be either a place or a transition.
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

/// An ordinary Petri net N = (S, T, F), where
/// - S is a finite, nonempty set of places,
/// - T is a finite, nonempty set of transitions,
/// - F ⊆ (S × T) ∪ (T × S) is the flow relation.
///
/// The public API uses [`Place`] and [`Transition`] exclusively.
/// Dense indices (`PlaceIdx` / `TransitionIdx` in `net::idx`) are `pub(crate)` for
/// internal analysis code.
#[derive(Debug, Clone)]
pub struct Net {
    /// Inner net structure, optimized for efficient analysis algorithms.
    #[allow(clippy::struct_field_names)]
    pub(crate) dense_net: DenseNet,

    /// Monotonic counter used when converting this [`Net`] back to a [`builder::NetBuilder`]
    /// so new nodes continue to receive unused ids. Ids are never reused for removed nodes.
    pub(crate) next_place_id: NonZeroU32,
    /// Monotonic counter used when converting this [`Net`] back to a [`builder::NetBuilder`]
    /// so new nodes continue to receive unused ids. Ids are never reused for removed nodes.
    pub(crate) next_transition_id: NonZeroU32,
    /// Bidirectional mapping between public handles and dense ranks for this snapshot.
    pub(crate) mapping: DenseMapping,

    /// The annotations on the net.
    /// Boxed so that it only adds a single pointer's worth of overhead to the Net struct.
    pub labels: Option<Box<NetLabels>>,

    /// The visual properties of the net.
    /// Boxed so that it only adds a single pointer's worth of overhead to the Net struct.
    pub graphics: Option<Box<PnmlGraphics>>
}

impl Net {
    // todo: temporary solution, work this into NetBuilder ideally!
    pub(crate) fn set_labels(&mut self, labels: NetLabels) {
        self.labels = Some(Box::new(labels));
    }
    pub(crate) fn set_graphics(&mut self, graphics: PnmlGraphics) {
        self.graphics = Some(Box::new(graphics));
    }
}

impl Net {
    /// Creates a new net builder for constructing a net.
    #[must_use]
    pub fn builder() -> builder::NetBuilder {
        builder::NetBuilder::new()
    }

    /// Creates a system by combining this net with the given marking.
    pub fn with_initial_marking(self, initial_marking: impl Into<Marking<u32>>) -> PetriNet<Self> {
        PetriNet::new(self, initial_marking)
    }

    /// Returns the structural class of this net (cached at build time).
    #[must_use]
    pub const fn class(&self) -> NetClass {
        self.dense_net.class
    }

    /// A net is a circuit if it is both an S-net and a T-net.
    #[must_use]
    pub const fn is_circuit(&self) -> bool {
        self.dense_net.class.is_circuit()
    }

    /// A net is an S-net, or state machine, if every transition has exactly one input and one output place.
    #[must_use]
    pub const fn is_state_machine(&self) -> bool {
        self.dense_net.class.is_state_machine()
    }

    /// A net is a T-net, or marked graph, if every place has exactly one input and one output transition.
    #[must_use]
    pub const fn is_marked_graph(&self) -> bool {
        self.dense_net.class.is_marked_graph()
    }

    /// A net is free-choice if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    #[must_use]
    pub const fn is_free_choice(&self) -> bool {
        self.dense_net.class.is_free_choice()
    }

    /// A net is asymmetric-choice if for every two places s1, s2:
    /// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    #[must_use]
    pub const fn is_asymmetric_choice(&self) -> bool {
        self.dense_net.class.is_asymmetric_choice()
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
        self.dense_net.place_count()
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> u32 {
        self.dense_net.transition_count()
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
        self.mapping
            .place_idx(*place)
            .into_iter()
            .flat_map(|idx| {
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
        self.mapping
            .place_idx(*place)
            .into_iter()
            .flat_map(|idx| {
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
        self.dense_net.arcs().map(|idx_arc| match idx_arc {
            IdxArc::PlaceToTransition(p_idx, t_idx) => {
                let place = self.mapping.place(p_idx);
                let transition = self.mapping.transition(t_idx);
                Arc::PlaceToTransition(place, transition)
            },
            IdxArc::TransitionToPlace(t_idx, p_idx) => {
                let transition = self.mapping.transition(t_idx);
                let place = self.mapping.place(p_idx);
                Arc::TransitionToPlace(transition, place)
            },
        })
    }

    /// Checks if the net is strongly connected.
    #[must_use]
    pub fn is_strongly_connected(&self) -> bool {
        self.dense_net.is_strongly_connected()
    }

    /// Checks if the net is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause any place in the net to become unbounded.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        self.dense_net.is_structurally_bounded()
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: &Place) -> bool {
        self.mapping
            .place_idx(*place)
            .is_some_and(|p_idx| self.dense_net.is_place_structurally_bounded(&p_idx))
    }
}

impl AsRef<Net> for Net {
    fn as_ref(&self) -> &Net {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::api::Net;

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