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
//! ```ignore
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::labeled::NetLabels;
//!
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arcs((idle, start, busy, finish, idle));
//! let net = b.build().unwrap();
//! let idle_d = net.dense_place(idle);
//! let busy_d = net.dense_place(busy);
//! let start_d = net.dense_transition(start);
//! let finish_d = net.dense_transition(finish);
//!
//! let mut labels = NetLabels::new(&net);
//! labels
//!     .set_place_name(idle_d, "Idle")
//!     .set_place_name(busy_d, "Busy")
//!     .set_transition_name(start_d, "Start")
//!     .set_transition_name(finish_d, "Finish")
//!     .set_net_name("Producer-consumer");
//!
//! assert_eq!(labels.place_name(idle_d), Some("Idle"));
//! assert_eq!(labels.transition_name(finish_d), Some("Finish"));
//!
//! // Iterate named places without constructing raw indices
//! for (place, name) in labels.named_places() {
//!     println!("{place}: {name}");
//! }
//! ```

use crate::net::{Arc, Net, Place, PlaceMap, Transition, TransitionMap};
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
///
/// # Indexing
///
/// All per-node accessors accept the same [`Place`], [`Transition`], and
/// [`Arc`] handles used by [`Net`](crate::net::Net). Passing a handle whose
/// index is out of range for this label set will return `None`/silently do
/// nothing — the same contract as out-of-range indexing into [`PlaceMap`].
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
}

impl NetLabels {
    /// Creates an empty label set sized for the given net. All per-node labels
    /// start as `None`.
    #[must_use]
    pub fn new(net: &Net) -> Self {
        Self {
            place_names: PlaceMap::new(net.place_count() as usize),
            place_ids: PlaceMap::new(net.place_count() as usize),
            transition_names: TransitionMap::new(net.transition_count() as usize),
            transition_ids: TransitionMap::new(net.transition_count() as usize),
            ..Default::default()
        }
    }

    /// Creates an empty label set sized for `n_places` places and
    /// `n_transitions` transitions.
    ///
    /// Prefer [`NetLabels::new`] when you have the net available. This
    /// constructor exists for cases where only counts are known (e.g. when
    /// building labels incrementally alongside a builder).
    #[must_use]
    pub fn with_capacity(n_places: usize, n_transitions: usize) -> Self {
        Self {
            place_names: PlaceMap::new(n_places),
            place_ids: PlaceMap::new(n_places),
            transition_names: TransitionMap::new(n_transitions),
            transition_ids: TransitionMap::new(n_transitions),
            ..Default::default()
        }
    }

    /// Returns the human-readable name of `place`, if set.
    #[must_use]
    pub fn place_name(&self, place: Place) -> Option<&str> {
        self.place_names.get(place)?.as_deref()
    }

    /// Sets the human-readable name of `place`. Returns `&mut self` for
    /// chaining.
    pub fn set_place_name(&mut self, place: Place, name: impl Into<String>) -> &mut Self {
        if let Some(slot) = self.place_names.get_mut(place) {
            *slot = Some(name.into());
        }
        self
    }

    /// Clears the name of `place`.
    pub fn clear_place_name(&mut self, place: Place) -> &mut Self {
        if let Some(slot) = self.place_names.get_mut(place) {
            *slot = None;
        }
        self
    }

    /// Returns the stable identifier of `place`, if set.
    #[must_use]
    pub fn place_id(&self, place: Place) -> Option<&str> {
        self.place_ids.get(place)?.as_deref()
    }

    /// Sets the stable identifier of `place`.
    pub fn set_place_id(&mut self, place: Place, id: impl Into<String>) -> &mut Self {
        if let Some(slot) = self.place_ids.get_mut(place) {
            *slot = Some(id.into());
        }
        self
    }

    /// Returns the human-readable name of `transition`, if set.
    #[must_use]
    pub fn transition_name(&self, transition: Transition) -> Option<&str> {
        self.transition_names.get(transition)?.as_deref()
    }

    /// Sets the human-readable name of `transition`.
    pub fn set_transition_name(
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
    pub fn clear_transition_name(&mut self, transition: Transition) -> &mut Self {
        if let Some(slot) = self.transition_names.get_mut(transition) {
            *slot = None;
        }
        self
    }

    /// Returns the stable identifier of `transition`, if set.
    #[must_use]
    pub fn transition_id(&self, transition: Transition) -> Option<&str> {
        self.transition_ids.get(transition)?.as_deref()
    }

    /// Sets the stable identifier of `transition`.
    pub fn set_transition_id(
        &mut self,
        transition: Transition,
        id: impl Into<String>,
    ) -> &mut Self {
        if let Some(slot) = self.transition_ids.get_mut(transition) {
            *slot = Some(id.into());
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

    /// Iterates over `(Place, name)` pairs for all places that have a name set.
    pub fn named_places(&self) -> impl Iterator<Item = (Place, &str)> {
        self.place_names
            .iter()
            .filter_map(|(p, n)| n.as_deref().map(|name| (p, name)))
    }

    /// Iterates over `(Transition, name)` pairs for all transitions that have a
    /// name set.
    pub fn named_transitions(&self) -> impl Iterator<Item = (Transition, &str)> {
        self.transition_names
            .iter()
            .filter_map(|(t, n)| n.as_deref().map(|name| (t, name)))
    }

    /// Returns the name of the place at dense position `index`, or `None`.
    #[must_use]
    pub fn place_name_at(&self, index: usize) -> Option<&str> {
        self.place_names
            .get(Place::from_index(index as u32))?
            .as_deref()
    }

    /// Returns the name of the transition at dense position `index`, or `None`.
    #[must_use]
    pub fn transition_name_at(&self, index: usize) -> Option<&str> {
        self.transition_names
            .get(Transition::from_index(index as u32))?
            .as_deref()
    }

    /// Sets the name of the place at dense position `index`.
    pub fn set_place_name_at(
        &mut self,
        index: usize,
        name: impl Into<String>,
    ) -> &mut Self {
        self.set_place_name(Place::from_index(index as u32), name)
    }

    /// Sets the name of the transition at dense position `index`.
    pub fn set_transition_name_at(
        &mut self,
        index: usize,
        name: impl Into<String>,
    ) -> &mut Self {
        self.set_transition_name(Transition::from_index(index as u32), name)
    }
}

impl NetLabels {
    /// Constructs a `NetLabels` directly from pre-built maps. Used internally
    /// by the PNML converter; not part of the public API surface.
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
        }
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
        let p0d = net.dense_place(p0);
        let p1d = net.dense_place(p1);
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0d, "Idle");
        assert_eq!(l.place_name(p0d), Some("Idle"));
        assert_eq!(l.place_name(p1d), None);
    }

    #[test]
    fn clear_place_name() {
        let (net, p0, _, _, _) = make_net();
        let p0d = net.dense_place(p0);
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0d, "Idle");
        l.clear_place_name(p0d);
        assert_eq!(l.place_name(p0d), None);
    }

    #[test]
    fn set_and_get_transition_name() {
        let (net, _, _, t0, _) = make_net();
        let t0d = net.dense_transition(t0);
        let mut l = NetLabels::new(&net);
        l.set_transition_name(t0d, "Fire");
        assert_eq!(l.transition_name(t0d), Some("Fire"));
    }

    #[test]
    fn chaining() {
        let (net, p0, p1, t0, t1) = make_net();
        let p0d = net.dense_place(p0);
        let p1d = net.dense_place(p1);
        let t0d = net.dense_transition(t0);
        let t1d = net.dense_transition(t1);
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0d, "A")
            .set_place_name(p1d, "B")
            .set_transition_name(t0d, "X")
            .set_transition_name(t1d, "Y")
            .set_net_name("My net");
        assert_eq!(l.place_name(p0d), Some("A"));
        assert_eq!(l.place_name(p1d), Some("B"));
        assert_eq!(l.transition_name(t0d), Some("X"));
        assert_eq!(l.transition_name(t1d), Some("Y"));
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
        let p0d = net.dense_place(p0);
        let p1d = net.dense_place(p1);
        let mut l = NetLabels::new(&net);
        l.set_place_name(p0d, "Alpha");
        let named: Vec<_> = l.named_places().collect();
        assert_eq!(named, vec![(p0d, "Alpha")]);
        assert!(!named.iter().any(|(p, _)| *p == p1d));
    }

    #[test]
    fn named_transitions_iterator() {
        let (net, _, _, t0, t1) = make_net();
        let t0d = net.dense_transition(t0);
        let t1d = net.dense_transition(t1);
        let mut l = NetLabels::new(&net);
        l.set_transition_name(t0d, "Start");
        let named: Vec<_> = l.named_transitions().collect();
        assert_eq!(named, vec![(t0d, "Start")]);
        assert!(!named.iter().any(|(t, _)| *t == t1d));
    }

    #[test]
    fn out_of_range_is_silent() {
        let (net, _, _, _, _) = make_net();
        let mut l = NetLabels::new(&net);
        let ghost = Place::from_index(99);
        l.set_place_name(ghost, "Ghost");
        assert_eq!(l.place_name(ghost), None);
    }

    #[test]
    fn net_level_labels() {
        let mut l = NetLabels::with_capacity(0, 0);
        l.set_net_name("Ring").set_net_id("n0").set_net_description("A token ring.");
        assert_eq!(l.net_name(), Some("Ring"));
        assert_eq!(l.net_id(), Some("n0"));
        assert_eq!(l.net_description(), Some("A token ring."));
    }
}
