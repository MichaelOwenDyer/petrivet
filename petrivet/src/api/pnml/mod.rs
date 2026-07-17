//! PNML (Petri Net Markup Language) serialization and deserialization.
//!
//! This module implements the PNML 2009 grammar as defined at:
//! <https://www.pnml.org/version-2009/version-2009.php>
//!
//! The data model covers the complete PNML core structure (`pnmlcoremodel.rng`),
//! the P/T net type definition (`ptnet.pntd`), and the special-arc extension
//! (`specialarcs.rng`). All net-type-specific labels that are not yet used by
//! this library are preserved in the data model with comments indicating where
//! to hook them in when support is added.
//!
//! # PNML Document Structure
//!
//! ```text
//! <pnml>                           ← PnmlDocument
//!   <net id="..." type="...">      ← Net  (one or more per document)
//!     <name><text>...</text></name>
//!     <page id="...">              ← Page (one or more per net; pages nest)
//!       <place id="...">           ← Place
//!         <name>...</name>
//!         <initialMarking>         ← P/T net: non-negative integer token count
//!           <text>N</text>
//!         </initialMarking>
//!         <graphics>...</graphics>
//!       </place>
//!       <transition id="...">      ← Transition
//!         <name>...</name>
//!         <graphics>...</graphics>
//!       </transition>
//!       <arc id="..." source="..." target="...">  ← Arc
//!         <inscription>            ← P/T net: positive integer arc weight
//!           <text>N</text>
//!         </inscription>
//!         <arctype>normal|inhibitor|read|reset</arctype>  ← extension
//!         <graphics>...</graphics>
//!       </arc>
//!       <referencePlace id="..." ref="..."/>    ← cross-page alias for a place
//!       <referenceTransition id="..." ref="..."/>  ← cross-page alias for a transition
//!       <page id="...">...</page>  ← nested sub-pages are allowed
//!     </page>
//!     <toolspecific tool="..." version="...">  ← arbitrary tool-specific XML
//!       ...
//!     </toolspecific>
//!   </net>
//! </pnml>
//! ```
//!
//! # Net Type URIs
//!
//! The `type` attribute on `<net>` identifies the Petri net type:
//!
//! | URI | Net type |
//! |-----|----------|
//! | `http://www.pnml.org/version-2009/grammar/ptnet` | Place/Transition net (P/T net) |
//! | `http://www.pnml.org/version-2009/grammar/pnmlcoremodel` | Bare core model (no type-specific labels) |
//! | `http://www.pnml.org/version-2009/grammar/symmetricnet` | Symmetric net (Colored PN subset) |
//! | `http://www.pnml.org/version-2009/grammar/highlevelnet` | High-level Petri net graph |
//! | `http://www.pnml.org/version-2009/grammar/pt-hlpng` | High-level P/T net graph |
//! | `http://www.pnml.org/version-2009/extensions/inhibitorptnet` | P/T net with inhibitor arcs |
//! | `http://www.pnml.org/version-2009/extensions/resetptnet` | P/T net with reset arcs |
//! | `http://www.pnml.org/version-2009/extensions/resetinhibitorptnet` | P/T net with inhibitor and reset arcs |

use crate::pnml::graphics::{AnnotationGraphics, NodeGraphics};
use serde::{Deserialize, Serialize};

pub mod convert;
pub mod export;
pub mod graphics;
pub mod net;
pub mod labels;
pub mod nupn;

pub use nupn::{NupnIdList, NupnMetadata, NupnSize, NupnStructure, NupnUnit};

pub mod net_type {
    pub const PT_NET: &str = "http://www.pnml.org/version-2009/grammar/ptnet";
    pub const CORE_MODEL: &str = "http://www.pnml.org/version-2009/grammar/pnmlcoremodel";
    pub const SYMMETRIC_NET: &str = "http://www.pnml.org/version-2009/grammar/symmetricnet";
    pub const HIGH_LEVEL_NET: &str = "http://www.pnml.org/version-2009/grammar/highlevelnet";
    pub const PT_HLPNG: &str = "http://www.pnml.org/version-2009/grammar/pt-hlpng";
    pub const INHIBITOR_NET: &str = "http://www.pnml.org/version-2009/extensions/inhibitorptnet";
    pub const RESET_NET: &str = "http://www.pnml.org/version-2009/extensions/resetptnet";
    pub const RESET_INHIBITOR_NET: &str = "http://www.pnml.org/version-2009/extensions/resetinhibitorptnet";
}

/// The root element of a PNML file. A single `.pnml` file may contain one or
/// more independent Petri net models.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "pnml")]
pub struct PnmlDocument {
    #[serde(rename = "net", default)]
    pub nets: Vec<net::PnmlNet>,
}

/// A page groups net objects (places, transitions, arcs, and sub-pages).
///
/// Pages support hierarchical decomposition: a net may be split across multiple
/// pages, with [`ReferencePlace`] and [`ReferenceTransition`] nodes providing
/// cross-page references. When flattening to a `Net` structure, all pages should
/// be walked recursively.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    #[serde(rename = "@id")]
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    /// All net objects contained in this page (places, transitions, arcs,
    /// reference nodes, and nested sub-pages), in document order.
    #[serde(rename = "$value", default)]
    pub objects: Vec<PageObject>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<NodeGraphics>,

    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
}

/// Any object that can appear as a direct child of a [`Page`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageObject {
    Place(net::Place),
    Transition(net::Transition),
    Arc(net::Arc),
    /// A reference node that acts as an alias for a [`Place`] defined on
    /// another page. Used for cross-page connections.
    ReferencePlace(ReferencePlace),
    /// A reference node that acts as an alias for a [`Transition`] defined
    /// on another page. Used for cross-page connections.
    ReferenceTransition(ReferenceTransition),
    /// A nested sub-page. Pages can be arbitrarily nested.
    Page(Page),
}

/// A reference to a [`Place`] defined on another page. Allows arcs to span
/// across page boundaries without duplicating the actual place node.
///
/// Validation constraints (from the grammar):
/// - `ref` MUST refer to the `id` of a place or another reference place.
/// - `ref` MUST NOT create a cycle of reference places.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencePlace {
    #[serde(rename = "@id")]
    pub id: String,

    /// The `id` of the place (or another reference place) this node aliases.
    #[serde(rename = "@ref")]
    pub refers_to: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<NodeGraphics>,

    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
}

/// A reference to a [`Transition`] defined on another page.
///
/// Validation constraints (from the grammar):
/// - `ref` MUST refer to the `id` of a transition or another reference transition.
/// - `ref` MUST NOT create a cycle of reference transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceTransition {
    #[serde(rename = "@id")]
    pub id: String,

    /// The `id` of the transition (or another reference transition) this node aliases.
    #[serde(rename = "@ref")]
    pub refers_to: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<NodeGraphics>,

    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
}

/// A human-readable name annotation. The text value is wrapped in a `<text>`
/// child element. The annotation may also carry graphical positioning.
///
/// ```xml
/// <name>
///   <text>MyPlace</text>
///   <graphics><offset x="22" y="-10"/></graphics>
/// </name>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name {
    /// The actual name string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Graphical offset of the name label relative to its owning node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<AnnotationGraphics>,
}

/// A label carrying a non-negative integer value, used for `<initialMarking>`
/// in P/T nets. The integer is wrapped in a `<text>` child element.
///
/// ```xml
/// <initialMarking>
///   <text>3</text>
///   <graphics><offset x="22" y="20"/></graphics>
/// </initialMarking>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NaturalNumberLabel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<AnnotationGraphics>,
}

/// A label carrying a positive integer value (≥ 1), used for `<inscription>`
/// (arc weight) in P/T nets.
///
/// ```xml
/// <inscription>
///   <text>1</text>
/// </inscription>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositiveIntegerLabel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<AnnotationGraphics>,
}

/// The type of a special arc, from the `specialarcs.rng` extension. Used by
/// inhibitor, reset, and combined inhibitor-reset net type definitions.
///
/// ```xml
/// <arctype>inhibitor</arctype>
/// ```
///
/// TODO(inhibitor-reset-nets): Wire this into the net conversion logic when
/// inhibitor/reset arc support is added to the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArcType {
    Normal,
    Inhibitor,
    Read,
    Reset,
}

/// Tool-specific annotation block (`<toolspecific tool="…" version="…">`).
///
/// For [`tool = "nupn"` and `version = "1.1"`](https://mcc.lip6.fr/2026/nupn.php)
/// (MCC nested-unit metadata), [`Self::nupn_size`] and [`Self::nupn_structure`]
/// are populated. Other tools may use arbitrary XML: unknown **element** tags
/// are ignored by `quick-xml` as long as this struct does not use [`$value`](https://github.com/tafia/quick-xml/issues/596);
/// direct text nodes are captured in [`Self::content`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpecific {
    #[serde(rename = "@tool")]
    pub tool: String,

    #[serde(rename = "@version")]
    pub version: String,

    /// MCC NUPN: `<size places="…" transitions="…" arcs="…"/>`.
    #[serde(rename = "size", default, skip_serializing_if = "Option::is_none")]
    pub nupn_size: Option<nupn::NupnSize>,

    /// MCC NUPN: `<structure …>…</structure>`.
    #[serde(rename = "structure", default, skip_serializing_if = "Option::is_none")]
    pub nupn_structure: Option<nupn::NupnStructure>,

    /// Direct text inside `<toolspecific>` (non-NUPN tools).
    #[serde(rename = "$text", default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl PnmlDocument {
    /// Deserialize a `PnmlDocument` from an XML string.
    ///
    /// # Errors
    ///
    /// Returns a [`quick_xml::DeError`] if the input is not valid XML or does
    /// not conform to the expected PNML structure.
    pub fn from_xml(xml: &str) -> Result<Self, quick_xml::DeError> {
        quick_xml::de::from_str(xml)
    }

    /// Serialize a `PnmlDocument` to a pretty-printed XML string.
    ///
    /// # Errors
    ///
    /// Returns a [`quick_xml::SeError`] if serialization of any field fails
    /// (e.g. a string value contains characters that cannot be encoded as XML).
    pub fn to_xml(&self) -> Result<String, quick_xml::SeError> {
        let mut buf = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        buf.push('\n');
        let mut ser = quick_xml::se::Serializer::new(&mut buf);
        ser.indent(' ', 2);
        self.serialize(ser)?;
        Ok(buf)
    }
}

impl std::fmt::Display for PnmlDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_xml().map_err(|_| std::fmt::Error).and_then(|s| f.write_str(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal well-formed P/T net document.
    const MINIMAL_XML: &str = r#"
        <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
          <net id="net1" type="http://www.pnml.org/version-2009/grammar/ptnet">
            <name><text>Minimal</text></name>
            <page id="page0">
              <place id="p0">
                <name><text>P0</text></name>
                <initialMarking><text>1</text></initialMarking>
              </place>
              <place id="p1">
                <name><text>P1</text></name>
              </place>
              <transition id="t0">
                <name><text>T0</text></name>
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
    fn parse_minimal_pt_net() {
        let doc = PnmlDocument::from_xml(MINIMAL_XML).expect("parse failed");
        assert_eq!(doc.nets.len(), 1);

        let net = &doc.nets[0];
        assert_eq!(net.id, "net1");
        assert_eq!(net.net_type, net_type::PT_NET);
        assert_eq!(net.name.as_ref().and_then(|n| n.text.as_deref()), Some("Minimal"));
        assert_eq!(net.pages.len(), 1);

        let page = &net.pages[0];
        let places: Vec<_> = page.objects.iter().filter_map(|o| {
            if let PageObject::Place(p) = o { Some(p) } else { None }
        }).collect();
        let transition_count = page.objects.iter().filter(|o| matches!(o, PageObject::Transition(_))).count();
        let arc_count = page.objects.iter().filter(|o| matches!(o, PageObject::Arc(_))).count();

        assert_eq!(places.len(), 2);
        assert_eq!(transition_count, 1);
        assert_eq!(arc_count, 2);

        let p0 = places.iter().find(|p| p.id == "p0").expect("p0");
        assert_eq!(p0.initial_marking.as_ref().and_then(|m| m.text), Some(1));

        let p1 = places.iter().find(|p| p.id == "p1").expect("p1");
        assert!(p1.initial_marking.is_none());
    }

    /// Round-trip: parse → serialize → parse and check the result is identical.
    #[test]
    fn round_trip_minimal() {
        let doc1 = PnmlDocument::from_xml(MINIMAL_XML).expect("first parse");
        let xml = doc1.to_xml().expect("serialize");
        let doc2 = PnmlDocument::from_xml(&xml).expect("second parse");

        assert_eq!(doc1.nets.len(), doc2.nets.len());
        let n1 = &doc1.nets[0];
        let n2 = &doc2.nets[0];
        assert_eq!(n1.id, n2.id);
        assert_eq!(n1.net_type, n2.net_type);
        assert_eq!(n1.pages[0].objects.len(), n2.pages[0].objects.len());
    }

    /// Verify that graphics data is preserved during deserialization.
    #[test]
    #[allow(clippy::float_cmp)]
    fn parse_graphics() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n1" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0">
                  <place id="pl0">
                    <name>
                      <text>Fork</text>
                      <graphics><offset x="22" y="-10"/></graphics>
                    </name>
                    <graphics>
                      <position x="500" y="692"/>
                    </graphics>
                    <initialMarking>
                      <text>1</text>
                      <graphics><offset x="22" y="20"/></graphics>
                    </initialMarking>
                  </place>
                  <arc id="a0" source="pl0" target="pl0">
                    <graphics>
                      <position x="100" y="200"/>
                      <line shape="curve"/>
                    </graphics>
                  </arc>
                </page>
              </net>
            </pnml>
        "#;
        let doc = PnmlDocument::from_xml(xml).expect("parse failed");
        let page = &doc.nets[0].pages[0];

        let place = page.objects.iter().find_map(|o| {
            if let PageObject::Place(p) = o { Some(p) } else { None }
        }).expect("place");

        let pos = place.graphics.as_ref()
            .and_then(|g| g.position.as_ref())
            .expect("position");
        assert_eq!(pos.x, 500.0);
        assert_eq!(pos.y, 692.0);

        let name_offset = place.name.as_ref()
            .and_then(|n| n.graphics.as_ref())
            .and_then(|g| g.offset.as_ref())
            .expect("name offset");
        assert_eq!(name_offset.x, 22.0);
        assert_eq!(name_offset.y, -10.0);

        let arc = page.objects.iter().find_map(|o| {
            if let PageObject::Arc(a) = o { Some(a) } else { None }
        }).expect("arc");
        let waypoints = &arc.graphics.as_ref().expect("arc graphics").waypoints;
        assert_eq!(waypoints.len(), 1);
        assert_eq!(waypoints[0].x, 100.0);
        assert_eq!(waypoints[0].y, 200.0);
    }

    /// Verify that the arc-type extension is parsed.
    #[test]
    fn parse_arc_type_inhibitor() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n1" type="http://www.pnml.org/version-2009/extensions/inhibitorptnet">
                <page id="p0">
                  <arc id="a0" source="p0" target="t0">
                    <arctype>inhibitor</arctype>
                  </arc>
                </page>
              </net>
            </pnml>
        "#;
        let doc = PnmlDocument::from_xml(xml).expect("parse failed");
        let arc = doc.nets[0].pages[0].objects.iter().find_map(|o| {
            if let PageObject::Arc(a) = o { Some(a) } else { None }
        }).expect("arc");
        assert_eq!(arc.arc_type, Some(ArcType::Inhibitor));
    }

    /// NUPN `<toolspecific>` blocks (MCC benchmark archives) contain nested
    /// elements; they must not break deserialization.
    #[test]
    fn parse_toolspecific_nupn_inner_elements() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n1" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="page">
                  <place id="p0"><initialMarking><text>1</text></initialMarking></place>
                  <toolspecific tool="nupn" version="1.1">
                    <size places="1" transitions="0" arcs="0"/>
                    <structure units="1" root="u0" safe="true">
                      <unit id="u0"><places>p0</places><subunits/></unit>
                    </structure>
                  </toolspecific>
                </page>
              </net>
            </pnml>
        "#;
        let doc = PnmlDocument::from_xml(xml).expect("parse failed");
        let page = &doc.nets[0].pages[0];
        let ts = page.tool_specific.iter().find(|t| t.tool == "nupn").expect("nupn");
        assert_eq!(ts.version, "1.1");
        assert!(ts.content.is_none());
        let sz = ts.nupn_size.as_ref().expect("nupn size");
        assert_eq!(sz.places, 1);
        assert_eq!(sz.transitions, 0);
        assert_eq!(sz.arcs, 0);
        let st = ts.nupn_structure.as_ref().expect("nupn structure");
        assert!(st.unit_safe);
        assert_eq!(st.root_unit_id, "u0");
        assert_eq!(st.units.len(), 1);
        assert_eq!(st.units[0].id, "u0");
        assert_eq!(st.units[0].places.id_vec(), vec!["p0"]);
        assert!(st.units[0].subunits.is_empty());
        let meta = nupn::NupnMetadata::extract_from_pnml_net(&doc.nets[0]).expect("metadata");
        assert!(meta.unit_safe_declared());
        let p0 = page.objects.iter().find_map(|o| {
            if let PageObject::Place(p) = o { Some(p) } else { None }
        }).expect("place");
        assert_eq!(p0.initial_marking.as_ref().and_then(|m| m.text), Some(1));
    }

    /// Multiple nets in a single document.
    #[test]
    fn parse_multiple_nets() {
        let xml = r#"
            <pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
              <net id="n1" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0"/>
              </net>
              <net id="n2" type="http://www.pnml.org/version-2009/grammar/ptnet">
                <page id="p0"/>
              </net>
            </pnml>
        "#;
        let doc = PnmlDocument::from_xml(xml).expect("parse failed");
        assert_eq!(doc.nets.len(), 2);
        assert_eq!(doc.nets[0].id, "n1");
        assert_eq!(doc.nets[1].id, "n2");
    }
}
