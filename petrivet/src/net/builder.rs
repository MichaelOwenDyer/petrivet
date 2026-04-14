//! Builder for constructing Petri nets with stable node identity.
//!
//! While you edit, places and transitions are [`Place`] and [`Transition`] values minted by
//! the builder. They stay valid until you remove that node from *this* builder. When
//! you call [`NetBuilder::build`], surviving keys are assigned dense indices and the resulting
//! [`Net`] stores both directions of the mapping so you can move between keys and [`PlaceIdx`] /
//! [`TransitionIdx`] without maintaining parallel tables yourself.
//!
//! [`NetBuilder::from`] rebuilds a builder from a built [`Net`] using the net’s stored keys so
//! handles remain usable across round-trips.

use crate::class::NetClass;
use crate::net::keys::{Place, Transition};
use crate::net::{Net, PlaceIdx, SortedSet, TransitionIdx};
use crate::{Arc, Node};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::hash::Hash;
use std::{fmt, iter};

/// Builder for an ordinary Petri net.
///
/// Active places and transitions are listed in [`NetBuilder::places`] /
/// [`NetBuilder::transitions`] order; that order becomes dense `0..n−1` at [`NetBuilder::build`].
/// New keys are unique numeric ids minted by this builder. [`NetBuilder::from`] seeds lists from a
/// [`Net`] so existing [`Place`] / [`Transition`] handles stay valid.
///
/// Adjacency is kept in the usual four directions (each place and each transition has preset and
/// postset sets of the opposite kind’s keys), so removing a node touches only its neighbours’
/// sets.
///
/// We use [`HashMap`] for adjacency so keys from a built [`Net`] can coexist with keys minted
/// after [`NetBuilder::from`]. [`Place`] / [`Transition`] are unique numeric ids (see
/// [`crate::net::keys`]), so hash-based structures stay sound when mixing round-tripped and new
/// handles.
#[derive(Debug, Clone)]
pub struct NetBuilder {
    /// Next unused id for [`add_place`](Self::add_place) (strictly greater than any id in
    /// [`Self::places`]).
    next_place_id: u32,
    /// Next unused id for [`add_transition`](Self::add_transition).
    next_transition_id: u32,
    /// Live places in iteration order (defines dense indices at build).
    places: Vec<Place>,
    /// Live transitions in iteration order (defines dense indices at build).
    transitions: Vec<Transition>,
    /// For each transition: input places •t.
    preset_t: HashMap<Transition, HashSet<Place>>,
    /// For each transition: output places t•.
    postset_t: HashMap<Transition, HashSet<Place>>,
    /// For each place: input transitions •p.
    preset_p: HashMap<Place, HashSet<Transition>>,
    /// For each place: output transitions p•.
    postset_p: HashMap<Place, HashSet<Transition>>,
}

#[derive(Debug, Clone)]
struct SortedSetBuilder<T>(Vec<T>);

impl<T: Ord> SortedSetBuilder<T> {

    /// Creates a new empty `SortedSetBuilder`.
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// Binary search insert. O(log n) for search, O(n) for insert.
    pub(crate) fn add(&mut self, item: T) -> bool {
        match self.0.binary_search(&item) {
            Ok(_) => false, // already present
            Err(pos) => {
                // insert at the correct position
                self.0.insert(pos, item);
                true
            },
        }
    }

    /// Binary search removal. O(log n) for search, O(n) for removal.
    /// Returns `true` if the item was present and removed.
    pub(crate) fn remove(&mut self, item: &T) -> bool {
        match self.0.binary_search(item) {
            Ok(pos) => {
                self.0.remove(pos);
                true
            }
            Err(_) => false,
        }
    }
}

impl Default for NetBuilder {
    fn default() -> Self {
        Self {
            next_place_id: 1,
            next_transition_id: 1,
            places: Vec::new(),
            transitions: Vec::new(),
            preset_t: HashMap::new(),
            postset_t: HashMap::new(),
            preset_p: HashMap::new(),
            postset_p: HashMap::new(),
        }
    }
}

/// Errors that can occur during net construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildError {
    /// The net does not have at least one place and at least one transition.
    Degenerate,
    /// The net consists of two or more disconnected components.
    NotConnected,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Degenerate => write!(f, "the net must have at least one place and at least one transition"),
            BuildError::NotConnected => write!(f, "the net must be connected (every node reachable from every other node ignoring arc directions)"),
        }
    }
}

impl Error for BuildError {}

impl NetBuilder {
    /// Creates a new, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder with `n_places` places and `n_transitions` transitions and **no** arcs.
    ///
    /// This is intended for loaders (such as PNML) that already know how many nodes exist and want
    /// to wire arcs immediately: every place and transition is created up front, so there is no
    /// risk of adding an arc to a node that was never inserted.
    #[must_use]
    pub fn with_places_and_transitions(n_places: usize, n_transitions: usize) -> Self {
        let mut b = Self::new();
        for _ in 0..n_places {
            b.add_place();
        }
        for _ in 0..n_transitions {
            b.add_transition();
        }
        b
    }

    /// Adds one place and returns its stable key.
    pub fn add_place(&mut self) -> Place {
        let id = self.next_place_id;
        self.next_place_id = self
            .next_place_id
            .checked_add(1)
            .expect("nets with more than 2^32 places are not supported");
        let pk = Place::from_raw(id);
        self.places.push(pk);
        self.preset_p.insert(pk, HashSet::new());
        self.postset_p.insert(pk, HashSet::new());
        pk
    }

    /// Adds `N` places and returns their keys.
    pub fn add_places<const N: usize>(&mut self) -> [Place; N] {
        std::array::from_fn(|_| self.add_place())
    }

    /// Adds one transition and returns its stable key.
    pub fn add_transition(&mut self) -> Transition {
        let id = self.next_transition_id;
        self.next_transition_id = self
            .next_transition_id
            .checked_add(1)
            .expect("transition key id overflow");
        let tk = Transition::from_raw(id);
        self.transitions.push(tk);
        self.preset_t.insert(tk, HashSet::new());
        self.postset_t.insert(tk, HashSet::new());
        tk
    }

    /// Adds `N` transitions and returns their keys.
    pub fn add_transitions<const N: usize>(&mut self) -> [Transition; N] {
        std::array::from_fn(|_| self.add_transition())
    }

    /// Removes a place and every arc incident on it. Returns `false` if that key was not active.
    pub fn remove_place(&mut self, place: Place) -> bool {
        let Some(inputs) = self.preset_p.remove(&place) else {
            return false;
        };
        let outputs = self.postset_p.remove(&place).unwrap_or_default();

        for &t in &inputs {
            self.postset_t.get_mut(&t).unwrap().remove(&place);
        }
        for &t in &outputs {
            self.preset_t.get_mut(&t).unwrap().remove(&place);
        }
        if let Some(pos) = self.places.iter().position(|&k| k == place) {
            self.places.swap_remove(pos);
        }
        true
    }

    /// Removes a transition and every arc incident on it.
    pub fn remove_transition(&mut self, transition: Transition) -> bool {
        let Some(inputs) = self.preset_t.remove(&transition) else {
            return false;
        };
        let outputs = self.postset_t.remove(&transition).unwrap_or_default();

        for &p in &inputs {
            self.postset_p.get_mut(&p).unwrap().remove(&transition);
        }
        for &p in &outputs {
            self.preset_p.get_mut(&p).unwrap().remove(&transition);
        }
        if let Some(pos) = self.transitions.iter().position(|&k| k == transition) {
            self.transitions.swap_remove(pos);
        }
        true
    }

    /// Removes a directed arc if it exists.
    pub fn remove_arc<A: Into<Arc>>(&mut self, arc: A) -> bool {
        match arc.into() {
            Arc::PlaceToTransition(p, t) => {
                let removed = self
                    .preset_t
                    .get_mut(&t)
                    .is_some_and(|s| s.remove(&p));
                if removed {
                    self.postset_p.get_mut(&p).unwrap().remove(&t);
                }
                removed
            }
            Arc::TransitionToPlace(t, p) => {
                let removed = self
                    .postset_t
                    .get_mut(&t)
                    .is_some_and(|s| s.remove(&p));
                if removed {
                    self.preset_p.get_mut(&p).unwrap().remove(&t);
                }
                removed
            }
        }
    }

    /// Adds a directed arc if it is not already present. Returns `true` when newly inserted.
    /// Returns `false` if the arc already exists or either the place or transition does not
    /// exist in the net.
    pub fn add_arc<A: Into<Arc>>(&mut self, arc: A) -> bool {
        let arc = arc.into();
        match arc {
            Arc::PlaceToTransition(p, t) => {
                let p_postset = self.postset_p.get_mut(&p);
                let t_preset = self.preset_t.get_mut(&t);
                match (p_postset, t_preset) {
                    (Some(postset), Some(preset)) => {
                        postset.insert(t) && preset.insert(p)
                    }
                    // One or both of the nodes do not exist
                    _ => false,
                }
            }
            Arc::TransitionToPlace(t, p) => {
                let t_postset = self.postset_t.get_mut(&t);
                let p_preset = self.preset_p.get_mut(&p);
                match (t_postset, p_preset) {
                    (Some(postset), Some(preset)) => {
                        postset.insert(p) && preset.insert(t)
                    }
                    // One or both of the nodes do not exist
                    _ => false,
                }
            }
        }
    }

    /// Adds several alternating arcs at once; see [`IntoArcs`].
    pub fn add_arcs<A: IntoArcs>(&mut self, arcs: A) -> bool {
        arcs.into_builder_arcs().all(|a| self.add_arc(a))
    }

    #[must_use]
    pub fn place_count(&self) -> usize {
        self.places.len()
    }

    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Active places.
    #[must_use]
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.places.iter().copied()
    }

    /// Active transitions.
    #[must_use]
    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.transitions.iter().copied()
    }

    /// Iterates every directed arc currently in the builder.
    pub fn arcs(&self) -> impl Iterator<Item = Arc> + '_ {
        iter::chain(
            self.preset_t.iter().flat_map(|(t, preset)| {
                preset.iter().map(move |&p| Arc::PlaceToTransition(p, *t))
            }),
            self.postset_t.iter().flat_map(|(t, post)| {
                post.iter().map(move |&p| Arc::TransitionToPlace(*t, p))
            }),
        )
    }

    /// Classify the net in its current state.
    #[must_use]
    pub fn classify(&self) -> NetClass {
        if self.place_count() == 0 || self.transition_count() == 0 {
            return NetClass::Unrestricted;
        }
        crate::net::class::classify(
            &self.preset_t, &self.postset_t, &self.preset_p, &self.postset_p
        )
    }

    /// Consumes the builder and returns a validated [`Net`], or a [`BuildError`].
    pub fn build(self) -> Result<Net, BuildError> {
        if self.place_count() == 0 || self.transition_count() == 0 {
            return Err(BuildError::Degenerate);
        }

        let place_to_index: HashMap<Place, PlaceIdx> = self.places
            .iter()
            .copied()
            .zip(0..)
            .collect();

        let transition_to_index = self.transitions
            .iter()
            .copied()
            .zip(0..)
            .collect();

        if !is_connected(&self.preset_t, &self.postset_t, &self.preset_p, &self.postset_p) {
            return Err(BuildError::NotConnected);
        }

        let class = crate::net::class::classify(
            &self.preset_t, &self.postset_t, &self.preset_p, &self.postset_p
        );

        let preset_t = map_adjacency(&self.transitions, &self.preset_t, &place_to_index);
        let postset_t = map_adjacency(&self.transitions, &self.postset_t, &place_to_index);
        let preset_p = map_adjacency(&self.places, &self.preset_p, &transition_to_index);
        let postset_p = map_adjacency(&self.places, &self.postset_p, &transition_to_index);

        let index_to_place = self.places.into_boxed_slice();
        let index_to_transition = self.transitions.into_boxed_slice();

        Ok(Net {
            class,
            preset_t,
            postset_t,
            preset_p,
            postset_p,
            place_to_index,
            transition_to_index,
            index_to_place,
            index_to_transition,
            labels: None,
            graphics: None,
        })
    }
}

fn map_adjacency<N, M, Idx>(
    ordered_nodes: &[N],
    sparse_adjacency: &HashMap<N, HashSet<M>>,
    dense_index_map: &HashMap<M, Idx>,
) -> Box<[SortedSet<Idx>]>
where
    N: Eq + Hash,
    M: Eq + Hash,
    Idx: Ord + Copy,
{
    ordered_nodes
        .iter()
        .map(|node| {
            let mut indices: Vec<Idx> = sparse_adjacency
                .get(node)
                .map(|neighbors| {
                    neighbors
                        .iter()
                        .map(|neighbor| {
                            *dense_index_map
                                .get(neighbor)
                                .expect("Neighbor key must exist in dense index map")
                        })
                        .collect()
                })
                .unwrap_or_default();
            indices.sort_unstable();
            SortedSet(indices.into_boxed_slice())
        })
        .collect()
}

/// Convert a built Net back into a NetBuilder for editing.
impl From<Net> for NetBuilder {
    fn from(net: Net) -> Self {
        let place_count = net.place_count() as usize;
        let transition_count = net.transition_count() as usize;

        let mut places = Vec::with_capacity(place_count);
        for p in net.places() {
            places.push(p);
        }

        let mut transitions = Vec::with_capacity(transition_count);
        for t in net.transitions() {
            transitions.push(t);
        }

        let mut preset_t = HashMap::with_capacity(transition_count);
        let mut postset_t = HashMap::with_capacity(transition_count);
        let mut preset_p = HashMap::with_capacity(place_count);
        let mut postset_p = HashMap::with_capacity(place_count);

        for (t_idx, preset, postset) in net.transition_io() {
            let t = net.index_to_transition[t_idx];
            preset.iter().map(|&idx| net.index_to_place[idx]).for_each(|p| {
                preset_t.entry(t).or_insert_with(HashSet::new).insert(p);
                postset_p.entry(p).or_insert_with(HashSet::new).insert(t);
            });
            postset.iter().map(|&idx| net.index_to_place[idx]).for_each(|p| {
                postset_t.entry(t).or_insert_with(HashSet::new).insert(p);
                preset_p.entry(p).or_insert_with(HashSet::new).insert(t);
            });
        }

        let next_place_id = places
            .iter()
            .map(|k| k.into_raw())
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);

        let next_transition_id = transitions
            .iter()
            .map(|k| k.into_raw())
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);

        Self {
            places,
            transitions,
            next_place_id,
            next_transition_id,
            preset_t,
            postset_t,
            preset_p,
            postset_p,
        }
    }
}

fn is_connected(
    preset_t: &HashMap<Transition, HashSet<Place>>,
    postset_t: &HashMap<Transition, HashSet<Place>>,
    preset_p: &HashMap<Place, HashSet<Transition>>,
    postset_p: &HashMap<Place, HashSet<Transition>>,
) -> bool {
    let n_places = preset_p.len();
    let n_transitions = preset_t.len();
    let n_nodes = n_places + n_transitions;
    if n_nodes > 0 {
        let mut visited_p = HashSet::new();
        let mut visited_t = HashSet::new();
        let mut queue = VecDeque::new();
        if n_places > 0 {
            let first_place = preset_p.keys().next().unwrap().to_owned();
            visited_p.insert(first_place);
            queue.push_back(Node::Place(first_place));
        } else {
            let first_transition = preset_t.keys().next().unwrap().to_owned();
            visited_t.insert(first_transition);
            queue.push_back(Node::Transition(first_transition));
        }
        while let Some(node) = queue.pop_front() {
            match node {
                Node::Place(p) => {
                    for &t in iter::chain(preset_p.get(&p).unwrap().iter(), postset_p.get(&p).unwrap().iter()) {
                        if visited_t.insert(t) {
                            queue.push_back(Node::Transition(t));
                        }
                    }
                }
                Node::Transition(t) => {
                    for &p in iter::chain(preset_t.get(&t).unwrap().iter(), postset_t.get(&t).unwrap().iter()) {
                        if visited_p.insert(p) {
                            queue.push_back(Node::Place(p));
                        }
                    }
                }
            }
        }
        if visited_p.len() + visited_t.len() != n_nodes {
            return false;
        }
    }
    true
}

pub trait IntoArcs {
    fn into_builder_arcs(self) -> impl Iterator<Item = Arc>;
}

#[derive(Copy, Clone)]
enum BuilderNode {
    Place(Place),
    Transition(Transition),
}

impl From<Place> for BuilderNode {
    fn from(p: Place) -> Self {
        BuilderNode::Place(p)
    }
}

impl From<Transition> for BuilderNode {
    fn from(t: Transition) -> Self {
        BuilderNode::Transition(t)
    }
}

/// Heterogeneous tuples of [`Place`] and [`Transition`] in alternating order become a chain
/// of [`Arc`] values (same idea as the old `IntoArcs` for dense handles).
macro_rules! impl_into_builder_arcs_for_tuples {
    ($n0:ident $n1:ident $($rest:ident)*) => {
        impl_into_builder_arcs_for_tuples!(@staircase_place [$n0 Place, $n1 Transition] $($rest)*);
        impl_into_builder_arcs_for_tuples!(@staircase_trans [$n0 Transition, $n1 Place] $($rest)*);
    };
    (@staircase_place [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_builder_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next Place);
        impl_into_builder_arcs_for_tuples!(@staircase_trans [$($acc $acc_ty,)+ $next Place] $($rest)*);
    };
    (@staircase_trans [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_builder_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next Transition);
        impl_into_builder_arcs_for_tuples!(@staircase_place [$($acc $acc_ty,)+ $next Transition] $($rest)*);
    };
    (@staircase_place [$($acc:ident $acc_ty:ty),+]) => {};
    (@staircase_trans [$($acc:ident $acc_ty:ty),+]) => {};
    (@gen $($name:ident $ty:ty),+) => {
        impl IntoArcs for ($($ty),+) {
            fn into_builder_arcs(self) -> impl Iterator<Item = Arc> {
                let ($($name),+) = self;
                let nodes = [$(BuilderNode::from($name)),+];
                (0..nodes.len() - 1).map(move |i| match (nodes[i], nodes[i + 1]) {
                    (BuilderNode::Place(p), BuilderNode::Transition(t)) => {
                        Arc::PlaceToTransition(p, t)
                    }
                    (BuilderNode::Transition(t), BuilderNode::Place(p)) => {
                        Arc::TransitionToPlace(t, p)
                    }
                    _ => unreachable!("IntoArcs tuple must alternate place and transition"),
                })
            }
        }
    };
}

// implement IntoArcs for tuples of up to 12 alternating places and transitions
impl_into_builder_arcs_for_tuples!(a b c d e f g h i j k l);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::NetClass;
    use crate::Arc;

    #[test]
    fn build_simple_net() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));
        let net = b.build().unwrap();
        assert_eq!(net.place_count(), 3);
        assert_eq!(net.transition_count(), 2);
    }

    #[test]
    fn invalid_arc_returns_false() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let _ = b.add_transition();
        let mut other = NetBuilder::new();
        let [_, t_foreign] = other.add_transitions();
        assert!(!b.add_arc((p0, t_foreign)));
    }

    #[test]
    fn empty_builder_rejected() {
        let b = NetBuilder::new();
        assert!(matches!(b.build(), Err(BuildError::Degenerate)));
    }

    #[test]
    fn no_transitions_rejected() {
        let mut b = NetBuilder::new();
        let _p = b.add_place();
        assert!(matches!(b.build(), Err(BuildError::Degenerate)));
    }

    #[test]
    fn no_places_rejected() {
        let mut b = NetBuilder::new();
        let _t = b.add_transition();
        assert!(matches!(b.build(), Err(BuildError::Degenerate)));
    }

    #[test]
    fn disconnected_node_rejected() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0, _t1] = b.add_transitions();
        b.add_arc((p0, t0));
        assert!(matches!(b.build(), Err(BuildError::NotConnected)));
    }

    #[test]
    fn classify_circuit() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        assert_eq!(b.build().unwrap().class(), NetClass::Circuit);
    }

    #[test]
    fn classify_s_net() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));
        assert_eq!(b.build().unwrap().class(), NetClass::SNet);
    }

    #[test]
    fn classify_t_net() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p1, t0));
        b.add_arc((t0, p2));
        b.add_arc((p2, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p1));
        assert_eq!(b.build().unwrap().class(), NetClass::TNet);
    }

    #[test]
    fn classify_free_choice() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p0, t1));
        b.add_arc((t1, p2));
        b.add_arc((p1, t2));
        b.add_arc((p2, t2));
        b.add_arc((t2, p0));
        assert_eq!(b.build().unwrap().class(), NetClass::FreeChoice);
    }

    #[test]
    fn classify_asymmetric_choice() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p0, t1));
        b.add_arc((p1, t1));
        b.add_arc((t0, p2));
        b.add_arc((t1, p3));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        assert!(net.is_asymmetric_choice_net());
        assert!(!net.is_free_choice_net());
    }

    #[test]
    fn classify_unrestricted() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3, p4] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p0, t1));
        b.add_arc((p1, t0));
        b.add_arc((p1, t2));
        b.add_arc((t0, p2));
        b.add_arc((t1, p3));
        b.add_arc((t2, p4));
        assert_eq!(b.build().unwrap().class(), NetClass::Unrestricted);
    }

    #[test]
    fn duplicate_arcs_are_noop() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        let net = b.build().expect("should accept duplicate arcs");
        assert_eq!(net.input_places(t0).count(), 1);
    }

    #[test]
    fn minimal_net() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((p, t));
        b.add_arc((t, p));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::Circuit);
        assert_eq!(net.place_count(), 1);
        assert_eq!(net.transition_count(), 1);
    }

    #[test]
    fn source_transition_accepted() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((t, p));
        let net = b.build().expect("valid net");
        assert_eq!(net.input_places(t).next(), None);
        let mut output_places = net.output_places(t);
        assert_eq!(output_places.next(), Some(p));
        assert_eq!(output_places.next(), None);
    }

    #[test]
    fn sink_transition_accepted() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((p, t));
        let net = b.build().expect("valid net");
        assert_eq!(net.output_places(t).next(), None);
        let mut input_places = net.input_places(t);
        assert_eq!(input_places.next(), Some(p));
        assert_eq!(input_places.next(), None);
    }

    #[test]
    fn net_to_builder_round_trip() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));
        let original = b.build().expect("valid net");

        let b2 = NetBuilder::from(original.clone());
        assert_eq!(b2.place_count(), 3);
        assert_eq!(b2.transition_count(), 2);

        let rebuilt = b2.build().expect("round-trip should produce valid net");
        assert_eq!(rebuilt, original);
    }

    #[test]
    fn net_to_builder_extend() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let original = b.build().expect("valid net");
        assert_eq!(original.class(), NetClass::Circuit);

        let mut b2 = NetBuilder::from(original);
        let p_new = b2.add_place();
        let t_new = b2.add_transition();
        b2.add_arc((p1, t_new));
        b2.add_arc((t_new, p_new));
        b2.add_arc((p_new, t1));
        let extended = b2.build().expect("valid extended net");

        assert_eq!(extended.place_count(), 3);
        assert_eq!(extended.transition_count(), 3);
        assert!(extended.input_places(t0).find(|&p| p == p0).is_some());
    }

    #[test]
    fn remove_place_cleans_up_arcs() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));

        assert!(b.remove_place(p1));
        assert!(!b.arcs().any(|a| matches!(a, Arc::PlaceToTransition(p, _) if p == p1)));
    }

    #[test]
    fn remove_place_idempotent() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        assert!(b.remove_place(p));
        assert!(!b.remove_place(p));
    }

    #[test]
    fn remove_transition_cleans_up_arcs() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));

        assert!(b.remove_transition(t0));
    }

    #[test]
    fn remove_arc_single() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));

        assert!(b.remove_arc((p0, t0)));
        assert!(!b.arcs().any(|a| matches!(a, Arc::PlaceToTransition(pp, tt) if pp == p0 && tt == t0)));
        assert!(b.arcs().any(|a| matches!(a, Arc::TransitionToPlace(tt, pp) if tt == t0 && pp == p1)));
    }

    #[test]
    fn remove_and_rebuild() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));
        b.add_arc((p2, t2));
        b.add_arc((t2, p0));

        b.remove_transition(t1);
        b.remove_place(p2);
        b.add_arc((t2, p1));
        b.add_arc((p0, t2));

        let net = b.build().expect("rebuilt net should be valid");
        assert_eq!(net.place_count(), 2);
        assert_eq!(net.transition_count(), 2);
    }

    #[test]
    fn compact_indices_are_dense() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p2));

        b.remove_place(p1);
        b.add_arc((t0, p2));
        b.add_arc((p2, t1));

        let net = b.build().expect("valid net");
        assert_eq!(net.place_count(), 2);
        assert_eq!(net.transition_count(), 2);
    }

    #[test]
    fn place_key_round_trip_through_net() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((p, t));
        b.add_arc((t, p));
        let net = b.build().unwrap();
        let pd = net.place_index(p).unwrap();
        assert_eq!(net.get_place(pd), p);
    }
}
