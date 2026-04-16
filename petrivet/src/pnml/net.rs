use crate::pnml::graphics::{EdgeGraphics, NodeGraphics};
use crate::pnml::{net_type, ArcType, Name, NaturalNumberLabel, Page, PositiveIntegerLabel, ToolSpecific};
use serde::{Deserialize, Serialize};

/// A single Petri net model. The `type` URI selects the net type and therefore
/// which type-specific labels (`initialMarking`, `inscription`, etc.) are
/// meaningful on places, transitions, and arcs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnmlNet {
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