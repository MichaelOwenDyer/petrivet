//! Conversion from petrivet's native types into the PNML data model.
//!
//! This is the inverse of [`convert`](super::convert): where `convert` turns a
//! parsed [`PnmlDocument`] into a native [`PetriNet`], this module rebuilds a
//! [`PnmlDocument`] from a native [`Net`] or [`PetriNet`]. The entry points are:
//!
//! - [`Net::to_pnml`] — exports the net structure without a marking.
//! - [`PetriNet::to_pnml`] — additionally exports the system's current marking
//!   as `<initialMarking>` labels.
//!
//! Serialize the resulting document with [`PnmlDocument::to_xml`].
//!
//! # Identifier assignment
//!
//! PNML requires every net object to carry a document-unique `id`. Exported
//! identifiers are chosen as follows:
//!
//! - When the net's [`NetLabels`] record an identifier for an object (as
//!   populated by the PNML importer), that identifier is reused verbatim, so a
//!   net imported from PNML exports with its original identifiers.
//! - Otherwise a fresh identifier is generated deterministically in the net's
//!   iteration order: `p0`, `p1`, … for places, `t0`, `t1`, … for transitions,
//!   `a0`, `a1`, … for arcs. Generated identifiers skip any label-provided
//!   identifier, so the two sources cannot collide.
//! - If two objects carry the same label-provided identifier (possible only
//!   through manual [`NetLabels`] mutation), the first object in iteration
//!   order keeps it and later claimants receive generated identifiers, so the
//!   exported document always satisfies the PNML uniqueness constraint.
//!
//! # Fidelity
//!
//! The exported document reflects the native model exactly: every place,
//! transition, and arc of the net is exported, and nothing else. The native
//! model is unweighted, so no arc `<inscription>` weight other than the
//! implicit 1 is ever written. `<initialMarking>` is written only for places
//! with a positive token count. Names and graphics stored in the net's
//! [`NetLabels`] and [`PnmlGraphics`](crate::pnml::graphics::PnmlGraphics)
//! are re-attached to the corresponding elements, and NUPN metadata is
//! re-emitted as a net-level `<toolspecific tool="nupn" version="1.1">` block.
//!
//! # Known limitations
//!
//! - All elements are exported onto a single page; the page structure of an
//!   imported document is not preserved (the importer already flattens pages,
//!   see [`PnmlGraphics`](crate::pnml::graphics::PnmlGraphics)).
//! - Annotation graphics attached to a zero-count `<initialMarking>` are
//!   dropped, because the label itself is not written.

use super::{net, net_type, Name, NaturalNumberLabel, Page, PageObject, PnmlDocument, PositiveIntegerLabel, ToolSpecific};
use crate::pnml::graphics::AnnotationGraphics;
use crate::pnml::labels::NetLabels;
use crate::prelude::{Arc, Marking, Net, PetriNet, Place, Transition};
use ahash::{HashMap, HashSet, HashSetExt};

/// Allocates document-unique identifiers for exported net objects.
struct IdAllocator {
    /// All label-provided identifiers of live net objects. Generated
    /// identifiers must avoid these even before they are claimed.
    reserved: HashSet<String>,
    /// Identifiers already given to an exported element.
    assigned: HashSet<String>,
}

impl IdAllocator {
    fn new() -> Self {
        Self {
            reserved: HashSet::new(),
            assigned: HashSet::new(),
        }
    }

    /// Records a label-provided identifier so that generated identifiers avoid it.
    fn reserve(&mut self, id: &str) {
        self.reserved.insert(id.to_owned());
    }

    /// Uses `id` if it is present and no exported element has claimed it yet;
    /// otherwise generates a fresh identifier with `prefix` (repairing
    /// duplicate label-provided identifiers).
    fn claim_or_generate(&mut self, id: Option<&str>, prefix: &str, counter: &mut u32) -> String {
        if let Some(id) = id
            && !self.assigned.contains(id)
        {
            self.assigned.insert(id.to_owned());
            return id.to_owned();
        }
        self.generate(prefix, counter)
    }

    /// Generates the next free identifier of the form `{prefix}{n}`.
    fn generate(&mut self, prefix: &str, counter: &mut u32) -> String {
        loop {
            let candidate = format!("{prefix}{counter}");
            *counter += 1;
            if !self.reserved.contains(&candidate) && !self.assigned.contains(&candidate) {
                self.assigned.insert(candidate.clone());
                return candidate;
            }
        }
    }
}

/// Builds a `<name>` annotation when a text or an annotation graphic is present.
fn make_name(text: Option<&str>, graphics: Option<AnnotationGraphics>) -> Option<Name> {
    if text.is_none() && graphics.is_none() {
        return None;
    }
    Some(Name {
        text: text.map(str::to_owned),
        graphics,
    })
}

/// Builds the PNML representation of a single native net, with `marking`
/// written as the `<initialMarking>` labels when provided.
#[expect(clippy::too_many_lines)]
fn export_net(pt_net: &Net, marking: Option<&Marking<u32>>) -> net::PnmlNet {
    let labels = pt_net.labels.as_deref();
    let graphics = pt_net.graphics.as_deref();

    // Reserve every label-provided identifier of a live object up front, so
    // that identifiers generated for unlabeled objects cannot collide with an
    // identifier claimed later.
    let mut alloc = IdAllocator::new();
    if let Some(labels) = labels {
        if let Some(id) = labels.net_id() {
            alloc.reserve(id);
        }
        for p in pt_net.places() {
            if let Some(id) = labels.place_id(p) {
                alloc.reserve(id);
            }
        }
        for t in pt_net.transitions() {
            if let Some(id) = labels.transition_id(t) {
                alloc.reserve(id);
            }
        }
        for arc in pt_net.arcs() {
            if let Some(id) = labels.arc_id(arc) {
                alloc.reserve(id);
            }
        }
    }

    let mut net_counter = 0;
    let net_id = alloc.claim_or_generate(
        labels.and_then(NetLabels::net_id),
        "net",
        &mut net_counter,
    );

    let mut place_counter = 0;
    let place_ids: HashMap<Place, String> = pt_net
        .places()
        .map(|p| {
            let id = alloc.claim_or_generate(
                labels.and_then(|l| l.place_id(p)),
                "p",
                &mut place_counter,
            );
            (p, id)
        })
        .collect();

    let mut transition_counter = 0;
    let transition_ids: HashMap<Transition, String> = pt_net
        .transitions()
        .map(|t| {
            let id = alloc.claim_or_generate(
                labels.and_then(|l| l.transition_id(t)),
                "t",
                &mut transition_counter,
            );
            (t, id)
        })
        .collect();

    let mut objects = Vec::with_capacity(pt_net.node_count() + pt_net.arc_count());

    for p in pt_net.places() {
        let tokens = marking.map_or(0, |m| m.get(p));
        let initial_marking = (tokens > 0).then(|| NaturalNumberLabel {
            text: Some(u64::from(tokens)),
            graphics: graphics.and_then(|g| g.place_marking_graphics.get(&p).cloned()),
        });
        objects.push(PageObject::Place(net::Place {
            id: place_ids[&p].clone(),
            name: make_name(
                labels.and_then(|l| l.place_name(p)),
                graphics.and_then(|g| g.place_name_graphics.get(&p).cloned()),
            ),
            initial_marking,
            graphics: graphics.and_then(|g| g.place_graphics.get(&p).cloned()),
            tool_specific: Vec::new(),
        }));
    }

    for t in pt_net.transitions() {
        objects.push(PageObject::Transition(net::Transition {
            id: transition_ids[&t].clone(),
            name: make_name(
                labels.and_then(|l| l.transition_name(t)),
                graphics.and_then(|g| g.transition_name_graphics.get(&t).cloned()),
            ),
            graphics: graphics.and_then(|g| g.transition_graphics.get(&t).cloned()),
            tool_specific: Vec::new(),
        }));
    }

    let mut arc_counter = 0;
    for arc in pt_net.arcs() {
        let (source, target) = match arc {
            Arc::PlaceToTransition(p, t) => (place_ids[&p].clone(), transition_ids[&t].clone()),
            Arc::TransitionToPlace(t, p) => (transition_ids[&t].clone(), place_ids[&p].clone()),
        };
        // The native model is unweighted; an inscription (implicit weight 1) is
        // written only to carry annotation graphics preserved from an import.
        let inscription = graphics
            .and_then(|g| g.arc_inscription_graphics.get(&arc).cloned())
            .map(|g| PositiveIntegerLabel {
                text: Some(1),
                graphics: Some(g),
            });
        objects.push(PageObject::Arc(net::Arc {
            id: alloc.claim_or_generate(
                labels.and_then(|l| l.arc_id(arc)),
                "a",
                &mut arc_counter,
            ),
            source,
            target,
            name: make_name(labels.and_then(|l| l.arc_name(arc)), None),
            inscription,
            arc_type: None,
            graphics: graphics.and_then(|g| g.arc_graphics.get(&arc).cloned()),
            tool_specific: Vec::new(),
        }));
    }

    let mut page_counter = 0;
    let page = Page {
        id: alloc.generate("page", &mut page_counter),
        name: None,
        objects,
        graphics: None,
        tool_specific: Vec::new(),
    };

    let tool_specific: Vec<ToolSpecific> = labels
        .and_then(NetLabels::nupn)
        .map(|nupn| ToolSpecific {
            tool: "nupn".to_owned(),
            version: "1.1".to_owned(),
            nupn_size: Some(nupn.size.clone()),
            nupn_structure: Some(nupn.structure.clone()),
            content: None,
        })
        .into_iter()
        .collect();

    net::PnmlNet {
        id: net_id,
        net_type: net_type::PT_NET.to_owned(),
        name: make_name(labels.and_then(NetLabels::net_name), None),
        pages: vec![page],
        tool_specific,
    }
}

impl Net {
    /// Exports this net as a single-net PNML document.
    ///
    /// The document contains one P/T `<net>` holding all places, transitions,
    /// and arcs of this net on a single page. No `<initialMarking>` labels are
    /// written; use [`PetriNet::to_pnml`] to export a marked system. Names,
    /// identifiers, graphics, and NUPN metadata are taken from the net's
    /// [`labels`](Net::labels) and [`graphics`](Net::graphics) fields when
    /// present; see the [module documentation](self) for the identifier
    /// assignment rules.
    ///
    /// Serialize the result with [`PnmlDocument::to_xml`].
    #[must_use]
    pub fn to_pnml(&self) -> PnmlDocument {
        PnmlDocument {
            nets: vec![export_net(self, None)],
        }
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Exports this system as a single-net PNML document.
    ///
    /// Equivalent to [`Net::to_pnml`], with the system's current marking
    /// written as the `<initialMarking>` labels of the exported places. The
    /// current marking is used because it is the de-facto initial marking for
    /// all analysis procedures; call [`reset`](Self::reset) first if the
    /// system has been simulated and the original initial marking is intended.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::builder::NetBuilder;
    /// use petrivet::pnml::PnmlDocument;
    /// use petrivet::prelude::PetriNet;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let t0 = b.add_transition();
    /// b.add_arcs((p0, t0, p1));
    /// let net = b.build().unwrap();
    ///
    /// let sys = PetriNet::new(net, [(p0, 1)]);
    /// let xml = sys.to_pnml().to_xml().unwrap();
    /// let reparsed = PnmlDocument::from_xml(&xml).unwrap();
    /// assert_eq!(reparsed.nets.len(), 1);
    /// ```
    #[must_use]
    pub fn to_pnml(&self) -> PnmlDocument {
        let marking = self.marking();
        PnmlDocument {
            nets: vec![export_net(self.net.as_ref(), Some(&marking))],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::NetBuilder;

    /// A two-place, two-transition cycle with two tokens on `p0`.
    fn simple_system() -> PetriNet<Net> {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let pt_net = b.build().expect("valid net");
        PetriNet::new(pt_net, [(p0, 2)])
    }

    fn places_of(pnml_net: &net::PnmlNet) -> Vec<&net::Place> {
        pnml_net.pages[0]
            .objects
            .iter()
            .filter_map(|o| if let PageObject::Place(p) = o { Some(p) } else { None })
            .collect()
    }

    fn arcs_of(pnml_net: &net::PnmlNet) -> Vec<&net::Arc> {
        pnml_net.pages[0]
            .objects
            .iter()
            .filter_map(|o| if let PageObject::Arc(a) = o { Some(a) } else { None })
            .collect()
    }

    #[test]
    fn unlabeled_net_exports_generated_ids() {
        let doc = simple_system().to_pnml();
        assert_eq!(doc.nets.len(), 1);
        let pnml_net = &doc.nets[0];
        assert_eq!(pnml_net.id, "net0");
        assert_eq!(pnml_net.net_type, net_type::PT_NET);
        assert_eq!(pnml_net.pages.len(), 1);
        assert_eq!(pnml_net.pages[0].id, "page0");

        let mut place_ids: Vec<&str> = places_of(pnml_net).iter().map(|p| p.id.as_str()).collect();
        place_ids.sort_unstable();
        assert_eq!(place_ids, ["p0", "p1"]);

        let mut arc_ids: Vec<&str> = arcs_of(pnml_net).iter().map(|a| a.id.as_str()).collect();
        arc_ids.sort_unstable();
        assert_eq!(arc_ids, ["a0", "a1", "a2", "a3"]);
    }

    #[test]
    fn marking_written_only_for_marked_places() {
        let doc = simple_system().to_pnml();
        let places = places_of(&doc.nets[0]);
        let marked: Vec<_> = places
            .iter()
            .filter_map(|p| p.initial_marking.as_ref().and_then(|m| m.text))
            .collect();
        assert_eq!(marked, [2], "exactly one place carries the two tokens");
        assert_eq!(
            places.iter().filter(|p| p.initial_marking.is_none()).count(),
            1,
            "the unmarked place has no initialMarking label"
        );
    }

    #[test]
    fn net_without_marking_exports_no_initial_marking() {
        let (net, _, _) = simple_system().into_parts();
        let doc = net.to_pnml();
        assert!(places_of(&doc.nets[0]).iter().all(|p| p.initial_marking.is_none()));
    }

    #[test]
    fn label_ids_and_names_are_reused() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let t0 = b.add_transition();
        b.add_arcs((p0, t0, p1));
        let mut pt_net = b.build().expect("valid net");

        let mut labels = NetLabels::new();
        labels
            .set_net_id("demo1")
            .set_net_name("Demo")
            .set_place_id(&p0, "idle-place")
            .set_place_name(p0, "Idle");
        pt_net.labels = Some(Box::new(labels));

        let doc = pt_net.to_pnml();
        let pnml_net = &doc.nets[0];
        assert_eq!(pnml_net.id, "demo1");
        assert_eq!(pnml_net.name.as_ref().and_then(|n| n.text.as_deref()), Some("Demo"));

        let places = places_of(pnml_net);
        let idle = places
            .iter()
            .find(|p| p.id == "idle-place")
            .expect("label-provided id is reused");
        assert_eq!(idle.name.as_ref().and_then(|n| n.text.as_deref()), Some("Idle"));
    }

    #[test]
    fn generated_ids_avoid_label_ids() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let t0 = b.add_transition();
        b.add_arcs((p0, t0, p1));
        let mut pt_net = b.build().expect("valid net");

        // One place claims "p0"; the generator must skip it for the other,
        // regardless of iteration order.
        let mut labels = NetLabels::new();
        labels.set_place_id(&p0, "p0");
        pt_net.labels = Some(Box::new(labels));

        let doc = pt_net.to_pnml();
        let mut place_ids: Vec<&str> = places_of(&doc.nets[0]).iter().map(|p| p.id.as_str()).collect();
        place_ids.sort_unstable();
        assert_eq!(place_ids, ["p0", "p1"]);
    }

    #[test]
    fn duplicate_label_ids_are_repaired() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let t0 = b.add_transition();
        b.add_arcs((p0, t0, p1));
        let mut pt_net = b.build().expect("valid net");

        // Both places claim the same identifier; the export must not emit it twice.
        let mut labels = NetLabels::new();
        labels.set_place_id(&p0, "dup").set_place_id(&p1, "dup");
        pt_net.labels = Some(Box::new(labels));

        let doc = pt_net.to_pnml();
        let mut place_ids: Vec<&str> = places_of(&doc.nets[0]).iter().map(|p| p.id.as_str()).collect();
        place_ids.sort_unstable();
        place_ids.dedup();
        assert_eq!(place_ids.len(), 2, "identifiers must remain unique");
        assert!(place_ids.contains(&"dup"), "the first claimant keeps the identifier");
    }

    #[test]
    fn arc_endpoints_reference_exported_node_ids() {
        let doc = simple_system().to_pnml();
        let pnml_net = &doc.nets[0];
        let node_ids: HashSet<&str> = pnml_net.pages[0]
            .objects
            .iter()
            .filter_map(|o| match o {
                PageObject::Place(p) => Some(p.id.as_str()),
                PageObject::Transition(t) => Some(t.id.as_str()),
                _ => None,
            })
            .collect();
        let arcs = arcs_of(pnml_net);
        assert_eq!(arcs.len(), 4);
        for arc in arcs {
            assert!(node_ids.contains(arc.source.as_str()), "dangling source {}", arc.source);
            assert!(node_ids.contains(arc.target.as_str()), "dangling target {}", arc.target);
            assert!(arc.inscription.is_none(), "unweighted arcs carry no inscription");
        }
    }
}
