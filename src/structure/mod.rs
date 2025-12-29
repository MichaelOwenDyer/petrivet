pub mod invariants;
pub mod class;

use nalgebra::{DMatrix, Dyn, MatrixView, U1};
use num_traits::Zero;
use petgraph::Graph;
use std::fmt;
use std::hash::Hash;
use petgraph::graph::NodeIndex;
use crate::behavior::{Marking, Tokens};
use crate::structure::invariants::IncidenceMatrix;

pub type Index = usize;

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

    /// The input markings of each transition, indexed by transition ID.
    /// That is, the minimal marking that enables the transition.
    input_markings: Box<[Marking]>,

    /// The incidence markings of each transition, indexed by transition ID.
    /// That is, the marking that represents the net effect of firing the transition once.
    incidence_markings: Box<[Marking]>,

    /// The output places of each transition, indexed by transition ID.
    postset: Box<[Box<[Place]>]>,

    preset_p: Box<[Box<[Transition]>]>,

    postset_p: Box<[Box<[Transition]>]>,

    /// The incidence matrix of the net.
    incidence_matrix: IncidenceMatrix,
}

impl Net {
    #[must_use]
    pub fn n_places(&self) -> Index {
        self.n_places
    }
    
    /// Returns an iterator over all places in the net.
    pub fn places(&self) -> impl Iterator<Item = Place> {
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
                    preset.map(move |p| Arc::PlaceTransition(p, t)),
                    postset.map(move |p| Arc::TransitionPlace(t, p)),
                )
            })
    }

    pub fn preset_t(&self, transition: Transition) -> impl Iterator<Item = Place> {
        self.preset[transition.index].iter().copied()
    }

    pub fn postset_t(&self, transition: Transition) -> impl Iterator<Item = Place> {
        self.postset[transition.index].iter().copied()
    }

    pub fn preset_p(&self, place: Place) -> impl Iterator<Item = Transition> {
        self.preset_p[place.index].iter().copied()
    }

    pub fn postset_p(&self, place: Place) -> impl Iterator<Item = Transition> {
        self.preset_p[place.index].iter().copied()
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

    /// This function checks if the net is a free-choice net by checking the free-choice property:
    /// (t1  ̸= t2 ∧•t1 ∩•t2 ̸= ∅) ⇒•t1 = •t2
    #[must_use]
    pub fn is_free_choice(&self) -> bool {
        self.transitions().all(|t1| {
            self.transitions().all(|t2| {
                if t1 == t2 {
                    true
                } else if self.preset_t(t1).any(|p1| self.preset_t(t2).any(|p2| p1 == p2)) {
                    // t1 and t2 share an input place
                    // A free-choice net requires that any two
                    self.preset_t(t1).all(|p1| self.preset_t(t2).any(|p2| p1 == p2))
                } else {
                    true
                }
            })
        })
    }

    #[must_use]
    pub fn is_enabled_in<T>(&self, marking: &Marking<T>, transition: Transition) -> bool
    where
        T: PartialOrd<Tokens>,
    {
        marking >= self.input_marking(transition)
    }

    pub fn enabled_transitions<'a, T>(&'a self, marking: &'a Marking<T>) -> impl Iterator<Item = Transition> + 'a
    where
        Tokens: PartialOrd<T>,
    {
        self.transitions().filter(move |&t| self.input_marking(t) <= marking)
    }

    /// Returns the minimal marking that enables the given transition.
    /// That is, the marking that puts one token in each input place of the given transition.
    #[must_use]
    pub fn input_marking(&self, transition: Transition) -> &Marking {
        &self.input_markings[transition.index]
    }

    /// Returns the minimal marking produced by the given transition.
    /// That is, the marking that puts one token in each output place of the given transition.
    #[must_use]
    pub fn output_marking(&self, transition: Transition) -> Marking {
        let mut marking = Marking::zeroes(self.n_places());
        for place in self.postset_t(transition) {
            marking[place] = Tokens(1);
        }
        marking
    }

    /// Returns the incidence marking of the given transition.
    /// That is, the marking that represents the net effect of firing the transition once.
    #[must_use]
    pub fn incidence_marking(&self, transition: Transition) -> &Marking {
        &self.incidence_markings[transition.index]
    }

    #[must_use]
    pub fn n_nodes(&self) -> Index {
        self.n_transitions + self.n_places
    }

    pub fn nodes(&self) -> impl Iterator<Item = Node> {
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
                .map(|p| map[&Node::Place(p)])
                .for_each(|input_place_idx| {
                    graph.add_edge(input_place_idx, transition_idx, ());
                });
            self.postset_t(t)
                .map(|p| map[&Node::Place(p)])
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

pub mod builder {
    use crate::structure::{Arc, IncidenceMatrix, Index, Net, Place, Transition};
    use crate::structure::class::{Circuit, FreeChoiceNet, SNet, StructureClass, TNet};
    use nalgebra::DMatrix;
    use num_traits::{One, Zero};
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::ops::AddAssign;
    use crate::behavior::{Marking, Tokens};

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
            let mut preset = vec![Vec::new(); self.num_transitions].into_boxed_slice();
            let mut postset = vec![Vec::new(); self.num_transitions].into_boxed_slice();
            let mut preset_p = vec![Vec::new(); self.num_places].into_boxed_slice();
            let mut postset_p = vec![Vec::new(); self.num_places].into_boxed_slice();
            let mut matrix: DMatrix<i8> = DMatrix::zeros(self.num_places, self.num_transitions);
            for arc in self.arcs {
                match arc {
                    Arc::PlaceTransition(place, transition) => {
                        if place.index >= self.num_places || transition.index >= self.num_transitions {
                            return Err(BuildError::InvalidArc);
                        }
                        preset[transition.index].push(place);
                        postset_p[place.index].push(transition);
                        matrix[(place.index, transition.index)] -= 1;
                    }
                    Arc::TransitionPlace(transition, place) => {
                        if transition.index >= self.num_transitions || place.index >= self.num_places {
                            return Err(BuildError::InvalidArc);
                        }
                        postset[transition.index].push(place);
                        preset_p[place.index].push(transition);
                        matrix[(place.index, transition.index)] += 1;
                    }
                }
            }
            if Iterator::zip(preset.iter(), postset.iter()).any(|(pre, post)| pre.is_empty() && post.is_empty())
                || Iterator::zip(preset_p.iter(), postset_p.iter()).any(|(pre, post)| pre.is_empty() && post.is_empty()) {
                return Err(BuildError::NotConnected);
            }
            let incidence_matrix = IncidenceMatrix(matrix);
            let input_markings = Iterator::zip(preset.iter(), postset.iter()).map(|(pre, _post)| {
                let mut marking = Marking::zeroes(self.num_places);
                for place in pre {
                    marking[*place] = Tokens(1);
                }
                marking
            }).collect();
            let incidence_markings = Iterator::zip(preset.iter(), postset.iter()).map(|(pre, post)| {
                let mut marking = Marking::zeroes(self.num_places);
                for place in pre {
                    marking[*place] -= Tokens(1);
                }
                for place in post {
                    marking[*place] += Tokens(1);
                }
                marking
            }).collect();
            let net = Net {
                n_places: self.num_places,
                n_transitions: self.num_transitions,
                input_markings,
                incidence_markings,
                preset: preset.into_iter().map(Vec::into_boxed_slice).collect(),
                postset: postset.into_iter().map(Vec::into_boxed_slice).collect(),
                preset_p: preset_p.into_iter().map(Vec::into_boxed_slice).collect(),
                postset_p: postset_p.into_iter().map(Vec::into_boxed_slice).collect(),
                incidence_matrix,
            };
            Ok(match (net.is_s_net(), net.is_t_net()) {
                (true, true) => StructureClass::Circuit(Circuit(net)),
                (true, false) => StructureClass::SNet(SNet(net)),
                (false, true) => StructureClass::TNet(TNet(net)),
                (false, false) => {
                    if net.is_free_choice() {
                        StructureClass::FreeChoiceNet(FreeChoiceNet(net))
                    } else {
                        StructureClass::Unrestricted(net)
                    }
                }
            })
        }
    }

    mod test {
        use crate::structure::class::StructureClass;

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
            assert!(matches!(net, StructureClass::Circuit(_)));
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
            assert!(matches!(net, StructureClass::SNet(_)));
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
            assert!(matches!(net, StructureClass::TNet(_)));
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
            assert!(matches!(net, StructureClass::FreeChoiceNet(_)));
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
            assert!(matches!(net, StructureClass::Unrestricted(_)));
        }
    }
}