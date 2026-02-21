//! Petrivet: a Rust library for modeling, simulating, and analyzing Petri nets.
//!
//! # Quick Start
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::marking::Marking;
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
//! ```

pub mod net;
pub mod marking;

// Legacy modules — kept temporarily during migration, will be removed.
#[doc(hidden)]
#[allow(unused, dead_code)]
pub mod structure;
#[doc(hidden)]
#[allow(unused, dead_code)]
pub mod behavior;
#[doc(hidden)]
#[allow(unused, dead_code)]
pub mod analysis;
#[doc(hidden)]
#[allow(unused, dead_code)]
pub mod dipn;
