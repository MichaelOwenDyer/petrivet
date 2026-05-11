//! Stable opaque handles for places and transitions.
//!
//! [`Ord`] and [`PartialOrd`] on these types follow **creation order**:
//! [`Place`] and [`Transition`] ids are assigned monotonically when minted by
//! [`NetBuilder`](crate::net::builder::NetBuilder).

use std::num::NonZeroU32;

/// A place in a net, often represented visually by a circle.
///
/// Comparisons use **creation order**; see the [`crate::net::nodes`] module.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub(crate) NonZeroU32);

/// A transition in a net, often represented visually by a square / rectangle.
///
/// Comparisons use **creation order**; see the [`crate::net::nodes`] module.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition(pub(crate) NonZeroU32);
