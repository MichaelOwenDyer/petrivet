//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

pub mod builder;
pub mod class;

use std::fmt;

/// A place in the net, identified by a dense index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub(crate) usize);

impl Place {
    /// Creates a place from a raw index.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Place(index)
    }

    /// Returns the raw index of this place.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.0)
    }
}

/// A transition in the net, identified by a dense index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition(pub(crate) usize);

impl Transition {
    /// Creates a transition from a raw index.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Transition(index)
    }

    /// Returns the raw index of this transition.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// An arc in the flow relation.
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

/// A node in the net: either a place or a transition.
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

/// An ordinary Petri net N = (S, T, F).
/// All arc weights are implicitly 1. No place capacities.
///
/// Given x ∈ S ∪ T, the set •x = {y | (y, x) ∈ F} is called the preset of x,
/// and the set x• = {y | (x, y) ∈ F} is called the postset of x.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    /// Number of places in the net.
    n_places: usize,
    /// Number of transitions in the net.
    n_transitions: usize,
    /// Transition presets indexed by transition index:
    /// for each transition t, the list of places in •t.
    preset: Box<[Box<[Place]>]>,
    /// Transition postsets indexed by transition index:
    /// for each transition t, the list of places in t•.
    postset: Box<[Box<[Place]>]>,
    /// Place presets indexed by place index:
    /// for each place p, the list of transitions in •p.
    preset_p: Box<[Box<[Transition]>]>,
    /// Place postsets indexed by place index:
    /// for each place p, the list of transitions in p•.
    postset_p: Box<[Box<[Transition]>]>,
}

impl Net {
    /// Number of places in the net.
    #[must_use]
    pub fn n_places(&self) -> usize {
        self.n_places
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn n_transitions(&self) -> usize {
        self.n_transitions
    }

    /// Iterator over all places.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        (0..self.n_places).map(Place)
    }

    /// Iterator over all transitions.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        (0..self.n_transitions).map(Transition)
    }

    /// Iterator over all nodes (places then transitions).
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        self.places().map(Node::Place).chain(self.transitions().map(Node::Transition))
    }

    /// Iterator over all arcs.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.transitions().flat_map(move |t| {
            let pt = self.preset_t(t).iter().map(move |&p| Arc::PlaceToTransition(p, t));
            let tp = self.postset_t(t).iter().map(move |&p| Arc::TransitionToPlace(t, p));
            pt.chain(tp)
        })
    }

    /// Preset of a transition: input places (•t).
    /// Sorted by place index in ascending order.
    pub fn preset_t(&self, t: Transition) -> &[Place] {
        self.preset[t.0].as_ref()
    }

    /// Postset of a transition: output places (t•).
    /// Sorted by place index in ascending order.
    pub fn postset_t(&self, t: Transition) -> &[Place] {
        self.postset[t.0].as_ref()
    }

    /// Preset of a place: transitions that produce into this place (•p).
    /// Sorted by transition index in ascending order.
    pub fn preset_p(&self, p: Place) -> &[Transition] {
        self.preset_p[p.0].as_ref()
    }

    /// Postset of a place: transitions that consume from this place (p•).
    /// Sorted by transition index in ascending order.
    pub fn postset_p(&self, p: Place) -> &[Transition] {
        self.postset_p[p.0].as_ref()
    }

    /// A net is an S-net if every transition has exactly one input and one output place.
    #[must_use]
    pub fn is_s_net(&self) -> bool {
        self.transitions().all(|t| self.preset_t(t).len() == 1 && self.postset_t(t).len() == 1)
    }

    /// A net is a T-net if every place has exactly one input and one output transition.
    #[must_use]
    pub fn is_t_net(&self) -> bool {
        self.places().all(|p| self.preset_p(p).len() == 1 && self.postset_p(p).len() == 1)
    }

    /// A net is a circuit if it is both an S-net and a T-net.
    #[must_use]
    pub fn is_circuit(&self) -> bool {
        self.is_s_net() && self.is_t_net()
    }

    /// A net is free-choice if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    #[must_use]
    pub fn is_free_choice(&self) -> bool {
        self.transitions().all(|t1| {
            self.transitions().all(|t2| {
                if t1 == t2 {
                    return true;
                }
                let presets_overlap = self.preset_t(t1).iter().any(|p1| {
                    self.preset_t(t2).iter().any(|p2| p1 == p2)
                });
                if presets_overlap {
                    self.preset_t(t1) == self.preset_t(t2)
                } else {
                    true
                }
            })
        })
    }

    /// Returns the structural class of this net.
    #[must_use]
    pub fn classify(&self) -> NetClass {
        match (self.is_s_net(), self.is_t_net()) {
            (true, true) => NetClass::Circuit,
            (true, false) => NetClass::SNet,
            (false, true) => NetClass::TNet,
            (false, false) if self.is_free_choice() => NetClass::FreeChoice,
            _ => NetClass::Unrestricted,
        }
    }

    /// Checks if the net is strongly connected using Kosaraju's algorithm.
    #[must_use]
    pub fn is_strongly_connected(&self) -> bool {
        use petgraph::graph::NodeIndex;
        let mut graph = petgraph::Graph::<Node, ()>::with_capacity(
            self.n_places + self.n_transitions,
            self.arcs().count(),
        );
        let place_indices: Vec<NodeIndex> = self.places().map(|p| graph.add_node(Node::Place(p))).collect();
        let trans_indices: Vec<NodeIndex> = self.transitions().map(|t| graph.add_node(Node::Transition(t))).collect();
        for t in self.transitions() {
            for p in self.preset_t(t) {
                graph.add_edge(place_indices[p.0], trans_indices[t.0], ());
            }
            for p in self.postset_t(t) {
                graph.add_edge(trans_indices[t.0], place_indices[p.0], ());
            }
        }
        petgraph::algo::kosaraju_scc(&graph).len() == 1
    }
}

/// Structural classification of a Petri net.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum NetClass {
    /// The most restrictive class of Petri net:
    /// a single directed cycle of alternating places and transitions.
    /// Circuits model purely sequential processes with no choices or concurrency.
    Circuit,
    /// Every transition has exactly one input and one output place.
    /// S-nets model sequential processes and choices, but not concurrency.
    SNet,
    /// Every place has exactly one input and one output transition.
    /// T-nets model concurrency and synchronization, but not choices.
    TNet,
    /// If two transitions share any input place, they share all input places.
    /// Free-choice nets model concurrency and choices, but eliminate complex
    /// conflicts where two transitions share some but not all input places.
    FreeChoice,
    /// No structural restrictions.
    /// Can model arbitrary concurrency, choices, and conflicts.
    Unrestricted,
}

impl AsRef<Net> for Net {
    fn as_ref(&self) -> &Net {
        self
    }
}

impl fmt::Display for NetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetClass::Circuit => write!(f, "Circuit"),
            NetClass::SNet => write!(f, "S-net"),
            NetClass::TNet => write!(f, "T-net"),
            NetClass::FreeChoice => write!(f, "Free-choice"),
            NetClass::Unrestricted => write!(f, "Unrestricted"),
        }
    }
}
