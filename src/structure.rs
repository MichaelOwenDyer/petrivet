use nalgebra::{DMatrix, Dyn, MatrixView, U1};
use num_traits::Zero;
use petgraph::Graph;
use std::fmt;
use std::hash::Hash;
use petgraph::graph::NodeIndex;

pub type Index = u8;

/// A place is a location in the net where tokens can be stored.
/// It is identified by a unique identifier, which is typically a numeric ID.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub index: Index,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.index)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition {
    pub index: Index,
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.index)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Node {
    Place(Place),
    Transition(Transition),
}

impl From<Place> for Node {
    fn from(place: Place) -> Node {
        Node::Place(place)
    }
}

impl From<Transition> for Node {
    fn from(transition: Transition) -> Node {
        Node::Transition(transition)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arc {
    PlaceTransition(Place, Transition),
    TransitionPlace(Transition, Place),
}

impl From<(Place, Transition)> for Arc {
    fn from((place, transition): (Place, Transition)) -> Self {
        Arc::PlaceTransition(place, transition)
    }
}

impl From<(Transition, Place)> for Arc {
    fn from((transition, place): (Transition, Place)) -> Self {
        Arc::TransitionPlace(transition, place)
    }
}

// pub trait NetTrait {
//     fn places(&self) -> impl Iterator<Item = Place>;
//     fn transitions(&self) -> impl Iterator<Item = Transition>;
//     fn arcs(&self) -> impl Iterator<Item = Arc>;
//     fn preset_t(&self, transition: Transition) -> Vec<Place>;
//     fn postset_t(&self, transition: Transition) -> Vec<Place>;
//     fn preset_p(&self, place: Place) -> Vec<Transition>;
//     fn postset_p(&self, place: Place) -> Vec<Transition>;
// }

/// The incidence matrix of a net describes the net effect of firing each transition on the marking of each place.
/// It is a |S| x |T| matrix N where:
/// - N(s, t) = |t•(s)| - |•t(s)|
/// The rows correspond to places and the columns to transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidenceMatrix(pub(crate) DMatrix<i8>);

/// An S-invariant is a vector I: S → Q such that I * N = 0.
pub struct SInvariant(Box<[usize]>);

pub struct TInvariant(Box<[usize]>);

impl IncidenceMatrix {
    // fn s_invariants(&self) {
        // self.0.cast::<f64>().svd_unordered(true, true).solve()
    // }
}

impl IncidenceMatrix {
    #[must_use]
    pub fn column(&self, transition: Transition) -> MatrixView<'_, i8, Dyn, U1, U1, Dyn> {
        self.0.column(transition.index.into())
    }
    #[must_use]
    pub fn row(&self, transition: Transition) -> MatrixView<'_, i8, U1, Dyn, U1, Dyn> {
        self.0.row(transition.index.into())
    }
}

/// A net N = (S, T, F) consists of
/// a finite set of [places](Place) S (circles),
/// a finite set of [transitions](Transition) T (rectangles), and
/// a flow relation (arrows) F ⊆ (S × T) ∪ (T × S).
/// The places and transitions of a net are called nodes.
/// The elements of F are called [arcs](Arc).
///
/// Given x ∈ S ∪ T, the set •x = {y | (y, x) ∈ F} is called the preset of x,
/// and the set x• = {y | (x, y) ∈ F} is called the postset of x.
/// For X ⊆ S ∪ T, we define •X = ∪<sub>x∈X</sub> •x and X• = ∪<sub>x∈X</sub> x•.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    /// Denotes the number of places, densely indexed from 0 to num_places-1
    n_places: Index,

    /// Denotes the number of transitions, densely indexed from 0 to num_transitions-1
    n_transitions: Index,

    /// The input places of each transition, indexed by transition ID.
    preset: Box<[Box<[Place]>]>,

    /// The output places of each transition, indexed by transition ID.
    postset: Box<[Box<[Place]>]>,

    preset_p: Box<[Box<[Transition]>]>,

    postset_p: Box<[Box<[Transition]>]>,

    /// The incidence matrix of the net.
    pub(crate) incidence_matrix: IncidenceMatrix,
}

impl Net {
    #[must_use]
    pub fn n_places(&self) -> Index {
        self.n_places
    }
    
    /// Returns an iterator over all places in the net.
    pub fn places(&self) -> impl Iterator<Item=Place> {
        (Index::zero()..self.n_places).map(|index| Place { index })
    }

    #[must_use]
    pub fn n_transitions(&self) -> Index {
        self.n_transitions
    }

    /// Returns an iterator over all transitions in the net.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> {
        (Index::zero()..self.n_transitions).map(|index| Transition { index })
    }

    /// Returns an iterator over all arcs in the net.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> {
        self.transitions()
            .map(|t| (t, self.preset_t(t), self.postset_t(t)))
            .flat_map(|(t, preset, postset)| {
                Iterator::chain(
                    preset.map(move |&p| Arc::PlaceTransition(p, t)),
                    postset.map(move |&p| Arc::TransitionPlace(t, p)),
                )
            })
    }

    pub fn preset_t(&self, transition: Transition) -> impl Iterator<Item = &Place> {
        self.preset[usize::from(transition.index)].iter()
    }

    pub fn postset_t(&self, transition: Transition) -> impl Iterator<Item = &Place> {
        self.postset[usize::from(transition.index)].iter()
    }

    pub fn preset_p(&self, place: Place) -> impl Iterator<Item = &Transition> {
        self.preset_p[usize::from(place.index)].iter()
    }

    pub fn postset_p(&self, place: Place) -> impl Iterator<Item = &Transition> {
        self.preset_p[usize::from(place.index)].iter()
    }

    #[must_use]
    pub fn is_circuit(&self) -> bool {
        self.is_s_net() && self.is_t_net()
    }

    #[must_use]
    pub fn is_s_net(&self) -> bool {
        self.transitions().all(|t| self.preset_t(t).count() == 1 && self.postset_t(t).count() == 1)
    }

    #[must_use]
    pub fn is_t_net(&self) -> bool {
        self.places().all(|p| self.preset_p(p).count() == 1 && self.postset_p(p).count() == 1)
    }

    #[must_use]
    pub fn is_free_choice(&self) -> bool {
        self.transitions().all(|t1| {
            self.transitions().all(|t2| {
                if t1 == t2 {
                    true
                } else if self.preset_t(t1).any(|p1| self.preset_t(t2).any(|p2| p1 == p2)) {
                    // t1 and t2 share an input place
                    // check if they have all the same input places
                    self.preset_t(t1).all(|p1| self.preset_t(t2).any(|p2| p1 == p2))
                } else {
                    true
                }
            })
        })
    }

    #[must_use]
    pub fn n_nodes(&self) -> Index {
        self.n_transitions + self.n_places
    }

    pub fn nodes(&self) -> impl Iterator<Item=Node> {
        Iterator::chain(
            self.places().map(Node::Place),
            self.transitions().map(Node::Transition),
        )
    }

    #[must_use]
    pub fn to_graph(&self) -> Graph<Node, ()> {
        let mut graph = Graph::with_capacity(self.nodes().count(), self.arcs().count());
        let map: ahash::HashMap<Node, NodeIndex> = self.nodes()
            .map(|node| (node, graph.add_node(node)))
            .collect();
        self.transitions().for_each(|t| {
            let transition_idx = map[&Node::Transition(t)];
            self.preset_t(t)
                .map(|&p| map[&Node::Place(p)])
                .for_each(|input_place_idx| {
                    graph.add_edge(input_place_idx, transition_idx, ());
                });
            self.postset_t(t)
                .map(|&p| map[&Node::Place(p)])
                .for_each(|output_place_idx| {
                    graph.add_edge(transition_idx, output_place_idx, ());
                });
        });
        graph
    }

    #[must_use]
    pub fn is_strongly_connected(&self) -> bool {
        let graph = self.to_graph();
        let sccs = petgraph::algo::kosaraju_scc(&graph);
        sccs.len() == 1
    }
}

/// A net N = (S, T, F) is a `Free-Choice Net` if •t x s• ⊆ F
/// for every s ∈ S and t ∈ T such that (s, t) ∈ F.
///
/// Alternative definitions:
/// - A net is free-choice if for every two transitions t1, t2 ∈ T,
///   if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
///   In other words, if two transitions share any input place,
///   they must share all input places.
///
/// - A net is free-choice if for every two places s1, s2 ∈ S,
///   if s1• ∩ s2• ≠ ∅ then s1• = s2•.
///   In other words, if two places share any output transition,
///   they must share all output transitions.
///
/// Free-choice nets can model both choice and concurrency,
/// but with a key restriction: contested resources (places with multiple output transitions)
/// must be consumed entirely by any transition they enable.
///
/// Commoner's Liveness Theorem:
/// A free-choice net (N, M<sub>0</sub>) is live iff every siphon of N
/// contains a trap marked at M<sub>0</sub>.
///
/// Boundedness Theorem:
/// (Heck's Boundedness Theorem)
/// Let (N, M<sub>0</sub>) be a live free-choice system.
/// Then (N, M<sub>0</sub>) is bounded iff every place of N belongs to an S-component.
/// An S-component is a subnet N' = (S', T', F') of a net N such that:
/// - N' is a strongly connected S-net.
/// - T' = •S' ∪ S'• (all transitions connected to places in S' are included in T').
///
/// Let (N, M<sub>0</sub>) be a live and bounded free-choice system and let s be a place of N.
/// We have max{M(s) | M is reachable} = min{M<sub>0</sub>(S') | S' is an S-component containing s}.
/// Intuitively, a place can only have as many tokens as the minimum number of tokens
/// in any S-component it belongs to.
/// If all places belong to some S-component, then the entire net is bounded.
///
/// Simultaneous Liveness and Boundedness Theorem:
/// A free-choice system (N, M<sub>0</sub>) is live and bounded iff
/// 1. N has a positive S-invariant
/// 2. N has a positive T-invariant
/// 3. The rank of the incidence matrix of N is equal to c - 1, where c is the number of clusters of N.
/// 4. Every proper siphon of N is marked at M<sub>0</sub>.
///
/// Reachability theorem:
/// Let (N, M<sub>0</sub>) be a live and bounded free-choice system.
/// A marking M is reachable from M<sub>0</sub> iff there exists X ∈ N^|T| such that:
/// - M = M<sub>0</sub> + N * X, where N is the incidence matrix of N
/// - (N<sub>U</sub>, M<sub>U</sub>) has no unmarked traps,
///   where U = {t ∈ T | X(t) = 0}, N<sub>U</sub> is the subnet induced by U,
///   and M<sub>U</sub> is the projection of M onto the places of N<sub>U</sub>.
///
/// This problem is decidable in polynomial time (!).
/// Given: a live, bounded, and cyclic free-choice system (N,M0) and a marking M
/// Decide: is M reachable?
///
/// A live and bounded free-choice system (N, M<sub>0</sub>) is cyclic iff
/// M<sub>0</sub> marks every proper trap of N.
///
/// Shortest sequence theorem:
/// Let (N, M<sub>0</sub>) be a b-bounded free-choice system and let M be a reachable marking.
/// Then there is a firing sequence M<sub>0</sub> <sup>σ</sup>→ M
/// such that `|σ| ≤ bn(n+1)(n+2)/6`, where n = |T| is the number of transitions of N.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeChoiceNet(Net);

/// A net N = (S, T, F) is a `T-net` if |•s| = |s•| = 1 for every place s ∈ S.
/// This means each place has exactly one input and one output transition.
/// T-nets can model concurrency and synchronization, but not choices.
/// (N, M<sub>0</sub>) is a `T-system` if N is a T-net.
/// This structural restriction implies several important properties:
///
/// - Fundamental property:
///
///   Notation: Let γ be a circuit of a net N and let M be a marking of N. We denote
///   by M(γ) the number if tokens of γ under M, that is, M(γ) = Σ<sub>s∈γ</sub> M(s).
///
///   Let γ be a circuit of a T-system (N, M<sub>0</sub>) and let M be a reachable marking.
///   Then M(γ) = M<sub>0</sub>(γ).
///   Intuitively, the number of tokens in each circuit is constant.
///
/// - Liveness theorem:
///   A T-system (N, M<sub>0</sub>) is live iff M<sub>0</sub>(γ) > 0 for every circuit γ of N.
///   Intuitively, a T-system is live iff every circuit contains at least one token.
///
/// - Boundedness theorem:
///   A live T-system (N, M<sub>0</sub>) is bounded iff N is strongly connected.
///   A place s of a live T-system (N,M<sub>0</sub>) is bounded iff it belongs to some circuit γ,
///   and b-bounded iff M<sub>0</sub>(γ) ≤ b.
///   More specifically, max{M(s) | M is reachable} = min{M<sub>0</sub>(γ) | γ contains s}.
///   Intuitively, a place can only have as many tokens as the minimum number of tokens in any
///   circuit it belongs to. If all places belong to some circuit, then the entire net is strongly
///   connected and thus bounded.
///
/// - Reachability theorem:
///   Let (N,M<sub>0</sub>) be a live T-system.
///   A marking M is reachable from M0 iff M<sub>0</sub> ∼ M.
///   For ordinary nets, reachability implies M<sub>0</sub> ∼ M,
///   but the converse is not true in general.
///   This is a very powerful result, as it allows to decide reachability by solving a system of
///   linear equations, as opposed to only disproving reachability when no solution exists.
///
/// - T-invariants of T-nets:
///   Let N = (S, T, F) be a T-net. A vector J: T → Q is a T-invariant of N
///   iff J = (x, ..., x) for some x ∈ Q.
///   Intuitively, firing all transitions the same number of times has no net effect on the marking.
///
/// - Let N be a strongly connected T-net. For every marking M<sub>0</sub> the following statements
///   are equivalent:
///   1. (N, M<sub>0</sub>) is live.
///   2. (N, M<sub>0</sub>) is deadlock-free.
///   3. (N, M<sub>0</sub>) has an infinite firing sequence.
///
/// - Genrich's theorem:
///   Let N be a strongly connected T-net with at least one place and one transition.
///   There exists a marking M<sub>0</sub> such that (N, M<sub>0</sub>) is live and 1-bounded.
///
/// - Let (N, M<sub>0</sub>) be a 1-bounded T-system (live or not).
///   For any two markings M<sub>1</sub> and M<sub>2</sub>, if M<sub>2</sub> is reachable from M<sub>1</sub>,
///   then it can be reached in at most n(n-1)/2 steps, where n = |T| is the number of transitions.
///
/// - Let (N, M<sub>0</sub>) be a b-bounded T-system (live or not).
///   For any marking M reachable from M<sub>0</sub>, there exists a firing sequence
///   M<sub>0</sub> <sup>σ</sup>→ M such that |σ| ≤ b * n(n-1)/2, where n = |T| is the number of transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TNet(Net);

/// A net N = (S, T, F) is an `S-net` if |•t| = |t•| = 1 for every transition t ∈ T.
/// This means each transition has exactly one input and one output place.
/// S-nets can model sequential processes and choices, but not concurrency.
/// (N, M<sub>0</sub>) is an `S-system` if N is an S-net.
/// This structural restriction implies several important properties:
/// - Fundamental property:
///   Let (N,M0) be an S-system with N = (S,T,F).
///   Then M<sub>0</sub>(S) = M(S) for every reachable marking M.
/// - Liveness theorem:
///   An S-system (N, M<sub>0</sub>) where N = (S, T, F) is live
///   iff N is strongly connected and M<sub>0</sub>(S) > 0.
/// - Boundedness theorem:
///   A live S-system (N, M<sub>0</sub>) where N = (S, T, F) is b-bounded
///   iff M<sub>0</sub>(S) ≤ b.
/// - Reachability theorem:
///   Let (N, M<sub>0</sub>) be a live S-system with N = (S, T, F)
///   and let M be a marking of N. M is reachable from M<sub>0</sub>
///   iff M(S) = M<sub>0</sub>(S).
/// - S-invariants of S-nets:
///   Let N = (S, T, F) be an S-net. A vector I: S → Q is an S-invariant of N
///   iff I = (x, ..., x) for some x ∈ Q.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SNet(pub(crate) Net);

/// A net N = (S, T, F) is a `circuit` iff it is both an S-net and a T-net,
/// i.e., |•t| = |t•| = 1 for every t ∈ T and |•s| = |s•| = 1 for every s ∈ S.
///
/// Liveness theorem:
/// A circuit (N, M<sub>0</sub>) is live iff M<sub>0</sub>(S) > 0.
///
/// Boundedness theorem:
/// A circuit (N, M<sub>0</sub>) is b-bounded iff M<sub>0</sub>(S) ≤ b.
///
/// Reachability theorem:
/// A marking M is reachable from M<sub>0</sub> in a circuit (N, M<sub>0</sub>)
/// iff M(S) = M<sub>0</sub>(S).
///
/// S-invariants and T-invariants of circuits:
/// Let N = (S, T, F) be a circuit. A vector I: S → Q is an S-invariant of N
/// iff I = (x, ..., x) for some x ∈ Q. Similarly, a vector J: T → Q is a T-invariant of N
/// iff J = (y, ..., y) for some y ∈ Q.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Circuit(Net);

/// Structural classification of Petri nets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureClass {
    /// The most general class of Petri nets, with no structural restrictions.
    Unrestricted(Net),
    /// Subclass of unrestricted nets where for every two transitions t1, t2 ∈ T,
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    FreeChoiceNet(FreeChoiceNet),
    /// Subclass of free-choice nets where |•s| = |s•| = 1 for every place s ∈ S.
    TNet(TNet),
    /// Subclass of free-choice nets where |•t| = |t•| = 1 for every transition t ∈ T.
    SNet(SNet),
    /// A net fulfilling both the S-net and T-net properties.
    Circuit(Circuit),
}

impl StructureClass {
    #[must_use]
    pub fn into_inner(self) -> Net {
        match self {
            StructureClass::Unrestricted(net) => net,
            StructureClass::FreeChoiceNet(FreeChoiceNet(net)) => net,
            StructureClass::TNet(TNet(net)) => net,
            StructureClass::SNet(SNet(net)) => net,
            StructureClass::Circuit(Circuit(net)) => net,
        }
    }
    pub fn inner(&self) -> &Net {
        match self {
            StructureClass::Unrestricted(net) => net,
            StructureClass::FreeChoiceNet(FreeChoiceNet(net)) => net,
            StructureClass::TNet(TNet(net)) => net,
            StructureClass::SNet(SNet(net)) => net,
            StructureClass::Circuit(Circuit(net)) => net,
        }
    }
}

impl TryFrom<Net> for Circuit {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_circuit() {
            Ok(Circuit(net))
        } else {
            Err(net)
        }
    }
}

impl TryFrom<Net> for SNet {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_s_net() {
            Ok(SNet(net))
        } else {
            Err(net)
        }
    }
}

impl TryFrom<Net> for TNet {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_t_net() {
            Ok(TNet(net))
        } else {
            Err(net)
        }
    }
}

impl TryFrom<Net> for FreeChoiceNet {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_free_choice() {
            Ok(FreeChoiceNet(net))
        } else {
            Err(net)
        }
    }
}

pub mod builder {
    use crate::structure::{Arc, Circuit, FreeChoiceNet, IncidenceMatrix, Index, Net, Place, SNet, StructureClass, TNet, Transition};
    use nalgebra::DMatrix;
    use num_traits::{One, Zero};
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::ops::AddAssign;

    #[derive(Debug, Clone)]
    pub struct NetBuilder {
        num_places: Index,
        num_transitions: Index,
        arcs: Vec<Arc>,
    }

    #[derive(Debug)]
    pub enum BuildError {
        /// Not connected: the net can be partitioned into disjoint subnets.
        NotConnected,
        /// Arc references non-existent place or transition.
        InvalidArc,
    }

    impl Display for BuildError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                BuildError::NotConnected => write!(f, "the net is not connected"),
                BuildError::InvalidArc => write!(f, "an arc references a non-existent place or transition"),
            }
        }
    }

    impl Error for BuildError {}

    impl Default for NetBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    impl NetBuilder {
        /// Creates a new, empty builder.
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let builder = NetBuilder::new();
        /// ```
        #[must_use]
        pub fn new() -> Self {
            Self {
                num_places: Index::zero(),
                num_transitions: Index::zero(),
                arcs: Vec::new(),
            }
        }

        /// Adds a new place to the net, returning a safe handle to it.
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let mut builder = NetBuilder::new();
        /// let p1 = builder.add_place();
        /// let p2 = builder.add_place();
        /// ```
        pub fn add_place(&mut self) -> Place {
            let place = Place { index: self.num_places };
            self.num_places.add_assign(Index::one());
            place
        }

        /// Adds a new transition, returning a safe handle.
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let mut builder = NetBuilder::new();
        /// let t1 = builder.add_transition();
        /// let t2 = builder.add_transition();
        /// ```
        pub fn add_transition(&mut self) -> Transition {
            let transition = Transition { index: self.num_transitions };
            self.num_transitions.add_assign(Index::one());
            transition
        }

        /// Adds N places to the net, returning an array of safe handles.
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let mut builder = NetBuilder::new();
        /// let [p1, p2, p3] = builder.add_places();
        /// let [p4, p5] = builder.add_places();
        /// ```
        pub fn add_places<const N: usize>(&mut self) -> [Place; N] {
            let mut places = [Place { index: Index::zero() }; N];
            for place in &mut places {
                *place = self.add_place();
            }
            places
        }

        /// Adds N transitions to the net, returning an array of safe handles.
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let mut builder = NetBuilder::new();
        /// let [t1, t2] = builder.add_transitions();
        /// let [t3, t4, t5] = builder.add_transitions();
        pub fn add_transitions<const N: usize>(&mut self) -> [Transition; N] {
            let mut transitions = [Transition { index: Index::zero() }; N];
            for transition in &mut transitions {
                *transition = self.add_transition();
            }
            transitions
        }

        /// Creates an arc in the net.
        /// Accepts any type that implements Into<Arc>, enabling ergonomic syntax:
        /// - `add_arc((place_id, transition_id))` for place-to-transition arcs
        /// - `add_arc((transition_id, place_id))` for transition-to-place arcs
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let mut builder = NetBuilder::new();
        /// let [p0, p1] = builder.add_places();
        /// let [t0] = builder.add_transitions();
        /// builder.add_arc((p0, t0)); // place to transition
        /// builder.add_arc((t0, p1)); // transition to place
        /// ```
        pub fn add_arc<A: Into<Arc>>(&mut self, arc: A) {
            self.arcs.push(arc.into());
        }

        /// Consumes the builder to produce a validated `OrdinaryNet`.
        /// Returns an error if the net is invalid.
        /// Validation checks:
        /// - At least one place and one transition exist.
        /// - All places and transitions are referenced by at least one arc.
        /// - All arcs reference valid place and transition IDs.
        /// ```
        /// use petrivet::structure::builder::NetBuilder;
        /// let mut builder = NetBuilder::new();
        /// let [p0, p1] = builder.add_places();
        /// let [t0] = builder.add_transitions();
        /// builder.add_arc((p0, t0));
        /// builder.add_arc((t0, p1));
        /// let net = builder.build().unwrap();
        /// ```
        ///
        /// # Errors
        /// This function will return an error if:
        /// - The net is not connected (some places or transitions are isolated).
        /// - An arc references a non-existent place or transition.
        pub fn build(self) -> Result<StructureClass, BuildError> {
            // check if all places and transitions are referenced by at least one arc
            let mut preset = vec![Vec::new(); self.num_transitions.into()].into_boxed_slice();
            let mut postset = vec![Vec::new(); self.num_transitions.into()].into_boxed_slice();
            let mut preset_p = vec![Vec::new(); self.num_places.into()].into_boxed_slice();
            let mut postset_p = vec![Vec::new(); self.num_places.into()].into_boxed_slice();
            let mut matrix: DMatrix<i8> = DMatrix::zeros(self.num_places.into(), self.num_transitions.into());
            for arc in self.arcs {
                match arc {
                    Arc::PlaceTransition(place, transition) => {
                        if place.index >= self.num_places || transition.index >= self.num_transitions {
                            return Err(BuildError::InvalidArc);
                        }
                        preset[usize::from(transition.index)].push(place);
                        postset_p[usize::from(place.index)].push(transition);
                        matrix[(place.index.into(), transition.index.into())] -= 1;
                    }
                    Arc::TransitionPlace(transition, place) => {
                        if transition.index >= self.num_transitions || place.index >= self.num_places {
                            return Err(BuildError::InvalidArc);
                        }
                        postset[usize::from(transition.index)].push(place);
                        preset_p[usize::from(place.index)].push(transition);
                        matrix[(place.index.into(), transition.index.into())] += 1;
                    }
                }
            }
            if Iterator::zip(preset.iter(), postset.iter()).any(|(pre, post)| pre.is_empty() && post.is_empty())
                || Iterator::zip(preset_p.iter(), postset_p.iter()).any(|(pre, post)| pre.is_empty() && post.is_empty()) {
                return Err(BuildError::NotConnected);
            }
            let incidence_matrix = IncidenceMatrix(matrix);
            let net = Net {
                n_places: self.num_places,
                n_transitions: self.num_transitions,
                preset: preset.into_iter().map(Vec::into_boxed_slice).collect(),
                postset: postset.into_iter().map(Vec::into_boxed_slice).collect(),
                preset_p: preset_p.into_iter().map(Vec::into_boxed_slice).collect(),
                postset_p: postset_p.into_iter().map(Vec::into_boxed_slice).collect(),
                incidence_matrix,
            };
            Circuit::try_from(net).map(StructureClass::Circuit)
                .or_else(|net| SNet::try_from(net).map(StructureClass::SNet))
                .or_else(|net| TNet::try_from(net).map(StructureClass::TNet))
                .or_else(|net| FreeChoiceNet::try_from(net).map(StructureClass::FreeChoiceNet))
                .or_else(|net| Ok(StructureClass::Unrestricted(net)))
        }
    }

    mod test {
        #[test]
        fn test_builder() {
            let mut builder = super::NetBuilder::new();
            let [p0, p1, p2] = builder.add_places();
            let [t0, t1] = builder.add_transitions();
            builder.add_arc((p0, t0));
            builder.add_arc((t0, p1));
            builder.add_arc((p1, t1));
            builder.add_arc((t1, p2));
            let net = builder.build().unwrap().into_inner();
            assert_eq!(net.n_places, 3);
            assert_eq!(net.n_transitions, 2);
            assert_eq!(net.arcs().count(), 4);
        }

        #[test]
        fn test_builder_invalid_arc() {
            let mut builder = super::NetBuilder::new();
            let mut other_builder = super::NetBuilder::new();
            let p0 = builder.add_place();
            let _ = builder.add_transition();
            let [_, t1] = other_builder.add_transitions();
            builder.add_arc((p0, t1)); // t1 does not exist in builder
            assert!(matches!(builder.build(), Err(super::BuildError::InvalidArc)));
        }

        #[test]
        fn test_builder_not_connected() {
            let mut builder = super::NetBuilder::new();
            let p0 = builder.add_place();
            let [t0, _t1] = builder.add_transitions();
            builder.add_arc((p0, t0));
            // _t1 is not connected
            assert!(matches!(builder.build(), Err(super::BuildError::NotConnected)));
        }

        #[test]
        fn test_builder_circuit() {
            let mut builder = super::NetBuilder::new();
            let [p0, p1] = builder.add_places();
            let [t0, t1] = builder.add_transitions();
            builder.add_arc((p0, t0));
            builder.add_arc((t0, p1));
            builder.add_arc((p1, t1));
            builder.add_arc((t1, p0));
            let net = builder.build().unwrap();
            assert!(matches!(net, crate::structure::StructureClass::Circuit(_)));
        }

        #[test]
        fn test_builder_s_net() {
            let mut builder = super::NetBuilder::new();
            let [p0, p1, p2] = builder.add_places();
            let [t0, t1] = builder.add_transitions();
            builder.add_arc((p0, t0));
            builder.add_arc((t0, p1));
            builder.add_arc((p1, t1));
            builder.add_arc((t1, p2));
            let net = builder.build().unwrap();
            assert!(matches!(net, crate::structure::StructureClass::SNet(_)));
        }

        #[test]
        fn test_builder_t_net() {
            let mut builder = super::NetBuilder::new();
            let [p0, p1] = builder.add_places();
            let [t0, t1, t2] = builder.add_transitions();
            builder.add_arc((p0, t0));
            builder.add_arc((t0, p1));
            builder.add_arc((p1, t1));
            builder.add_arc((t1, p0));
            builder.add_arc((p0, t2));
            builder.add_arc((t2, p1));
            let net = builder.build().unwrap();
            assert!(matches!(net, crate::structure::StructureClass::TNet(_)));
        }

        #[test]
        fn test_builder_free_choice_net() {
            let mut builder = super::NetBuilder::new();
            let [p0, p1, p2] = builder.add_places();
            let [t0, t1] = builder.add_transitions();
            builder.add_arc((p0, t0));
            builder.add_arc((p0, t1));
            builder.add_arc((t0, p1));
            builder.add_arc((t1, p2));
            let net = builder.build().unwrap();
            assert!(matches!(net, crate::structure::StructureClass::FreeChoiceNet(_)));
        }

        #[test]
        fn test_builder_unrestricted_net() {
            let mut builder = super::NetBuilder::new();
            let [p0, p1] = builder.add_places();
            let [t0, t1] = builder.add_transitions();
            builder.add_arc((p0, t0));
            builder.add_arc((t0, p1));
            builder.add_arc((p1, t1));
            let net = builder.build().unwrap();
            assert!(matches!(net, crate::structure::StructureClass::Unrestricted(_)));
        }
    }
}