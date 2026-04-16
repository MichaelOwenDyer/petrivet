//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

pub mod builder;
pub mod class;
pub mod keys;
pub mod sorted_set;
pub mod metadata;

pub use keys::{Place, Transition};
pub use sorted_set::SortedSet;

use crate::{analysis, ApiMarking, System};
use crate::class::NetClass;
use std::collections::HashMap;

use metadata::NetLabels;
use crate::pnml::convert::PnmlGraphics;
use std::iter::Peekable;
use std::ops::Index;
use crate::marking::IdxMarking;
use crate::state_space::explorer::TokenOps;

pub trait IteratorExt: Iterator + Sized {
    fn nths<I>(self, indices: I) -> Nths<Self, I::IntoIter>
    where
        I: IntoIterator<Item = usize>,
    {
        Nths {
            iter: self.enumerate(),
            indices: indices.into_iter().peekable(),
        }
    }
}

// Implement this for all types that implement Iterator
impl<I: Iterator> IteratorExt for I {}

pub struct Nths<I: Iterator, J: Iterator<Item = usize>> {
    iter: std::iter::Enumerate<I>,
    indices: Peekable<J>,
}

impl<I, J> Iterator for Nths<I, J>
where
    I: Iterator,
    J: Iterator<Item = usize>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((i, val)) = self.iter.next() {
            if let Some(&target) = self.indices.peek() {
                if i == target {
                    self.indices.next();
                    return Some(val);
                }
            } else {
                // Optimization: if no more indices, stop iterating
                return None;
            }
        }
        None
    }
}

/// A place in a built [`Net`], identified by a dense index in `0 .. place_count`.
///
/// This is a crate-internal handle used by analysis algorithms. External users
/// interact with [`Place`] instead.
pub(crate) type PlaceIdx = usize;

/// A transition in a built [`Net`], identified by a dense index in `0 .. transition_count`.
///
/// This is a crate-internal handle used by analysis algorithms. External users
/// interact with [`Transition`] instead.
pub(crate) type TransitionIdx = usize;

/// An arc in the flow relation, using public key handles.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Arc {
    PlaceToTransition(Place, Transition),
    TransitionToPlace(Transition, Place),
}

impl From<(Place, Transition)> for Arc {
    fn from((p, t): (Place, Transition)) -> Self {
        Arc::PlaceToTransition(p, t)
    }
}

impl From<(Transition, Place)> for Arc {
    fn from((t, p): (Transition, Place)) -> Self {
        Arc::TransitionToPlace(t, p)
    }
}

/// A node in the net: either a place or a transition, using public key handles.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
/// Dense indices ([`PlaceIdx`] / [`TransitionIdx`]) are `pub(crate)` for
/// internal analysis code.
#[derive(Debug, Clone)]
pub struct Net {
    /// Structural class of the net, cached at build time for efficient queries.
    class: NetClass,

    /// Maps the public place handle to its internal dense index.
    place_to_index: HashMap<Place, PlaceIdx>,
    /// Maps the public transition handle to its internal dense index.
    transition_to_index: HashMap<Transition, TransitionIdx>,
    /// Maps internal dense place indices back to their public handles.
    index_to_place: Box<[Place]>,
    /// Maps internal dense transition indices back to their public handles.
    index_to_transition: Box<[Transition]>,

    /// Transition presets: for each transition t, the sorted set of places in •t.
    preset_t: Box<[SortedSet<PlaceIdx>]>,
    /// Transition postsets: for each transition t, the sorted set of places in t•.
    postset_t: Box<[SortedSet<PlaceIdx>]>,
    /// Place presets: for each place p, the sorted set of transitions in •p.
    preset_p: Box<[SortedSet<TransitionIdx>]>,
    /// Place postsets: for each place p, the sorted set of transitions in p•.
    postset_p: Box<[SortedSet<TransitionIdx>]>,

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
    pub fn with_marking(self, marking: impl IntoIterator<Item = (Place, u32)>) -> System<Self> {
        System::new(self, marking)
    }

    /// Returns the structural class of this net (cached at build time).
    #[must_use]
    pub fn class(&self) -> NetClass {
        self.class
    }

    /// A net is a circuit if it is both an S-net and a T-net.
    #[must_use]
    pub fn is_circuit(&self) -> bool {
        self.class.is_circuit()
    }

    /// A net is an S-net if every transition has exactly one input and one output place.
    #[must_use]
    pub fn is_s_net(&self) -> bool {
        self.class.is_s_net()
    }

    /// A net is a T-net if every place has exactly one input and one output transition.
    #[must_use]
    pub fn is_t_net(&self) -> bool {
        self.class.is_t_net()
    }

    /// A net is free-choice if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    #[must_use]
    pub fn is_free_choice_net(&self) -> bool {
        self.class.is_free_choice()
    }

    /// A net is asymmetric-choice if for every two places s1, s2:
    /// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    #[must_use]
    pub fn is_asymmetric_choice_net(&self) -> bool {
        self.class.is_asymmetric_choice()
    }

    /// Number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> u32 {
        self.index_to_place.len() as u32
    }

    /// Iterator over all internal places.
    pub(crate) fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
        0..self.place_count() as usize
    }

    /// Iterator over all places in dense index order.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.index_to_place.iter().copied()
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> u32 {
        self.preset_t.len() as u32
    }

    /// Iterator over all internal transitions.
    pub(crate) fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
        0..self.transition_count() as usize
    }

    /// Iterator over all places in dense index order.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.index_to_transition.iter().copied()
    }

    pub(crate) fn transition_io(&self) -> impl Iterator<Item = (TransitionIdx, &SortedSet<PlaceIdx>, &SortedSet<PlaceIdx>)> + '_ {
        self.transition_indices()
            .zip(self.preset_t.iter().zip(self.postset_t.iter()))
            .map(|(t, (preset, postset))| (t, preset, postset))
    }

    /// Number of nodes in the net (places + transitions).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.preset_p.len() + self.preset_t.len()
    }

    /// Number of arcs in the net.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        std::iter::zip(&self.preset_p, &self.postset_p)
            .map(|(pre, post)| pre.len() + post.len())
            .sum()
    }

    /// Iterator over all nodes (places then transitions) as [`Node`].
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        Iterator::chain(
            self.places().map(Node::Place),
            self.transitions().map(Node::Transition),
        )
    }

    /// Iterator over all arcs, yielding key-based [`Arc`] values.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.transitions()
            .zip(self.preset_t.iter().zip(self.postset_t.iter()))
            .flat_map(move |(tk, (preset, postset))| {
                let input_arcs = self.places()
                    .nths(preset.iter().copied())
                    .map(move |pk| Arc::PlaceToTransition(pk, tk));
                let output_arcs = self.places()
                    .nths(postset.iter().copied())
                    .map(move |pk| Arc::TransitionToPlace(tk, pk));
                Iterator::chain(input_arcs, output_arcs)
            })
    }

    // TODO: Reconsider return type of input/output methods?
    /// Preset of a transition: places that this transition consumes tokens from (•t).
    #[must_use]
    pub fn input_places(&self, t: Transition) -> impl Iterator<Item = Place> + '_ {
        let dt = *self.transition_to_index.get(&t).expect("transition key");
        self.preset_t[dt]
            .iter()
            .map(|&p| self.index_to_place[p])
    }

    /// Postset of a transition: places that this transition produces tokens into (t•).
    #[must_use]
    pub fn output_places(&self, t: Transition) -> impl Iterator<Item = Place> + '_ {
        let dt = *self.transition_to_index.get(&t).expect("transition key");
        self.postset_t[dt]
            .iter()
            .map(|&p| self.index_to_place[p])
    }

    /// Preset of a place: transitions that produce tokens into this place (•p).
    #[must_use]
    pub fn input_transitions(&self, p: Place) -> impl Iterator<Item = Transition> + '_ {
        let dp = *self.place_to_index.get(&p).expect("place key");
        self.preset_p[dp]
            .iter()
            .map(|&t| self.index_to_transition[t])
    }

    /// Postset of a place: transitions that consume tokens from this place (p•).
    #[must_use]
    pub fn output_transitions(&self, p: Place) -> impl Iterator<Item = Transition> + '_ {
        let dp = *self.place_to_index.get(&p).expect("place key");
        self.postset_p[dp]
            .iter()
            .map(|&t| self.index_to_transition[t])
    }

    /// Dense preset of a transition (•t), for analysis code.
    #[must_use]
    pub(crate) fn preset_t(&self, t: TransitionIdx) -> &SortedSet<PlaceIdx> {
        &self.preset_t[t]
    }

    /// Dense postset of a transition (t•), for analysis code.
    #[must_use]
    pub(crate) fn postset_t(&self, t: TransitionIdx) -> &SortedSet<PlaceIdx> {
        &self.postset_t[t]
    }

    /// Dense preset of a place (•p), for analysis code.
    #[must_use]
    pub(crate) fn preset_p(&self, p: PlaceIdx) -> &SortedSet<TransitionIdx> {
        &self.preset_p[p]
    }

    /// Dense postset of a place (p•), for analysis code.
    #[must_use]
    pub(crate) fn postset_p(&self, p: PlaceIdx) -> &SortedSet<TransitionIdx> {
        &self.postset_p[p]
    }

    /// Translate a [`Place`] to its dense [`PlaceIdx`] index.
    #[must_use]
    pub(crate) fn place_index(&self, key: Place) -> Option<PlaceIdx> {
        self.place_to_index.get(&key).copied()
    }

    /// Translate a [`Transition`] to its dense [`TransitionIdx`] index.
    #[must_use]
    pub(crate) fn transition_index(&self, key: Transition) -> Option<TransitionIdx> {
        self.transition_to_index.get(&key).copied()
    }

    /// Translate a dense [`PlaceIdx`] back to its [`Place`].
    #[must_use]
    pub(crate) fn get_place(&self, p: PlaceIdx) -> Place {
        self.index_to_place[p]
    }

    /// Translate a dense [`TransitionIdx`] back to its [`Transition`].
    #[must_use]
    pub(crate) fn get_transition(&self, t: TransitionIdx) -> Transition {
        self.index_to_transition[t]
    }

    pub(crate) fn convert_marking<T: TokenOps>(&self, marking: IdxMarking<T>) -> ApiMarking<T> {
        self.places().zip(marking).collect()
    }

    pub(crate) fn convert_api_marking<T: TokenOps>(&self, api_marking: ApiMarking<T>) -> IdxMarking<T> {
        let mut marking = IdxMarking::zeros(self.place_count());
        api_marking.into_iter().for_each(|(place, count)| {
            if let Some(dense) = self.place_index(place) {
                marking[dense] = count;
            }
        });
        marking
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
        let mut graph = petgraph::Graph::<(), ()>::with_capacity(self.node_count(), self.arc_count());
        let p_indices: Box<[NodeIndex]> = self.places()
            .map(|_| graph.add_node(()))
            .collect();
        let t_indices: Box<[NodeIndex]> = self.transitions()
            .map(|_| graph.add_node(()))
            .collect();
        for ((t, _tk), (preset, postset)) in self.transitions()
            .enumerate()
            .zip(self.preset_t.iter().zip(self.postset_t.iter())) {
            for &p in preset {
                graph.add_edge(p_indices[p], t_indices[t], ());
            }
            for &p in postset {
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
    pub fn is_place_structurally_bounded(&self, pk: Place) -> bool {
        self.place_index(pk).map_or(false, |place| {
            analysis::semi_decision::find_place_subvariant_covering(self, place).is_some()
        })
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
