//! Structural classification types for Petri nets.
//!
//! The primary type is [`ClassifiedNet`], returned by the builder. It carries
//! the structural class internally so analysis methods dispatch automatically.
//!
//! For power users who want compile-time guarantees, newtype wrappers
//! ([`SNet`], [`TNet`], [`Circuit`], [`FreeChoiceNet`]) are available via
//! [`TryFrom`] conversions.

use crate::net::{Net, NetClass};
use std::ops::Deref;

/// A net with its structural class determined at build time.
///
/// This is the default type returned by [`NetBuilder::build`](super::builder::NetBuilder::build).
/// Analysis methods on [`System<ClassifiedNet>`](crate::analysis::System) dispatch
/// to the best algorithm based on the stored class.
///
/// Users don't need to interact with the classification directly — just build
/// the net and ask questions. The library picks the best algorithm silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedNet {
    net: Net,
    class: NetClass,
}

impl ClassifiedNet {
    pub(crate) fn new(net: Net, class: NetClass) -> Self {
        Self { net, class }
    }

    /// Returns the structural class detected at build time.
    #[must_use]
    pub fn class(&self) -> NetClass {
        self.class
    }

    /// Returns a reference to the underlying [`Net`].
    #[must_use]
    pub fn net(&self) -> &Net {
        &self.net
    }

    /// Consumes this wrapper and returns the inner [`Net`].
    #[must_use]
    pub fn into_net(self) -> Net {
        self.net
    }
}

impl Deref for ClassifiedNet {
    type Target = Net;
    fn deref(&self) -> &Net {
        &self.net
    }
}

impl AsRef<Net> for ClassifiedNet {
    fn as_ref(&self) -> &Net {
        &self.net
    }
}

// ---------------------------------------------------------------------------
// Newtype wrappers for compile-time structural guarantees
// ---------------------------------------------------------------------------

macro_rules! net_subclass {
    ($(#[$meta:meta])* $name:ident, $class:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(pub(crate) Net);

        impl Deref for $name {
            type Target = Net;
            fn deref(&self) -> &Net { &self.0 }
        }

        impl AsRef<Net> for $name {
            fn as_ref(&self) -> &Net { &self.0 }
        }

        impl TryFrom<Net> for $name {
            type Error = Net;
            fn try_from(net: Net) -> Result<Self, Net> {
                if net.classify() == NetClass::$class {
                    Ok($name(net))
                } else {
                    Err(net)
                }
            }
        }

        impl TryFrom<ClassifiedNet> for $name {
            type Error = ClassifiedNet;
            fn try_from(cn: ClassifiedNet) -> Result<Self, ClassifiedNet> {
                if cn.class == NetClass::$class {
                    Ok($name(cn.net))
                } else {
                    Err(cn)
                }
            }
        }

        impl From<$name> for Net {
            fn from(n: $name) -> Net { n.0 }
        }
    };
}

net_subclass!(
    /// An S-net: every transition has exactly one input and one output place.
    /// Models sequential processes and choices, but not concurrency.
    SNet, SNet
);

net_subclass!(
    /// A T-net: every place has exactly one input and one output transition.
    /// Models concurrency and synchronization, but not choices.
    TNet, TNet
);

net_subclass!(
    /// A circuit: both an S-net and a T-net.
    Circuit, Circuit
);

net_subclass!(
    /// A free-choice net: if two transitions share any input place, they share
    /// all input places. Enables polynomial-time liveness and reachability analysis.
    FreeChoiceNet, FreeChoice
);
