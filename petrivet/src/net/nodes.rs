//! Stable opaque handles for places and transitions.

use std::num::NonZeroU32;

/// A place in a net, often represented visually by a circle.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub(crate) NonZeroU32);

/// A transition in a net, often represented visually by a square / rectangle.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition(pub(crate) NonZeroU32);
