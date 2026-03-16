//! Conversion from the PNML data model into petrivet's native types.
//!
//! The entry points are methods on [`super::Net`]:
//!
//! - [`super::Net::to_petri_net`] — URI-dispatched conversion returning a
//!   [`PetriNetKind`] enum. Use this when you do not know the net type in
//!   advance or when you want to handle multiple net types uniformly.
//! - [`super::Net::to_pt_system`] — convenience method that asserts the net
//!   is a P/T net and fails with [`PnmlConversionError::WrongNetType`] if not.
//!
//! Both methods return a triple `(System<Net>, NetLabels, PnmlGraphics)`.
//!
//! [`super::PnmlDocument::to_petri_nets`] applies the dispatch conversion to
//! every net in the document and returns one result per net.
//!
//! # Page flattening
//!
//! PNML organises places, transitions, and arcs inside `<page>` elements that
//! can be arbitrarily nested. This converter flattens all pages recursively
//! into a single pool of nodes. Page membership is **not** preserved in the
//! output — see [`PnmlGraphics`] for the note on what *is* preserved and what
//! is a known limitation.
//!
//! # Reference nodes
//!
//! `<referencePlace>` and `<referenceTransition>` elements are resolved to the
//! canonical node they point to (following chains of references if necessary).
//! Cycles in the reference chain are treated as unresolvable and cause a
//! [`PnmlConversionError::UnresolvedReference`].
//!
//! # Unsupported net types
//!
//! Only P/T nets (`http://www.pnml.org/version-2009/grammar/ptnet`) are
//! converted into a runnable [`System`]. All other net type URIs produce
//! [`PetriNetKind::Unsupported`] — the PNML data model is still fully parsed,
//! but no attempt is made to interpret type-specific labels whose semantics
//! are not yet implemented. This is intentional: silently ignoring high-level
//! inscriptions or color declarations would produce structurally wrong nets.

use std::collections::HashMap;

use crate::labeled::NetLabels;
use crate::net::builder::{BuildError, NetBuilder};
use crate::net::{Arc, Net, Place, Transition};
use crate::system::System;

use super::{
    AnnotationGraphics, EdgeGraphics, NodeGraphics, PageObject, PnmlDocument,
    net_type,
};

/// Errors that can occur when converting a [`super::Net`] PNML model into a
/// native petrivet [`System`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PnmlConversionError {
    /// The net's type URI does not match the requested conversion. The URI
    /// that was found is returned.
    WrongNetType(String),

    /// An arc's `source` or `target` attribute refers to an ID that does not
    /// correspond to any place or transition in this net (after resolving all
    /// reference nodes).
    UnresolvedArcEndpoint {
        arc_id: String,
        endpoint_id: String,
    },

    /// A `<referencePlace>` or `<referenceTransition>` `ref` attribute points
    /// to an ID that cannot be resolved, or the reference chain forms a cycle.
    UnresolvedReference(String),

    /// Multiple nodes in the document share the same `id` attribute.
    DuplicateId(String),

    /// The topology could not form a valid net (empty or disconnected).
    InvalidTopology(String),
}

impl std::fmt::Display for PnmlConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongNetType(uri) =>
                write!(f, "expected a P/T net but found type URI '{uri}'"),
            Self::UnresolvedArcEndpoint { arc_id, endpoint_id } =>
                write!(f, "arc '{arc_id}': endpoint '{endpoint_id}' does not resolve to any place or transition"),
            Self::UnresolvedReference(id) =>
                write!(f, "reference node '{id}' could not be resolved (dangling or cyclic reference)"),
            Self::DuplicateId(id) =>
                write!(f, "duplicate PNML id '{id}'"),
            Self::InvalidTopology(msg) =>
                write!(f, "invalid net topology: {msg}"),
        }
    }
}

impl std::error::Error for PnmlConversionError {}

impl From<BuildError> for PnmlConversionError {
    fn from(e: BuildError) -> Self {
        Self::InvalidTopology(e.to_string())
    }
}

/// Graphical layout data extracted from a PNML document.
///
/// This type is returned alongside [`System`] and [`NetLabels`] by the
/// conversion functions. It is a read-only record of the layout information
/// present in the source PNML file. The library has no mechanism to actually
/// *use* graphical data for rendering; this type exists solely so that
/// round-tripping through petrivet does not silently discard layout.
///
/// # Known limitation: page structure
///
/// PNML's hierarchical page structure is **not** preserved. All nodes are
/// flattened into a single pool during conversion. If you re-serialise to PNML
/// after conversion, all elements will be placed on a single default page.
/// Preserving page membership would require additional bookkeeping that is not
/// yet implemented.
///
/// Graphics are indexed by the same [`Place`], [`Transition`], and [`Arc`]
/// handles as the rest of the library. Arcs use a sparse [`HashMap`] because
/// most arcs carry no graphics and the library has no dense arc index.
#[derive(Debug, Clone, Default)]
pub struct PnmlGraphics {
    /// Node graphics for each place, indexed by `Place::idx`.
    pub place_graphics: Box<[Option<NodeGraphics>]>,
    /// Annotation graphics for each place's `<name>` label.
    pub place_name_graphics: Box<[Option<AnnotationGraphics>]>,
    /// Annotation graphics for each place's `<initialMarking>` label.
    pub place_marking_graphics: Box<[Option<AnnotationGraphics>]>,

    /// Node graphics for each transition, indexed by `Transition::idx`.
    pub transition_graphics: Box<[Option<NodeGraphics>]>,
    /// Annotation graphics for each transition's `<name>` label.
    pub transition_name_graphics: Box<[Option<AnnotationGraphics>]>,

    /// Edge graphics for each arc (sparse).
    pub arc_graphics: HashMap<Arc, EdgeGraphics>,
    /// Annotation graphics for each arc's inscription label (sparse).
    pub arc_inscription_graphics: HashMap<Arc, AnnotationGraphics>,
}

/// The result of a URI-dispatched PNML conversion.
///
/// Each variant corresponds to a class of Petri net type. Currently only P/T
/// nets are converted into a runnable [`System`]; all other type URIs produce
/// [`PetriNetKind::Unsupported`].
///
/// # Adding support for new net types
///
/// Add a new variant here (e.g. `ColoredNet(System<ColoredNet>, NetLabels,
/// PnmlGraphics)`) and extend the `match` in [`super::Net::to_petri_net`].
#[derive(Debug)]
pub enum PetriNetKind {
    /// A fully converted P/T net system, ready for simulation and analysis.
    ///
    /// The three components are boxed to keep the enum variant size manageable
    /// given that [`PnmlGraphics`] alone can be several hundred bytes.
    PtNet(Box<System<Net>>, Box<NetLabels>, Box<PnmlGraphics>),

    /// The net's type URI is recognised but not yet supported by this library.
    /// The raw PNML data model is available via [`super::PnmlDocument`] if
    /// needed.
    ///
    /// TODO(colored-nets): replace with `ColoredNet(...)` variant.
    /// TODO(timed-nets):   replace with `TimedNet(...)` variant.
    /// TODO(inhibitor-reset-nets): replace with `ExtendedPtNet(...)` variant.
    Unsupported {
        /// The net type URI that was found.
        uri: String,
        /// The net's `id` attribute, for diagnostic purposes.
        net_id: String,
    },
}

/// All data collected by walking the page tree of a single PNML net.
struct FlatNet<'a> {
    places:      Vec<&'a super::Place>,
    transitions: Vec<&'a super::Transition>,
    arcs:        Vec<&'a super::Arc>,
    /// Maps every PNML `id` that belongs to a place to the place.
    place_by_id: HashMap<&'a str, &'a super::Place>,
    /// Maps every PNML `id` that belongs to a transition to the transition.
    trans_by_id: HashMap<&'a str, &'a super::Transition>,
    /// Maps reference-node IDs to the `ref` target they point at.
    /// Used for a second pass to resolve chains.
    ref_map:     HashMap<&'a str, &'a str>,
}

impl<'a> FlatNet<'a> {
    fn new() -> Self {
        Self {
            places: Vec::new(),
            transitions: Vec::new(),
            arcs: Vec::new(),
            place_by_id: HashMap::new(),
            trans_by_id: HashMap::new(),
            ref_map: HashMap::new(),
        }
    }

    /// Walk one page and all nested sub-pages recursively, collecting nodes.
    fn visit_page(
        &mut self,
        page: &'a super::Page,
    ) -> Result<(), PnmlConversionError> {
        for obj in &page.objects {
            match obj {
                PageObject::Place(p) => {
                    if self.place_by_id.insert(p.id.as_str(), p).is_some()
                        || self.trans_by_id.contains_key(p.id.as_str())
                    {
                        return Err(PnmlConversionError::DuplicateId(p.id.clone()));
                    }
                    self.places.push(p);
                }
                PageObject::Transition(t) => {
                    if self.trans_by_id.insert(t.id.as_str(), t).is_some()
                        || self.place_by_id.contains_key(t.id.as_str())
                    {
                        return Err(PnmlConversionError::DuplicateId(t.id.clone()));
                    }
                    self.transitions.push(t);
                }
                PageObject::Arc(a) => {
                    self.arcs.push(a);
                }
                PageObject::ReferencePlace(rp) => {
                    self.ref_map.insert(rp.id.as_str(), rp.refers_to.as_str());
                }
                PageObject::ReferenceTransition(rt) => {
                    self.ref_map.insert(rt.id.as_str(), rt.refers_to.as_str());
                }
                PageObject::Page(sub) => {
                    self.visit_page(sub)?;
                }
            }
        }
        Ok(())
    }

    /// Resolve `id` through any chain of reference nodes, returning the
    /// canonical place or transition ID. Returns `None` if the chain is
    /// dangling or cyclic (detected by a depth limit).
    fn resolve<'b>(&self, mut id: &'b str) -> Option<&'b str>
    where
        'a: 'b,
    {
        // A chain longer than the total number of nodes must contain a cycle.
        let max_depth = self.ref_map.len() + 1;
        for _ in 0..max_depth {
            if self.place_by_id.contains_key(id) || self.trans_by_id.contains_key(id) {
                return Some(id);
            }
            match self.ref_map.get(id) {
                Some(&next) => id = next,
                None => return None,
            }
        }
        None // cycle detected
    }
}

/// Build a `(System<Net>, NetLabels, PnmlGraphics)` triple from the flat
/// representation of a P/T net. This function contains all the business logic
/// shared by `to_petri_net` and `to_pt_system`.
#[expect(clippy::too_many_lines)]
fn convert_pt_net(
    pnml_net: &super::Net,
) -> Result<(System<Net>, NetLabels, PnmlGraphics), PnmlConversionError> {
    let mut flat = FlatNet::new();
    for page in &pnml_net.pages {
        flat.visit_page(page)?;
    }

    let n_places = flat.places.len();
    let n_transitions = flat.transitions.len();

    // Maps PNML id → library index handle.
    let place_index: HashMap<&str, Place> = flat.places
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), Place::from_index(i)))
        .collect();

    let trans_index: HashMap<&str, Transition> = flat.transitions
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), Transition::from_index(i)))
        .collect();

    let mut builder = NetBuilder::with_capacity(n_places, n_transitions);

    for pnml_arc in &flat.arcs {
        let src_id = flat.resolve(&pnml_arc.source).ok_or_else(|| {
            PnmlConversionError::UnresolvedArcEndpoint {
                arc_id: pnml_arc.id.clone(),
                endpoint_id: pnml_arc.source.clone(),
            }
        })?;
        let tgt_id = flat.resolve(&pnml_arc.target).ok_or_else(|| {
            PnmlConversionError::UnresolvedArcEndpoint {
                arc_id: pnml_arc.id.clone(),
                endpoint_id: pnml_arc.target.clone(),
            }
        })?;

        match (place_index.get(src_id), trans_index.get(src_id),
               place_index.get(tgt_id), trans_index.get(tgt_id)) {
            (Some(&p), None, None, Some(&t)) => { builder.add_arc((p, t)); }
            (None, Some(&t), Some(&p), None) => { builder.add_arc((t, p)); }
            _ => {
                // Arc connects two places, two transitions, or has an endpoint
                // that resolves to neither — treat as unresolved.
                let bad = if place_index.contains_key(src_id) || trans_index.contains_key(src_id) {
                    pnml_arc.target.clone()
                } else {
                    pnml_arc.source.clone()
                };
                return Err(PnmlConversionError::UnresolvedArcEndpoint {
                    arc_id: pnml_arc.id.clone(),
                    endpoint_id: bad,
                });
            }
        }
    }

    let net = builder.build()?;

    let mut tokens = vec![0u32; n_places];
    for pnml_place in &flat.places {
        if let Some(marking) = &pnml_place.initial_marking
            && let Some(count) = marking.text
        {
            let idx = place_index[pnml_place.id.as_str()].idx;
            tokens[idx] = u32::try_from(count).unwrap_or(u32::MAX);
        }
    }
    let system = System::new(net, tokens);

    let mut place_names = vec![None; n_places].into_boxed_slice();
    let mut place_ids   = vec![None; n_places].into_boxed_slice();
    for pnml_place in &flat.places {
        let idx = place_index[pnml_place.id.as_str()].idx;
        place_ids[idx]   = Some(pnml_place.id.clone());
        place_names[idx] = pnml_place.name.as_ref()
            .and_then(|n| n.text.clone());
    }

    let mut transition_names = vec![None; n_transitions].into_boxed_slice();
    let mut transition_ids   = vec![None; n_transitions].into_boxed_slice();
    for pnml_trans in &flat.transitions {
        let idx = trans_index[pnml_trans.id.as_str()].idx;
        transition_ids[idx]   = Some(pnml_trans.id.clone());
        transition_names[idx] = pnml_trans.name.as_ref()
            .and_then(|n| n.text.clone());
    }

    let mut arc_names: HashMap<Arc, String> = HashMap::new();
    let mut arc_ids:   HashMap<Arc, String> = HashMap::new();
    for pnml_arc in &flat.arcs {
        // We only populate arc labels for arcs that resolved successfully.
        let Some(src_id) = flat.resolve(&pnml_arc.source) else { continue };
        let Some(tgt_id) = flat.resolve(&pnml_arc.target) else { continue };
        let arc = match (place_index.get(src_id), trans_index.get(src_id),
                         place_index.get(tgt_id), trans_index.get(tgt_id)) {
            (Some(&p), None, None, Some(&t)) => Arc::PlaceToTransition(p, t),
            (None, Some(&t), Some(&p), None) => Arc::TransitionToPlace(t, p),
            _ => continue,
        };
        arc_ids.insert(arc, pnml_arc.id.clone());
        if let Some(name) = pnml_arc.name.as_ref().and_then(|n| n.text.clone()) {
            arc_names.insert(arc, name);
        }
    }

    let labels = NetLabels::from_raw(
        place_names,
        place_ids,
        transition_names,
        transition_ids,
        arc_names,
        arc_ids,
        pnml_net.name.as_ref().and_then(|n| n.text.clone()),
        Some(pnml_net.id.clone()),
    );

    let mut place_graphics        = vec![None; n_places];
    let mut place_name_graphics   = vec![None; n_places];
    let mut place_marking_graphics = vec![None; n_places];
    for pnml_place in &flat.places {
        let idx = place_index[pnml_place.id.as_str()].idx;
        place_graphics[idx].clone_from(&pnml_place.graphics);
        place_name_graphics[idx] = pnml_place.name.as_ref()
            .and_then(|n| n.graphics.clone());
        place_marking_graphics[idx] = pnml_place.initial_marking.as_ref()
            .and_then(|m| m.graphics.clone());
    }

    let mut transition_graphics      = vec![None; n_transitions];
    let mut transition_name_graphics = vec![None; n_transitions];
    for pnml_trans in &flat.transitions {
        let idx = trans_index[pnml_trans.id.as_str()].idx;
        transition_graphics[idx].clone_from(&pnml_trans.graphics);
        transition_name_graphics[idx] = pnml_trans.name.as_ref()
            .and_then(|n| n.graphics.clone());
    }

    let mut arc_graphics             = HashMap::new();
    let mut arc_inscription_graphics = HashMap::new();
    for pnml_arc in &flat.arcs {
        let Some(src_id) = flat.resolve(&pnml_arc.source) else { continue };
        let Some(tgt_id) = flat.resolve(&pnml_arc.target) else { continue };
        let arc = match (place_index.get(src_id), trans_index.get(src_id),
                         place_index.get(tgt_id), trans_index.get(tgt_id)) {
            (Some(&p), None, None, Some(&t)) => Arc::PlaceToTransition(p, t),
            (None, Some(&t), Some(&p), None) => Arc::TransitionToPlace(t, p),
            _ => continue,
        };
        if let Some(g) = pnml_arc.graphics.clone() {
            arc_graphics.insert(arc, g);
        }
        if let Some(g) = pnml_arc.inscription.as_ref().and_then(|i| i.graphics.clone()) {
            arc_inscription_graphics.insert(arc, g);
        }
    }

    let graphics = PnmlGraphics {
        place_graphics:          place_graphics.into_boxed_slice(),
        place_name_graphics:     place_name_graphics.into_boxed_slice(),
        place_marking_graphics:  place_marking_graphics.into_boxed_slice(),
        transition_graphics:     transition_graphics.into_boxed_slice(),
        transition_name_graphics: transition_name_graphics.into_boxed_slice(),
        arc_graphics,
        arc_inscription_graphics,
    };

    Ok((system, labels, graphics))
}

impl super::Net {
    /// Converts this PNML net into a native petrivet type by dispatching on
    /// the net's type URI.
    ///
    /// - P/T nets produce [`PetriNetKind::PtNet`].
    /// - All other type URIs produce [`PetriNetKind::Unsupported`] — the PNML
    ///   data model is fully parsed but no library type is constructed.
    ///
    /// Use [`Self::to_pt_system`] if you know you are working with a P/T net
    /// and want a direct result without matching on the enum.
    ///
    /// # Errors
    ///
    /// Returns [`PnmlConversionError`] if the P/T net topology is invalid or
    /// the document contains structural errors (duplicate IDs, dangling arc
    /// endpoints, etc.). Unsupported net types never produce an error; they
    /// are represented as [`PetriNetKind::Unsupported`].
    pub fn to_petri_net(&self) -> Result<PetriNetKind, PnmlConversionError> {
        match self.net_type.as_str() {
            net_type::PT_NET => {
                let (sys, labels, graphics) = convert_pt_net(self)?;
                Ok(PetriNetKind::PtNet(Box::new(sys), Box::new(labels), Box::new(graphics)))
            }
            other => Ok(PetriNetKind::Unsupported {
                uri: other.to_owned(),
                net_id: self.id.clone(),
            }),
        }
    }

    /// Converts this PNML net into a P/T net system, asserting that the net's
    /// type URI is [`net_type::PT_NET`].
    ///
    /// Returns `(System<Net>, NetLabels, PnmlGraphics)`.
    ///
    /// # Errors
    ///
    /// - [`PnmlConversionError::WrongNetType`] if the type URI is not the P/T
    ///   net URI.
    /// - Any other [`PnmlConversionError`] variant if the topology is
    ///   structurally invalid.
    pub fn to_pt_system(
        &self,
    ) -> Result<(System<Net>, NetLabels, PnmlGraphics), PnmlConversionError> {
        if self.net_type != net_type::PT_NET {
            return Err(PnmlConversionError::WrongNetType(self.net_type.clone()));
        }
        convert_pt_net(self)
    }
}

impl PnmlDocument {
    /// Converts every net in the document, returning one result per net.
    ///
    /// The order of results matches the order of `<net>` elements in the
    /// document. Errors for individual nets do not affect the conversion of
    /// other nets.
    pub fn to_petri_nets(&self) -> Vec<Result<PetriNetKind, PnmlConversionError>> {
        self.nets.iter().map(super::Net::to_petri_net).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pnml::PnmlDocument;

    fn parse(xml: &str) -> PnmlDocument {
        PnmlDocument::from_xml(xml).expect("parse failed")
    }

    /// Minimal PT net: p0 -[1]-> t0 -[1]-> p1, initial marking [1, 0].
    const MINIMAL_PT: &str = r#"
        <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
          <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
            <name><text>Minimal</text></name>
            <page id="page0">
              <place id="p0">
                <name><text>Source</text></name>
                <initialMarking><text>1</text></initialMarking>
              </place>
              <place id="p1">
                <name><text>Sink</text></name>
              </place>
              <transition id="t0">
                <name><text>Flow</text></name>
              </transition>
              <arc id="a0" source="p0" target="t0">
                <inscription><text>1</text></inscription>
              </arc>
              <arc id="a1" source="t0" target="p1"/>
            </page>
          </net>
        </pnml>
    "#;

    #[test]
    fn convert_minimal_pt_net() {
        let doc = parse(MINIMAL_PT);
        let (sys, labels, _graphics) = doc.nets[0].to_pt_system().expect("conversion failed");

        assert_eq!(sys.net().place_count(), 2);
        assert_eq!(sys.net().transition_count(), 1);
        assert_eq!(sys.current_marking().iter().copied().collect::<Vec<u32>>(), vec![1, 0]);

        assert_eq!(labels.net_name(), Some("Minimal"));
        assert_eq!(labels.net_id(), Some("n0"));
    }

    #[test]
    fn labels_populated_correctly() {
        let doc = parse(MINIMAL_PT);
        let (sys, labels, _) = doc.nets[0].to_pt_system().unwrap();

        // Places are assigned indices in document order.
        let p0 = Place::from_index(0);
        let p1 = Place::from_index(1);
        let t0 = Transition::from_index(0);

        assert_eq!(labels.place_name(p0), Some("Source"));
        assert_eq!(labels.place_name(p1), Some("Sink"));
        assert_eq!(labels.place_id(p0), Some("p0"));
        assert_eq!(labels.place_id(p1), Some("p1"));
        assert_eq!(labels.transition_name(t0), Some("Flow"));
        assert_eq!(labels.transition_id(t0), Some("t0"));

        // Arc IDs are stored in the labels map.
        let arc_pt = Arc::PlaceToTransition(p0, sys.net().postset_p(p0).iter().copied().next().unwrap());
        assert!(labels.arc_id(arc_pt).is_some());
    }

    #[test]
    fn missing_initial_marking_defaults_to_zero() {
        let doc = parse(MINIMAL_PT);
        let (sys, _, _) = doc.nets[0].to_pt_system().unwrap();
        // p1 has no <initialMarking>; should default to 0.
        assert_eq!(sys.current_marking().iter().nth(1).copied(), Some(0));
    }

    #[test]
    fn dispatch_returns_pt_net_variant() {
        let doc = parse(MINIMAL_PT);
        let kind = doc.nets[0].to_petri_net().unwrap();
        assert!(matches!(kind, PetriNetKind::PtNet(..)));
    }

    #[test]
    fn dispatch_unsupported_net_type() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n1" type="http://www.pnml.org/version-2009/grammar/symmetricnet">
                <page id="p0"/>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let kind = doc.nets[0].to_petri_net().unwrap();
        assert!(matches!(
            kind,
            PetriNetKind::Unsupported { uri, .. } if uri.contains("symmetricnet")
        ));
    }

    #[test]
    fn to_pt_system_wrong_type_error() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n1" type="http://www.pnml.org/version-2009/grammar/highlevelnet">
                <page id="p0"/>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let err = doc.nets[0].to_pt_system().unwrap_err();
        assert!(matches!(err, PnmlConversionError::WrongNetType(_)));
    }

    #[test]
    fn dangling_arc_endpoint_is_error() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0">
                  <place id="p0"/>
                  <transition id="t0"/>
                  <arc id="a0" source="p0" target="DOES_NOT_EXIST"/>
                </page>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let err = doc.nets[0].to_pt_system().unwrap_err();
        assert!(matches!(err, PnmlConversionError::UnresolvedArcEndpoint { .. }));
    }

    #[test]
    fn duplicate_id_is_error() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0">
                  <place id="same"/>
                  <transition id="same"/>
                </page>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let err = doc.nets[0].to_pt_system().unwrap_err();
        assert!(matches!(err, PnmlConversionError::DuplicateId(_)));
    }

    #[test]
    fn multi_page_net_is_flattened() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="page0">
                  <place id="p0"><initialMarking><text>1</text></initialMarking></place>
                  <transition id="t0"/>
                  <arc id="a0" source="p0" target="t0"/>
                </page>
                <page id="page1">
                  <place id="p1"/>
                  <arc id="a1" source="t0" target="p1"/>
                </page>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let (sys, _, _) = doc.nets[0].to_pt_system().expect("should flatten two pages");
        assert_eq!(sys.net().place_count(), 2);
        assert_eq!(sys.net().transition_count(), 1);
    }

    #[test]
    fn nested_page_is_flattened() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="outer">
                  <place id="p0"><initialMarking><text>1</text></initialMarking></place>
                  <page id="inner">
                    <place id="p1"/>
                    <transition id="t0"/>
                    <arc id="a0" source="p0" target="t0"/>
                    <arc id="a1" source="t0" target="p1"/>
                  </page>
                </page>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let (sys, _, _) = doc.nets[0].to_pt_system().expect("should flatten nested page");
        assert_eq!(sys.net().place_count(), 2);
        assert_eq!(sys.net().transition_count(), 1);
    }

    #[test]
    fn graphics_are_extracted() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0">
                  <place id="pl0">
                    <graphics><position x="10" y="20"/></graphics>
                    <initialMarking>
                      <text>1</text>
                      <graphics><offset x="5" y="5"/></graphics>
                    </initialMarking>
                  </place>
                  <transition id="t0">
                    <graphics><position x="50" y="20"/></graphics>
                  </transition>
                  <arc id="a0" source="pl0" target="t0">
                    <graphics><position x="30" y="20"/></graphics>
                  </arc>
                  <place id="p_sink"/>
                  <arc id="a1" source="t0" target="p_sink"/>
                </page>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let (sys, _, graphics) = doc.nets[0].to_pt_system().unwrap();

        let pl0 = Place::from_index(0);
        let t0  = Transition::from_index(0);

        // Place position
        #[allow(clippy::float_cmp)]
        {
            let pos = graphics.place_graphics[pl0.idx]
                .as_ref().and_then(|g| g.position.as_ref()).unwrap();
            assert_eq!(pos.x, 10.0);
            assert_eq!(pos.y, 20.0);
        }

        // Marking annotation graphics
        assert!(graphics.place_marking_graphics[pl0.idx].is_some());

        // Transition position
        #[allow(clippy::float_cmp)]
        {
            let pos = graphics.transition_graphics[t0.idx]
                .as_ref().and_then(|g| g.position.as_ref()).unwrap();
            assert_eq!(pos.x, 50.0);
        }

        // Arc graphics (sparse map)
        let p_sink = Place::from_index(1);
        let arc = Arc::PlaceToTransition(pl0, sys.net().postset_p(pl0).iter().copied()
            .find(|&t| t == t0).unwrap());
        assert!(graphics.arc_graphics.contains_key(&arc));
        let _ = p_sink;
    }

    #[test]
    fn document_to_petri_nets() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n0" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0">
                  <place id="pl0"><initialMarking><text>1</text></initialMarking></place>
                  <transition id="t0"/>
                  <arc id="a0" source="pl0" target="t0"/>
                  <place id="pl1"/>
                  <arc id="a1" source="t0" target="pl1"/>
                </page>
              </net>
              <net id="n1" type="http://www.pnml.org/version-2009/grammar/symmetricnet">
                <page id="p0"/>
              </net>
            </pnml>
        "#;
        let doc = parse(xml);
        let results = doc.to_petri_nets();
        assert_eq!(results.len(), 2);
        assert!(matches!(results[0], Ok(PetriNetKind::PtNet(..))));
        assert!(matches!(results[1], Ok(PetriNetKind::Unsupported { .. })));
    }
}
