use ahash::{HashSet, HashSetExt};
use crate::core::net::DenseNet;
use crate::core::net::idx_set::{PlaceIdxSet, TransitionIdxSet};
use crate::core::system::marking::IdxMarking;

#[derive(Debug, Clone)]
pub struct DeadlockCache {
    /// The set of observed deadlock markings.
    deadlocks: HashSet<IdxMarking<u32>>,
    /// Whether the system has been fully explored for deadlocks.
    complete: bool,
}

impl DeadlockCache {
    pub fn new() -> Self {
        Self {
            deadlocks: HashSet::new(),
            complete: false,
        }
    }

    /// Returns Some(true) if the system is deadlock-free, Some(false) if it has deadlocks,
    /// and None if the deadlock-freedom status is unknown.
    pub fn is_deadlock_free(&self) -> Option<bool> {
        if !self.deadlocks.is_empty() {
            Some(false)
        } else if self.complete {
            Some(true)
        } else {
            None
        }
    }

    pub fn mark_complete(&mut self) {
        self.complete = true;
    }

    pub fn mark_deadlock_free(&mut self) {
        debug_assert!(self.deadlocks.is_empty(), "Cannot mark deadlock-free when deadlocks have been observed.");
        self.complete = true;
    }

    pub fn add_deadlock(&mut self, deadlock: &IdxMarking<u32>) {
        debug_assert!(!self.complete, "Cannot add deadlock when cache is marked complete.");
        if !self.deadlocks.contains(deadlock) {
            self.deadlocks.insert(deadlock.clone());
        }
    }
}

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