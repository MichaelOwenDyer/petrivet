//! Builder for constructing Petri nets with stable node identity.
//!
//! While you edit, places and transitions are [`PlaceKey`] and [`TransitionKey`] values minted by
//! the builder. They stay valid until you remove that node from *this* builder. When
//! you call [`NetBuilder::build`], surviving keys are assigned dense indices and the resulting
//! [`Net`] stores both directions of the mapping so you can move between keys and [`Place`] /
//! [`Transition`] without maintaining parallel tables yourself.
//!
//! [`NetBuilder::from`] rebuilds a builder from a built [`Net`] using the net’s stored keys so
//! handles remain usable across round-trips.

use crate::class::NetClass;
use crate::net::keys::{PlaceKey, TransitionKey};
use crate::net::{DenseNode, Net, Place, SortedSet, Transition};
use crate::node_map::IndexMap;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::{fmt, iter};

/// A directed arc in the flow relation while the net is still under construction.
///
/// Ordinary [`Arc`] uses dense [`Place`] / [`Transition`] handles for a built [`Net`]; this type
/// is the parallel story for [`PlaceKey`] / [`TransitionKey`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BuilderArc {
    /// An arc from a place to a transition (place → transition).
    PlaceToTransition(PlaceKey, TransitionKey),
    /// An arc from a transition to a place (transition → place).
    TransitionToPlace(TransitionKey, PlaceKey),
}

impl From<(PlaceKey, TransitionKey)> for BuilderArc {
    fn from((p, t): (PlaceKey, TransitionKey)) -> Self {
        BuilderArc::PlaceToTransition(p, t)
    }
}

impl From<(TransitionKey, PlaceKey)> for BuilderArc {
    fn from((t, p): (TransitionKey, PlaceKey)) -> Self {
        BuilderArc::TransitionToPlace(t, p)
    }
}

/// Builder for an ordinary Petri net.
///
/// Active places and transitions are listed in [`NetBuilder::place_keys`] /
/// [`NetBuilder::transition_keys`] order; that order becomes dense `0..n−1` at [`NetBuilder::build`].
/// New keys are unique numeric ids minted by this builder. [`NetBuilder::from`] seeds lists from a
/// [`Net`] so existing [`PlaceKey`] / [`TransitionKey`] handles stay valid.
///
/// Adjacency is kept in the usual four directions (each place and each transition has preset and
/// postset sets of the opposite kind’s keys), so removing a node touches only its neighbours’
/// sets.
///
/// We use [`HashMap`] for adjacency so keys from a built [`Net`] can coexist with keys minted
/// after [`NetBuilder::from`]. [`PlaceKey`] / [`TransitionKey`] are unique numeric ids (see
/// [`crate::net::keys`]), so hash-based structures stay sound when mixing round-tripped and new
/// handles.
#[derive(Debug, Clone)]
pub struct NetBuilder {
    /// Live places in iteration order (defines dense indices at build).
    places: Vec<PlaceKey>,
    /// Live transitions in iteration order (defines dense indices at build).
    transitions: Vec<TransitionKey>,
    /// Unique
    place_set: HashSet<PlaceKey>,
    transition_set: HashSet<TransitionKey>,
    /// Next unused id for [`add_place`](Self::add_place) (strictly greater than any id in
    /// [`Self::places`]).
    next_place_id: u64,
    /// Next unused id for [`add_transition`](Self::add_transition).
    next_transition_id: u64,
    /// For each transition: input places •t.
    preset_t: HashMap<TransitionKey, SortedSet<PlaceKey>>,
    /// For each transition: output places t•.
    postset_t: HashMap<TransitionKey, SortedSet<PlaceKey>>,
    /// For each place: input transitions •p.
    preset_p: HashMap<PlaceKey, SortedSet<TransitionKey>>,
    /// For each place: output transitions p•.
    postset_p: HashMap<PlaceKey, SortedSet<TransitionKey>>,
}

impl Default for NetBuilder {
    fn default() -> Self {
        Self {
            places: Vec::new(),
            transitions: Vec::new(),
            place_set: HashSet::new(),
            transition_set: HashSet::new(),
            next_place_id: 1,
            next_transition_id: 1,
            preset_t: HashMap::new(),
            postset_t: HashMap::new(),
            preset_p: HashMap::new(),
            postset_p: HashMap::new(),
        }
    }
}

/// Errors that can occur during net construction.
#[derive(Debug)]
pub enum BuildError {
    /// The net has no places or no transitions.
    Empty,
    /// The net has more than one weakly connected component (ignoring arc direction).
    NotConnected,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Empty => write!(f, "the net must have at least one place and one transition"),
            BuildError::NotConnected => write!(f, "the net has disconnected nodes"),
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
    pub fn add_place(&mut self) -> PlaceKey {
        let id = self.next_place_id;
        self.next_place_id = self
            .next_place_id
            .checked_add(1)
            .expect("place key id overflow");
        let pk = PlaceKey::from_raw(id);
        self.places.push(pk);
        self.place_set.insert(pk);
        self.preset_p.insert(pk, SortedSet::new());
        self.postset_p.insert(pk, SortedSet::new());
        pk
    }

    /// Adds `N` places and returns their keys.
    pub fn add_places<const N: usize>(&mut self) -> [PlaceKey; N] {
        std::array::from_fn(|_| self.add_place())
    }

    /// Adds one transition and returns its stable key.
    pub fn add_transition(&mut self) -> TransitionKey {
        let id = self.next_transition_id;
        self.next_transition_id = self
            .next_transition_id
            .checked_add(1)
            .expect("transition key id overflow");
        let tk = TransitionKey::from_raw(id);
        self.transitions.push(tk);
        self.transition_set.insert(tk);
        self.preset_t.insert(tk, SortedSet::new());
        self.postset_t.insert(tk, SortedSet::new());
        tk
    }

    /// Adds `N` transitions and returns their keys.
    pub fn add_transitions<const N: usize>(&mut self) -> [TransitionKey; N] {
        std::array::from_fn(|_| self.add_transition())
    }

    /// Removes a place and every arc incident on it. Returns `false` if that key was not active.
    pub fn remove_place(&mut self, place: PlaceKey) -> bool {
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
        self.place_set.remove(&place);
        if let Some(pos) = self.places.iter().position(|&k| k == place) {
            self.places.swap_remove(pos);
        }
        true
    }

    /// Removes a transition and every arc incident on it.
    pub fn remove_transition(&mut self, transition: TransitionKey) -> bool {
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
        self.transition_set.remove(&transition);
        if let Some(pos) = self.transitions.iter().position(|&k| k == transition) {
            self.transitions.swap_remove(pos);
        }
        true
    }

    /// Removes a directed arc if it exists.
    pub fn remove_arc<A: Into<BuilderArc>>(&mut self, arc: A) -> bool {
        match arc.into() {
            BuilderArc::PlaceToTransition(p, t) => {
                let removed = self
                    .preset_t
                    .get_mut(&t)
                    .is_some_and(|s| s.remove(&p));
                if removed {
                    self.postset_p.get_mut(&p).unwrap().remove(&t);
                }
                removed
            }
            BuilderArc::TransitionToPlace(t, p) => {
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
    pub fn add_arc<A: Into<BuilderArc>>(&mut self, arc: A) -> bool {
        let arc = arc.into();
        match arc {
            BuilderArc::PlaceToTransition(p, t) => {
                let p_postset = self.postset_p.get_mut(&p);
                let t_preset = self.preset_t.get_mut(&t);
                match (p_postset, t_preset) {
                    (Some(postset), Some(preset)) => {
                        postset.add(t) && preset.add(p)
                    }
                    // One or both of the nodes do not exist
                    _ => false,
                }
            }
            BuilderArc::TransitionToPlace(t, p) => {
                let t_postset = self.postset_t.get_mut(&t);
                let p_preset = self.preset_p.get_mut(&p);
                match (t_postset, p_preset) {
                    (Some(postset), Some(preset)) => {
                        postset.add(p) && preset.add(t)
                    }
                    // One or both of the nodes do not exist
                    _ => false,
                }
            }
        }
    }

    /// Adds several alternating arcs at once; see [`IntoBuilderArcs`].
    pub fn add_arcs<A: IntoBuilderArcs>(&mut self, arcs: A) -> bool {
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

    /// Active place keys.
    #[must_use]
    pub fn place_keys(&self) -> impl Iterator<Item = PlaceKey> + '_ {
        self.places.iter().copied()
    }

    /// Active transition keys.
    #[must_use]
    pub fn transition_keys(&self) -> impl Iterator<Item = TransitionKey> + '_ {
        self.transitions.iter().copied()
    }

    /// Iterates every directed arc currently in the builder.
    pub fn arcs(&self) -> impl Iterator<Item = BuilderArc> + '_ {
        iter::chain(
            self.preset_t.iter().flat_map(|(t, preset)| {
                preset.iter().map(move |&p| BuilderArc::PlaceToTransition(p, *t))
            }),
            self.postset_t.iter().flat_map(|(t, post)| {
                post.iter().map(move |&p| BuilderArc::TransitionToPlace(*t, p))
            }),
        )
    }

    #[must_use]
    pub fn has_place(&self, place: PlaceKey) -> bool {
        self.place_set.contains(&place)
    }

    #[must_use]
    pub fn has_transition(&self, transition: TransitionKey) -> bool {
        self.transition_set.contains(&transition)
    }

    #[must_use]
    pub fn classify(&self) -> NetClass {
        if self.place_count() == 0 || self.transition_count() == 0 {
            return NetClass::Unrestricted;
        }
        let DenseAdjacencyMaps {
            preset_t,
            postset_t,
            preset_p,
            postset_p
        } = dense_adjacency_maps(self);
        crate::net::class::classify(&preset_t, &postset_t, &preset_p, &postset_p)
    }

    /// Consumes the builder and returns a validated [`Net`], or a [`BuildError`].
    pub fn build(self) -> Result<Net, BuildError> {
        let place_count = self.place_count();
        let transition_count = self.transition_count();
        if place_count == 0 || transition_count == 0 {
            return Err(BuildError::Empty);
        }

        let place_key_to_dense: HashMap<PlaceKey, Place> = self
            .places
            .iter()
            .enumerate()
            .map(|(i, &pk)| (pk, Place::from_index(i as u32)))
            .collect();

        let transition_key_to_dense: HashMap<TransitionKey, Transition> = self
            .transitions
            .iter()
            .enumerate()
            .map(|(i, &tk)| (tk, Transition::from_index(i as u32)))
            .collect();

        let preset_t = map_transition_adjacency(
            &self,
            &place_key_to_dense,
            |b, tk| b.preset_t.get(&tk).expect("preset_t entry for transition")
        );
        let postset_t = map_transition_adjacency(
            &self,
            &place_key_to_dense,
            |b, tk| b.postset_t.get(&tk).expect("postset_t entry for transition")
        );
        let preset_p = map_place_adjacency(
            &self,
            &transition_key_to_dense,
            |b, pk| b.preset_p.get(&pk).expect("preset_p entry for place")
        );
        let postset_p = map_place_adjacency(
            &self,
            &transition_key_to_dense,
            |b, pk| b.postset_p.get(&pk).expect("postset_p entry for place")
        );

        if !is_connected(&preset_t, &postset_t, &preset_p, &postset_p) {
            return Err(BuildError::NotConnected);
        }

        let class = crate::net::class::classify(&preset_t, &postset_t, &preset_p, &postset_p);

        let mut place_key_for_index = IndexMap::new(place_count);
        for (i, &pk) in self.places.iter().enumerate() {
            place_key_for_index[Place::from_index(i as u32)] = pk;
        }
        let mut transition_key_for_index = IndexMap::new(transition_count);
        for (i, &tk) in self.transitions.iter().enumerate() {
            transition_key_for_index[Transition::from_index(i as u32)] = tk;
        }

        Ok(Net {
            class,
            preset_t,
            postset_t,
            preset_p,
            postset_p,
            place_index_for_key: place_key_to_dense,
            transition_index_for_key: transition_key_to_dense,
            place_key_for_index,
            transition_key_for_index,
        })
    }
}

impl From<Net> for NetBuilder {
    fn from(net: Net) -> Self {
        let place_count = net.place_count() as usize;
        let transition_count = net.transition_count() as usize;

        let mut places = Vec::with_capacity(place_count);
        let mut place_set = HashSet::with_capacity(place_count);
        for i in 0..place_count {
            let pk = net.place_key(Place::from_index(i as u32));
            places.push(pk);
            place_set.insert(pk);
        }

        let mut transitions = Vec::with_capacity(transition_count);
        let mut transition_set = HashSet::with_capacity(transition_count);
        for i in 0..transition_count {
            let tk = net.transition_key(Transition::from_index(i as u32));
            transitions.push(tk);
            transition_set.insert(tk);
        }

        let mut preset_t = HashMap::new();
        let mut postset_t = HashMap::new();
        let mut preset_p = HashMap::new();
        let mut postset_p = HashMap::new();

        for &tk in &transitions {
            preset_t.insert(tk, SortedSet::new());
            postset_t.insert(tk, SortedSet::new());
        }
        for &pk in &places {
            preset_p.insert(pk, SortedSet::new());
            postset_p.insert(pk, SortedSet::new());
        }

        for (ti, &tk) in transitions.iter().enumerate() {
            let dt = Transition::from_index(ti as u32);
            for &p in net.dense_input_places(dt) {
                let pk = places[p.idx as usize];
                preset_t.get_mut(&tk).expect("preset_t row").add(pk);
                postset_p.get_mut(&pk).expect("postset_p row").add(tk);
            }
            for &p in net.dense_output_places(dt) {
                let pk = places[p.idx as usize];
                postset_t.get_mut(&tk).expect("postset_t row").add(pk);
                preset_p.get_mut(&pk).expect("preset_p row").add(tk);
            }
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
            place_set,
            transition_set,
            next_place_id,
            next_transition_id,
            preset_t,
            postset_t,
            preset_p,
            postset_p,
        }
    }
}

fn map_transition_adjacency(
    builder: &NetBuilder,
    place_idx: &HashMap<PlaceKey, Place>,
    get: impl Fn(&NetBuilder, TransitionKey) -> &SortedSet<PlaceKey>,
) -> IndexMap<Transition, SortedSet<Place>> {
    let n = builder.transition_count();
    let mut out: IndexMap<Transition, SortedSet<Place>> = IndexMap::new(n);
    for (ti, &tk) in builder.transitions.iter().enumerate() {
        let t = Transition::from_index(ti as u32);
        for &pk in get(builder, tk).iter() {
            out[t].add(*place_idx.get(&pk).expect("place key in dense map"));
        }
    }
    out
}

fn map_place_adjacency(
    builder: &NetBuilder,
    trans_idx: &HashMap<TransitionKey, Transition>,
    get: impl Fn(&NetBuilder, PlaceKey) -> &SortedSet<TransitionKey>,
) -> IndexMap<Place, SortedSet<Transition>> {
    let n = builder.place_count();
    let mut out: IndexMap<Place, SortedSet<Transition>> = IndexMap::new(n);
    for (pi, &pk) in builder.places.iter().enumerate() {
        let p = Place::from_index(pi as u32);
        for &tk in get(builder, pk) {
            out[p].add(*trans_idx.get(&tk).expect("transition key in dense map"));
        }
    }
    out
}

struct DenseAdjacencyMaps {
    preset_t: IndexMap<Transition, SortedSet<Place>>,
    postset_t: IndexMap<Transition, SortedSet<Place>>,
    preset_p: IndexMap<Place, SortedSet<Transition>>,
    postset_p: IndexMap<Place, SortedSet<Transition>>,
}

fn dense_adjacency_maps(
    builder: &NetBuilder,
) -> DenseAdjacencyMaps {
    let place_key_to_dense: HashMap<PlaceKey, Place> = builder
        .places
        .iter()
        .enumerate()
        .map(|(i, &pk)| (pk, Place::from_index(i as u32)))
        .collect();

    let transition_key_to_dense: HashMap<TransitionKey, Transition> = builder
        .transitions
        .iter()
        .enumerate()
        .map(|(i, &tk)| (tk, Transition::from_index(i as u32)))
        .collect();

    let preset_t = map_transition_adjacency(builder, &place_key_to_dense, |b, tk| {
        b.preset_t.get(&tk).expect("preset_t entry for transition")
    });
    let postset_t = map_transition_adjacency(builder, &place_key_to_dense, |b, tk| {
        b.postset_t.get(&tk).expect("postset_t entry for transition")
    });
    let preset_p = map_place_adjacency(builder, &transition_key_to_dense, |b, pk| {
        b.preset_p.get(&pk).expect("preset_p entry for place")
    });
    let postset_p = map_place_adjacency(builder, &transition_key_to_dense, |b, pk| {
        b.postset_p.get(&pk).expect("postset_p entry for place")
    });

    DenseAdjacencyMaps { preset_t, postset_t, preset_p, postset_p }
}

fn is_connected(
    preset_t: &IndexMap<Transition, SortedSet<Place>>,
    postset_t: &IndexMap<Transition, SortedSet<Place>>,
    preset_p: &IndexMap<Place, SortedSet<Transition>>,
    postset_p: &IndexMap<Place, SortedSet<Transition>>,
) -> bool {
    let n_places = preset_p.len();
    let n_transitions = preset_t.len();
    let n_nodes = n_places + n_transitions;
    if n_nodes > 0 {
        let mut visited_p = vec![false; n_places].into_boxed_slice();
        let mut visited_t = vec![false; n_transitions].into_boxed_slice();
        let mut queue = VecDeque::new();
        if n_places > 0 {
            visited_p[0] = true;
            queue.push_back(DenseNode::Place(Place::from_index(0)));
        } else {
            visited_t[0] = true;
            queue.push_back(DenseNode::Transition(Transition::from_index(0)));
        }
        while let Some(node) = queue.pop_front() {
            match node {
                DenseNode::Place(p) => {
                    for &t in iter::chain(&preset_p[p], &postset_p[p]) {
                        if !visited_t[t.idx as usize] {
                            visited_t[t.idx as usize] = true;
                            queue.push_back(DenseNode::Transition(t));
                        }
                    }
                }
                DenseNode::Transition(t) => {
                    for &p in iter::chain(&preset_t[t], &postset_t[t]) {
                        if !visited_p[p.idx as usize] {
                            visited_p[p.idx as usize] = true;
                            queue.push_back(DenseNode::Place(p));
                        }
                    }
                }
            }
        }
        if iter::chain(visited_t.iter(), visited_p.iter()).any(|v| !*v) {
            return false;
        }
    }
    true
}

pub trait IntoBuilderArcs {
    fn into_builder_arcs(self) -> impl Iterator<Item = BuilderArc>;
}

#[derive(Copy, Clone)]
enum BuilderNode {
    Place(PlaceKey),
    Transition(TransitionKey),
}

impl From<PlaceKey> for BuilderNode {
    fn from(p: PlaceKey) -> Self {
        BuilderNode::Place(p)
    }
}

impl From<TransitionKey> for BuilderNode {
    fn from(t: TransitionKey) -> Self {
        BuilderNode::Transition(t)
    }
}

/// Heterogeneous tuples of [`PlaceKey`] and [`TransitionKey`] in alternating order become a chain
/// of [`BuilderArc`] values (same idea as the old `IntoArcs` for dense handles).
macro_rules! impl_into_builder_arcs_for_tuples {
    ($n0:ident $n1:ident $($rest:ident)*) => {
        impl_into_builder_arcs_for_tuples!(@staircase_place [$n0 PlaceKey, $n1 TransitionKey] $($rest)*);
        impl_into_builder_arcs_for_tuples!(@staircase_trans [$n0 TransitionKey, $n1 PlaceKey] $($rest)*);
    };
    (@staircase_place [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_builder_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next PlaceKey);
        impl_into_builder_arcs_for_tuples!(@staircase_trans [$($acc $acc_ty,)+ $next PlaceKey] $($rest)*);
    };
    (@staircase_trans [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_builder_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next TransitionKey);
        impl_into_builder_arcs_for_tuples!(@staircase_place [$($acc $acc_ty,)+ $next TransitionKey] $($rest)*);
    };
    (@staircase_place [$($acc:ident $acc_ty:ty),+]) => {};
    (@staircase_trans [$($acc:ident $acc_ty:ty),+]) => {};
    (@gen $($name:ident $ty:ty),+) => {
        impl IntoBuilderArcs for ($($ty),+) {
            fn into_builder_arcs(self) -> impl Iterator<Item = BuilderArc> {
                let ($($name),+) = self;
                let nodes = [$(BuilderNode::from($name)),+];
                (0..nodes.len() - 1).map(move |i| match (nodes[i], nodes[i + 1]) {
                    (BuilderNode::Place(p), BuilderNode::Transition(t)) => {
                        BuilderArc::PlaceToTransition(p, t)
                    }
                    (BuilderNode::Transition(t), BuilderNode::Place(p)) => {
                        BuilderArc::TransitionToPlace(t, p)
                    }
                    _ => unreachable!("IntoBuilderArcs tuple must alternate place and transition"),
                })
            }
        }
    };
}

impl_into_builder_arcs_for_tuples!(a b c d e f g h i j k l);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::NetClass;

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
        assert!(matches!(b.build(), Err(BuildError::Empty)));
    }

    #[test]
    fn no_transitions_rejected() {
        let mut b = NetBuilder::new();
        let _p = b.add_place();
        assert!(matches!(b.build(), Err(BuildError::Empty)));
    }

    #[test]
    fn no_places_rejected() {
        let mut b = NetBuilder::new();
        let _t = b.add_transition();
        assert!(matches!(b.build(), Err(BuildError::Empty)));
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
        assert!(!b.has_place(p1));
        assert!(!b.arcs().any(|a| matches!(a, BuilderArc::PlaceToTransition(p, _) if p == p1)));
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
        assert!(!b.has_transition(t0));
    }

    #[test]
    fn remove_arc_single() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));

        assert!(b.remove_arc((p0, t0)));
        assert!(!b.arcs().any(|a| matches!(a, BuilderArc::PlaceToTransition(pp, tt) if pp == p0 && tt == t0)));
        assert!(b.arcs().any(|a| matches!(a, BuilderArc::TransitionToPlace(tt, pp) if tt == t0 && pp == p1)));
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
        let pd = net.dense_place(p);
        assert_eq!(net.place_key(pd), p);
    }
}
