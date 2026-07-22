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
//! use petrivet::builder::NetBuilder;
//! use petrivet::pnml::labels::NetLabels;
//!
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arcs((idle, start, busy, finish, idle));
//! let net = b.build().unwrap();
//!
//! let mut labels = NetLabels::new();
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
use crate::pnml::nupn::NupnMetadata;
use ahash::{HashMap, HashMapExt};

/// Human-readable labels and metadata for the elements of a single Petri net.
///
/// Labels are purely presentational: they have no effect on structural
/// classification, reachability analysis, or simulation. The struct is
/// intentionally decoupled from [`Net`](Net) and
/// [`System`](crate::system::PetriNet) — callers hold the three values
/// independently and compose them as needed.
///
/// It is undefined behavior to access labels for a place/transition/arc
/// that does not exist in the net, but out-of-range accesses will not panic.
///
/// # Construction
///
/// Build directly with [`NetLabels::new`] and set individual labels via the
/// fluent setter methods, or obtain a fully-populated instance from a PNML
/// document via `pnml::Net::to_pt_system` (requires the `pnml` feature).
#[derive(Debug, Clone, Default)]
pub struct NetLabels {
    place_names: HashMap<Place, String>,
    place_ids: HashMap<Place, String>,
    transition_names: HashMap<Transition, String>,
    transition_ids: HashMap<Transition, String>,
    arc_names: HashMap<Arc, String>,
    arc_ids: HashMap<Arc, String>,

    /// Optional name for the net as a whole.
    net_name: Option<String>,
    /// Optional stable identifier for the net (e.g. original PNML `net id`).
    net_id: Option<String>,
    /// Optional free-text description of the net.
    net_description: Option<String>,

    /// [NUPN](https://mcc.lip6.fr/2026/nupn.php) nested-unit Petri net metadata.
    nupn: Option<NupnMetadata>,
}

impl NetLabels {
    /// Creates an empty label set sized for the given net. All per-node labels
    /// start as `None`. Stores an internal copy of the key→index mapping so
    /// that all subsequent accessors work with [`Place`]/[`Transition`]
    /// handles directly.
    #[must_use]
    pub fn new() -> Self {
        Self {
            place_names: HashMap::new(),
            place_ids: HashMap::new(),
            transition_names: HashMap::new(),
            transition_ids: HashMap::new(),
            ..Default::default()
        }
    }

    /// Returns the human-readable name of `place`, if set.
    #[must_use]
    pub fn place_name(&self, p: Place) -> Option<&str> {
        self.place_names.get(&p).map(String::as_str)
    }

    /// Sets the human-readable name of `place`. Returns `&mut self` for chaining.
    pub fn set_place_name(&mut self, p: Place, name: impl Into<String>) -> &mut Self {
        self.place_names.insert(p, name.into());
        self
    }

    /// Clears the name of `place`.
    pub fn clear_place_name(&mut self, p: Place) -> &mut Self {
        self.place_names.remove(&p);
        self
    }

    /// Returns the stable identifier of `place`, if set.
    #[must_use]
    pub fn place_id(&self, p: Place) -> Option<&str> {
        self.place_ids.get(&p).map(String::as_str)
    }

    /// Sets the stable identifier of `place`.
    pub fn set_place_id(&mut self, p: &Place, id: impl Into<String>) -> &mut Self {
        self.place_ids.insert(*p, id.into());
        self
    }

    /// Returns the human-readable name of `transition`, if set.
    #[must_use]
    pub fn transition_name(&self, t: Transition) -> Option<&str> {
        self.transition_names.get(&t).map(String::as_str)
    }

    /// Sets the human-readable name of `transition`.
    pub fn set_transition_name(&mut self, t: Transition, name: impl Into<String>) -> &mut Self {
        self.transition_names.insert(t, name.into());
        self
    }

    /// Clears the name of `transition`.
    pub fn clear_transition_name(&mut self, t: Transition) -> &mut Self {
        self.transition_names.remove(&t);
        self
    }

    /// Returns the stable identifier of `transition`, if set.
    #[must_use]
    pub fn transition_id(&self, t: Transition) -> Option<&str> {
        self.transition_ids.get(&t).map(String::as_str)
    }

    /// Sets the stable identifier of `transition`.
    pub fn set_transition_id(&mut self, t: Transition, id: impl Into<String>) -> &mut Self {
        self.transition_ids.insert(t, id.into());
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

    /// Nested-unit (NUPN) metadata from PNML, when the source file contained a
    /// `<toolspecific tool="nupn" version="1.1">` block.
    #[must_use]
    pub const fn nupn(&self) -> Option<&NupnMetadata> {
        self.nupn.as_ref()
    }

    /// Attach or clear NUPN metadata (mainly for tests and custom importers).
    pub fn set_nupn(&mut self, nupn: Option<NupnMetadata>) -> &mut Self {
        self.nupn = nupn;
        self
    }

    /// Iterates over `(Place, name)` pairs for all places that have a name set.
    pub fn named_places(&self) -> impl Iterator<Item = (Place, &str)> {
        self.place_names.iter().map(|(k, v)| (*k, v.as_str()))
    }

    /// Iterates over `(Transition, name)` pairs for all transitions that have a
    /// name set (crate-internal).
    pub fn named_transitions(&self) -> impl Iterator<Item = (Transition, &str)> {
        self.transition_names.iter().map(|(k, v)| (*k, v.as_str()))
    }
}

impl NetLabels {
    /// Constructs a `NetLabels` directly from pre-built maps.
    #[expect(clippy::too_many_arguments)]
    #[expect(clippy::missing_const_for_fn)]
    pub(crate) fn from_raw(
        place_names: HashMap<Place, String>,
        place_ids: HashMap<Place, String>,
        transition_names: HashMap<Transition, String>,
        transition_ids: HashMap<Transition, String>,
        arc_names: HashMap<Arc, String>,
        arc_ids: HashMap<Arc, String>,
        net_name: Option<String>,
        net_id: Option<String>,
        nupn: Option<NupnMetadata>,
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
            nupn,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pnml::labels::NetLabels;
    use crate::prelude::{Arc, NetBuilder};

    #[test]
    fn set_and_get_place_name() {
        let [p0, p1] = NetBuilder::new().add_places();
        let mut l = NetLabels::new();
        l.set_place_name(p0, "Idle");
        assert_eq!(l.place_name(p0), Some("Idle"));
        assert_eq!(l.place_name(p1), None);
    }

    #[test]
    fn clear_place_name() {
        let p0 = NetBuilder::new().add_place();
        let mut l = NetLabels::new();
        l.set_place_name(p0, "Idle");
        l.clear_place_name(p0);
        assert_eq!(l.place_name(p0), None);
    }

    #[test]
    fn set_and_get_transition_name() {
        let t0 = NetBuilder::new().add_transition();
        let mut l = NetLabels::new();
        l.set_transition_name(t0, "Fire");
        assert_eq!(l.transition_name(t0), Some("Fire"));
    }

    #[test]
    fn chaining() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        let mut l = NetLabels::new();
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
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let t0 = b.add_transition();
        let arc = Arc::PlaceToTransition(p0, t0);
        let mut l = NetLabels::new();
        l.set_arc_name(arc, "flow").set_arc_id(arc, "a0");
        assert_eq!(l.arc_name(arc), Some("flow"));
        assert_eq!(l.arc_id(arc), Some("a0"));
        l.clear_arc_name(arc);
        assert_eq!(l.arc_name(arc), None);
    }

    #[test]
    fn named_places_iterator() {
        let [p0, p1] = NetBuilder::new().add_places();
        let mut l = NetLabels::new();
        l.set_place_name(p0, "Alpha");
        let named: Vec<_> = l.named_places().collect();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0], (p0, "Alpha"));
        assert!(!named.iter().any(|(p, _)| *p == p1));
    }

    #[test]
    fn named_transitions_iterator() {
        let [t0, t1] = NetBuilder::new().add_transitions();
        let mut l = NetLabels::new();
        l.set_transition_name(t0, "Start");
        let named: Vec<_> = l.named_transitions().collect();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0], (t0, "Start"));
        assert!(!named.iter().any(|(t, _)| *t == t1));
    }

    #[test]
    fn net_level_labels() {
        let mut l = NetLabels::new();
        l.set_net_name("Ring")
            .set_net_id("n0")
            .set_net_description("A token ring.");
        assert_eq!(l.net_name(), Some("Ring"));
        assert_eq!(l.net_id(), Some("n0"));
        assert_eq!(l.net_description(), Some("A token ring."));
    }
}
