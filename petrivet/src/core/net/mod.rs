use crate::core::state_space::TokenOps;
use crate::core::system::marking::IdxMarking;
use crate::net::class::NetClass;
use incidence::IdxIncidenceMatrix;

pub mod path;
pub mod idx_set;
pub mod siphon_trap;
pub mod incidence;
pub mod structural_boundedness;

/// A place in a built [`DenseNet`], identified by a dense index in `0 .. place_count`.
///
/// This is a crate-internal index used by analysis algorithms.
/// External users interact with [`Place`] instead.
pub type PlaceIdx = usize;

/// A transition in a built [`DenseNet`], identified by a dense index in `0 .. transition_count`.
///
/// This is a crate-internal index used by analysis algorithms.
/// External users interact with [`Transition`] instead.
pub type TransitionIdx = usize;

/// Arc using internal dense indices for places and transitions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IdxArc {
    PlaceToTransition(PlaceIdx, TransitionIdx),
    TransitionToPlace(TransitionIdx, PlaceIdx),
}

/// Node using internal dense indices for places and transitions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IdxNode {
    Place(PlaceIdx),
    Transition(TransitionIdx),
}

/// The structure of a Net compressed into a packed format optimized for analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseNet {
    /// Structural class of the net, used for optimizations.
    pub class: NetClass,
    /// True if the net is strongly connected, i.e. every node is reachable from
    /// every other node. This is an important property for some algorithms,
    /// so we compute it at build time.
    pub is_strongly_connected: bool,
    /// The incidence matrix of the net, which encodes the net effect of each transition on each place.
    pub incidence_matrix: IdxIncidenceMatrix,
    /// Transition presets: for each transition t, the set of places in `•t`.
    pub preset_t: Box<[Box<[PlaceIdx]>]>,
    /// Transition postsets: for each transition t, the set of places in `t•`.
    pub postset_t: Box<[Box<[PlaceIdx]>]>,
    /// Place presets: for each place p, the set of transitions in `•p`.
    pub preset_p: Box<[Box<[TransitionIdx]>]>,
    /// Place postsets: for each place p, the set of transitions in `p•`.
    pub postset_p: Box<[Box<[TransitionIdx]>]>,
}

impl DenseNet {
    /// Number of places in the net.
    #[must_use]
    pub fn place_count(&self) -> usize {
        self.preset_p.len()
    }

    /// Number of transitions in the net.
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.preset_t.len()
    }

    /// Number of nodes in the net (places + transitions).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.preset_p.len() + self.preset_t.len()
    }

    /// Number of arcs in the net.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        std::iter::zip(&self.preset_p, &self.postset_p)
            .map(|(pre, post)| pre.len() + post.len())
            .sum()
    }

    /// Iterator over all internal places.
    pub fn place_indices(&self) -> impl Iterator<Item=PlaceIdx> + '_ {
        0..self.place_count() as PlaceIdx
    }

    /// Iterator over all internal transitions.
    pub fn transition_indices(&self) -> impl Iterator<Item=TransitionIdx> + '_ {
        0..self.transition_count() as TransitionIdx
    }

    /// Iterator over all arcs in the net, represented as pairs of internal indices.
    pub fn arc_indices(&self) -> impl Iterator<Item=IdxArc> + '_ {
        self.place_indices()
            .zip(self.preset_p.iter().zip(self.postset_p.iter()))
            .flat_map(|(p_idx, (preset, postset))| {
                std::iter::chain(
                    preset
                        .iter()
                        .map(move |&t_idx| IdxArc::TransitionToPlace(t_idx, p_idx)),
                    postset
                        .iter()
                        .map(move |&t_idx| IdxArc::PlaceToTransition(p_idx, t_idx)),
                )
            })
    }

    /// Returns true if the provided transition is enabled at the given marking,
    /// i.e. if all places in its preset have at least one token in the marking.
    pub fn is_enabled_in<T: TokenOps>(&self, t: TransitionIdx, marking: &IdxMarking<T>) -> bool {
        self.preset_t[t].iter().all(|&p| marking[p].at_least_one())
    }

    /// Applies the net effect of a transition to the given marking in-place.
    /// Assumes the transition is enabled.
    pub fn fire<T: TokenOps>(&self, t: TransitionIdx, marking: &mut IdxMarking<T>) {
        for &p in &self.preset_t[t] {
            marking[p].decrement();
        }
        for &p in &self.postset_t[t] {
            marking[p].increment();
        }
    }

    /// Reverts the net effect of a transition on the given marking in-place.
    /// Assumes the transition is backwards enabled.
    pub fn unfire<T: TokenOps>(&self, t: TransitionIdx, marking: &mut IdxMarking<T>) {
        for &p in &self.postset_t[t] {
            marking[p].decrement();
        }
        for &p in &self.preset_t[t] {
            marking[p].increment();
        }
    }

    /// Returns true if the given marking enables no transitions in the net.
    pub fn is_deadlock<T: TokenOps>(&self, marking: &IdxMarking<T>) -> bool {
        self.transition_indices()
            .all(|t| !self.is_enabled_in(t, marking))
    }

    /// Computes the incidence matrix N of the net.
    #[must_use]
    pub fn incidence_matrix(&self) -> IdxIncidenceMatrix {
        IdxIncidenceMatrix::new(self)
    }

    /// Checks if the net is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause any place in the net to become unbounded.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        structural_boundedness::find_positive_place_subinvariant(self).is_some()
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: PlaceIdx) -> bool {
        structural_boundedness::find_place_subinvariant(self, |&p| p == place).is_some()
    }
}
