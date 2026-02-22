//! Petrivet: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//! # Quick Start
//!
//! ```
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
//! ```

pub mod net;
pub mod marking;
pub mod system;
pub mod state_space;
