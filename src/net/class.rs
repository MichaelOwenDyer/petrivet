use std::{fmt, iter};
use crate::{Place, Transition};

/// Structural classification of a Petri net.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum NetClass {
    /// The most restrictive class of Petri net:
    /// a single directed cycle of alternating places and transitions.
    /// Circuits model purely sequential processes with no choices or concurrency.
    Circuit,
    /// Every transition has exactly one input and one output place.
    /// S-nets model sequential processes and choices, but not concurrency.
    SNet,
    /// Every place has exactly one input and one output transition.
    /// T-nets model concurrency and synchronization, but not choices.
    TNet,
    /// If two transitions share any input place, they share all input places.
    /// Free-choice nets model concurrency and choices, but eliminate complex
    /// conflicts where two transitions share some but not all input places.
    FreeChoice,
    /// No structural restrictions.
    /// Can model arbitrary concurrency, choices, and conflicts.
    Unrestricted,
}

impl fmt::Display for NetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetClass::Circuit => write!(f, "Circuit"),
            NetClass::SNet => write!(f, "S-net"),
            NetClass::TNet => write!(f, "T-net"),
            NetClass::FreeChoice => write!(f, "Free-choice"),
            NetClass::Unrestricted => write!(f, "Unrestricted"),
        }
    }
}

pub fn classify_net(
    preset_t: &[Box<[Place]>],
    postset_t: &[Box<[Place]>],
    preset_p: &[Box<[Transition]>],
    postset_p: &[Box<[Transition]>],
) -> NetClass {
    let is_s_net = is_s_net(preset_t, postset_t);
    let is_t_net = is_t_net(preset_p, postset_p);
    match (is_s_net, is_t_net) {
        (true, true) => NetClass::Circuit,
        (true, false) => NetClass::SNet,
        (false, true) => NetClass::TNet,
        (false, false) if is_free_choice_net(postset_p, preset_t) => NetClass::FreeChoice,
        _ => NetClass::Unrestricted,
    }
}

pub fn is_s_net(preset_t: &[Box<[Place]>], postset_t: &[Box<[Place]>]) -> bool {
    iter::zip(preset_t, postset_t).all(|(pre, post)| {
        pre.len() == 1 && post.len() == 1
    })
}

pub fn is_t_net(preset_p: &[Box<[Transition]>], postset_p: &[Box<[Transition]>]) -> bool {
    iter::zip(preset_p, postset_p).all(|(pre, post)| {
        pre.len() == 1 && post.len() == 1
    })
}

pub fn is_free_choice_net(postset_p: &[Box<[Transition]>], preset_t: &[Box<[Place]>]) -> bool {
    postset_p.iter().all(|postset| {
        postset.len() == 1 || postset.windows(2).all(|t| {
            preset_t[t[0].idx] == preset_t[t[1].idx]
        })
    })
}