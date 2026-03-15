//! Petrivet: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//! # Quick Start
//!
//! ```
//! use petrivet::{CoverabilityExplorer, ExplorationOrder};
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
//! println!("Marking after firing: {}", sys.current_marking());
//!
//! let mut cg = CoverabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);
//! for s in cg.take(10) {
//!     if s.is_new {
//!         println!("{:#?}", s.marking);
//!     }
//! }
//! ```

pub mod net;
pub mod marking;
pub mod system;
pub mod state_space;
pub mod analysis;
pub mod literature;

pub use analysis::model::LivenessLevel;
pub use marking::*;
pub use net::*;
pub use state_space::coverability::*;
pub use state_space::explorer::*;
pub use state_space::reachability::*;
pub use system::System;
