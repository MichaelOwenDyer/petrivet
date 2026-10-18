use crate::core::net::DenseNet;
use crate::core::net::idx_set::{PlaceIdxSet, TransitionIdxSet};
use crate::core::system::marking::IdxMarking;

impl DenseNet {
    /// Returns true if some transition is neither on an unmarked circuit nor reachable from one.
    ///
    /// In a marked graph the token count of a circuit is invariant, so transitions on
    /// an unmarked circuit and everything downstream of them fire only finitely often.
    /// All other transitions are live. The system is deadlock-free iff at least one transition
    /// is live: a live transition can never be dead at a deadlock, and if every transition fires
    /// only finitely often then a longest firing sequence ends in a deadlock.
    pub fn some_transition_is_not_downstream_of_an_unmarked_circuit(
        &self,
        marking: &IdxMarking<u32>,
    ) -> bool {
        let mut live_transitions = TransitionIdxSet::all_of(self.transition_count());
        let mut stack = self.transitions_on_unmarked_circuits(marking).collect::<Vec<_>>();
        while let Some(t_idx) = stack.pop() {
            if live_transitions.contains(t_idx) {
                live_transitions.remove(t_idx);
                for &p_idx in &self.postset_t[t_idx] {
                    stack.extend(self.postset_p[p_idx].iter().copied());
                }
            }
        }
        !live_transitions.is_empty()
    }

    /// Returns true if some marked place can never reach a place with an empty postset.
    pub fn some_token_can_never_reach_a_sink(&self, marking: &IdxMarking<u32>) -> bool {
        let mut sinks = PlaceIdxSet::none_of(self.place_count());
        for p_idx in self.place_indices() {
            if self.postset_p[p_idx].is_empty() {
                sinks.add(p_idx);
            }
        }
        let places_that_can_reach_a_sink = self.pre_star(sinks);
        !marking.support().is_subset(&places_that_can_reach_a_sink)
    }
}