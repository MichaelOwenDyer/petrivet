//! Human-readable labels and metadata for Petri net elements.
//!
//! [`NetLabels`] is a companion to [`Net`] and [`System`] that holds
//! human-readable names, optional identifiers, and other presentational
//! metadata for the net's places, transitions, and arcs. It is deliberately
//! kept separate from the structural and behavioral types so that analysis
//! code never pays for metadata it does not need.
//!
//! # Usage
//!
//! ```rust
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::labeled::NetLabels;
//!
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arcs((idle, start, busy, finish, idle));
//! let net = b.build().unwrap();
//!
//! let mut labels = NetLabels::new(&net);
//! labels
//!     .set_place_name(idle, "Idle")
//!     .set_place_name(busy, "Busy")
//!     .set_transition_name(start, "Start")
//!     .set_transition_name(finish, "Finish")
//!     .set_net_name("Producer-consumer");
//!
//! assert_eq!(labels.place_name(idle), Some("Idle"));
//! assert_eq!(labels.transition_name(finish), Some("Finish"));
//! ```

use crate::net::{Arc, Net, Place, PlaceKey, PlaceMap, Transition, TransitionKey, TransitionMap};
use std::collections::HashMap;

/// Human-readable labels and metadata for the elements of a single Petri net.
///
/// Labels are purely presentational: they have no effect on structural
/// classification, reachability analysis, or simulation. The struct is
/// intentionally decoupled from [`Net`](crate::net::Net) and
/// [`System`](crate::system::System) — callers hold the three values
/// independently and compose them as needed.
///
/// # Construction
///
/// Build directly with [`NetLabels::new`] and set individual labels via the
/// fluent setter methods, or obtain a fully-populated instance from a PNML
/// document via `pnml::Net::to_pt_system` (requires the `pnml` feature).
#[derive(Debug, Clone, Default)]
pub struct NetLabels {
    place_names: PlaceMap<Option<String>>,
    place_ids: PlaceMap<Option<String>>,
    transition_names: TransitionMap<Option<String>>,
    transition_ids: TransitionMap<Option<String>>,

    /// Per-arc human-readable name (sparse; most arcs are unnamed).
    arc_names: HashMap<Arc, String>,
    /// Per-arc stable identifier (sparse).
    arc_ids: HashMap<Arc, String>,

    /// Optional name for the net as a whole.
    net_name: Option<String>,
    /// Optional stable identifier for the net (e.g. original PNML `net id`).
    net_id: Option<String>,
    /// Optional free-text description of the net.
    net_description: Option<String>,

    /// Key-to-dense-index maps cloned from the net at construction time.
    /// Allow key-based lookup without requiring a `&Net` reference.
    place_key_to_dense: HashMap<PlaceKey, Place>,
    transition_key_to_dense: HashMap<TransitionKey, Transition>,
    /// Dense-to-key reverse maps for the named-key iterators.
    place_dense_to_key: HashMap<Place, PlaceKey>,
    transition_dense_to_key: HashMap<Transition, TransitionKey>,
}

impl NetLabels {
    /// Creates an empty label set sized for the given net. All per-node labels
    /// start as `None`. Stores an internal copy of the key→index mapping so
    /// that all subsequent accessors work with [`PlaceKey`]/[`TransitionKey`]
    /// handles directly.
    #[must_use]
    pub fn new(net: &Net) -> Self {
        let place_key_to_dense = net.place_key_map().clone();
        let transition_key_to_dense = net.transition_key_map().clone();
        let place_dense_to_key = place_key_to_dense.iter().map(|(&k, &p)| (p, k)).collect();
        let transition_dense_to_key = transition_key_to_dense.iter().map(|(&k, &t)| (t, k)).collect();
        Self {
            place_names: PlaceMap::new(net.place_count() as usize),
            place_ids: PlaceMap::new(net.place_count() as usize),
            transition_names: TransitionMap::new(net.transition_count() as usize),
            transition_ids: TransitionMap::new(net.transition_count() as usize),
            place_key_to_dense,
            transition_key_to_dense,
            place_dense_to_key,
            transition_dense_to_key,
            ..Default::default()
        }
    }

    /// Creates an empty label set sized for `n_places` places and
    /// `n_transitions` transitions, without key-to-index mapping.
    ///
    /// Prefer [`NetLabels::new`] when you have the net available. This
    /// constructor exists for cases where only counts are known (e.g. when
    /// building labels incrementally alongside a builder). Per-node accessors
    /// that take [`PlaceKey`]/[`TransitionKey`] will silently do nothing /
    /// return `None` until the struct is built through a net-aware path.
    #[must_use]
    pub(crate) fn with_capacity(n_places: usize, n_transitions: usize) -> Self {
        Self {
            place_names: PlaceMap::new(n_places),
            place_ids: PlaceMap::new(n_places),
            transition_names: TransitionMap::new(n_transitions),
            transition_ids: TransitionMap::new(n_transitions),
            ..Default::default()
        }
    }

    /// Looks up the dense index for a place key (internal helper).
    fn dense_place(&self, pk: PlaceKey) -> Option<Place> {
        self.place_key_to_dense.get(&pk).copied()
    }

    /// Looks up the dense index for a transition key (internal helper).
    fn dense_transition(&self, tk: TransitionKey) -> Option<Transition> {
        self.transition_key_to_dense.get(&tk).copied()
    }

    /// Returns the human-readable name of `place`, if set.
    #[must_use]
    pub fn place_name(&self, pk: PlaceKey) -> Option<&str> {
        self.place_names.get(self.dense_place(pk)?)?.as_deref()
    }

    /// Returns the human-readable name of a place by its dense index (crate-internal).
    #[must_use]
    pub(crate) fn place_name_dense(&self, place: Place) -> Option<&str> {
        self.place_names.get(place)?.as_deref()
    }

    /// Sets the human-readable name of `place`. Returns `&mut self` for chaining.
    pub fn set_place_name(&mut self, pk: PlaceKey, name: impl Into<String>) -> &mut Self {
        if let Some(dense) = self.dense_place(pk) {
            if let Some(slot) = self.place_names.get_mut(dense) {
                *slot = Some(name.into());
            }
        }
        self
    }

    /// Sets the human-readable name of a place by its dense index (crate-internal).
    pub(crate) fn set_place_name_dense(&mut self, place: Place, name: impl Into<String>) -> &mut Self {
        if let Some(slot) = self.place_names.get_mut(place) {
            *slot = Some(name.into());
        }
        self
    }

    /// Clears the name of `place`.
    pub fn clear_place_name(&mut self, pk: PlaceKey) -> &mut Self {
        if let Some(dense) = self.dense_place(pk) {
            if let Some(slot) = self.place_names.get_mut(dense) {
                *slot = None;
            }
        }
        self
    }

    /// Returns the stable identifier of `place`, if set.
    #[must_use]
    pub fn place_id(&self, pk: PlaceKey) -> Option<&str> {
        self.place_ids.get(self.dense_place(pk)?)?.as_deref()
    }

    /// Sets the stable identifier of `place`.
    pub fn set_place_id(&mut self, pk: PlaceKey, id: impl Into<String>) -> &mut Self {
        if let Some(dense) = self.dense_place(pk) {
            if let Some(slot) = self.place_ids.get_mut(dense) {
                *slot = Some(id.into());
            }
        }
        self
    }

    /// Returns the human-readable name of `transition`, if set.
    #[must_use]
    pub fn transition_name(&self, tk: TransitionKey) -> Option<&str> {
        self.transition_names.get(self.dense_transition(tk)?)?.as_deref()
    }

    /// Returns the human-readable name of a transition by its dense index (crate-internal).
    #[must_use]
    pub(crate) fn transition_name_dense(&self, transition: Transition) -> Option<&str> {
        self.transition_names.get(transition)?.as_deref()
    }

    /// Sets the human-readable name of `transition`.
    pub fn set_transition_name(&mut self, tk: TransitionKey, name: impl Into<String>) -> &mut Self {
        if let Some(dense) = self.dense_transition(tk) {
            if let Some(slot) = self.transition_names.get_mut(dense) {
                *slot = Some(name.into());
            }
        }
        self
    }

    /// Sets the human-readable name of a transition by its dense index (crate-internal).
    pub(crate) fn set_transition_name_dense(
        &mut self,
        transition: Transition,
        name: impl Into<String>,
    ) -> &mut Self {
        if let Some(slot) = self.transition_names.get_mut(transition) {
            *slot = Some(name.into());
        }
        self
    }

    /// Clears the name of `transition`.
    pub fn clear_transition_name(&mut self, tk: TransitionKey) -> &mut Self {
        if let Some(dense) = self.dense_transition(tk) {
            if let Some(slot) = self.transition_names.get_mut(dense) {
                *slot = None;
            }
        }
        self
    }

    /// Returns the stable identifier of `transition`, if set.
    #[must_use]
    pub fn transition_id(&self, tk: TransitionKey) -> Option<&str> {
        self.transition_ids.get(self.dense_transition(tk)?)?.as_deref()
    }

    /// Sets the stable identifier of `transition`.
    pub fn set_transition_id(&mut self, tk: TransitionKey, id: impl Into<String>) -> &mut Self {
        if let Some(dense) = self.dense_transition(tk) {
            if let Some(slot) = self.transition_ids.get_mut(dense) {
                *slot = Some(id.into());
            }
        }
        self
    }

    /// Returns the human-readable name of `arc`, if set.
    #[must_use]
    pub fn arc_name(&self, arc: Arc) -> Option<&str> {
        self.arc_names.get(&arc).map(String::as_str)
    }

    /// Sets the human-readable name of `arc`.
    pub fn set_arc_name(&mut self, arc: Arc, name: impl Into<String>) -> &mut Self {
        self.arc_names.insert(arc, name.into());
        self
    }

    /// Clears the name of `arc`.
    pub fn clear_arc_name(&mut self, arc: Arc) -> &mut Self {
        self.arc_names.remove(&arc);
        self
    }

    /// Returns the stable identifier of `arc`, if set.
    #[must_use]
    pub fn arc_id(&self, arc: Arc) -> Option<&str> {
        self.arc_ids.get(&arc).map(String::as_str)
    }

    /// Sets the stable identifier of `arc`.
    pub fn set_arc_id(&mut self, arc: Arc, id: impl Into<String>) -> &mut Self {
        self.arc_ids.insert(arc, id.into());
        self
    }

    /// Returns the name of the net, if set.
    #[must_use]
    pub fn net_name(&self) -> Option<&str> {
        self.net_name.as_deref()
    }

    /// Sets the name of the net.
    pub fn set_net_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.net_name = Some(name.into());
        self
    }

    /// Returns the stable identifier of the net, if set.
    #[must_use]
    pub fn net_id(&self) -> Option<&str> {
        self.net_id.as_deref()
    }

    /// Sets the stable identifier of the net.
    pub fn set_net_id(&mut self, id: impl Into<String>) -> &mut Self {
        self.net_id = Some(id.into());
        self
    }

    /// Returns the description of the net, if set.
    #[must_use]
    pub fn net_description(&self) -> Option<&str> {
        self.net_description.as_deref()
    }

    /// Sets the description of the net.
    pub fn set_net_description(&mut self, description: impl Into<String>) -> &mut Self {
        self.net_description = Some(description.into());
        self
    }

    /// Iterates over `(PlaceKey, name)` pairs for all places that have a name set.
    pub fn named_place_keys(&self) -> impl Iterator<Item = (PlaceKey, &str)> {
        self.place_names
            .iter()
            .filter_map(|(p, n)| {
                let pk = *self.place_dense_to_key.get(&p)?;
                n.as_deref().map(|name| (pk, name))
            })
    }

    /// Iterates over `(Place, name)` pairs for all places that have a name set (crate-internal).
    pub(crate) fn named_places(&self) -> impl Iterator<Item = (Place, &str)> {
        self.place_names
            .iter()
            .filter_map(|(p, n)| n.as_deref().map(|name| (p, name)))
    }

    /// Iterates over `(TransitionKey, name)` pairs for all transitions that have a name set.
    pub fn named_transition_keys(&self) -> impl Iterator<Item = (TransitionKey, &str)> {
        self.transition_names
            .iter()
            .filter_map(|(t, n)| {
                let tk = *self.transition_dense_to_key.get(&t)?;
                n.as_deref().map(|name| (tk, name))
            })
    }

    /// Iterates over `(Transition, name)` pairs for all transitions that have a
    /// name set (crate-internal).
    pub(crate) fn named_transitions(&self) -> impl Iterator<Item = (Transition, &str)> {
        self.transition_names
            .iter()
            .filter_map(|(t, n)| n.as_deref().map(|name| (t, name)))
    }

    /// Returns the name of the place at dense position `index`, or `None` (crate-internal).
    #[must_use]
    pub(crate) fn place_name_at(&self, index: usize) -> Option<&str> {
        self.place_names
            .get(Place::from_index(index as u32))?
            .as_deref()
    }

    /// Returns the name of the transition at dense position `index`, or `None` (crate-internal).
    #[must_use]
    pub(crate) fn transition_name_at(&self, index: usize) -> Option<&str> {
        self.transition_names
            .get(Transition::from_index(index as u32))?
            .as_deref()
    }

    /// Sets the name of the place at dense position `index` (crate-internal).
    pub(crate) fn set_place_name_at(
        &mut self,
        index: usize,
        name: impl Into<String>,
    ) -> &mut Self {
        self.set_place_name_dense(Place::from_index(index as u32), name)
    }

    /// Sets the name of the transition at dense position `index` (crate-internal).
    pub(crate) fn set_transition_name_at(
        &mut self,
        index: usize,
        name: impl Into<String>,
    ) -> &mut Self {
        self.set_transition_name_dense(Transition::from_index(index as u32), name)
    }
}

impl NetLabels {
    /// Constructs a `NetLabels` directly from pre-built maps. Used internally
    /// by the PNML converter; not part of the public API surface.
    ///
    /// Key-to-dense maps must be attached separately via `with_net` if
    /// key-based public accessors are needed.
    #[cfg(feature = "pnml")]
    #[expect(clippy::too_many_arguments)]
    #[expect(clippy::missing_const_for_fn)]
    pub(crate) fn from_raw(
        place_names: PlaceMap<Option<String>>,
        place_ids: PlaceMap<Option<String>>,
        transition_names: TransitionMap<Option<String>>,
        transition_ids: TransitionMap<Option<String>>,
        arc_names: HashMap<Arc, String>,
        arc_ids: HashMap<Arc, String>,
        net_name: Option<String>,
        net_id: Option<String>,
    ) -> Self {
        Self {
            place_names,
            place_ids,
            transition_names,
            transition_ids,
            arc_names,
            arc_ids,
            net_name,
            net_id,
            net_description: None,
            place_key_to_dense: HashMap::new(),
            transition_key_to_dense: HashMap::new(),
            place_dense_to_key: HashMap::new(),
            transition_dense_to_key: HashMap::new(),
        }
    }

    /// Attaches key-to-index maps from a net, enabling key-based accessors.
    /// Called after `from_raw` when the `Net` is available.
    #[cfg(feature = "pnml")]
    pub(crate) fn with_net(mut self, net: &Net) -> Self {
        self.place_key_to_dense = net.place_key_map().clone();
        self.transition_key_to_dense = net.transition_key_map().clone();
        self.place_dense_to_key = self.place_key_to_dense.iter().map(|(&k, &p)| (p, k)).collect();
        self.transition_dense_to_key = self.transition_key_to_dense.iter().map(|(&k, &t)| (t, k)).collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Arc, Place, PlaceKey, TransitionKey, builder::NetBuilder};

    fn make_net() -> (Net, PlaceKey, PlaceKey, TransitionKey, TransitionKey) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap();
        (net, p0, p1, t0, t1)
    }

    #[test]
    fn set_and_get_place_name() {
        let (net, p0, p1, _, _) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0, "Idle");
        assert_eq!(l.place_name(p0), Some("Idle"));
        assert_eq!(l.place_name(p1), None);
    }

    #[test]
    fn clear_place_name() {
        let (net, p0, _, _, _) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0, "Idle");
        l.clear_place_name(p0);
        assert_eq!(l.place_name(p0), None);
    }

    #[test]
    fn set_and_get_transition_name() {
        let (net, _, _, t0, _) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_transition_name(t0, "Fire");
        assert_eq!(l.transition_name(t0), Some("Fire"));
    }

    #[test]
    fn chaining() {
        let (net, p0, p1, t0, t1) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0, "A")
            .set_place_name(p1, "B")
            .set_transition_name(t0, "X")
            .set_transition_name(t1, "Y")
            .set_net_name("My net");
        assert_eq!(l.place_name(p0), Some("A"));
        assert_eq!(l.place_name(p1), Some("B"));
        assert_eq!(l.transition_name(t0), Some("X"));
        assert_eq!(l.transition_name(t1), Some("Y"));
        assert_eq!(l.net_name(), Some("My net"));
    }

    #[test]
    fn arc_labels() {
        let (net, p0, _, t0, _) = make_net();
        let arc = Arc::PlaceToTransition(p0, t0);
        let mut l = NetLabels::new(&net);
        l.set_arc_name(arc, "flow").set_arc_id(arc, "a0");
        assert_eq!(l.arc_name(arc), Some("flow"));
        assert_eq!(l.arc_id(arc), Some("a0"));
        l.clear_arc_name(arc);
        assert_eq!(l.arc_name(arc), None);
    }

    #[test]
    fn named_places_iterator() {
        let (net, p0, p1, _, _) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0, "Alpha");
        let named: Vec<_> = l.named_place_keys().collect();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0], (p0, "Alpha"));
        assert!(!named.iter().any(|(p, _)| *p == p1));
    }

    #[test]
    fn named_transitions_iterator() {
        let (net, _, _, t0, t1) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_transition_name(t0, "Start");
        let named: Vec<_> = l.named_transition_keys().collect();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0], (t0, "Start"));
        assert!(!named.iter().any(|(t, _)| *t == t1));
    }

    #[test]
    fn out_of_range_is_silent() {
        let (net, _, _, _, _) = make_net();
        let mut l = NetLabels::new(&net);
        let ghost = Place::from_index(99);
        l.set_place_name_dense(ghost, "Ghost");
        assert_eq!(l.place_name_dense(ghost), None);
    }

    #[test]
    fn net_level_labels() {
        let (net, _, _, _, _) = make_net();
        let mut l = NetLabels::new(&net);
        l.set_net_name("Ring").set_net_id("n0").set_net_description("A token ring.");
        assert_eq!(l.net_name(), Some("Ring"));
        assert_eq!(l.net_id(), Some("n0"));
        assert_eq!(l.net_description(), Some("A token ring."));
    }
}
