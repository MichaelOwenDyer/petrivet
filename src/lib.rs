//! Petrivet: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//! # Quick Start
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::System;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arc((p0, t0));
//! b.add_arc((t0, p1));
//! b.add_arc((p1, t1));
//! b.add_arc((t1, p0));
//!
//! let net = b.build().expect("valid net");
//! println!("Class: {}", net.class());
//!
//! let mut sys = System::new(net, [1, 0]);
//! sys.choose_and_fire(|enabled| enabled.first());
//! println!("Marking after firing: {}", sys.marking());
//! ```

pub mod net;
pub mod marking;
pub mod system;
