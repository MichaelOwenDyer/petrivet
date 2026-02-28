//! Builder for constructing Petri nets.

use std::collections::VecDeque;
use crate::net::{Arc, Net, Place, SortedSet, Transition};
use std::error::Error;
use std::{fmt, iter};
use crate::class::NetClass;
use crate::Node;

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
    /// The net does not consist of a single connected component;
    /// it has multiple contiguous subnets with no arcs between them.
    /// Each subnet should be built separately.
    NotConnected,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        let p = Place { idx: self.n_places() };
        self.preset_p.push(SortedSet::new());
        self.postset_p.push(SortedSet::new());
        p
    }

    /// Adds a single transition, returning its handle.
    pub fn add_transition(&mut self) -> Transition {
        let t = Transition { idx: self.n_transitions() };
        self.preset_t.push(SortedSet::new());
        self.postset_t.push(SortedSet::new());
        t
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

    /// Adds an arc to the net.
    ///
    /// Accepts `(Place, Transition)` or `(Transition, Place)` tuples.
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

    /// Number of places added so far.
    #[must_use]
    pub fn n_places(&self) -> usize {
        self.preset_p.len()
    }

    /// Number of transitions added so far.
    #[must_use]
    pub fn n_transitions(&self) -> usize {
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
    /// Returns [`BuildError::NotConnected`] if any place or transition has no arcs.
    pub fn build(self) -> Result<Net, BuildError> {
        let class = self.classify();
        let preset_t = self.preset_t.into_boxed_slice();
        let postset_t = self.postset_t.into_boxed_slice();
        let preset_p = self.preset_p.into_boxed_slice();
        let postset_p = self.postset_p.into_boxed_slice();

        // the net must consist of a single connected component;
        // multiple components mean multiple independent nets, so they should be built separately
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
        assert_eq!(net.n_places(), 3);
        assert_eq!(net.n_transitions(), 2);
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
        assert!(net.is_asymmetric_choice());
        assert!(!net.is_free_choice());
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
        assert_eq!(net.n_places(), 1);
        assert_eq!(net.n_transitions(), 1);
    }

    /// Source transition (no input places) — builder should accept this.
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

    /// Sink transition (no output places) — builder should accept this.
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
        assert_eq!(b2.n_places(), 3);
        assert_eq!(b2.n_transitions(), 2);

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

        assert_eq!(extended.n_places(), 3);
        assert_eq!(extended.n_transitions(), 3);
        // Original handles still work
        assert!(extended.preset_t(t0).contains(&p0));
        assert!(extended.postset_t(t0).contains(&p1));
    }
}
