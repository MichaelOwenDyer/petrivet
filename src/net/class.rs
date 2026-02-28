use std::{fmt, iter};
use crate::{Place, Transition};

/// Structural classification of a Petri net.
///
/// The classes form an inclusion hierarchy (each is a subclass of the next):
///
/// ```text
/// Circuit ⊂ S-net ⊂ Free-choice ⊂ AsymmetricChoice ⊂ Unrestricted
/// Circuit ⊂ T-net ⊂ Free-choice ⊂ AsymmetricChoice ⊂ Unrestricted
/// ```
///
/// `classify_net` returns the most specific class.
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
    /// For every two places s1, s2: if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    /// Asymmetric-choice nets allow one-sided resource sharing (e.g. a shared
    /// resource plus a private resource), but forbid symmetric conflicts.
    AsymmetricChoice,
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
            NetClass::AsymmetricChoice => write!(f, "Asymmetric-choice"),
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
        (false, false) if is_asymmetric_choice_net(postset_p) => NetClass::AsymmetricChoice,
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

/// A net is asymmetric-choice if for every two places s1, s2:
/// if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
///
/// Since postsets are sorted, subset checking uses a linear merge.
pub fn is_asymmetric_choice_net(postset_p: &[Box<[Transition]>]) -> bool {
    for i in 0..postset_p.len() {
        for j in (i + 1)..postset_p.len() {
            let a = &postset_p[i];
            let b = &postset_p[j];
            if !sorted_disjoint(a, b) && !sorted_subset(a, b) && !sorted_subset(b, a) {
                return false;
            }
        }
    }
    true
}

/// Check if two sorted slices are disjoint.
fn sorted_disjoint(a: &[Transition], b: &[Transition]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].idx.cmp(&b[j].idx) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => return false,
        }
    }
    true
}

/// Check if sorted slice `a` is a subset of sorted slice `b`.
fn sorted_subset(a: &[Transition], b: &[Transition]) -> bool {
    let mut j = 0;
    for &elem in a {
        while j < b.len() && b[j].idx < elem.idx {
            j += 1;
        }
        if j >= b.len() || b[j].idx != elem.idx {
            return false;
        }
        j += 1;
    }
    true
}