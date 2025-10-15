//! Classes of petri nets:
//! Low-Level Net Classes (The "Workhorses")
//! These nets primarily model control flow and resource synchronization.
//! 1. Elementary / Condition-Event (E/C) Nets: The most basic form. Places can hold at most one token (bool). They are inherently safe (1-bounded).
//!
//! 2. Place/Transition (P/T) Nets: The standard model. Places hold a count of tokens (usize). This is the most common "general purpose" net.
//! - Generalized P/T Nets: An extension where arcs have integer weights (w > 1), representing bulk consumption/production.
//! - Capacity-Restricted P/T Nets: Places have a maximum token capacity.
//!
//! 3. State Machines (SM): A structural restriction of P/T nets where every transition has exactly one input and one output place (|•t| = |t•| = 1). They model sequential processes and choices, but not concurrency.
//!
//! 4. Marked Graphs (MG): A structural restriction where every place has exactly one input and one output transition (|•p| = |p•| = 1). They model concurrency and synchronization, but not choices.
//!
//! 5. Free-Choice (FC) Nets: A beautiful unification of SMs and MGs. They allow both choice and concurrency, but with a key restriction: if a place has multiple output transitions (a choice), it must be the only input place for all of them. This simplifies liveness analysis dramatically.
//!
//! 6. Extended Free-Choice (EFC) and Asymmetric Choice (AC) Nets: These are further generalizations of the Free-Choice structure that relax the choice rule in specific ways, creating a hierarchy of complexity.
//!
//! High-Level Net Classes (The "Intelligent Systems")
//! These nets model complex data flow in addition to control flow.
//!
//! Coloured Petri Nets (CPN): The most prominent high-level model. Tokens have "colours" (data types, e.g., structs, enums). Arcs are inscribed with functions that manipulate these colours, and transitions can have complex guards (boolean conditions) that must be met for them to fire.
//!
//! Behavioral Extensions (The "Specialists")
//! These are not new classes but rather modifications to the firing rule that can be applied to almost any of the above classes.
//!
//! Inhibitor Nets: Add "inhibitor arcs" that enable a transition only when the input place is empty. This adds the power of a "zero test," making the model Turing-complete.
//!
//! Reset Nets: Add "reset arcs" that empty all tokens from a place when a transition fires.
//!
//! Timed & Stochastic Nets: Associate a time delay (deterministic or probabilistic) with transitions or places, used for performance analysis.
#![feature(step_trait)]

pub mod structure;
pub mod behavior;
