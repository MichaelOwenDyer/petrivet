//! [Nested-Unit Petri Net](https://mcc.lip6.fr/2026/nupn.php) (NUPN) metadata
//! carried in PNML as `<toolspecific tool="nupn" version="1.1">…</toolspecific>`.
//!
//! MCC attaches this block to describe concurrent structure: a tree of *units*,
//! each owning *local* places and child units. When [`NupnStructure::unit_safe`]
//! is `true`, the file author asserts **unit-safety** (at most one local token
//! per unit along an active branch of the hierarchy; see the MCC definition).
//! Under that assumption, each unit contributes linear inequalities that hold on
//! every reachable marking (token counts on local places of a unit sum to at
//! most one in a one-safe setting, with exclusivity across an active chain).
//! Verification tools can exploit such structure—for example by encoding the
//! marking with one “active unit” pointer per hierarchy level instead of one
//! bit per place—potentially shrinking the symbolic state description from
//! linear in the number of places to roughly linear in the number of units
//! (hence the “logarithmic reductions” wording when the unit tree is shallow
//! compared to the flat place count).

use super::net;
use super::{Page, PageObject, ToolSpecific};
use serde::{Deserialize, Serialize};

/// Declared place / transition / arc counts from `<size …/>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NupnSize {
    #[serde(rename = "@places")]
    pub places: u64,
    #[serde(rename = "@transitions")]
    pub transitions: u64,
    #[serde(rename = "@arcs")]
    pub arcs: u64,
}

/// Whitespace-separated PNML ids inside `<places>` or `<subunits>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NupnIdList {
    #[serde(rename = "$text", default)]
    pub raw: String,
}

impl NupnIdList {
    /// Iterator over place or unit identifiers in document order of tokens.
    #[must_use]
    pub fn ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.raw.split_whitespace()
    }

    /// Collected id strings (allocating).
    #[must_use]
    pub fn id_vec(&self) -> Vec<String> {
        self.ids().map(str::to_owned).collect()
    }

    /// True when there is no id (empty element or only whitespace).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.split_whitespace().next().is_none()
    }
}

/// One `<unit id="…">` with local places and child units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NupnUnit {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(default)]
    pub places: NupnIdList,
    #[serde(default)]
    pub subunits: NupnIdList,
}

/// `<structure units="U" root="R" safe="S">…</structure>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NupnStructure {
    /// Declared number of units (including the root); should match [`Self::units`].len().
    #[serde(rename = "@units")]
    pub unit_count: u64,
    /// `id` of the root unit.
    #[serde(rename = "@root")]
    pub root_unit_id: String,
    /// When `true`, the author asserts the **unit-safe** property (MCC / NUPN definition).
    #[serde(rename = "@safe")]
    pub unit_safe: bool,
    #[serde(rename = "unit", default)]
    pub units: Vec<NupnUnit>,
}

/// Parsed NUPN block: `<size/>` plus `<structure>…</structure>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NupnMetadata {
    pub size: NupnSize,
    pub structure: NupnStructure,
}

impl NupnMetadata {
    /// Shorthand for [`Self::structure`].[`NupnStructure::unit_safe`].
    #[must_use]
    pub fn unit_safe_declared(&self) -> bool {
        self.structure.unit_safe
    }

    /// Extract the first well-formed NUPN block from `net` (net-level
    /// `<toolspecific>` first, then depth-first over pages).
    #[must_use]
    pub fn extract_from_pnml_net(net: &net::PnmlNet) -> Option<Self> {
        for ts in &net.tool_specific {
            if let Some(m) = Self::from_tool_specific(ts) {
                return Some(m);
            }
        }
        for page in &net.pages {
            if let Some(m) = Self::from_page(page) {
                return Some(m);
            }
        }
        None
    }

    fn from_tool_specific(ts: &ToolSpecific) -> Option<Self> {
        if ts.tool != "nupn" || ts.version != "1.1" {
            return None;
        }
        match (&ts.nupn_size, &ts.nupn_structure) {
            (Some(size), Some(structure)) => Some(Self {
                size: size.clone(),
                structure: structure.clone(),
            }),
            _ => None,
        }
    }

    fn from_page(page: &Page) -> Option<Self> {
        for ts in &page.tool_specific {
            if let Some(m) = Self::from_tool_specific(ts) {
                return Some(m);
            }
        }
        for obj in &page.objects {
            if let PageObject::Page(sub) = obj {
                if let Some(m) = Self::from_page(sub) {
                    return Some(m);
                }
            }
        }
        None
    }
}
