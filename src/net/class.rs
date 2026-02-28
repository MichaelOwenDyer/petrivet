use std::fmt;
use crate::{Place, Transition};
use super::sorted_set::SortedSet;

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

#[must_use]
pub fn classify(
    preset_t: &[SortedSet<Place>],
    postset_t: &[SortedSet<Place>],
    preset_p: &[SortedSet<Transition>],
    postset_p: &[SortedSet<Transition>],
) -> NetClass {
    let is_s = is_s_net(preset_t, postset_t);
    let is_t = is_t_net(preset_p, postset_p);
    match (is_s, is_t) {
        (true, true) => NetClass::Circuit,
        (true, false) => NetClass::SNet,
        (false, true) => NetClass::TNet,
        (false, false) if is_free_choice_net(postset_p, preset_t) => NetClass::FreeChoice,
        (false, false) if is_asymmetric_choice_net(postset_p) => NetClass::AsymmetricChoice,
        _ => NetClass::Unrestricted,
    }
}

/// S-net: |•t| = 1 and |t•| = 1 for every transition t.
fn is_s_net(transition_presets: &[SortedSet<Place>], transition_postsets: &[SortedSet<Place>]) -> bool {
    std::iter::zip(transition_presets, transition_postsets).all(|(pre, post)| {
        pre.len() == 1 && post.len() == 1
    })
}

/// T-net: |•p| = 1 and |p•| = 1 for every place p.
fn is_t_net(place_presets: &[SortedSet<Transition>], place_postsets: &[SortedSet<Transition>]) -> bool {
    std::iter::zip(place_presets, place_postsets).all(|(pre, post)| {
        pre.len() == 1 && post.len() == 1
    })
}

/// Free-choice: ∀ p1, p2: p1• ∩ p2• ≠ ∅ ⟹ p1• = p2•.
/// Equivalently: for every place p, all transitions in p• share the same preset.
fn is_free_choice_net(place_postsets: &[SortedSet<Transition>], preset_t: &[SortedSet<Place>]) -> bool {
    place_postsets.iter().all(|postset| {
        postset.windows(2).all(|t| {
            preset_t[t[0].idx] == preset_t[t[1].idx]
        })
    })
}

/// Asymmetric-choice: ∀ p1, p2: p1• ∩ p2• ≠ ∅ ⟹ p1• ⊆ p2• ∨ p2• ⊆ p1•.
fn is_asymmetric_choice_net(place_postsets: &[SortedSet<Transition>]) -> bool {
    place_postsets.iter().all(|a| {
        place_postsets.iter().all(|b| {
            !a.intersects(b) || a.is_subset_of(b) || b.is_subset_of(a)
        })
    })
}
