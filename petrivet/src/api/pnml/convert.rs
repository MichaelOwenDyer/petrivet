//! Conversion from the PNML data model into petrivet's native types.
//!
//! The entry points are methods on [`super::PnmlNet`]:
//!
//! - [`super::PnmlNet::to_petri_net`] — URI-dispatched conversion returning a
//!   [`PetriNetKind`] enum. Use this when you do not know the net type in
//!   advance or when you want to handle multiple net types uniformly.
//! - [`super::PnmlNet::to_pt_system`] — convenience method that asserts the net
//!   is a P/T net and fails with [`PnmlConversionError::WrongNetType`] if not.
//!
//! Both methods return a triple `(System<Net>, NetLabels, PnmlGraphics)`.
//!
//! [`PnmlDocument::to_petri_nets`] applies the dispatch conversion to
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
//! converted into a runnable [`PetriNet`]. All other net type URIs produce
//! [`PetriNetKind::Unsupported`] — the PNML data model is still fully parsed,
//! but no attempt is made to interpret type-specific labels whose semantics
//! are not yet implemented. This is intentional: silently ignoring high-level
//! inscriptions or color declarations would produce structurally wrong nets.

use super::{net, net_type, nupn::NupnMetadata, PageObject, PnmlDocument};
use crate::pnml::graphics::PnmlGraphics;
use crate::pnml::labels::NetLabels;
use crate::prelude::{Arc, Marking, Net, NetBuilder, NetError, PetriNet, Place, Transition};
use ahash::{HashMap, HashMapExt};

/// Errors that can occur when converting a [`super::PnmlNet`] PNML model into a
/// native petrivet [`PetriNet`].
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
    InvalidTopology(NetError),
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
            Self::InvalidTopology(build_err) =>
                write!(f, "invalid net topology: {build_err}"),
        }
    }
}

impl std::error::Error for PnmlConversionError {}

impl From<NetError> for PnmlConversionError {
    fn from(e: NetError) -> Self {
        Self::InvalidTopology(e)
    }
}

/// The result of a URI-dispatched PNML conversion.
///
/// Each variant corresponds to a class of Petri net type. Currently only P/T
/// nets are converted into a runnable [`PetriNet`]; all other type URIs produce
/// [`PetriNetKind::Unsupported`].
///
/// # Adding support for new net types
///
/// Add a new variant here (e.g. `ColoredNet(System<ColoredNet>, NetLabels,
/// PnmlGraphics)`) and extend the `match` in [`super::PnmlNet::to_petri_net`].
#[derive(Debug)]
pub enum PetriNetKind {
    /// A fully converted P/T net system, ready for simulation and analysis.
    PtNet(Box<PetriNet<Net>>),

    /// The net's type URI is recognised but not yet supported by this library.
    /// The raw PNML data model is available via [`PnmlDocument`] if
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
    places:      Vec<&'a net::Place>,
    transitions: Vec<&'a net::Transition>,
    arcs:        Vec<&'a net::Arc>,
    /// Maps every PNML `id` that belongs to a place to the place.
    place_by_id: HashMap<&'a str, &'a net::Place>,
    /// Maps every PNML `id` that belongs to a transition to the transition.
    trans_by_id: HashMap<&'a str, &'a net::Transition>,
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
    pnml_net: &net::PnmlNet,
) -> Result<PetriNet<Net>, PnmlConversionError> {
    let mut flat = FlatNet::new();
    for page in &pnml_net.pages {
        flat.visit_page(page)?;
    }

    let mut builder = NetBuilder::new();

    let places: HashMap<&str, Place> = flat
        .places
        .iter()
        .map(|p| (p.id.as_str(), builder.add_place()))
        .collect();

    let transitions: HashMap<&str, Transition> = flat
        .transitions
        .iter()
        .map(|t| (t.id.as_str(), builder.add_transition()))
        .collect();

    for &pnml_arc in &flat.arcs {
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

        match (
            places.get(src_id),
            transitions.get(src_id),
            places.get(tgt_id),
            transitions.get(tgt_id),
        ) {
            (Some(&p), None, None, Some(&t)) => {
                builder.add_arc((p, t));
            }
            (None, Some(&t), Some(&p), None) => {
                builder.add_arc((t, p));
            }
            _ => {
                let bad = if places.contains_key(src_id) || transitions.contains_key(src_id) {
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

    // Initial marking: index by the dense Place handle.
    let initial_marking: Marking<u32> = flat.places
        .iter()
        .filter_map(|p| {
            p.initial_marking.as_ref().and_then(|m| m.text).map(|count| {
                (places[p.id.as_str()], u32::try_from(count).unwrap_or(u32::MAX))
            })
        })
        .collect();

    let mut place_names = HashMap::new();
    let mut place_ids_map = HashMap::new();
    for pnml_place in &flat.places {
        let p = places[pnml_place.id.as_str()];
        place_ids_map.insert(p, pnml_place.id.clone());
        pnml_place.name.as_ref()
            .and_then(|n| n.text.clone())
            .map(|n| place_names.insert(p, n));
    }

    let mut transition_names = HashMap::new();
    let mut transition_ids_map = HashMap::new();
    for pnml_trans in &flat.transitions {
        let t = transitions[pnml_trans.id.as_str()];
        transition_ids_map.insert(t, pnml_trans.id.clone());
        pnml_trans.name.as_ref()
            .and_then(|n| n.text.clone())
            .map(|n| transition_names.insert(t, n));
    }

    let mut arc_names: HashMap<Arc, String> = HashMap::new();
    let mut arc_ids: HashMap<Arc, String> = HashMap::new();
    for pnml_arc in &flat.arcs {
        let Some(src_id) = flat.resolve(&pnml_arc.source) else { continue };
        let Some(tgt_id) = flat.resolve(&pnml_arc.target) else { continue };
        let arc = match (places.get(src_id), transitions.get(src_id),
                         places.get(tgt_id), transitions.get(tgt_id)) {
            (Some(&p), None, None, Some(&t)) => Arc::PlaceToTransition(p, t),
            (None, Some(&t), Some(&p), None) => Arc::TransitionToPlace(t, p),
            _ => continue,
        };
        arc_ids.insert(arc, pnml_arc.id.clone());
        if let Some(name) = pnml_arc.name.as_ref().and_then(|n| n.text.clone()) {
            arc_names.insert(arc, name);
        }
    }

    let nupn = NupnMetadata::extract_from_pnml_net(pnml_net);

    let labels = NetLabels::from_raw(
        place_names,
        place_ids_map,
        transition_names,
        transition_ids_map,
        arc_names,
        arc_ids,
        pnml_net.name.as_ref().and_then(|n| n.text.clone()),
        Some(pnml_net.id.clone()),
        nupn,
    );

    // Graphics
    let mut place_graphics = HashMap::new();
    let mut place_name_graphics = HashMap::new();
    let mut place_marking_graphics = HashMap::new();
    for pnml_place in &flat.places {
        let p = places[pnml_place.id.as_str()];
        pnml_place.graphics
            .as_ref()
            .map(|g| place_graphics.insert(p, g.clone()));
        pnml_place.name.as_ref()
            .and_then(|n| n.graphics.clone())
            .map(|n| place_name_graphics.insert(p, n));
        pnml_place.initial_marking.as_ref()
            .and_then(|m| m.graphics.clone())
            .map(|m| place_marking_graphics.insert(p, m));
    }

    let mut transition_graphics = HashMap::new();
    let mut transition_name_graphics = HashMap::new();
    for pnml_trans in &flat.transitions {
        let t = transitions[pnml_trans.id.as_str()];
        pnml_trans.graphics.as_ref()
            .map(|g| transition_graphics.insert(t, g.clone()));
        pnml_trans.name.as_ref()
            .and_then(|n| n.graphics.clone())
            .map(|g| transition_name_graphics.insert(t, g));
    }

    let mut arc_graphics = HashMap::new();
    let mut arc_inscription_graphics = HashMap::new();
    for pnml_arc in &flat.arcs {
        let Some(src_id) = flat.resolve(&pnml_arc.source) else { continue };
        let Some(tgt_id) = flat.resolve(&pnml_arc.target) else { continue };
        let arc = match (
            places.get(src_id),
            transitions.get(src_id),
            places.get(tgt_id),
            transitions.get(tgt_id)
        ) {
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
        place_graphics,
        place_name_graphics,
        place_marking_graphics,
        transition_graphics,
        transition_name_graphics,
        arc_graphics,
        arc_inscription_graphics,
    };

    let mut net = builder.build()?;
    net.set_labels(labels);
    net.set_graphics(graphics);
    let system = PetriNet::new(net, initial_marking);

    Ok(system)
}

impl net::PnmlNet {
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
                let sys = convert_pt_net(self)?;
                Ok(PetriNetKind::PtNet(Box::new(sys)))
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
    pub fn to_pt_system(&self) -> Result<PetriNet<Net>, PnmlConversionError> {
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
        self.nets.iter().map(net::PnmlNet::to_petri_net).collect()
    }
}