use crate::core::analysis::incidence::IncidenceMatrix;
use crate::core::analysis::semi_decision;
use crate::core::class::NetClass;
use crate::core::marking::IdxMarking;
use crate::core::unique_sorted_slice::UniqueSortedSlice;

/// A place in a built [`Net`], identified by a dense index in `0 .. place_count`.
///
/// This is a crate-internal index used by analysis algorithms. External users
/// interact with [`Place`] instead.
pub type PlaceIdx = usize;

/// A transition in a built [`DenseNet`], identified by a dense index in `0 .. transition_count`.
///
/// This is a crate-internal index used by analysis algorithms. External users
/// interact with [`Transition`] instead.
pub type TransitionIdx = usize;

/// Arc using internal dense indices for places and transitions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IdxArc {
    PlaceToTransition(PlaceIdx, TransitionIdx),
    TransitionToPlace(TransitionIdx, PlaceIdx),
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
    /// Transition presets: for each transition t, the set of places in `•t`.
    pub preset_t: Box<[UniqueSortedSlice<PlaceIdx>]>,
    /// Transition postsets: for each transition t, the set of places in `t•`.
    pub postset_t: Box<[UniqueSortedSlice<PlaceIdx>]>,
    /// Place presets: for each place p, the set of transitions in `•p`.
    pub preset_p: Box<[UniqueSortedSlice<TransitionIdx>]>,
    /// Place postsets: for each place p, the set of transitions in `p•`.
    pub postset_p: Box<[UniqueSortedSlice<TransitionIdx>]>,
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
    pub fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
        0..self.place_count() as PlaceIdx
    }

    /// Iterator over all internal transitions.
    pub fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
        0..self.transition_count() as TransitionIdx
    }

    /// Iterator over all arcs in the net, represented as pairs of internal indices.
    pub fn arc_indices(&self) -> impl Iterator<Item = IdxArc> + '_ {
        self.place_indices()
            .zip(self.preset_p.iter().zip(self.postset_p.iter()))
            .flat_map(|(p_idx, (preset, postset))| {
                std::iter::chain(
                    preset.iter().map(move |&t_idx| IdxArc::TransitionToPlace(t_idx, p_idx)),
                    postset.iter().map(move |&t_idx| IdxArc::PlaceToTransition(p_idx, t_idx)),
                )
            })
    }

    /// Returns true if the provided transition is enabled at the given marking,
    /// i.e. if all places in its preset have at least one token in the marking.
    pub fn is_enabled_in(&self, t: TransitionIdx, marking: &IdxMarking<u32>) -> bool {
        self.preset_t[t].iter().all(|&p| marking[p] >= 1)
    }

    /// Returns true if the given marking enables no transitions in the net.
    pub fn is_deadlock(&self, marking: &IdxMarking<u32>) -> bool {
        self.transition_indices().all(|t| !self.is_enabled_in(t, marking))
    }

    /// Computes the incidence matrix N of the net.
    #[must_use]
    pub fn incidence_matrix(&self) -> IncidenceMatrix {
        IncidenceMatrix::new(self)
    }

    /// Checks if the net is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause any place in the net to become unbounded.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        semi_decision::find_positive_place_subvariant(self).is_some()
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: PlaceIdx) -> bool {
        semi_decision::find_semipositive_place_subvariant(
            self,
            |&p| p == place
        ).is_some()
    }
}