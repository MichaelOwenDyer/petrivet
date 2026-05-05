//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

pub mod builder;
pub mod class;
pub mod nodes;
pub mod sorted_set;
pub mod system;
pub mod marking;

pub use nodes::{Place, Transition};
pub use sorted_set::SortedSet;

use crate::class::NetClass;
use crate::{Marking, System};
use std::collections::HashMap;

use crate::net::idx::{DenseNet, PlaceIdx, TransitionIdx};
use crate::pnml::graphics::PnmlGraphics;
use crate::pnml::labels::NetLabels;
use crate::state_space::explorer::TokenOps;
use marking::IdxMarking;
use std::iter::Peekable;

pub(crate) mod idx {
    use crate::analysis::incidence::IncidenceMatrix;
    use crate::class::NetClass;
    use crate::{analysis, SortedSet};

    /// A place in a built [`Net`], identified by a dense index in `0 .. place_count`.
    ///
    /// This is a crate-internal handle used by analysis algorithms. External users
    /// interact with [`Place`] instead.
    pub type PlaceIdx = usize;

    /// A transition in a built [`Net`], identified by a dense index in `0 .. transition_count`.
    ///
    /// This is a crate-internal handle used by analysis algorithms. External users
    /// interact with [`Transition`] instead.
    pub type TransitionIdx = usize;

    /// Arc using internal indices.
    pub enum IdxArc {
        PlaceToTransition(PlaceIdx, TransitionIdx),
        TransitionToPlace(TransitionIdx, PlaceIdx),
    }

    /// The structure of a Net compressed into a packed format optimized for analysis.
    #[derive(Debug, Clone)]
    pub struct DenseNet {
        /// Structural class of the net, cached at build time for efficient queries.
        pub class: NetClass,
        /// Transition presets: for each transition t, the places in •t,
        /// sorted by their internal dense index for efficient set operations.
        pub preset_t: Box<[SortedSet<PlaceIdx>]>,
        /// Transition postsets: for each transition t, the of places in t•.
        pub postset_t: Box<[SortedSet<PlaceIdx>]>,
        /// Place presets: for each place p, the sorted set of transitions in •p.
        pub preset_p: Box<[SortedSet<TransitionIdx>]>,
        /// Place postsets: for each place p, the sorted set of transitions in p•.
        pub postset_p: Box<[SortedSet<TransitionIdx>]>,
    }

    impl DenseNet {
        /// A net is a circuit if it is both an S-net and a T-net.
        #[must_use]
        pub const fn is_circuit(&self) -> bool {
            self.class.is_circuit()
        }

        /// A net is an S-net, or state machine, if every transition has exactly one input and one output place.
        #[must_use]
        pub const fn is_state_machine(&self) -> bool {
            self.class.is_state_machine()
        }

        /// A net is a T-net, or marked graph, if every place has exactly one input and one output transition.
        #[must_use]
        pub const fn is_marked_graph(&self) -> bool {
            self.class.is_marked_graph()
        }

        /// A net is free-choice if for every two transitions t1, t2:
        /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
        #[must_use]
        pub const fn is_free_choice_net(&self) -> bool {
            self.class.is_free_choice()
        }

        /// A net is asymmetric-choice if for every two places s1, s2:
        /// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
        #[must_use]
        pub const fn is_asymmetric_choice_net(&self) -> bool {
            self.class.is_asymmetric_choice()
        }

        /// Iterator over all internal places.
        pub fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
            0..self.place_count() as usize
        }

        /// Number of places in the net.
        #[must_use]
        pub fn place_count(&self) -> u32 {
            u32::try_from(self.preset_p.len()).expect("cannot be built with more than u32::MAX places")
        }

        /// Number of transitions in the net.
        #[must_use]
        pub fn transition_count(&self) -> u32 {
            u32::try_from(self.preset_t.len()).expect("cannot be built with more than u32::MAX transitions")
        }

        /// Iterator over all internal transitions.
        pub fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
            0..self.transition_count() as usize
        }

        /// Returns an iterator over all transition indices and associated index presets and index postsets.
        pub fn transition_io(&self) -> impl Iterator<Item = (TransitionIdx, &SortedSet<PlaceIdx>, &SortedSet<PlaceIdx>)> + '_ {
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

        pub fn arcs(&self) -> impl Iterator<Item = IdxArc> + '_ {
            self.place_indices()
                .zip(self.preset_p.iter().zip(self.postset_p.iter()))
                .flat_map(|(p_idx, (preset, postset))| {
                    std::iter::chain(
                        preset.iter().map(move |&t_idx| IdxArc::TransitionToPlace(t_idx, p_idx)),
                        postset.iter().map(move |&t_idx| IdxArc::PlaceToTransition(p_idx, t_idx)),
                    )
                })
        }

        /// Computes the incidence matrix N of the net.
        #[must_use]
        pub fn incidence_matrix(&self) -> IncidenceMatrix {
            IncidenceMatrix::new(self)
        }

        /// Checks if the net is strongly connected using Kosaraju's algorithm.
        #[must_use]
        pub fn is_strongly_connected(&self) -> bool {
            use petgraph::graph::NodeIndex;
            let mut graph = petgraph::Graph::<(), ()>::with_capacity(self.node_count(), self.arc_count());
            let p_indices: Box<[NodeIndex]> = self.place_indices()
                .map(|_| graph.add_node(()))
                .collect();
            let t_indices: Box<[NodeIndex]> = self.transition_indices()
                .map(|_| graph.add_node(()))
                .collect();
            self.transition_io()
                .flat_map(|(t_idx, preset, postset)| {
                    let transition_node = t_indices[t_idx];
                    let preset = preset.iter()
                        .map(|&p_idx| p_indices[p_idx])
                        .map(move |place_node| (place_node, transition_node));
                    let postset = postset.iter()
                        .map(|&p_idx| p_indices[p_idx])
                        .map(move |place_node| (transition_node, place_node));
                    std::iter::chain(preset, postset)
                })
                .for_each(|(from, to)| {
                    graph.add_edge(from, to, ());
                });
            petgraph::algo::kosaraju_scc(&graph).len() == 1
        }

        /// Checks if the net is structurally bounded.
        /// This means that there exists no initial marking
        /// which would cause any place in the net to become unbounded.
        #[must_use]
        pub fn is_structurally_bounded(&self) -> bool {
            analysis::semi_decision::find_positive_place_subvariant(self).is_some()
        }

        /// Checks if a single place is structurally bounded.
        /// This means that there exists no initial marking
        /// which would cause this place to become unbounded.
        #[must_use]
        pub fn is_place_structurally_bounded(&self, place: &PlaceIdx) -> bool {
            analysis::semi_decision::find_semipositive_place_subvariant(
                self,
                |p| p == place
            ).is_some()
        }
    }

    impl PartialEq for DenseNet {
        fn eq(&self, other: &Self) -> bool {
            self.class == other.class
                && self.preset_t == other.preset_t
                && self.postset_t == other.postset_t
                && self.preset_p == other.preset_p
                && self.postset_p == other.postset_p
        }
    }

    impl Eq for DenseNet {}
}

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
        for (i, val) in self.iter.by_ref() {
            if let Some(&target) = self.indices.peek() {
                if i == target {
                    self.indices.next();
                    return Some(val);
                }
            } else {
                return None;
            }
        }
        None
    }
}

/// An arc in the flow relation, using public key handles.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Arc {
    PlaceToTransition(Place, Transition),
    TransitionToPlace(Transition, Place),
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
    /// Inner net structure, optimized for efficient analysis algorithms.
    pub(crate) core: DenseNet,

    /// Maps the public place handle to its internal dense index.
    pub(crate) place_to_index: HashMap<Place, PlaceIdx>,
    /// Maps the public transition handle to its internal dense index.
    pub(crate) transition_to_index: HashMap<Transition, TransitionIdx>,
    /// Maps internal dense place indices back to their public handles.
    pub(crate) index_to_place: Box<[Place]>,
    /// Maps internal dense transition indices back to their public handles.
    pub(crate) index_to_transition: Box<[Transition]>,

    /// The annotations on the net.
    /// Boxed so that it only adds a single pointer's worth of overhead to the Net struct.
    pub labels: Option<Box<NetLabels>>,

    /// The visual properties of the net.
    /// Boxed so that it only adds a single pointer's worth of overhead to the Net struct.
    pub graphics: Option<Box<PnmlGraphics>>
}

impl Net {
    /// Convert an internal index marking to a public marking.
    pub(crate) fn to_marking<T: TokenOps>(&self, marking: IdxMarking<T>) -> Marking<T> {
        self.places().zip(marking).collect()
    }

    /// Convert a public marking to an internal index marking.
    pub(crate) fn to_idx_marking<T: TokenOps>(&self, api_marking: Marking<T>) -> IdxMarking<T> {
        let mut marking = IdxMarking::zeros(self.place_count());
        api_marking.into_iter().for_each(|(place, count)| {
            if let Some(&dense) = self.place_to_index.get(&place) {
                marking[dense] = count;
            }
        });
        marking
    }
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
    pub fn with_marking(self, initial_marking: impl Into<Marking>) -> System<Self> {
        System::new(self, initial_marking)
    }

    /// Returns the structural class of this net (cached at build time).
    #[must_use]
    pub const fn class(&self) -> NetClass {
        self.core.class
    }

    /// A net is a circuit if it is both an S-net and a T-net.
    #[must_use]
    pub const fn is_circuit(&self) -> bool {
        self.core.is_circuit()
    }

    /// A net is an S-net, or state machine, if every transition has exactly one input and one output place.
    #[must_use]
    pub const fn is_state_machine(&self) -> bool {
        self.core.is_state_machine()
    }

    /// A net is a T-net, or marked graph, if every place has exactly one input and one output transition.
    #[must_use]
    pub const fn is_marked_graph(&self) -> bool {
        self.core.is_marked_graph()
    }

    /// A net is free-choice if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    #[must_use]
    pub const fn is_free_choice_net(&self) -> bool {
        self.core.is_free_choice_net()
    }

    /// A net is asymmetric-choice if for every two places s1, s2:
    /// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    #[must_use]
    pub const fn is_asymmetric_choice_net(&self) -> bool {
        self.core.is_asymmetric_choice_net()
    }

    /// Iterator over all places.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.index_to_place.iter().copied()
    }

    /// Iterator over all places.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.index_to_transition.iter().copied()
    }

    /// Number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> u32 {
        self.core.place_count()
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> u32 {
        self.core.transition_count()
    }

    /// Number of nodes in the net (places + transitions).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.core.node_count()
    }

    /// Number of arcs in the net.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        self.core.arc_count()
    }

    /// Iterator over all nodes (places then transitions) as [`Node`].
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        Iterator::chain(
            self.places().map(Node::Place),
            self.transitions().map(Node::Transition),
        )
    }

    /// Returns the transitions which deposit tokens onto the given place (•p).
    pub fn place_preset(&self, place: &Place) -> impl Iterator<Item = Transition> + '_ {
        self.place_to_index.get(place).map(|&idx| {
            self.core.preset_p[idx].iter().map(|&idx| self.index_to_transition[idx])
        })
            .unwrap() // todo: return empty for unknown place?
    }

    /// Returns the transitions which consume tokens from the given place (p•).
    pub fn place_postset(&self, place: &Place) -> impl Iterator<Item = Transition> + '_ {
        self.place_to_index.get(place).map(|&idx| {
            self.core.postset_p[idx].iter().map(|&idx| self.index_to_transition[idx])
        })
            .unwrap()
    }

    /// Returns the places from which the given transition consumes tokens (•t).
    pub fn transition_preset(&self, transition: &Transition) -> impl Iterator<Item = Place> + '_ {
        self.transition_to_index.get(transition).map(|&idx| {
            self.core.preset_t[idx].iter().map(|&idx| self.index_to_place[idx])
        })
            .unwrap()
    }

    /// Returns the places onto which the given transition produces tokens (t•).
    pub fn transition_postset(&self, transition: &Transition) -> impl Iterator<Item = Place> + '_ {
        self.transition_to_index.get(transition).map(|&idx| {
            self.core.postset_t[idx].iter().map(|&idx| self.index_to_place[idx])
        })
            .unwrap()
    }

    /// Iterator over all arcs, yielding key-based [`Arc`] values.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.core.arcs().map(|idx_arc| match idx_arc {
            idx::IdxArc::PlaceToTransition(p_idx, t_idx) => {
                let place = self.index_to_place[p_idx];
                let transition = self.index_to_transition[t_idx];
                Arc::PlaceToTransition(place, transition)
            },
            idx::IdxArc::TransitionToPlace(t_idx, p_idx) => {
                let transition = self.index_to_transition[t_idx];
                let place = self.index_to_place[p_idx];
                Arc::TransitionToPlace(transition, place)
            },
        })
    }

    /// Translate a [`Place`] to its dense [`PlaceIdx`] index.
    #[must_use]
    pub(crate) fn place_index(&self, key: Place) -> Option<&PlaceIdx> {
        self.place_to_index.get(&key)
    }

    /// Translate a [`Transition`] to its dense [`TransitionIdx`] index.
    #[must_use]
    pub(crate) fn transition_index(&self, key: Transition) -> Option<&TransitionIdx> {
        self.transition_to_index.get(&key)
    }

    /// Checks if the net is strongly connected.
    #[must_use]
    pub fn is_strongly_connected(&self) -> bool {
        self.core.is_strongly_connected()
    }

    /// Checks if the net is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause any place in the net to become unbounded.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        self.core.is_structurally_bounded()
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: &Place) -> bool {
        self.place_to_index
            .get(place)
            .is_some_and(|p_idx| self.core.is_place_structurally_bounded(p_idx))
    }
}

impl PartialEq for Net {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
            && self.place_to_index == other.place_to_index
            && self.transition_to_index == other.transition_to_index
            && self.index_to_place == other.index_to_place
            && self.index_to_transition == other.index_to_transition
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
