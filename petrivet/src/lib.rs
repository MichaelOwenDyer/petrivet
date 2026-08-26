#![warn(clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::cargo_common_metadata, clippy::use_self, clippy::iter_on_single_items)]

//! `petrivet`: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//!
//!
//! # Quick Start
//!
//! ```
//! use petrivet::builder::NetBuilder;
//! use petrivet::state_space::ExplorationOrder;
//! let mut net = NetBuilder::new();
//! let [p0, p1] = net.add_places();
//! let [t0, t1] = net.add_transitions();
//! net.add_arc((p0, t0));
//! net.add_arc((t0, p1));
//! net.add_arc((p1, t1));
//! net.add_arc((t1, p0));
//!
//! let net = net.build().expect("valid net");
//! let mut sys = net.with_initial_marking([(p0, 1)]);
//! sys.try_fire(t0).expect("should be enabled");
//! println!("Marking after firing: {:?}", sys.marking());
//!
//! let mut cg = sys.explore_coverability(ExplorationOrder::BreadthFirst);
//! for state in cg.explore_iter().take(10) {
//!     if state.is_new {
//!         println!("{:#?}", state.marking);
//!     }
//! }
//! ```

pub(crate) mod api;
pub(crate) mod core;
pub mod literature;

pub use api::{builder, marking, net, parikh_vector, pnml, state_space, system};
pub use core::{boundedness, class, liveness};

pub mod prelude {
    pub use crate::api::{
        builder::{NetBuilder, NetError},
        marking::Marking,
        net::{Arc, Net, Node, Place, Transition},
        system::PetriNet,
    };
    pub use crate::core::{boundedness::Boundedness, class::NetClass, liveness::LivenessLevel};
}
