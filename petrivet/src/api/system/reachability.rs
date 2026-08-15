use crate::class::NetClass;
use crate::core::cegar::CegarResult;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;
use crate::system::lemma::Lemma;

#[derive(Debug, Clone)]
pub enum Reachability {
    /// The target marking is reachable from M₀.
    Reachable {
        firing_sequence: Vec<Transition>,
    },
    /// The target marking is definitely not reachable from M₀.
    Unreachable {
        contradiction: Vec<Lemma>,
    },
}

impl Reachability {
    /// Whether the target is definitely reachable.
    #[must_use]
    pub const fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable { .. })
    }

    /// Whether the target is definitely unreachable.
    #[must_use]
    pub const fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable { .. })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If there is an efficient (polynomial-time) procedure to determine
    /// whether the given target marking is reachable in this Petri net,
    /// returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_reachable(&self, target: &Marking<u32>) -> Option<bool> {
        // todo: efficient check necessary for reachability: maximal unmarked trap in target must be unmarked in M0
        match self.class() {
            NetClass::Circuit => Some(self.marking.sum() == target.total_tokens()),
            NetClass::StateMachine if self.is_live() => {
                Some(self.marking.sum() == target.total_tokens())
            }
            // NetClass::MarkedGraph if self.is_live() => Some(self.marking() ~ target) // todo: ~ relation
            // NetClass::FreeChoice if self.is_live() && self.is_bounded() => None, // todo: requires ILP + trap check
            _ => None,
        }
    }

    /// Whether `target` is reachable from the initial marking.
    ///
    /// Delegates to [`analyze_reachability`](Self::analyze_reachability).
    /// Returns `false` for inconclusive results.
    #[must_use]
    pub fn is_reachable(&self, target: impl Into<Marking<u32>>) -> bool {
        let target = target.into();
        self.is_efficiently_reachable(&target)
            .unwrap_or_else(|| self.analyze_reachability(target).is_reachable())
    }

    /// Analyzes reachability of a target marking.
    #[must_use]
    pub fn analyze_reachability(&self, target: impl Into<Marking<u32>>) -> Reachability {
        let m0 = &self.marking;
        let target = &self.mapping.decode(target.into());

        if m0 == target {
            return Reachability::Reachable {
                firing_sequence: Vec::new(),
            };
        }

        match self.dense_net.cegar_reachability(m0, target) {
            CegarResult::Satisfiable { marking: _, firing_sequence } => {
                Reachability::Reachable {
                    firing_sequence: firing_sequence
                        .into_iter()
                        .map(|t_idx| self.mapping.transition(t_idx))
                        .collect(),
                }
            }
            CegarResult::Unsatisfiable { contradiction } => {
                Reachability::Unreachable {
                    contradiction: contradiction
                        .into_iter()
                        .map(|lemma| self.mapping.lemma(lemma))
                        .collect(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;
    use crate::class::NetClass;

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, p0, _t0, p1, _t1) = crate::api::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_reachable([(p1, 1)]));
        assert!(sys.is_reachable([(p0, 1)]));
        assert!(!sys.is_reachable([(p0, 2)]));
        assert!(!sys.is_reachable([]));
    }

    #[test]
    fn t_net_reachability_dispatches() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((p1, t0));
        b.add_arcs((t0, p2, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p1));
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::MarkedGraph);
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        assert!(sys.is_reachable([(p2, 1)]));
        assert!(sys.is_reachable([(p0, 1), (p1, 1)]));
        assert!(!sys.is_reachable([(p1, 1)]));
    }

    #[test]
    fn general_net_reachability_fallback() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arcs((p0, t0, p1));
        b.add_arcs((p0, t1, p2));
        b.add_arcs((p1, t2, p0));
        b.add_arcs((p2, t2, p0));
        b.add_arc((p1, t1)); // extra arc to make it a general net
        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::General);
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_reachable([(p0, 1)]));
        assert!(sys.is_reachable([(p1, 1)]));
    }
}
