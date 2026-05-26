use crate::{Arc, Place, Transition};
use ahash::HashMap;
use serde::{Deserialize, Serialize};

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
/// Graphics are indexed by the same [`PlaceIdx`], [`TransitionIdx`], and [`Arc`]
/// handles as the rest of the library. Arcs use a sparse [`HashMap`] because
/// most arcs carry no graphics and the library has no dense arc index.
#[derive(Debug, Clone, Default)]
pub struct PnmlGraphics {
    /// Node graphics for each place, indexed by [`PlaceIdx`].
    pub place_graphics: HashMap<Place, NodeGraphics>,
    /// Annotation graphics for each place's `<name>` label.
    pub place_name_graphics: HashMap<Place, AnnotationGraphics>,
    /// Annotation graphics for each place's `<initialMarking>` label.
    pub place_marking_graphics: HashMap<Place, AnnotationGraphics>,

    /// Node graphics for each transition, indexed by [`TransitionIdx`].
    pub transition_graphics: HashMap<Transition, NodeGraphics>,
    /// Annotation graphics for each transition's `<name>` label.
    pub transition_name_graphics: HashMap<Transition, AnnotationGraphics>,

    /// Edge graphics for each arc (sparse).
    pub arc_graphics: HashMap<Arc, EdgeGraphics>,
    /// Annotation graphics for each arc's inscription label (sparse).
    pub arc_inscription_graphics: HashMap<Arc, AnnotationGraphics>,
}

impl PnmlGraphics {
    /// Returns the PNML position of the place at dense index `index`, if present.
    #[must_use]
    pub fn place_position(&self, index: &Place) -> Option<&Coordinates> {
        self.place_graphics
            .get(index)?
            .position
            .as_ref()
    }

    /// Returns the PNML position of the transition at dense index `index`, if present.
    #[must_use]
    pub fn transition_position(&self, transition: &Transition) -> Option<&Coordinates> {
        self.transition_graphics
            .get(transition)?
            .position
            .as_ref()
    }
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