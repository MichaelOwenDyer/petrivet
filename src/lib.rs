//! Petrivet: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//! # Quick Start
//!
//! ```
//! use petrivet::{CoverabilityGraph, ExplorationOrder};
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::System;
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
//! let mut sys = System::new(net, [1, 0]);
//! sys.choose_and_fire(|enabled| enabled.first());
//! println!("Marking after firing: {}", sys.marking());
//!
//! let mut cg = CoverabilityGraph::new(&sys, ExplorationOrder::BreadthFirst);
//! for s in cg.iter().take(10) {
//!     if s.is_new {
//!         println!("{:#?}", s.marking);
//!     }
//! }
//! ```

pub mod net;
pub mod marking;
pub mod system;
pub mod explorer;
pub mod coverability;
pub mod reachability;
pub mod analysis;

pub use net::*;
pub use marking::*;
pub use system::System;
pub use coverability::CoverabilityGraph;
pub use reachability::ReachabilityGraph;
pub use explorer::ExplorationOrder;
