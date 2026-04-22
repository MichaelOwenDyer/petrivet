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
use crate::net::idx::DenseNet;
use crate::net::nodes::{Place, Transition};
use crate::net::{Net, Node, SortedSet};
use crate::Arc;
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
/// [`crate::net::nodes`]), so hash-based structures stay sound when mixing round-tripped and new
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

impl Default for NetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during net construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    /// The net does not have at least one place and at least one transition.
    Degenerate,
    /// The net consists of two or more disconnected components.
    NotConnected,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::Degenerate => write!(f, "the net must have at least one place and at least one transition"),
            NetError::NotConnected => write!(f, "the net must be connected (every node reachable from every other node ignoring arc directions)"),
        }
    }
}

impl Error for NetError {}

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

impl NetBuilder {
    /// Creates a new, empty builder.
    #[must_use]
    pub fn new() -> Self {
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

    /// Adds one place and returns its stable key.
    ///
    /// # Panics
    ///
    /// Panics if you try to add more than `u32::MAX` places.
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
    ///
    /// # Panics
    ///
    /// Panics if you try to add more than `u32::MAX` transitions.
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
                        let added_to_post = postset.insert(t);
                        let added_to_pre = preset.insert(p);
                        debug_assert_eq!(added_to_post, added_to_pre, "Adjacency map desynchronization detected");
                        added_to_post
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
                        let added_to_post = postset.insert(p);
                        let added_to_pre = preset.insert(t);
                        debug_assert_eq!(added_to_post, added_to_pre, "Adjacency map desynchronization detected");
                        added_to_post
                    }
                    // One or both of the nodes do not exist
                    _ => false,
                }
            }
        }
    }

    /// Adds several alternating arcs at once; see [`IntoArcs`].
    pub fn add_arcs<A: IntoArcs>(&mut self, arcs: A) -> bool {
        arcs.into_arcs().all(|a| self.add_arc(a))
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

    #[must_use]
    pub fn place_count(&self) -> usize {
        self.places.len()
    }

    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Active places.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.places.iter().copied()
    }

    /// Active transitions.
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

    /// Classify the net in its current state while it is being built.
    /// Returns a [`NetClass`] or a [`NetError`] if the net is degenerate or disconnected.
    pub fn classify(&self) -> Result<NetClass, NetError> {
        if self.place_count() == 0 || self.transition_count() == 0 {
            return Err(NetError::Degenerate);
        }

        crate::net::class::classify(
            &self.preset_t, &self.postset_t, &self.preset_p, &self.postset_p
        ).ok_or(NetError::NotConnected)
    }

    /// Consumes the builder and returns a validated [`Net`], or a [`NetError`].
    pub fn build(self) -> Result<Net, NetError> {
        let class = self.classify()?;

        // Perform bandwidth reduction using the Reverse Cuthill-McKee algorithm.
        // This is a heuristic that tries to order nodes so that arcs mostly connect nearby indices,
        // which improves cache locality for traversal and state space exploration.
        let (ordered_places, ordered_transitions) =
            compute_rcm_ordering(&self.places, &self.transitions, &self.preset_t, &self.postset_t);

        // Map public handles to dense, cache-optimized indices
        let place_to_index = ordered_places
            .iter()
            .copied()
            .zip(0..)
            .collect();

        let transition_to_index = ordered_transitions
            .iter()
            .copied()
            .zip(0..)
            .collect();

        let preset_t = map_adjacency(&self.transitions, &self.preset_t, &place_to_index);
        let postset_t = map_adjacency(&self.transitions, &self.postset_t, &place_to_index);
        let preset_p = map_adjacency(&self.places, &self.preset_p, &transition_to_index);
        let postset_p = map_adjacency(&self.places, &self.postset_p, &transition_to_index);

        let index_to_place = ordered_places.into_boxed_slice();
        let index_to_transition = ordered_transitions.into_boxed_slice();

        let dense_net = DenseNet {
            class,
            preset_t,
            postset_t,
            preset_p,
            postset_p,
        };

        Ok(Net {
            core: dense_net,
            place_to_index,
            transition_to_index,
            index_to_place,
            index_to_transition,
            labels: None, // todo: add labels builder
            graphics: None, // todo: add graphics builder
        })
    }
}

/// Converts a sparse adjacency map (from builder) to a dense
/// adjacency list (for Net) using the provided index map.
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
            let indices: Vec<Idx> = sparse_adjacency
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
            SortedSet::new(indices)
        })
        .collect()
}

/// Computes the Reverse Cuthill-McKee (RCM) ordering for a bipartite Petri net.
/// Returns a tuple of permutation arrays: `(ordered_places, ordered_transitions)`.
fn compute_rcm_ordering(
    places: &[Place],
    transitions: &[Transition],
    preset_t: &HashMap<Transition, HashSet<Place>>,
    postset_t: &HashMap<Transition, HashSet<Place>>,
) -> (Vec<Place>, Vec<Transition>) {
    let p_count = places.len();
    let t_count = transitions.len();
    let total_nodes = p_count + t_count;

    // 1. Establish dense temporary indices mapping
    // Places -> [0 .. P), Transitions -> [P .. P + T)
    let p_map: HashMap<Place, usize> = places.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    let t_map: HashMap<Transition, usize> = transitions.iter().enumerate().map(|(i, &t)| (t, i + p_count)).collect();

    // 2. Build the unified undirected adjacency list
    let mut adj = vec![Vec::new(); total_nodes];

    for (t, t_idx) in t_map {
        let mut connect = |p: &Place| {
            let p_idx = p_map[p];
            adj[t_idx].push(p_idx);
            adj[p_idx].push(t_idx);
        };

        if let Some(pre) = preset_t.get(&t) {
            pre.iter().for_each(&mut connect);
        }
        if let Some(post) = postset_t.get(&t) {
            post.iter().for_each(&mut connect);
        }
    }

    // 3. Prepare the adjacency list for RCM
    let degrees: Vec<usize> = adj.iter().map(Vec::len).collect();
    for neighbors in &mut adj {
        neighbors.sort_unstable(); // Required before dedup
        neighbors.dedup();         // Merge arcs if a place is in both •t and t•
        neighbors.sort_unstable_by_key(|&n| degrees[n]); // The critical RCM requirement
    }

    // 4. George-Liu Heuristic: Find a pseudo-peripheral starting node
    // Uses a level-synchronous BFS to easily find the node at the maximum depth,
    // explicitly breaking ties by picking the one with the minimal degree.
    let find_furthest = |start: usize| -> (usize, usize) {
        let mut current_level = vec![start];
        let mut visited = vec![false; total_nodes];
        visited[start] = true;

        let mut depth = 0;
        loop {
            let mut next_level = Vec::new();
            for &node in &current_level {
                for &neighbor in &adj[node] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        next_level.push(neighbor);
                    }
                }
            }
            if next_level.is_empty() {
                break;
            }
            current_level = next_level;
            depth += 1;
        }

        let min_degree_node = current_level.into_iter()
            .min_by_key(|&n| degrees[n])
            .unwrap_or(start);

        (min_degree_node, depth)
    };

    let mut start_node = 0;
    let mut max_dist = 0;
    loop {
        let (furthest, dist) = find_furthest(start_node);
        if dist > max_dist {
            max_dist = dist;
            start_node = furthest;
        } else {
            break;
        }
    }

    // 5. Run the standard RCM BFS
    let mut rcm_order = Vec::with_capacity(total_nodes);
    let mut visited = vec![false; total_nodes];
    let mut queue = VecDeque::with_capacity(total_nodes);

    queue.push_back(start_node);
    visited[start_node] = true;

    while let Some(node) = queue.pop_front() {
        rcm_order.push(node);
        for &neighbor in &adj[node] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }

    // Standard Cuthill-McKee goes outside-in. Reverse it to get inside-out bandwidth reduction.
    rcm_order.reverse();

    // 6. Extract and separate the relative orderings
    let mut ordered_places = Vec::with_capacity(p_count);
    let mut ordered_transitions = Vec::with_capacity(t_count);

    for idx in rcm_order {
        if idx < p_count {
            ordered_places.push(places[idx]);
        } else {
            ordered_transitions.push(transitions[idx - p_count]);
        }
    }

    (ordered_places, ordered_transitions)
}

/// Convert a built `Net` back into a `NetBuilder` for editing.
impl From<Net> for NetBuilder {
    fn from(net: Net) -> Self {
        let next_place_id = net.places()
            .map(Place::into_raw)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .expect("Node ID overflow: cannot build from a net containing u32::MAX");

        let next_transition_id = net.transitions()
            .map(Transition::into_raw)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .expect("Node ID overflow: cannot build from a net containing u32::MAX");

        let places = net.places().collect();
        let transitions = net.transitions().collect();

        let mut preset_t = net.transitions()
            .map(|t| (t, HashSet::new()))
            .collect::<HashMap<_, _>>();
        let postset_t = net.transitions()
            .map(|t| (t, HashSet::new()))
            .collect::<HashMap<_, _>>();
        let preset_p = net.places()
            .map(|p| (p, HashSet::new()))
            .collect::<HashMap<_, _>>();
        let mut postset_p = net.places()
            .map(|p| (p, HashSet::new()))
            .collect::<HashMap<_, _>>();

        for t in net.transitions() {
            let mut insert = |p| {
                preset_t.get_mut(&t).unwrap().insert(p);
                postset_p.get_mut(&p).unwrap().insert(t);
            };
            net.transition_preset(&t).for_each(&mut insert);
            net.transition_postset(&t).for_each(&mut insert);
        }

        Self {
            next_place_id,
            next_transition_id,
            places,
            transitions,
            preset_t,
            postset_t,
            preset_p,
            postset_p,
        }
    }
}

pub trait IntoArcs {
    fn into_arcs(self) -> impl Iterator<Item = Arc>;
}

/// Heterogeneous tuples of [`Place`] and [`Transition`] in alternating order become a chain
/// of [`Arc`] values (same idea as the old `IntoArcs` for dense handles).
macro_rules! impl_into_arcs_for_tuples {
    ($n0:ident $n1:ident $($rest:ident)*) => {
        impl_into_arcs_for_tuples!(@staircase_place [$n0 Place, $n1 Transition] $($rest)*);
        impl_into_arcs_for_tuples!(@staircase_trans [$n0 Transition, $n1 Place] $($rest)*);
    };
    (@staircase_place [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next Place);
        impl_into_arcs_for_tuples!(@staircase_trans [$($acc $acc_ty,)+ $next Place] $($rest)*);
    };
    (@staircase_trans [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next Transition);
        impl_into_arcs_for_tuples!(@staircase_place [$($acc $acc_ty,)+ $next Transition] $($rest)*);
    };
    (@staircase_place [$($acc:ident $acc_ty:ty),+]) => {};
    (@staircase_trans [$($acc:ident $acc_ty:ty),+]) => {};
    (@gen $($name:ident $ty:ty),+) => {
        impl IntoArcs for ($($ty),+) {
            fn into_arcs(self) -> impl Iterator<Item = Arc> {
                let ($($name),+) = self;
                let nodes = [$(Node::from($name)),+];
                (0..nodes.len() - 1).map(move |i| match (nodes[i], nodes[i + 1]) {
                    (Node::Place(p), Node::Transition(t)) => {
                        Arc::PlaceToTransition(p, t)
                    }
                    (Node::Transition(t), Node::Place(p)) => {
                        Arc::TransitionToPlace(t, p)
                    }
                    _ => unreachable!("IntoArcs tuple must alternate place and transition"),
                })
            }
        }
    };
}

// implement IntoArcs for tuples of up to 12 alternating places and transitions
impl_into_arcs_for_tuples!(a b c d e f g h i j k l);

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
        assert!(matches!(b.build(), Err(NetError::Degenerate)));
    }

    #[test]
    fn no_transitions_rejected() {
        let mut b = NetBuilder::new();
        let _p = b.add_place();
        assert!(matches!(b.build(), Err(NetError::Degenerate)));
    }

    #[test]
    fn no_places_rejected() {
        let mut b = NetBuilder::new();
        let _t = b.add_transition();
        assert!(matches!(b.build(), Err(NetError::Degenerate)));
    }

    #[test]
    fn disconnected_node_rejected() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let [t0, _t1] = b.add_transitions();
        b.add_arc((p0, t0));
        assert!(matches!(b.build(), Err(NetError::NotConnected)));
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
        assert_eq!(b.build().unwrap().class(), NetClass::StateMachine);
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
        assert_eq!(b.build().unwrap().class(), NetClass::MarkedGraph);
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
    fn classify_general() {
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
        assert_eq!(b.build().unwrap().class(), NetClass::General);
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
        assert_eq!(net.transition_preset(&t0).count(), 1);
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
        assert_eq!(net.transition_preset(&t).next(), None);
        let mut output_places = net.transition_postset(&t);
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
        assert_eq!(net.transition_postset(&t).next(), None);
        let mut input_places = net.transition_preset(&t);
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
        assert!(extended.transition_preset(&t0).find(|&p| p == p0).is_some());
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
        let p_idx = net.place_to_index[&p];
        assert_eq!(net.index_to_place[p_idx], p);
    }
}
