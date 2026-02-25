//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

pub mod builder;
pub mod class;

use crate::analysis;
use std::fmt;

/// A place in the net, identified by a dense index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub(crate) idx: usize,
}

impl Place {
    /// Creates a place from a raw index.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Place { idx: index }
    }

    /// Returns the raw index of this place.
    #[must_use]
    pub fn index(self) -> usize {
        self.idx
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.idx)
    }
}

/// A transition in the net, identified by a dense index.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition {
    pub(crate) idx: usize,
}

impl Transition {
    /// Creates a transition from a raw index.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Transition { idx: index }
    }

    /// Returns the raw index of this transition.
    #[must_use]
    pub fn index(self) -> usize {
        self.idx
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.idx)
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
    preset_t: Box<[Box<[Place]>]>,
    /// Transition postsets indexed by transition index:
    /// for each transition t, the list of places in t•.
    postset_t: Box<[Box<[Place]>]>,
    /// Place presets indexed by place index:
    /// for each place p, the list of transitions in •p.
    preset_p: Box<[Box<[Transition]>]>,
    /// Place postsets indexed by place index:
    /// for each place p, the list of transitions in p•.
    postset_p: Box<[Box<[Transition]>]>,
}

impl Net {
    /// Creates a new net builder for constructing a net.
    #[must_use]
    pub fn builder() -> builder::NetBuilder {
        builder::NetBuilder::new()
    }

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

    /// Number of nodes in the net (places + transitions).
    #[must_use]
    pub fn n_nodes(&self) -> usize {
        self.n_places + self.n_transitions
    }

    /// Number of arcs in the net.
    #[must_use]
    pub fn n_arcs(&self) -> usize {
        // Either sum of all presets/postsets of transitions or places should give the same count.
        self.preset_t.iter().map(|preset| preset.len()).sum::<usize>()
            +
        self.postset_t.iter().map(|postset| postset.len()).sum::<usize>()
    }

    /// Iterator over all places.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        (0..self.n_places).map(|idx| Place { idx })
    }

    /// Iterator over all transitions.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        (0..self.n_transitions).map(|idx| Transition { idx })
    }

    /// Iterator over all nodes (places then transitions).
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        Iterator::chain(
            self.places().map(Node::Place),
            self.transitions().map(Node::Transition)
        )
    }

    /// Iterator over all arcs.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        self.transitions().flat_map(move |t| {
            let input_arcs = self.preset_t(t).iter().map(move |&p| Arc::PlaceToTransition(p, t));
            let output_arcs = self.postset_t(t).iter().map(move |&p| Arc::TransitionToPlace(t, p));
            Iterator::chain(input_arcs, output_arcs)
        })
    }

    /// Preset of a transition: input places (•t).
    /// Sorted by place index in ascending order.
    #[must_use]
    pub fn preset_t(&self, t: Transition) -> &[Place] {
        self.preset_t[t.idx].as_ref()
    }

    /// Postset of a transition: output places (t•).
    /// Sorted by place index in ascending order.
    #[must_use]
    pub fn postset_t(&self, t: Transition) -> &[Place] {
        self.postset_t[t.idx].as_ref()
    }

    /// Preset of a place: transitions that produce into this place (•p).
    /// Sorted by transition index in ascending order.
    #[must_use]
    pub fn preset_p(&self, p: Place) -> &[Transition] {
        self.preset_p[p.idx].as_ref()
    }

    /// Postset of a place: transitions that consume from this place (p•).
    /// Sorted by transition index in ascending order.
    #[must_use]
    pub fn postset_p(&self, p: Place) -> &[Transition] {
        self.postset_p[p.idx].as_ref()
    }

    /// A net is an S-net if every transition has exactly one input and one output place.
    /// Returns true for any S-net, including Circuits, and false otherwise.
    #[must_use]
    pub fn is_s_net(&self) -> bool {
        self.transitions().all(|t| self.preset_t(t).len() == 1 && self.postset_t(t).len() == 1)
    }

    /// A net is a T-net if every place has exactly one input and one output transition.
    /// Returns true for any T-net, including Circuits, and false otherwise.
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
    /// Equivalently: for every two places p1, p2:
    /// if p1• ∩ p2• ≠ ∅ then p1• = p2•.
    /// Intuitively, if two transitions share any input place, they share all input places;
    /// if any two places share an output transition, they share all output transitions.
    #[must_use]
    pub fn is_free_choice(&self) -> bool {
        self.places().all(|p| {
            let consumers = self.postset_p(p);
            if consumers.len() <= 1 {
                return true;
            }
            consumers.windows(2).all(|t| {
                self.preset_t(t[0]) == self.preset_t(t[1])
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

    /// Computes the incidence matrix N of the net.
    ///
    /// `N` is a `|P|` × `|T|` matrix where `N\[p\]\[t\]` is the net token change
    /// at place p when transition t fires: +1 if t produces to p, -1 if
    /// t consumes from p, 0 otherwise. For ordinary nets each entry is
    /// -1, 0, or +1.
    ///
    /// This convention allows the state equation to be written directly as
    /// `M' = M₀ + N · x`, where `x` is the firing count vector (no transpose needed).
    ///
    /// References:
    /// - Petri Net Primer (Best & Devillers), Definition 4.1
    /// - Murata 1989, §IV-B (uses the transposed convention A^T · x = ΔM)
    #[must_use]
    pub fn incidence_matrix(&self) -> analysis::structural::IncidenceMatrix {
        analysis::structural::IncidenceMatrix::new(self)
    }

    /// Checks if the net is strongly connected using Kosaraju's algorithm.
    #[must_use]
    pub fn is_strongly_connected(&self) -> bool {
        use petgraph::graph::NodeIndex;
        let mut graph = petgraph::Graph::<Node, ()>::with_capacity(self.n_nodes(), self.n_arcs());
        let p_indices: Vec<NodeIndex> = self.places().map(|p| graph.add_node(Node::Place(p))).collect();
        let t_indices: Vec<NodeIndex> = self.transitions().map(|t| graph.add_node(Node::Transition(t))).collect();
        self.arcs().for_each(|arc| match arc {
            Arc::PlaceToTransition(p, t) => {
                graph.add_edge(p_indices[p.idx], t_indices[t.idx], ());
            }
            Arc::TransitionToPlace(t, p) => {
                graph.add_edge(t_indices[t.idx], p_indices[p.idx], ());
            }
        });
        petgraph::algo::kosaraju_scc(&graph).len() == 1
    }

    /// Checks if the net is structurally bounded: bounded under *every*
    /// initial marking. Uses LP to check if each place is covered by a
    /// net subvariant (the structure does not allow unbounded growth at any place).
    ///
    ///
    /// This is a purely structural property (independent of the marking).
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        analysis::semi_decision::is_structurally_bounded(self)
    }

    /// Checks if a single place is structurally bounded: bounded under *every*
    /// initial marking. Uses LP to check if the target place is covered by a
    /// net subvariant (the structure does not allow unbounded growth at this place).
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: Place) -> bool {
        analysis::semi_decision::is_place_structurally_bounded(self, place)
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
        net.build().expect("valid net").into_net()
    }

    #[test]
    fn test_n_places() {
        let net = example_net();
        assert_eq!(net.n_places(), 2);
        assert_eq!(net.n_places(), net.places().count());
    }

    #[test]
    fn test_n_transitions() {
        let net = example_net();
        assert_eq!(net.n_transitions(), 2);
        assert_eq!(net.n_transitions(), net.transitions().count());
    }

    #[test]
    fn test_n_nodes() {
        let net = example_net();
        assert_eq!(net.n_nodes(), 4);
        assert_eq!(net.n_nodes(), net.nodes().count());
    }

    #[test]
    fn test_n_arcs() {
        let net = example_net();
        assert_eq!(net.n_arcs(), 4);
        assert_eq!(net.n_arcs(), net.arcs().count());
    }
}
