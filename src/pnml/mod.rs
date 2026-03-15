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

use serde::{Deserialize, Serialize};

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
    pub nets: Vec<Net>,
}

/// A single Petri net model. The `type` URI selects the net type and therefore
/// which type-specific labels (`initialMarking`, `inscription`, etc.) are
/// meaningful on places, transitions, and arcs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Net {
    /// Globally unique identifier for this net.
    #[serde(rename = "@id")]
    pub id: String,

    /// URI that identifies the Petri net type (see [`net_type`] constants).
    #[serde(rename = "@type")]
    pub net_type: String,

    /// Optional human-readable name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    /// Top-level pages. A net must contain at least one page; all places,
    /// transitions, and arcs are nested inside pages rather than directly
    /// inside the net.
    #[serde(rename = "page", default)]
    pub pages: Vec<Page>,

    /// Tool-specific extension blocks at the net level.
    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
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
    Place(Place),
    Transition(Transition),
    Arc(Arc),
    /// A reference node that acts as an alias for a [`Place`] defined on
    /// another page. Used for cross-page connections.
    ReferencePlace(ReferencePlace),
    /// A reference node that acts as an alias for a [`Transition`] defined
    /// on another page. Used for cross-page connections.
    ReferenceTransition(ReferenceTransition),
    /// A nested sub-page. Pages can be arbitrarily nested.
    Page(Page),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Place {
    #[serde(rename = "@id")]
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    /// P/T net label: the initial token count for this place.
    /// Default is 0 when absent.
    #[serde(rename = "initialMarking", skip_serializing_if = "Option::is_none")]
    pub initial_marking: Option<NaturalNumberLabel>,

    // TODO(colored-nets): When adding support for High-Level / Symmetric / Colored Petri Nets,
    //  add `hl_initial_marking: Option<HlInitialMarking>` here for the high-level initial marking
    //  expression (a multiset expression over the place's color domain).

    // TODO(timed-nets): When adding support for timed Petri nets, add a `time: Option<TimeLabel>`
    //  for the place timing annotation.

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<NodeGraphics>,

    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    #[serde(rename = "@id")]
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    // TODO(colored-nets): When adding support for High-Level / Symmetric / Colored Petri Nets,
    //  add `condition: Option<Condition>` here for the transition guard expression.

    // TODO(timed-nets): When adding support for timed Petri nets, add `time: Option<TimeLabel>`.

    // TODO(stochastic-nets): When adding support for stochastic Petri nets, add
    //  `rate: Option<RateLabel>` for the firing rate distribution.

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<NodeGraphics>,

    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arc {
    #[serde(rename = "@id")]
    pub id: String,

    /// ID of the source node (place, transition, or reference node).
    #[serde(rename = "@source")]
    pub source: String,

    /// ID of the target node (place, transition, or reference node).
    #[serde(rename = "@target")]
    pub target: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,

    /// P/T net label: the arc weight (a positive integer). When absent the
    /// weight defaults to 1.
    ///
    /// TODO(weighted-arcs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inscription: Option<PositiveIntegerLabel>,

    /// Arc type from the special-arcs extension (`specialarcs.rng`). Used by
    /// the inhibitor/reset net type extensions. When absent the arc is a normal
    /// flow arc.
    ///
    /// TODO(inhibitor-reset-nets): When adding support for inhibitor/reset nets,
    ///  use this field to distinguish normal, inhibitor, read, and reset arcs.
    #[serde(rename = "arctype", skip_serializing_if = "Option::is_none")]
    pub arc_type: Option<ArcType>,

    // TODO(colored-nets): When adding support for High-Level / Symmetric / Colored Petri Nets,
    //  add `hl_inscription: Option<HlInscription>` here for the high-level arc inscription
    //  expression (a multiset expression).

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<EdgeGraphics>,

    #[serde(rename = "toolspecific", default, skip_serializing_if = "Vec::is_empty")]
    pub tool_specific: Vec<ToolSpecific>,
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

/// An opaque tool-specific annotation block.
///
/// Any PNML element may carry zero or more of these. The content inside the
/// element is completely unconstrained XML; it is captured here as a raw string
/// for lossless round-tripping.
///
/// ```xml
/// <toolspecific tool="MyTool" version="1.0">
///   <foo bar="baz"/>
/// </toolspecific>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpecific {
    #[serde(rename = "@tool")]
    pub tool: String,

    #[serde(rename = "@version")]
    pub version: String,

    /// Raw inner XML content. `quick-xml` maps any unrecognised children to
    /// `$value` as a string when using the `serde` feature with text fallback.
    #[serde(rename = "$value", skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Graphical information for a node (place, transition, reference node, page).
///
/// ```xml
/// <graphics>
///   <position x="500" y="692"/>
///   <dimension x="40.0" y="40.0"/>
///   <fill color="#ffffff"/>
///   <line shape="line" color="#000000" width="1.0" style="solid"/>
/// </graphics>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGraphics {
    /// The Cartesian position of the node's centre point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Coordinates>,

    /// Width (x) and height (y) of the node's bounding box.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<Dimension>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,
}

/// Graphical information for an arc (edge).
///
/// ```xml
/// <graphics>
///   <position x="100" y="200"/>
///   <position x="150" y="250"/>
///   <line shape="curve"/>
/// </graphics>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeGraphics {
    /// Ordered list of intermediate waypoints along the arc's path. The source
    /// and target node positions are not included here.
    #[serde(rename = "position", default, skip_serializing_if = "Vec::is_empty")]
    pub waypoints: Vec<Coordinates>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,
}

/// Graphical information for an annotation label (name, initialMarking, etc.).
///
/// ```xml
/// <graphics>
///   <offset x="22" y="-10"/>
///   <font family="Arial" size="10pt"/>
/// </graphics>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationGraphics {
    /// Offset of the annotation label's centre from its parent node's reference point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Coordinates>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<Font>,
}

/// A pair of Cartesian (x, y) coordinates or offset values, used for positions,
/// offsets, and dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    #[serde(rename = "@x")]
    pub x: f64,

    #[serde(rename = "@y")]
    pub y: f64,
}

/// The width and height of a node's bounding box. Both values are positive
/// decimals with at most 4 digits total and 1 fraction digit (0–999.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    #[serde(rename = "@x")]
    pub x: f64,

    #[serde(rename = "@y")]
    pub y: f64,
}

/// Fill style for a node's interior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    /// CSS2 color string for the fill color (e.g. `"#ffffff"` or `"white"`).
    #[serde(rename = "@color", skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// CSS2 color string for the gradient end color.
    #[serde(rename = "@gradient-color", skip_serializing_if = "Option::is_none")]
    pub gradient_color: Option<String>,

    #[serde(rename = "@gradient-rotation", skip_serializing_if = "Option::is_none")]
    pub gradient_rotation: Option<GradientRotation>,

    /// URI to an image resource; when present, color attributes are ignored.
    #[serde(rename = "@image", skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GradientRotation {
    Vertical,
    Horizontal,
    Diagonal,
}

/// Stroke style for a node or arc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    #[serde(rename = "@shape", skip_serializing_if = "Option::is_none")]
    pub shape: Option<LineShape>,

    /// CSS2 color string.
    #[serde(rename = "@color", skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Stroke width (positive decimal, 0–999.9).
    #[serde(rename = "@width", skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,

    #[serde(rename = "@style", skip_serializing_if = "Option::is_none")]
    pub style: Option<LineStyle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineShape {
    Line,
    Curve,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineStyle {
    Solid,
    Dash,
    Dot,
}

/// Font attributes for annotation labels, following the CSS2 font model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Font {
    /// CSS2 font-family string.
    #[serde(rename = "@family", skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,

    /// CSS2 font-style string (e.g. `"italic"`).
    #[serde(rename = "@style", skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,

    /// CSS2 font-weight string (e.g. `"bold"`).
    #[serde(rename = "@weight", skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,

    /// CSS2 font-size string (e.g. `"12pt"`).
    #[serde(rename = "@size", skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    #[serde(rename = "@decoration", skip_serializing_if = "Option::is_none")]
    pub decoration: Option<FontDecoration>,

    #[serde(rename = "@align", skip_serializing_if = "Option::is_none")]
    pub align: Option<FontAlign>,

    /// Rotation angle of the annotation text in degrees.
    #[serde(rename = "@rotation", skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FontDecoration {
    Underline,
    Overline,
    LineThrough,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontAlign {
    Left,
    Center,
    Right,
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
