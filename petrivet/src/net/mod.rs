//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)
//!
//! [`Net`] is the public façade; the packed graph and crate-internal dense ranks live
//! in the private `net::idx` module (`DenseNet`, `PlaceIdx`, `TransitionIdx`).

pub mod builder;
pub mod class;
pub mod nodes;
pub mod sorted_set;
pub mod system;
pub mod marking;

pub use nodes::{Place, Transition};
pub use sorted_set::UniqueSortedSlice;

use crate::class::NetClass;
use crate::{Marking, PetriNet};
use std::collections::HashMap;

use crate::net::idx::{DenseNet, PlaceIdx, TransitionIdx};
use crate::pnml::graphics::PnmlGraphics;
use crate::pnml::labels::NetLabels;
use crate::state_space::explorer::TokenOps;
use marking::IdxMarking;
use std::iter::Peekable;
use std::num::NonZeroU32;

pub(crate) mod idx {
    use crate::analysis::incidence::IncidenceMatrix;
    use crate::class::NetClass;
    use crate::{analysis, UniqueSortedSlice};
    use crate::marking::IdxMarking;

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
        /// Transition presets: for each transition t, the set of places in `•t`.
        pub preset_t: Box<[UniqueSortedSlice<PlaceIdx>]>,
        /// Transition postsets: for each transition t, the set of places in `t•`.
        pub postset_t: Box<[UniqueSortedSlice<PlaceIdx>]>,
        /// Place presets: for each place p, the set of transitions in `•p`.
        pub preset_p: Box<[UniqueSortedSlice<TransitionIdx>]>,
        /// Place postsets: for each place p, the set of transitions in `p•`.
        pub postset_p: Box<[UniqueSortedSlice<TransitionIdx>]>,
    }

    impl DenseNet {
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
        pub fn transition_io(&self) -> impl Iterator<Item = (TransitionIdx, &UniqueSortedSlice<PlaceIdx>, &UniqueSortedSlice<PlaceIdx>)> + '_ {
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

        /// Returns true if the provided transition is enabled at the given marking,
        /// i.e. if all places in its preset have at least one token in the marking.
        pub(crate) fn is_enabled_in(&self, t: TransitionIdx, marking: &IdxMarking) -> bool {
            self.preset_t[t].iter().all(|&p| marking[p] >= 1)
        }

        /// Returns true if the given marking enables no transitions in the net.
        pub(crate) fn is_deadlock(&self, marking: &IdxMarking) -> bool {
            self.transition_indices().all(|t| !self.is_enabled_in(t, marking))
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
/// Dense indices (`PlaceIdx` / `TransitionIdx` in `net::idx`) are `pub(crate)` for
/// internal analysis code.
#[derive(Debug, Clone)]
pub struct Net {
    /// Inner net structure, optimized for efficient analysis algorithms.
    pub(crate) core: DenseNet,

    /// Monotonic counter used when converting this [`Net`] back to a [`builder::NetBuilder`]
    /// so new nodes continue to receive unused ids. Ids are never reused for removed nodes.
    pub(crate) next_place_id: NonZeroU32,
    /// Same role as [`Self::next_place_id`] for transitions.
    pub(crate) next_transition_id: NonZeroU32,
    /// Maps the public place handle to its internal dense index.
    pub(crate) place_indices: HashMap<Place, PlaceIdx>,
    /// Maps the public transition handle to its internal dense index.
    pub(crate) transition_indices: HashMap<Transition, TransitionIdx>,
    /// Maps internal dense place indices back to their public handles.
    pub(crate) ordered_places: Box<[Place]>,
    /// Maps internal dense transition indices back to their public handles.
    pub(crate) ordered_transitions: Box<[Transition]>,

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
            if let Some(&dense) = self.place_indices.get(&place) {
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
    pub fn with_initial_marking(self, initial_marking: impl Into<Marking>) -> PetriNet<Self> {
        PetriNet::new(self, initial_marking)
    }

    /// Returns the structural class of this net (cached at build time).
    #[must_use]
    pub const fn class(&self) -> NetClass {
        self.core.class
    }

    /// A net is a circuit if it is both an S-net and a T-net.
    #[must_use]
    pub const fn is_circuit(&self) -> bool {
        self.core.class.is_circuit()
    }

    /// A net is an S-net, or state machine, if every transition has exactly one input and one output place.
    #[must_use]
    pub const fn is_state_machine(&self) -> bool {
        self.core.class.is_state_machine()
    }

    /// A net is a T-net, or marked graph, if every place has exactly one input and one output transition.
    #[must_use]
    pub const fn is_marked_graph(&self) -> bool {
        self.core.class.is_marked_graph()
    }

    /// A net is free-choice if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    #[must_use]
    pub const fn is_free_choice_net(&self) -> bool {
        self.core.class.is_free_choice()
    }

    /// A net is asymmetric-choice if for every two places s1, s2:
    /// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    #[must_use]
    pub const fn is_asymmetric_choice_net(&self) -> bool {
        self.core.class.is_asymmetric_choice()
    }

    /// Iterator over all places.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.ordered_places.iter().copied()
    }

    /// Iterator over all places.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.ordered_transitions.iter().copied()
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
        self.place_indices.get(place).map(|&idx| {
            self.core.preset_p[idx].iter().map(|&idx| self.ordered_transitions[idx])
        })
            .unwrap() // todo: return empty for unknown place?
    }

    /// Returns the transitions which consume tokens from the given place (p•).
    pub fn place_postset(&self, place: &Place) -> impl Iterator<Item = Transition> + '_ {
        self.place_indices.get(place).map(|&idx| {
            self.core.postset_p[idx].iter().map(|&idx| self.ordered_transitions[idx])
        })
            .unwrap()
    }

    /// Returns the places from which the given transition consumes tokens (•t).
    pub fn transition_preset(&self, transition: &Transition) -> impl Iterator<Item = Place> + '_ {
        self.transition_indices.get(transition).map(|&idx| {
            self.core.preset_t[idx].iter().map(|&idx| self.ordered_places[idx])
        })
            .unwrap()
    }

    /// Returns the places onto which the given transition produces tokens (t•).
    pub fn transition_postset(&self, transition: &Transition) -> impl Iterator<Item = Place> + '_ {
        self.transition_indices.get(transition).map(|&idx| {
            self.core.postset_t[idx].iter().map(|&idx| self.ordered_places[idx])
        })
            .unwrap()
    }

    /// Iterator over all arcs, yielding key-based [`Arc`] values.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.core.arcs().map(|idx_arc| match idx_arc {
            idx::IdxArc::PlaceToTransition(p_idx, t_idx) => {
                let place = self.ordered_places[p_idx];
                let transition = self.ordered_transitions[t_idx];
                Arc::PlaceToTransition(place, transition)
            },
            idx::IdxArc::TransitionToPlace(t_idx, p_idx) => {
                let transition = self.ordered_transitions[t_idx];
                let place = self.ordered_places[p_idx];
                Arc::TransitionToPlace(transition, place)
            },
        })
    }

    /// Translate a [`Place`] to its dense [`PlaceIdx`] index.
    #[must_use]
    pub(crate) fn place_index(&self, key: Place) -> Option<&PlaceIdx> {
        self.place_indices.get(&key)
    }

    /// Translate a [`Transition`] to its dense [`TransitionIdx`] index.
    #[must_use]
    pub(crate) fn transition_index(&self, key: Transition) -> Option<&TransitionIdx> {
        self.transition_indices.get(&key)
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
        self.place_indices
            .get(place)
            .is_some_and(|p_idx| self.core.is_place_structurally_bounded(p_idx))
    }
}

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
