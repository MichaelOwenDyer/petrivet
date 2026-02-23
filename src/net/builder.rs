//! Builder for constructing Petri nets.

use crate::net::{Arc, Net, Place, Transition};
use crate::net::class::ClassifiedNet;
use std::error::Error;
use std::fmt;

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
        let p = Place(self.n_places);
        self.n_places += 1;
        p
    }

    /// Adds a single transition, returning its handle.
    pub fn add_transition(&mut self) -> Transition {
        let t = Transition(self.n_transitions);
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

        for arc in self.arcs {
            match arc {
                Arc::PlaceToTransition(p, t) => {
                    if p.0 >= self.n_places || t.0 >= self.n_transitions {
                        return Err(BuildError::InvalidArc);
                    }
                    preset[t.0].push(p);
                    postset_p[p.0].push(t);
                }
                Arc::TransitionToPlace(t, p) => {
                    if t.0 >= self.n_transitions || p.0 >= self.n_places {
                        return Err(BuildError::InvalidArc);
                    }
                    postset[t.0].push(p);
                    preset_p[p.0].push(t);
                }
            }
        }

        // Verify all nodes have at least one arc
        let all_transitions_connected = preset.iter().zip(postset.iter())
            .all(|(pre, post)| !pre.is_empty() || !post.is_empty());
        let all_places_connected = preset_p.iter().zip(postset_p.iter())
            .all(|(pre, post)| !pre.is_empty() || !post.is_empty());

        if !all_transitions_connected || !all_places_connected {
            return Err(BuildError::NotConnected);
        }

        // Verify the net is weakly connected (BFS over the bipartite graph
        // treated as undirected).
        let total = self.n_places + self.n_transitions;
        if total > 0 {
            let mut visited = vec![false; total];
            let mut queue = std::collections::VecDeque::new();
            visited[0] = true;
            queue.push_back(0);
            while let Some(node) = queue.pop_front() {
                if node < self.n_places {
                    let p = node;
                    for &t in &preset_p[p] {
                        let ti = self.n_places + t.0;
                        if !visited[ti] { visited[ti] = true; queue.push_back(ti); }
                    }
                    for &t in &postset_p[p] {
                        let ti = self.n_places + t.0;
                        if !visited[ti] { visited[ti] = true; queue.push_back(ti); }
                    }
                } else {
                    let t = node - self.n_places;
                    for &p in &preset[t] {
                        if !visited[p.0] { visited[p.0] = true; queue.push_back(p.0); }
                    }
                    for &p in &postset[t] {
                        if !visited[p.0] { visited[p.0] = true; queue.push_back(p.0); }
                    }
                }
            }
            if visited.iter().any(|&v| !v) {
                return Err(BuildError::NotConnected);
            }
        }

        for v in &mut preset { v.sort_unstable_by_key(|p| p.0); v.dedup(); }
        for v in &mut postset { v.sort_unstable_by_key(|p| p.0); v.dedup(); }
        for v in &mut preset_p { v.sort_unstable_by_key(|t| t.0); v.dedup(); }
        for v in &mut postset_p { v.sort_unstable_by_key(|t| t.0); v.dedup(); }

        let net = Net {
            n_places: self.n_places,
            n_transitions: self.n_transitions,
            preset: preset.into_iter().map(Vec::into_boxed_slice).collect(),
            postset: postset.into_iter().map(Vec::into_boxed_slice).collect(),
            preset_p: preset_p.into_iter().map(Vec::into_boxed_slice).collect(),
            postset_p: postset_p.into_iter().map(Vec::into_boxed_slice).collect(),
        };

        let class = net.classify();
        Ok(ClassifiedNet::new(net, class))
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
        assert_eq!(net.net().n_places(), 3);
        assert_eq!(net.net().n_transitions(), 2);
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
        assert_eq!(net.net().preset_t(t0).len(), 1);
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
        assert_eq!(net.net().n_places(), 1);
        assert_eq!(net.net().n_transitions(), 1);
    }

    /// Source transition (no input places) — builder should accept this.
    #[test]
    fn source_transition_accepted() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((t, p));
        let net = b.build().expect("valid net");
        assert!(net.net().preset_t(t).is_empty());
        assert_eq!(net.net().postset_t(t), &[p]);
    }

    /// Sink transition (no output places) — builder should accept this.
    #[test]
    fn sink_transition_accepted() {
        let mut b = NetBuilder::new();
        let p = b.add_place();
        let t = b.add_transition();
        b.add_arc((p, t));
        let net = b.build().expect("valid net");
        assert!(net.net().postset_t(t).is_empty());
        assert_eq!(net.net().preset_t(t), &[p]);
    }
}
