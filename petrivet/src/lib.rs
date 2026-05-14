#![warn(
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
)]
#![allow(
    clippy::cargo_common_metadata,
    clippy::use_self,
)]

//! Petrivet: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//! # Quick Start
//!
//! ```
//! use petrivet::{CoverabilityExplorer };
//! use petrivet::api::builder::NetBuilder;
//! use petrivet::api::net::system::PetriNet;
//!
//! let mut net = NetBuilder::new();
//! let [p0, p1] = net.add_places();
//! let [t0, t1] = net.add_transitions();
//! net.add_arc((p0, t0));
//! net.add_arc((t0, p1));
//! net.add_arc((p1, t1));
//! net.add_arc((t1, p0));
//!
//! let net = net.build().expect("valid net");
//! println!("Class: {}", net.class());
//!
//! let mut sys = PetriNet::new(net, [1, 0]);
//! sys.choose_and_fire(|enabled| enabled.first());
//! println!("Marking after firing: {}", sys.current_marking());
//!
//! let mut cg = sys.explore_coverability(ExplorationOrder::BreadthFirst);
//! for s in cg.explore_iter().take(10) {
//!     if s.is_new {
//!         println!("{:#?}", s.marking);
//!     }
//! }
//! ```

pub mod literature;
pub mod api;
pub(crate) mod core;

pub use api::net::{Arc, Net, Node, Place, Transition};
pub use api::system::PetriNet;
pub use api::*;
