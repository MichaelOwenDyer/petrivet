//! Builder for constructing Petri nets.

use crate::class::NetClass;
use crate::net::{Arc, Net, Place, SortedSet, Transition};
use crate::Node;
use std::collections::VecDeque;
use std::error::Error;
use std::{fmt, iter};

/// Builder for constructing an ordinary Petri net.
///
/// # Example
///
/// ```
/// use petrivet::net::builder::NetBuilder;
///
/// let mut net = NetBuilder::new();
/// let [p0, p1] = net.add_places();
/// let [t0] = net.add_transitions();
/// net.add_arc((p0, t0));
/// net.add_arc((t0, p1));
/// let net = net.build().unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct NetBuilder {
    preset_t: Vec<SortedSet<Place>>,
    postset_t: Vec<SortedSet<Place>>,
    preset_p: Vec<SortedSet<Transition>>,
    postset_p: Vec<SortedSet<Transition>>,
}

/// Errors that can occur during net construction.
#[derive(Debug)]
pub enum BuildError {
    /// The net has no places or no transitions.
    ///
    /// A Petri net requires at least one place and at least one transition
    /// for meaningful analysis. Linear algebra on the incidence matrix and
    /// most structural/behavioral theorems assume this.
    ///
    /// Reference: [Primer, Provision 4.5](crate::literature#provision-45--no-empty-petri-nets).
    Empty,
    /// The net does not consist of a single connected component;
    /// it has multiple contiguous subnets with no arcs between them.
    /// Each subnet should be built separately.
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

    /// Creates a new builder with pre-allocated capacity for the given number of places and transitions.
    #[must_use]
    pub fn with_capacity(n_places: usize, n_transitions: usize) -> Self {
        Self {
            preset_t: vec![SortedSet::new(); n_transitions],
            postset_t: vec![SortedSet::new(); n_transitions],
            preset_p: vec![SortedSet::new(); n_places],
            postset_p: vec![SortedSet::new(); n_places],
        }
    }

    /// Adds a single place, returning its handle.
    pub fn add_place(&mut self) -> Place {
        let p = Place { idx: self.place_count() };
        self.preset_p.push(SortedSet::new());
        self.postset_p.push(SortedSet::new());
        p
    }

    /// Adds N places, returning an array of handles.
    ///
    /// ```
    /// # use petrivet::net::builder::NetBuilder;
    /// let mut b = NetBuilder::new();
    /// let [p0, p1, p2] = b.add_places();
    /// ```
    pub fn add_places<const N: usize>(&mut self) -> [Place; N] {
        std::array::from_fn(|_| self.add_place())
    }

    /// Adds a single transition, returning its handle.
    pub fn add_transition(&mut self) -> Transition {
        let t = Transition { idx: self.transition_count() };
        self.preset_t.push(SortedSet::new());
        self.postset_t.push(SortedSet::new());
        t
    }

    /// Adds N transitions, returning an array of handles.
    ///
    /// ```
    /// # use petrivet::net::builder::NetBuilder;
    /// let mut b = NetBuilder::new();
    /// let [t0, t1] = b.add_transitions();
    /// ```
    pub fn add_transitions<const N: usize>(&mut self) -> [Transition; N] {
        std::array::from_fn(|_| self.add_transition())
    }

    /// Adds an arc to the net, if it is not already present.
    ///
    /// Accepts `(Place, Transition)` or `(Transition, Place)` tuples.
    ///
    /// # Returns
    ///
    /// True if the arc was added; false if it was already present.
    ///
    /// ```
    /// # use petrivet::net::builder::NetBuilder;
    /// let mut b = NetBuilder::new();
    /// let p0 = b.add_place();
    /// let t0 = b.add_transition();
    /// b.add_arc((p0, t0));  // place → transition
    /// b.add_arc((t0, p0));  // transition → place
    /// ```
    ///
    /// # Panics
    ///
    /// Will panic if the arc references a place or transition with an out-of-bounds index,
    /// e.g. if you use a handle from a different builder or add an arc before adding the referenced nodes.
    pub fn add_arc<A: Into<Arc>>(&mut self, arc: A) -> bool {
        let arc = arc.into();
        match arc {
            Arc::PlaceToTransition(p, t) => {
                if !self.preset_t[t.idx].add(p) {
                    return false; // arc already exists
                }
                self.postset_p[p.idx].add(t);
            }
            Arc::TransitionToPlace(t, p) => {
                if !self.postset_t[t.idx].add(p) {
                    return false; // arc already exists
                }
                self.preset_p[p.idx].add(t);
            }
        }
        true
    }

    /// Adds several arcs to the net at once, if they are not already present.
    ///
    /// Accepts tuples of alternating places and transitions, e.g.
    /// `(Place, Transition, Place)` or `(Transition, Place, Transition, Place, Transition)`.
    ///
    /// # Returns
    ///
    /// True if all arcs were added; false if any was already present.
    ///
    /// ```
    /// # use petrivet::net::builder::NetBuilder;
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arcs((p0, t0, p1));
    /// b.add_arcs((p0, t1, p1));
    /// ```
    ///
    /// # Panics
    ///
    /// Will panic if the arc references a place or transition with an out-of-bounds index,
    /// e.g. if you use a handle from a different builder or add an arc before adding the referenced nodes.
    pub fn add_arcs<A: IntoArcs>(&mut self, arcs: A) -> bool {
        arcs.into_arcs().all(|arc| self.add_arc(arc))
    }

    /// Number of places added so far.
    #[must_use]
    pub fn place_count(&self) -> usize {
        self.preset_p.len()
    }

    /// Number of transitions added so far.
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.preset_t.len()
    }

    /// Computes the structural class of the net being built in its current state.
    #[must_use]
    pub fn classify(&self) -> NetClass {
        crate::net::class::classify(
            &self.preset_t,
            &self.postset_t,
            &self.preset_p,
            &self.postset_p
        )
    }

    /// Consumes the builder and produces a validated net.
    ///
    /// The structural class is computed once and cached on the returned [`Net`],
    /// which provides insight into the optimal analysis algorithms to apply.
    ///
    /// # Errors
    ///
    /// - [`BuildError::Empty`] if the builder has zero places or zero transitions.
    /// - [`BuildError::NotConnected`] if any place or transition has no arcs.
    pub fn build(self) -> Result<Net, BuildError> {
        if self.place_count() == 0 || self.transition_count() == 0 {
            return Err(BuildError::Empty);
        }

        let class = self.classify();
        let preset_t = self.preset_t.into_boxed_slice();
        let postset_t = self.postset_t.into_boxed_slice();
        let preset_p = self.preset_p.into_boxed_slice();
        let postset_p = self.postset_p.into_boxed_slice();

        if !is_connected(&preset_t, &postset_t, &preset_p, &postset_p) {
            return Err(BuildError::NotConnected);
        }

        Ok(Net {
            class,
            preset_t,
            postset_t,
            preset_p,
            postset_p
        })
    }
}

/// Checks that the net is a single connected component,
/// i.e. every place and transition is reachable from every other via some path of arcs,
/// ignoring direction.
/// A net with multiple connected components should be built as multiple separate nets.
fn is_connected(
    preset_t: &[SortedSet<Place>],
    postset_t: &[SortedSet<Place>],
    preset_p: &[SortedSet<Transition>],
    postset_p: &[SortedSet<Transition>],
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
            queue.push_back(Node::Place(Place { idx: 0 }));
        } else {
            visited_t[0] = true;
            queue.push_back(Node::Transition(Transition { idx: 0 }));
        }
        while let Some(node) = queue.pop_front() {
            match node {
                Node::Place(p) => {
                    let idx = p.idx;
                    for &t in iter::chain(&preset_p[idx], &postset_p[idx]) {
                        if !visited_t[t.idx] {
                            visited_t[t.idx] = true;
                            queue.push_back(Node::Transition(t));
                        }
                    }
                }
                Node::Transition(t) => {
                    for &p in iter::chain(&preset_t[t.idx], &postset_t[t.idx]) {
                        if !visited_p[p.idx] {
                            visited_p[p.idx] = true;
                            queue.push_back(Node::Place(p));
                        }
                    }
                }
            }
        }
        if iter::chain(visited_t, visited_p).any(|v| !v) {
            return false;
        }
    }
    true
}

pub trait IntoArcs {
    fn into_arcs(self) -> impl Iterator<Item = Arc>;
}

/// Generates [`IntoArcs`] implementations for alternating `Place`/`Transition` tuples.
///
/// Provide a pool of N identifiers to generate impls for all tuple lengths from 3 to N,
/// for both Place-first `(Place, Transition, Place, ...)` and Transition-first
/// `(Transition, Place, Transition, ...)` sequences.
///
/// Each tuple element is converted to a [`Node`], and adjacent pairs are matched into
/// [`Arc`]s — the same idea as [`slice::windows`] but over a heterogeneous tuple that
/// has been projected into a homogeneous `[Node; N]` array.
macro_rules! impl_into_arcs_for_tuples {
    // Entry: bootstrap both Place-first and Transition-first staircases.
    ($n0:ident $n1:ident $($rest:ident)*) => {
        impl_into_arcs_for_tuples!(@staircase_place [$n0 Place, $n1 Transition] $($rest)*);
        impl_into_arcs_for_tuples!(@staircase_trans [$n0 Transition, $n1 Place] $($rest)*);
    };

    // Extend accumulator by one Place, emit impl, recurse.
    (@staircase_place [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next Place);
        impl_into_arcs_for_tuples!(@staircase_trans [$($acc $acc_ty,)+ $next Place] $($rest)*);
    };

    // Extend accumulator by one Transition, emit impl, recurse.
    (@staircase_trans [$($acc:ident $acc_ty:ty),+] $next:ident $($rest:ident)*) => {
        impl_into_arcs_for_tuples!(@gen $($acc $acc_ty,)+ $next Transition);
        impl_into_arcs_for_tuples!(@staircase_place [$($acc $acc_ty,)+ $next Transition] $($rest)*);
    };

    // Base: identifier pool exhausted.
    (@staircase_place [$($acc:ident $acc_ty:ty),+]) => {};
    (@staircase_trans [$($acc:ident $acc_ty:ty),+]) => {};

    // Core: emit a single `IntoArcs` impl from a typed identifier list.
    (@gen $($name:ident $ty:ty),+) => {
        impl IntoArcs for ($($ty),+) {
            fn into_arcs(self) -> impl Iterator<Item = Arc> {
                let ($($name),+) = self;
                let nodes = [$(Node::from($name)),+];
                (0..nodes.len() - 1).map(move |i| match (nodes[i], nodes[i + 1]) {
                    (Node::Place(p), Node::Transition(t)) => Arc::PlaceToTransition(p, t),
                    (Node::Transition(t), Node::Place(p)) => Arc::TransitionToPlace(t, p),
                    _ => unreachable!(),
                })
            }
        }
    };
}

impl_into_arcs_for_tuples!(a b c d e f g h i j k l);

/// Consume a [`Net`] back into a builder, preserving all places, transitions,
/// and arcs. The resulting builder can be extended with new nodes and arcs before building a new net.
impl From<Net> for NetBuilder {
    fn from(net: Net) -> Self {
        Self {
            preset_t: net.preset_t.into_vec(),
            postset_t: net.postset_t.into_vec(),
            preset_p: net.preset_p.into_vec(),
            postset_p: net.postset_p.into_vec(),
        }
    }
}


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
    #[should_panic(expected = "index out of bounds")]
    fn invalid_arc_panics() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let _ = b.add_transition();
        let mut other = NetBuilder::new();
        let [_, t_foreign] = other.add_transitions();
        b.add_arc((p0, t_foreign));
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
        // p0 feeds both t0 and t1; p1 feeds only t1.
        // p0• = {t0, t1}, p1• = {t1}. p1• ⊆ p0•, so AC but not FC.
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
        // Symmetric conflict: p0• = {t0, t1}, p1• = {t0, t2}.
        // Intersection {t0} is non-empty but neither is a subset of the other.
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

    /// Duplicate arcs are silently deduplicated (no-op on second add).
    #[test]
    fn duplicate_arcs_are_noop() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        let net = b.build().expect("should accept duplicate arcs");
        assert_eq!(net.preset_t(t0).len(), 1);
    }

    /// Single place, single transition, one arc each direction.
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

    /// Source transition (no input places) - builder should accept this.
    #[test]
    fn source_transition_accepted() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((t, p));
        let net = b.build().expect("valid net");
        assert!(net.preset_t(t).is_empty());
        assert_eq!(net.postset_t(t), &[p]);
    }

    /// Sink transition (no output places) - builder should accept this.
    #[test]
    fn sink_transition_accepted() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((p, t));
        let net = b.build().expect("valid net");
        assert!(net.postset_t(t).is_empty());
        assert_eq!(net.preset_t(t), &[p]);
    }

    /// Round-trip: `Net` → `NetBuilder` → `Net` preserves structure.
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

    /// Net → Builder → extend → build produces a valid augmented net.
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
        // Original handles still work
        assert!(extended.preset_t(t0).contains(&p0));
        assert!(extended.postset_t(t0).contains(&p1));
    }
}
