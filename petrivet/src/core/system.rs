use crate::core::marking::IdxMarking;
use crate::core::TransitionIdx;
use crate::Net;

/// Internal representation of a Petri net system with dense indexing for efficient state-space exploration.
#[derive(Debug, Clone)]
pub(crate) struct DensePetriNet<N: AsRef<Net>> {
    pub net: N,
    pub initial_marking: IdxMarking<u32>,
    pub current_marking: IdxMarking<u32>,
}

impl<N: AsRef<Net>> DensePetriNet<N> {
    pub(crate) fn into_parts(self) -> (N, IdxMarking<u32>, IdxMarking<u32>) {
        (self.net, self.initial_marking, self.current_marking)
    }

    pub(crate) fn net(&self) -> &Net {
        self.net.as_ref()
    }

    /// Resets the current marking to the initial marking.
    /// Returns the marking before the reset.
    pub(crate) fn reset(&mut self) -> IdxMarking<u32> {
        std::mem::replace(
            &mut self.current_marking,
            self.initial_marking.clone()
        )
    }

    /// Dense-index firing for internal use by the state-space explorer.
    pub(crate) fn is_enabled(&self, t: TransitionIdx) -> bool {
        self.net().core_net.is_enabled_in(t, &self.current_marking)
    }

    /// Returns the set of currently enabled transitions.
    pub(crate) fn enabled_transitions(&self) -> impl Iterator<Item = TransitionIdx> {
        self.net().core_net
            .transition_indices()
            .filter(|&t| self.is_enabled(t))
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub(crate) fn is_deadlocked(&self) -> bool {
        self.enabled_transitions().next().is_none()
    }

    /// Check-and-fire a specific transition.
    pub fn try_fire(&mut self, t: TransitionIdx) -> Result<(), ()> {
        if self.is_enabled(t) {
            self.fire_unchecked(t);
            Ok(())
        } else {
            Err(())
        }
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled.
    /// Token underflow will panic in debug mode and wrap in release mode.
    pub(crate) fn fire_unchecked(&mut self, t_idx: TransitionIdx) {
        for &p in &self.net.as_ref().core_net.preset_t[t_idx] {
            self.current_marking[p] -= 1;
        }
        for &p in &self.net.as_ref().core_net.postset_t[t_idx] {
            self.current_marking[p] += 1;
        }
    }
}