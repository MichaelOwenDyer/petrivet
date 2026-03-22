//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

pub mod builder;
pub mod class;
pub mod keys;
pub mod node_map;
pub mod sorted_set;

pub use keys::{PlaceKey, TransitionKey};
pub(crate) use node_map::{PlaceMap, TransitionMap};
pub use sorted_set::SortedSet;

use crate::analysis;
use crate::class::NetClass;
use std::collections::HashMap;
use std::fmt;
use crate::node_map::IndexMap;

/// A place in a built [`Net`], identified by a dense index in `0 .. place_count`.
///
/// This is a crate-internal handle used by analysis algorithms. External users
/// interact with [`PlaceKey`] instead.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Place {
    pub(crate) idx: u32,
}

impl Place {
    #[must_use]
    pub(crate) fn from_index(index: u32) -> Self {
        Place { idx: index }
    }

    #[must_use]
    pub(crate) fn index(self) -> u32 {
        self.idx
    }

    #[inline]
    #[must_use]
    pub(crate) fn usize_index(self) -> usize {
        self.idx as usize
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.idx)
    }
}

/// A transition in a built [`Net`], identified by a dense index in `0 .. transition_count`.
///
/// This is a crate-internal handle used by analysis algorithms. External users
/// interact with [`TransitionKey`] instead.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Transition {
    pub(crate) idx: u32,
}

impl Transition {
    #[must_use]
    pub(crate) fn from_index(index: u32) -> Self {
        Transition { idx: index }
    }

    #[must_use]
    pub(crate) fn index(self) -> u32 {
        self.idx
    }

    #[inline]
    #[must_use]
    pub(crate) fn usize_index(self) -> usize {
        self.idx as usize
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.idx)
    }
}

/// An arc in the flow relation, using public key handles.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Arc {
    PlaceToTransition(PlaceKey, TransitionKey),
    TransitionToPlace(TransitionKey, PlaceKey),
}

impl From<(PlaceKey, TransitionKey)> for Arc {
    fn from((p, t): (PlaceKey, TransitionKey)) -> Self {
        Arc::PlaceToTransition(p, t)
    }
}

impl From<(TransitionKey, PlaceKey)> for Arc {
    fn from((t, p): (TransitionKey, PlaceKey)) -> Self {
        Arc::TransitionToPlace(t, p)
    }
}

/// A node in the net: either a place or a transition, using public key handles.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Node {
    Place(PlaceKey),
    Transition(TransitionKey),
}

impl From<PlaceKey> for Node {
    fn from(p: PlaceKey) -> Self {
        Node::Place(p)
    }
}

impl From<TransitionKey> for Node {
    fn from(t: TransitionKey) -> Self {
        Node::Transition(t)
    }
}

/// Dense-index flavours of Arc and Node, for internal use by graph algorithms.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DenseNode {
    Place(Place),
    Transition(Transition),
}

impl From<Place> for DenseNode {
    fn from(p: Place) -> Self {
        DenseNode::Place(p)
    }
}

impl From<Transition> for DenseNode {
    fn from(t: Transition) -> Self {
        DenseNode::Transition(t)
    }
}

/// An ordinary Petri net N = (S, T, F), where
/// - S is a finite, nonempty set of places,
/// - T is a finite, nonempty set of transitions,
/// - F ⊆ (S × T) ∪ (T × S) is the flow relation.
///
/// The public API uses [`PlaceKey`] and [`TransitionKey`] exclusively.
/// Dense indices ([`Place`] / [`Transition`]) are `pub(crate)` for
/// internal analysis code.
#[derive(Debug, Clone)]
pub struct Net {
    /// Structural class of the net, cached at build time for efficient queries.
    class: NetClass,

    /// Transition presets: for each transition t, the sorted set of places in •t.
    preset_t: IndexMap<Transition, SortedSet<Place>>,
    /// Transition postsets: for each transition t, the sorted set of places in t•.
    postset_t: IndexMap<Transition, SortedSet<Place>>,
    /// Place presets: for each place p, the sorted set of transitions in •p.
    preset_p: IndexMap<Place, SortedSet<Transition>>,
    /// Place postsets: for each place p, the sorted set of transitions in p•.
    postset_p: IndexMap<Place, SortedSet<Transition>>,

    /// Given a public handle to a place, return the internal dense index of that place.
    ///
    /// Stored as a [`HashMap`] so key lookups do not depend on slot-map–style key equality across
    /// separate arenas.
    place_index_for_key: HashMap<PlaceKey, Place>,
    /// Given a public handle to a transition, return the internal dense index of that transition.
    transition_index_for_key: HashMap<TransitionKey, Transition>,
    /// Public place handles indexed by internal dense indices.
    place_key_for_index: IndexMap<Place, PlaceKey>,
    /// Public transition handles indexed by internal dense indices.
    transition_key_for_index: IndexMap<Transition, TransitionKey>,
}

impl Net {
    /// Creates a new net builder for constructing a net.
    #[must_use]
    pub fn builder() -> builder::NetBuilder {
        builder::NetBuilder::new()
    }

    /// Number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> u32 {
        self.preset_p.len() as u32
    }

    /// Iterator over all internal places.
    pub(crate) fn places(&self) -> impl Iterator<Item = Place> + '_ {
        (0..self.place_count()).map(Place::from_index)
    }

    /// Iterator over all places in dense index order.
    pub fn place_keys(&self) -> impl Iterator<Item = PlaceKey> + '_ {
        self.places().map(|p| self.place_key_for_index[p])
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> u32 {
        self.preset_t.len() as u32
    }

    /// Iterator over all internal transitions.
    pub(crate) fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        (0..self.transition_count()).map(Transition::from_index)
    }

    /// Iterator over all places in dense index order.
    pub fn transition_keys(&self) -> impl Iterator<Item = TransitionKey> + '_ {
        self.transitions().map(|t| self.transition_key_for_index[t])
    }

    /// Number of nodes in the net (places + transitions).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.preset_p.len() + self.preset_t.len()
    }

    /// Number of arcs in the net.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        std::iter::zip(self.preset_p.values(), self.postset_p.values())
            .map(|(pre, post)| pre.len() + post.len())
            .sum()
    }

    /// Iterator over all nodes (places then transitions) as [`Node`].
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        Iterator::chain(
            self.place_keys().map(Node::Place),
            self.transition_keys().map(Node::Transition),
        )
    }

    /// Iterator over all arcs, yielding key-based [`Arc`] values.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.transition_keys().flat_map(move |tk| {
            let input_arcs = self.input_places(tk).map(move |pk| Arc::PlaceToTransition(pk, tk));
            let output_arcs = self.output_places(tk).map(move |pk| Arc::TransitionToPlace(tk, pk));
            Iterator::chain(input_arcs, output_arcs)
        })
    }

    // TODO: Reconsider return type of input/output methods?
    /// Preset of a transition: places that this transition consumes tokens from (•t).
    #[must_use]
    pub fn input_places(&self, t: TransitionKey) -> impl Iterator<Item = PlaceKey> + '_ {
        let dt = *self.transition_index_for_key.get(&t).expect("transition key");
        self.preset_t[dt]
            .iter()
            .map(|&p| self.place_key_for_index[p])
    }

    /// Postset of a transition: places that this transition produces tokens into (t•).
    #[must_use]
    pub fn output_places(&self, t: TransitionKey) -> impl Iterator<Item = PlaceKey> + '_ {
        let dt = *self.transition_index_for_key.get(&t).expect("transition key");
        self.postset_t[dt]
            .iter()
            .map(|&p| self.place_key_for_index[p])
    }

    /// Preset of a place: transitions that produce tokens into this place (•p).
    #[must_use]
    pub fn input_transitions(&self, p: PlaceKey) -> impl Iterator<Item = TransitionKey> + '_ {
        let dp = *self.place_index_for_key.get(&p).expect("place key");
        self.preset_p[dp]
            .iter()
            .map(|&t| self.transition_key_for_index[t])
    }

    /// Postset of a place: transitions that consume tokens from this place (p•).
    #[must_use]
    pub fn output_transitions(&self, p: PlaceKey) -> impl Iterator<Item = TransitionKey> + '_ {
        let dp = *self.place_index_for_key.get(&p).expect("place key");
        self.postset_p[dp]
            .iter()
            .map(|&t| self.transition_key_for_index[t])
    }

    /// Dense preset of a transition (•t), for analysis code.
    #[must_use]
    pub(crate) fn dense_input_places(&self, t: Transition) -> &SortedSet<Place> {
        &self.preset_t[t]
    }

    /// Dense postset of a transition (t•), for analysis code.
    #[must_use]
    pub(crate) fn dense_output_places(&self, t: Transition) -> &SortedSet<Place> {
        &self.postset_t[t]
    }

    /// Dense preset of a place (•p), for analysis code.
    #[must_use]
    pub(crate) fn dense_input_transitions(&self, p: Place) -> &SortedSet<Transition> {
        &self.preset_p[p]
    }

    /// Dense postset of a place (p•), for analysis code.
    #[must_use]
    pub(crate) fn dense_output_transitions(&self, p: Place) -> &SortedSet<Transition> {
        &self.postset_p[p]
    }

    /// Translate a [`PlaceKey`] to its dense [`Place`] index.
    #[must_use]
    pub(crate) fn dense_place(&self, key: PlaceKey) -> Place {
        *self.place_index_for_key.get(&key).expect("place key")
    }

    /// Translate a [`TransitionKey`] to its dense [`Transition`] index.
    #[must_use]
    pub(crate) fn dense_transition(&self, key: TransitionKey) -> Transition {
        *self.transition_index_for_key.get(&key).expect("transition key")
    }

    /// Translate a dense [`Place`] back to its [`PlaceKey`].
    #[must_use]
    pub(crate) fn place_key(&self, p: Place) -> PlaceKey {
        self.place_key_for_index[p]
    }

    /// Translate a dense [`Transition`] back to its [`TransitionKey`].
    #[must_use]
    pub(crate) fn transition_key(&self, t: Transition) -> TransitionKey {
        self.transition_key_for_index[t]
    }

    /// A net is a circuit if it is both an S-net and a T-net.
    #[must_use]
    pub fn is_circuit(&self) -> bool {
        use NetClass::Circuit;
        matches!(self.class, Circuit)
    }

    /// A net is an S-net if every transition has exactly one input and one output place.
    #[must_use]
    pub fn is_s_net(&self) -> bool {
        use NetClass::*;
        matches!(self.class, Circuit | SNet)
    }

    /// A net is a T-net if every place has exactly one input and one output transition.
    #[must_use]
    pub fn is_t_net(&self) -> bool {
        use NetClass::*;
        matches!(self.class, Circuit | TNet)
    }

    /// A net is free-choice if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    #[must_use]
    pub fn is_free_choice_net(&self) -> bool {
        use NetClass::*;
        matches!(self.class, Circuit | SNet | TNet | FreeChoice)
    }

    /// A net is asymmetric-choice if for every two places s1, s2:
    /// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    #[must_use]
    pub fn is_asymmetric_choice_net(&self) -> bool {
        use NetClass::*;
        matches!(self.class, Circuit | SNet | TNet | FreeChoice | AsymmetricChoice)
    }

    /// Returns the structural class of this net (cached at build time).
    #[must_use]
    pub fn class(&self) -> NetClass {
        self.class
    }

    /// Computes the incidence matrix N of the net.
    #[must_use]
    pub fn incidence_matrix(&self) -> analysis::structural::IncidenceMatrix {
        analysis::structural::IncidenceMatrix::new(self)
    }

    /// Checks if the net is strongly connected using Kosaraju's algorithm.
    #[must_use]
    pub fn is_strongly_connected(&self) -> bool {
        use petgraph::graph::NodeIndex;
        let mut graph = petgraph::Graph::<DenseNode, ()>::with_capacity(self.node_count(), self.arc_count());
        let p_indices: IndexMap<Place, NodeIndex> = self.places().map(|p| graph.add_node(DenseNode::Place(p))).collect();
        let t_indices: IndexMap<Transition, NodeIndex> = self.transitions().map(|t| graph.add_node(DenseNode::Transition(t))).collect();
        for t in self.transitions() {
            for &p in self.dense_input_places(t) {
                graph.add_edge(p_indices[p], t_indices[t], ());
            }
            for &p in self.dense_output_places(t) {
                graph.add_edge(t_indices[t], p_indices[p], ());
            }
        }
        petgraph::algo::kosaraju_scc(&graph).len() == 1
    }

    /// Checks if the net is structurally bounded.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        analysis::semi_decision::find_positive_place_subvariant(self).is_some()
    }

    /// Checks if a single place is structurally bounded.
    #[must_use]
    pub fn is_place_structurally_bounded(&self, pk: PlaceKey) -> bool {
        let place = self.dense_place(pk);
        analysis::semi_decision::find_place_subvariant_covering(self, place).is_some()
    }
}

impl PartialEq for Net {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class
            && self.preset_t == other.preset_t
            && self.postset_t == other.postset_t
            && self.preset_p == other.preset_p
            && self.postset_p == other.postset_p
    }
}

impl Eq for Net {}

impl AsRef<Net> for Net {
    fn as_ref(&self) -> &Net {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_net() -> Net {
        let mut net = Net::builder();
        let [p0, p1] = net.add_places();
        let [t0, t1] = net.add_transitions();
        net.add_arc((p0, t0));
        net.add_arc((t0, p1));
        net.add_arc((p1, t1));
        net.add_arc((t1, p0));
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
