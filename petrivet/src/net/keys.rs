//! Stable opaque handles for places and transitions.
//!
//! Each key wraps a unique non-zero [`u64`] minted only by [`super::builder::NetBuilder`] and
//! preserved in [`super::Net`]. Ids are never reused across logical nodes, so keys are safe in
//! [`std::collections::HashMap`] and [`std::collections::HashSet`] even after
//! [`super::builder::NetBuilder::from`] round-trips that mix handles from a built [`super::Net`]
//! with newly minted keys.

use std::fmt;
use std::num::NonZeroU32;

/// Opaque handle to a place. Valid from the moment it is returned by
/// [`super::builder::NetBuilder::add_place`] through the lifetime of any
/// [`super::Net`] built from that builder (provided the place was not
/// removed before building).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub(crate) NonZeroU32);

/// Opaque handle to a transition. See [`Place`] for lifetime semantics.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition(pub(crate) NonZeroU32);

impl Place {
    #[must_use]
    pub(crate) fn from_raw(raw: u32) -> Self {
        Self(NonZeroU32::new(raw).expect("place key id must be non-zero"))
    }

    #[must_use]
    pub(crate) fn into_raw(self) -> u32 {
        self.0.get()
    }
}

impl Default for Place {
    fn default() -> Self {
        // [`super::node_map::IndexMap::new`] pads with defaults before every slot is filled.
        Self::from_raw(1)
    }
}

impl Transition {
    #[must_use]
    pub(crate) fn from_raw(raw: u32) -> Self {
        Self(NonZeroU32::new(raw).expect("transition key id must be non-zero"))
    }

    #[must_use]
    pub(crate) fn into_raw(self) -> u32 {
        self.0.get()
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::from_raw(1)
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.0.get())
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0.get())
    }
}
