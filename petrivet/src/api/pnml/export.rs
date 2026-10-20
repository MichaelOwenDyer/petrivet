//! Conversion from petrivet's native types into the PNML data model.
//!
//! This is the reverse of [`super::import`]. The entry points are:
//!
//! - [`PetriNet::to_pnml_net`] / [`PetriNet::to_pnml`] — export a system
//!   together with its **current** marking.
//! - [`Net::to_pnml_net`] / [`Net::to_pnml`] — export the bare structure (no
//!   tokens).
//!
//! # Identifiers
//!
//! If the net carries [`NetLabels`], the stored PNML ids are reused. Every
//! other node receives a generated id (`p{n}`, `t{n}`, `a{n}`) that is
//! guaranteed not to collide with any id already in use. Names are only
//! written when a label is present.
//!
//! # Output shape
//!
//! The output is always a single P/T net on a single page (`page0`). The
//! original page hierarchy is not preserved, see [`super::PnmlGraphics`].
//! Node order is deterministic.
//!
//! # Known limitations
//!
//! petrivet nets are ordinary (every arc has weight one) and have only normal
//! arcs. Consequently no `<inscription>` and no `<arctype>` is ever written.

use super::graphics::{AnnotationGraphics, NodeGraphics};
use super::nupn::NupnMetadata;
use super::{Name, NaturalNumberLabel, Page, PageObject, PnmlDocument, ToolSpecific, net, net_type};
use crate::net::{Arc, Net, Place, Transition};
use crate::pnml::labels::NetLabels;
use crate::system::PetriNet;
use crate::system::marking::Marking;
use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use std::hash::Hash;

/// Assigns a unique PNML id to every key, preferring ids from `labels`.
///
/// `claimed` must already contain every id that is reserved by labels so that
/// generated ids never collide with them.
fn generate_missing_ids<K: Copy + Eq + Hash>(
    keys: &[K],
    mut ids: HashMap<K, String>,
    prefix: &str,
    used: &mut HashSet<String>,
) -> HashMap<K, String> {
    let mut next = 0usize;
    for &key in keys {
        if ids.contains_key(&key) {
            continue;
        }
        let id = loop {
            let candidate = format!("{prefix}{next}");
            next += 1;
            if used.insert(candidate.clone()) {
                break candidate;
            }
        };
        ids.insert(key, id);
    }
    ids
}

/// Keeps the label id of each key if it is not already taken by an earlier key.
fn claim_label_ids<'a, K: Copy + Eq + Hash>(
    keys: &[K],
    label: impl Fn(K) -> Option<&'a str>,
    used: &mut HashSet<String>,
) -> HashMap<K, String> {
    let mut ids = HashMap::new();
    for &key in keys {
        if let Some(id) = label(key)
            && used.insert(id.to_owned())
        {
            ids.insert(key, id.to_owned());
        }
    }
    ids
}

fn name_label(text: Option<&str>, graphics: Option<&AnnotationGraphics>) -> Option<Name> {
    if text.is_none() && graphics.is_none() {
        return None;
    }
    Some(Name {
        text: text.map(str::to_owned),
        graphics: graphics.cloned(),
    })
}

const fn arc_sort_key(arc: &Arc) -> (u8, u32, u32) {
    match arc {
        Arc::PlaceToTransition(p, t) => (0, p.0.get(), t.0.get()),
        Arc::TransitionToPlace(t, p) => (1, t.0.get(), p.0.get()),
    }
}

fn nupn_tool_specific(nupn: &NupnMetadata) -> ToolSpecific {
    ToolSpecific {
        tool: "nupn".to_owned(),
        version: "1.1".to_owned(),
        nupn_size: Some(nupn.size.clone()),
        nupn_structure: Some(nupn.structure.clone()),
        content: None,
    }
}

fn export_pt_net(net: &Net, marking: &Marking<u32>) -> net::PnmlNet {
    let labels: Option<&NetLabels> = net.labels.as_deref();
    let graphics = net.graphics.as_deref();

    let mut places: Vec<Place> = net.places().collect();
    places.sort_unstable_by_key(|p| p.0);
    let mut transitions: Vec<Transition> = net.transitions().collect();
    transitions.sort_unstable_by_key(|t| t.0);
    let mut arcs: Vec<Arc> = net.arcs().collect();
    arcs.sort_unstable_by_key(arc_sort_key);

    // Reserve all label ids of every kind before generating any id, because
    // PNML ids share a single namespace.
    let mut used = HashSet::new();
    let place_ids = claim_label_ids(&places, |p| labels.and_then(|l| l.place_id(p)), &mut used);
    let trans_ids = claim_label_ids(
        &transitions,
        |t| labels.and_then(|l| l.transition_id(t)),
        &mut used,
    );
    let arc_ids = claim_label_ids(&arcs, |a| labels.and_then(|l| l.arc_id(a)), &mut used);
    let place_ids = generate_missing_ids(&places, place_ids, "p", &mut used);
    let trans_ids = generate_missing_ids(&transitions, trans_ids, "t", &mut used);
    let arc_ids = generate_missing_ids(&arcs, arc_ids, "a", &mut used);

    let mut objects = Vec::with_capacity(places.len() + transitions.len() + arcs.len());

    for &p in &places {
        let tokens = marking.get(p);
        objects.push(PageObject::Place(net::Place {
            id: place_ids[&p].clone(),
            name: name_label(
                labels.and_then(|l| l.place_name(p)),
                graphics.and_then(|g| g.place_name_graphics.get(&p)),
            ),
            initial_marking: (tokens > 0).then(|| NaturalNumberLabel {
                text: Some(u64::from(tokens)),
                graphics: graphics.and_then(|g| g.place_marking_graphics.get(&p).cloned()),
            }),
            graphics: graphics.and_then(|g| g.place_graphics.get(&p).cloned()),
            tool_specific: Vec::new(),
        }));
    }

    for &t in &transitions {
        objects.push(PageObject::Transition(net::Transition {
            id: trans_ids[&t].clone(),
            name: name_label(
                labels.and_then(|l| l.transition_name(t)),
                graphics.and_then(|g| g.transition_name_graphics.get(&t)),
            ),
            graphics: graphics.and_then(|g| g.transition_graphics.get(&t).cloned()),
            tool_specific: Vec::new(),
        }));
    }

    for &arc in &arcs {
        let (source, target) = match arc {
            Arc::PlaceToTransition(p, t) => (place_ids[&p].clone(), trans_ids[&t].clone()),
            Arc::TransitionToPlace(t, p) => (trans_ids[&t].clone(), place_ids[&p].clone()),
        };
        // An inscription is never written (all weights are 1), so inscription
        // graphics have nothing to attach to.
        objects.push(PageObject::Arc(net::Arc {
            id: arc_ids[&arc].clone(),
            source,
            target,
            name: name_label(labels.and_then(|l| l.arc_name(arc)), None),
            inscription: None,
            arc_type: None,
            graphics: graphics.and_then(|g| g.arc_graphics.get(&arc).cloned()),
            tool_specific: Vec::new(),
        }));
    }

    net::PnmlNet {
        id: labels
            .and_then(NetLabels::net_id)
            .map_or_else(|| "net0".to_owned(), str::to_owned),
        net_type: net_type::PT_NET.to_owned(),
        name: name_label(labels.and_then(NetLabels::net_name), None),
        pages: vec![Page {
            id: "page0".to_owned(),
            name: None,
            objects,
            graphics: None::<NodeGraphics>,
            tool_specific: Vec::new(),
        }],
        tool_specific: labels
            .and_then(NetLabels::nupn)
            .map(nupn_tool_specific)
            .into_iter()
            .collect(),
    }
}

impl Net {
    /// Converts the structure of this net into a PNML P/T net with no tokens.
    #[must_use]
    pub fn to_pnml_net(&self) -> net::PnmlNet {
        export_pt_net(self, &Marking::default())
    }

    /// Serializes the structure of this net as a PNML document (no tokens).
    ///
    /// # Errors
    ///
    /// Returns a [`quick_xml::SeError`] if serialization fails.
    pub fn to_pnml(&self) -> Result<String, quick_xml::SeError> {
        PnmlDocument {
            nets: vec![self.to_pnml_net()],
        }
        .to_xml()
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Converts this system into a PNML P/T net. The **current** marking is
    /// exported as the `initialMarking` of each place.
    #[must_use]
    pub fn to_pnml_net(&self) -> net::PnmlNet {
        export_pt_net(self.net.as_ref(), &self.marking())
    }

    /// Serializes this system as a PNML document containing a single P/T net,
    /// using the **current** marking as the initial marking.
    ///
    /// # Errors
    ///
    /// Returns a [`quick_xml::SeError`] if serialization fails.
    pub fn to_pnml(&self) -> Result<String, quick_xml::SeError> {
        PnmlDocument {
            nets: vec![self.to_pnml_net()],
        }
        .to_xml()
    }
}

#[cfg(test)]
mod tests {
    use crate::net::Net;
    use crate::net::builder::NetBuilder;
    use crate::pnml::{PageObject, PnmlDocument};
    use crate::system::PetriNet;

    fn two_place_cycle() -> PetriNet<Net> {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        PetriNet::new(b.build().unwrap(), [(p0, 1)])
    }

    fn ids(xml: &str) -> Vec<String> {
        let doc = PnmlDocument::from_xml(xml).unwrap();
        doc.nets[0].pages[0]
            .objects
            .iter()
            .map(|o| match o {
                PageObject::Place(p) => p.id.clone(),
                PageObject::Transition(t) => t.id.clone(),
                PageObject::Arc(a) => a.id.clone(),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn unlabelled_net_gets_generated_unique_ids_and_no_names() {
        let sys = two_place_cycle();
        let xml = sys.to_pnml().unwrap();
        let all = ids(&xml);
        assert_eq!(all.len(), 2 + 2 + 4);
        let unique: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len());
        assert!(!xml.contains("<name>"));
    }

    #[test]
    fn output_is_deterministic() {
        let sys = two_place_cycle();
        assert_eq!(sys.to_pnml().unwrap(), sys.to_pnml().unwrap());
    }

    #[test]
    fn exports_current_marking_not_initial() {
        let mut sys = two_place_cycle();
        sys.fire_any().unwrap();
        let reimported = PetriNet::from_pnml(&sys.to_pnml().unwrap()).unwrap();
        assert_eq!(reimported.marking(), sys.marking());
    }

    #[test]
    fn zero_marking_writes_no_initial_marking() {
        let net = two_place_cycle().into_parts().0;
        assert!(!net.to_pnml().unwrap().contains("initialMarking"));
    }

    #[test]
    fn generated_ids_avoid_label_ids() {
        use crate::pnml::labels::NetLabels;
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let t0 = b.add_transition();
        b.add_arcs((p0, t0, p1));
        let mut net = b.build().unwrap();
        let mut labels = NetLabels::new();
        // Occupies the id the generator would hand out first for places.
        labels.set_transition_id(t0, "p0");
        net.set_labels(labels);
        let all = ids(&net.to_pnml().unwrap());
        let unique: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len());
        assert!(all.contains(&"p0".to_owned()));
    }
}
