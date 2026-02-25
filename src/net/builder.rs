//! Builder for constructing Petri nets.

use std::collections::VecDeque;
use crate::net::{Arc, Net, Place, Transition};
use crate::net::class::ClassifiedNet;
use std::error::Error;
use std::{fmt, iter};
use crate::Node;

/// Builder for constructing an ordinary Petri net.
///
/// # Example
///
/// ```
/// use petrivet::net::builder::NetBuilder;
///
/// let mut builder = NetBuilder::new();
/// let [p0, p1] = builder.add_places();
/// let [t0] = builder.add_transitions();
/// builder.add_arc((p0, t0));
/// builder.add_arc((t0, p1));
/// let net = builder.build().unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct NetBuilder {
    n_places: usize,
    n_transitions: usize,
    arcs: Vec<Arc>,
}

/// Errors that can occur during net construction.
#[derive(Debug)]
pub enum BuildError {
    /// A place or transition has no arcs connecting it.
    NotConnected,
    /// An arc references a place or transition that doesn't exist.
    InvalidArc,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::NotConnected => write!(f, "the net has disconnected nodes"),
            BuildError::InvalidArc => write!(f, "an arc references a non-existent place or transition"),
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

    /// Adds a single place, returning its handle.
    pub fn add_place(&mut self) -> Place {
        let p = Place { idx: self.n_places };
        self.n_places += 1;
        p
    }

    /// Adds a single transition, returning its handle.
    pub fn add_transition(&mut self) -> Transition {
        let t = Transition { idx: self.n_transitions };
        self.n_transitions += 1;
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
    pub fn add_arc<A: Into<Arc>>(&mut self, arc: A) {
        self.arcs.push(arc.into());
    }

    /// Number of places added so far.
    #[must_use]
    pub fn n_places(&self) -> usize {
        self.n_places
    }

    /// Number of transitions added so far.
    #[must_use]
    pub fn n_transitions(&self) -> usize {
        self.n_transitions
    }

    /// Consumes the builder and produces a validated, classified net.
    ///
    /// The returned [`ClassifiedNet`] carries the structural class internally
    /// so that analysis methods can dispatch to the best algorithm automatically.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::InvalidArc`] if an arc references a non-existent node.
    /// Returns [`BuildError::NotConnected`] if any place or transition has no arcs.
    pub fn build(self) -> Result<ClassifiedNet, BuildError> {
        let mut preset: Vec<Vec<Place>> = vec![Vec::new(); self.n_transitions];
        let mut postset: Vec<Vec<Place>> = vec![Vec::new(); self.n_transitions];
        let mut preset_p: Vec<Vec<Transition>> = vec![Vec::new(); self.n_places];
        let mut postset_p: Vec<Vec<Transition>> = vec![Vec::new(); self.n_places];

        // Process arcs and populate preset/postset structures.
        for arc in self.arcs {
            match arc {
                Arc::PlaceToTransition(p, t) => {
                    if p.idx >= self.n_places || t.idx >= self.n_transitions {
                        return Err(BuildError::InvalidArc);
                    }
                    preset[t.idx].push(p);
                    postset_p[p.idx].push(t);
                }
                Arc::TransitionToPlace(t, p) => {
                    if t.idx >= self.n_transitions || p.idx >= self.n_places {
                        return Err(BuildError::InvalidArc);
                    }
                    postset[t.idx].push(p);
                    preset_p[p.idx].push(t);
                }
            }
        }

        // Verify the net is weakly connected (BFS over the bipartite graph treated as undirected).
        let n_nodes = self.n_places + self.n_transitions;
        if n_nodes > 0 {
            let mut visited_p = vec![false; self.n_places].into_boxed_slice();
            let mut visited_t = vec![false; self.n_transitions].into_boxed_slice();
            let mut queue = VecDeque::new();
            if self.n_places > 0 {
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
                        for &p in iter::chain(&preset[t.idx], &postset[t.idx]) {
                            if !visited_p[p.idx] {
                                visited_p[p.idx] = true;
                                queue.push_back(Node::Place(p));
                            }
                        }
                    }
                }
            }
            if iter::chain(visited_t, visited_p).any(|v| !v) {
                return Err(BuildError::NotConnected);
            }
        }

        for v in &mut preset { v.sort_unstable_by_key(|p| p.idx); v.dedup(); }
        for v in &mut postset { v.sort_unstable_by_key(|p| p.idx); v.dedup(); }
        for v in &mut preset_p { v.sort_unstable_by_key(|t| t.idx); v.dedup(); }
        for v in &mut postset_p { v.sort_unstable_by_key(|t| t.idx); v.dedup(); }

        let net = Net {
            n_places: self.n_places,
            n_transitions: self.n_transitions,
            preset_t: preset.into_iter().map(Vec::into_boxed_slice).collect(),
            postset_t: postset.into_iter().map(Vec::into_boxed_slice).collect(),
            preset_p: preset_p.into_iter().map(Vec::into_boxed_slice).collect(),
            postset_p: postset_p.into_iter().map(Vec::into_boxed_slice).collect(),
        };

        let class = net.classify();
        Ok(ClassifiedNet::new(net, class))
    }
}

/// Consume a [`Net`] back into a builder, preserving all places, transitions,
/// and arcs. New places and transitions can then be added with indices that
/// don't collide with the originals.
impl From<Net> for NetBuilder {
    fn from(net: Net) -> Self {
        let mut arcs = Vec::new();
        for t in net.transitions() {
            for &p in net.preset_t(t) {
                arcs.push(Arc::PlaceToTransition(p, t));
            }
            for &p in net.postset_t(t) {
                arcs.push(Arc::TransitionToPlace(t, p));
            }
        }
        Self {
            n_places: net.n_places(),
            n_transitions: net.n_transitions(),
            arcs,
        }
    }
}

/// Consume a [`ClassifiedNet`] back into a builder (classification is discarded
/// since the net will be re-classified on the next `build()`).
impl From<ClassifiedNet> for NetBuilder {
    fn from(cn: ClassifiedNet) -> Self {
        cn.into_net().into()
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
    fn invalid_arc_rejected() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let _ = b.add_transition();
        let mut other = NetBuilder::new();
        let [_, t_foreign] = other.add_transitions();
        b.add_arc((p0, t_foreign));
        assert!(matches!(b.build(), Err(BuildError::InvalidArc)));
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
    fn classify_unrestricted() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2, p3] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p2));
        b.add_arc((p0, t1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p3));
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
        assert_eq!(rebuilt.net(), original.net());
        assert_eq!(rebuilt.class(), original.class());
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
