//! Net structure: the static topology of a Petri net.
//!
//! A net N = (S, T, F) consists of:
//! - A finite set of places S
//! - A finite set of transitions T
//! - A flow relation F ⊆ (S × T) ∪ (T × S)

pub mod builder;
pub mod class;

use std::fmt;
use crate::analysis;

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
        self.preset[t.idx].as_ref()
    }

    /// Postset of a transition: output places (t•).
    /// Sorted by place index in ascending order.
    pub fn postset_t(&self, t: Transition) -> &[Place] {
        self.postset[t.idx].as_ref()
    }

    /// Preset of a place: transitions that produce into this place (•p).
    /// Sorted by transition index in ascending order.
    pub fn preset_p(&self, p: Place) -> &[Transition] {
        self.preset_p[p.idx].as_ref()
    }

    /// Postset of a place: transitions that consume from this place (p•).
    /// Sorted by transition index in ascending order.
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
    /// N is a |T| × |P| matrix where N\[t\]\[p\] = (number of arcs from t to p)
    /// minus (number of arcs from p to t). For ordinary nets each entry is
    /// -1, 0, or +1.
    ///
    /// The matrix is stored in row-major order (indexed by transition, then place).
    #[must_use]
    pub fn incidence_matrix(&self) -> IncidenceMatrix {
        IncidenceMatrix::new(self)
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
                graph.add_edge(place_indices[p.idx], trans_indices[t.idx], ());
            }
            for p in self.postset_t(t) {
                graph.add_edge(trans_indices[t.idx], place_indices[p.idx], ());
            }
        }
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

/// The incidence matrix N of a Petri net.
///
/// A |T| × |P| matrix stored in row-major order. Entry N\[t\]\[p\] is the net
/// token change at place p when transition t fires: +1 if t produces to p,
/// -1 if t consumes from p, 0 otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidenceMatrix {
    data: Vec<i32>,
    rows: usize,
    cols: usize,
}

impl IncidenceMatrix {
    /// Constructs the incidence matrix for a given net.
    #[must_use]
    pub fn new(net: &Net) -> Self {
        let rows = net.n_transitions;
        let cols = net.n_places;
        let mut data = vec![0; rows * cols];
        for t in net.transitions() {
            for &p in net.preset_t(t) {
                data[t.idx * cols + p.idx] -= 1;
            }
            for &p in net.postset_t(t) {
                data[t.idx * cols + p.idx] += 1;
            }
        }
        IncidenceMatrix { data, rows, cols }
    }

    /// Constructs an incidence matrix from raw data in row-major order.
    #[must_use]
    pub fn from_raw(data: Vec<i32>, rows: usize, cols: usize) -> Self {
        debug_assert_eq!(data.len(), rows * cols);
        Self { data, rows, cols }
    }

    /// Number of rows (transitions).
    #[must_use]
    pub fn n_rows(&self) -> usize {
        self.rows
    }

    /// Number of columns (places).
    #[must_use]
    pub fn n_cols(&self) -> usize {
        self.cols
    }

    /// Entry at (row, col) = N[transition][place].
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> i32 {
        self.data[row * self.cols + col]
    }

    /// Row slice for a given transition.
    #[must_use]
    pub fn row(&self, t: usize) -> &[i32] {
        let start = t * self.cols;
        &self.data[start..start + self.cols]
    }

    /// Returns a column vector (extracting one place across all transitions).
    #[must_use]
    pub fn col(&self, p: usize) -> Vec<i32> {
        (0..self.rows).map(|t| self.data[t * self.cols + p]).collect()
    }

    /// Returns the transpose (|P| × |T| matrix).
    #[must_use]
    pub fn transpose(&self) -> Self {
        let mut data = vec![0; self.rows * self.cols];
        for t in 0..self.rows {
            for p in 0..self.cols {
                data[p * self.rows + t] = self.data[t * self.cols + p];
            }
        }
        Self { data, rows: self.cols, cols: self.rows }
    }
}

impl fmt::Display for IncidenceMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in 0..self.rows {
            write!(f, "[")?;
            for (p, val) in self.row(t).iter().enumerate() {
                if p > 0 { write!(f, ", ")?; }
                write!(f, "{val:>3}")?;
            }
            writeln!(f, "]")?;
        }
        Ok(())
    }
}
