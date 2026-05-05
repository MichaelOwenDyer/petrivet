//! Stable opaque handles for places and transitions.

use std::num::NonZeroU32;

/// Opaque handle to a place.
///
/// Valid from the moment it is returned by [`NetBuilder::add_place`]
/// through the lifetime of any [`super::Net`] built from that builder
/// (provided the place was not removed before building).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(NonZeroU32);

/// Opaque handle to a transition.
///
/// Valid from the moment it is returned by [`NetBuilder::add_transition`]
/// through the lifetime of any [`super::Net`] built from that builder
/// (provided the transition was not removed before building).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition(NonZeroU32);

impl Place {
    #[must_use]
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(NonZeroU32::new(raw).expect("place key id must be non-zero"))
    }

    #[must_use]
    pub(crate) const fn into_raw(self) -> u32 {
        self.0.get()
    }
}

impl Transition {
    #[must_use]
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(NonZeroU32::new(raw).expect("transition key id must be non-zero"))
    }

    #[must_use]
    pub(crate) const fn into_raw(self) -> u32 {
        self.0.get()
    }
}

impl Default for Place {
    fn default() -> Self {
        Self::from_raw(1)
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::from_raw(1)
    }
}
