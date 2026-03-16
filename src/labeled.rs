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
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::labeled::NetLabels;
//!
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arcs((idle, start, busy, finish, idle));
//! let net = b.build().unwrap();
//!
//! let mut labels = NetLabels::new(net.place_count(), net.transition_count());
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

use crate::net::{Arc, Place, Transition};
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
/// index is out of range for this label set will panic in debug builds and
/// return `None`/silently do nothing in release builds — the same contract
/// as indexing into a [`Net`](crate::net::Net)'s preset/postset arrays.
#[derive(Debug, Clone, Default)]
pub struct NetLabels {
    /// Per-place human-readable name, indexed by `Place::idx`.
    place_names: Box<[Option<String>]>,
    /// Per-place stable identifier (e.g. original PNML `id` attribute).
    place_ids: Box<[Option<String>]>,

    /// Per-transition human-readable name, indexed by `Transition::idx`.
    transition_names: Box<[Option<String>]>,
    /// Per-transition stable identifier.
    transition_ids: Box<[Option<String>]>,

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
    /// Creates an empty label set sized for the given number of places and
    /// transitions. All per-node labels start as `None`.
    #[must_use]
    pub fn new(n_places: usize, n_transitions: usize) -> Self {
        Self {
            place_names: vec![None; n_places].into_boxed_slice(),
            place_ids: vec![None; n_places].into_boxed_slice(),
            transition_names: vec![None; n_transitions].into_boxed_slice(),
            transition_ids: vec![None; n_transitions].into_boxed_slice(),
            ..Default::default()
        }
    }

    /// Returns the human-readable name of `place`, if set.
    #[must_use]
    pub fn place_name(&self, place: Place) -> Option<&str> {
        self.place_names.get(place.idx)?.as_deref()
    }

    /// Sets the human-readable name of `place`. Returns `&mut self` for
    /// chaining.
    pub fn set_place_name(&mut self, place: Place, name: impl Into<String>) -> &mut Self {
        if let Some(slot) = self.place_names.get_mut(place.idx) {
            *slot = Some(name.into());
        }
        self
    }

    /// Clears the name of `place`.
    pub fn clear_place_name(&mut self, place: Place) -> &mut Self {
        if let Some(slot) = self.place_names.get_mut(place.idx) {
            *slot = None;
        }
        self
    }

    /// Returns the stable identifier of `place`, if set.
    #[must_use]
    pub fn place_id(&self, place: Place) -> Option<&str> {
        self.place_ids.get(place.idx)?.as_deref()
    }

    /// Sets the stable identifier of `place`.
    pub fn set_place_id(&mut self, place: Place, id: impl Into<String>) -> &mut Self {
        if let Some(slot) = self.place_ids.get_mut(place.idx) {
            *slot = Some(id.into());
        }
        self
    }

    /// Returns the human-readable name of `transition`, if set.
    #[must_use]
    pub fn transition_name(&self, transition: Transition) -> Option<&str> {
        self.transition_names.get(transition.idx)?.as_deref()
    }

    /// Sets the human-readable name of `transition`.
    pub fn set_transition_name(
        &mut self,
        transition: Transition,
        name: impl Into<String>,
    ) -> &mut Self {
        if let Some(slot) = self.transition_names.get_mut(transition.idx) {
            *slot = Some(name.into());
        }
        self
    }

    /// Clears the name of `transition`.
    pub fn clear_transition_name(&mut self, transition: Transition) -> &mut Self {
        if let Some(slot) = self.transition_names.get_mut(transition.idx) {
            *slot = None;
        }
        self
    }

    /// Returns the stable identifier of `transition`, if set.
    #[must_use]
    pub fn transition_id(&self, transition: Transition) -> Option<&str> {
        self.transition_ids.get(transition.idx)?.as_deref()
    }

    /// Sets the stable identifier of `transition`.
    pub fn set_transition_id(
        &mut self,
        transition: Transition,
        id: impl Into<String>,
    ) -> &mut Self {
        if let Some(slot) = self.transition_ids.get_mut(transition.idx) {
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
            .enumerate()
            .filter_map(|(i, n)| n.as_deref().map(|name| (Place::from_index(i), name)))
    }

    /// Iterates over `(Transition, name)` pairs for all transitions that have a
    /// name set.
    pub fn named_transitions(&self) -> impl Iterator<Item = (Transition, &str)> {
        self.transition_names
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_deref().map(|name| (Transition::from_index(i), name)))
    }
}

impl NetLabels {
    /// Constructs a `NetLabels` directly from pre-built vecs. Used internally
    /// by the PNML converter; not part of the public API surface.
    #[cfg(feature = "pnml")]
    #[expect(clippy::too_many_arguments)]
    #[expect(clippy::missing_const_for_fn)] // false positive: body constructs a HashMap
    pub(crate) fn from_raw(
        place_names: Box<[Option<String>]>,
        place_ids: Box<[Option<String>]>,
        transition_names: Box<[Option<String>]>,
        transition_ids: Box<[Option<String>]>,
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
    use crate::net::Arc;

    fn places() -> (Place, Place) {
        (Place::from_index(0), Place::from_index(1))
    }

    fn transitions() -> (Transition, Transition) {
        (Transition::from_index(0), Transition::from_index(1))
    }

    #[test]
    fn set_and_get_place_name() {
        let (p0, p1) = places();
        let mut l = NetLabels::new(2, 0);
        l.set_place_name(p0, "Idle");
        assert_eq!(l.place_name(p0), Some("Idle"));
        assert_eq!(l.place_name(p1), None);
    }

    #[test]
    fn clear_place_name() {
        let (p0, _) = places();
        let mut l = NetLabels::new(1, 0);
        l.set_place_name(p0, "Idle");
        l.clear_place_name(p0);
        assert_eq!(l.place_name(p0), None);
    }

    #[test]
    fn set_and_get_transition_name() {
        let (t0, _) = transitions();
        let mut l = NetLabels::new(0, 2);
        l.set_transition_name(t0, "Fire");
        assert_eq!(l.transition_name(t0), Some("Fire"));
    }

    #[test]
    fn chaining() {
        let (p0, p1) = places();
        let (t0, t1) = transitions();
        let mut l = NetLabels::new(2, 2);
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
        let (p0, _) = places();
        let (t0, _) = transitions();
        let arc = Arc::PlaceToTransition(p0, t0);
        let mut l = NetLabels::new(1, 1);
        l.set_arc_name(arc, "flow").set_arc_id(arc, "a0");
        assert_eq!(l.arc_name(arc), Some("flow"));
        assert_eq!(l.arc_id(arc), Some("a0"));
        l.clear_arc_name(arc);
        assert_eq!(l.arc_name(arc), None);
    }

    #[test]
    fn named_places_iterator() {
        let (p0, p1) = places();
        let mut l = NetLabels::new(2, 0);
        l.set_place_name(p0, "Alpha");
        let named: Vec<_> = l.named_places().collect();
        assert_eq!(named, vec![(p0, "Alpha")]);
        // p1 has no name, so it doesn't appear
        assert!(!named.iter().any(|(p, _)| *p == p1));
    }

    #[test]
    fn out_of_range_is_silent() {
        let mut l = NetLabels::new(1, 1);
        // Index 5 is out of range — setters silently do nothing
        l.set_place_name(Place::from_index(5), "Ghost");
        assert_eq!(l.place_name(Place::from_index(5)), None);
    }

    #[test]
    fn net_level_labels() {
        let mut l = NetLabels::new(0, 0);
        l.set_net_name("Ring").set_net_id("n0").set_net_description("A token ring.");
        assert_eq!(l.net_name(), Some("Ring"));
        assert_eq!(l.net_id(), Some("n0"));
        assert_eq!(l.net_description(), Some("A token ring."));
    }
}
