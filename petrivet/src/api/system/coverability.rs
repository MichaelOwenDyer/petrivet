use crate::core::cegar::CegarResult;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::prelude::PetriNet;
pub use crate::system::lemma::Lemma;

#[derive(Debug, Clone)]
pub enum Coverability {
    /// The target marking is coverable from M₀.
    Coverable {
        /// A reachable marking which covers the target marking.
        marking: Marking<u32>,
        /// A transition firing sequence from `M₀` to `covering_marking`.
        firing_sequence: Vec<Transition>,
    },
    /// The target marking is not coverable from M₀.
    Uncoverable {
        contradiction: Vec<Lemma>,
    },
}

impl Coverability {
    /// Whether the target is coverable.
    #[must_use]
    pub const fn is_coverable(&self) -> bool {
        matches!(self, Self::Coverable { .. })
    }

    /// Whether the target is not coverable.
    #[must_use]
    pub const fn is_uncoverable(&self) -> bool {
        matches!(self, Self::Uncoverable { .. })
    }
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns whether `target` is coverable from the initial marking.
    pub fn is_coverable(&self, target: impl Into<Marking<u32>>) -> bool {
        self.analyze_coverability(target.into()).is_coverable()
    }

    /// Analyzes coverability of a target marking with structured evidence.
    ///
    /// A marking `target` is **coverable** if there exists a reachable marking `M`
    /// such that `M(p) >= target(p)` for every place `p`.
    #[must_use]
    pub fn analyze_coverability(&self, target: impl Into<Marking<u32>>) -> Coverability {
        let m0 = &self.marking;
        let target = &self.mapping.decode(target.into());

        if m0 >= target {
            return Coverability::Coverable {
                firing_sequence: Vec::new(),
                marking: self.mapping.encode(m0.clone()),
            };
        }

        match self.dense_net.cegar_coverability(m0, target) {
            CegarResult::Satisfiable { marking, firing_sequence } => {
                Coverability::Coverable {
                    firing_sequence: firing_sequence
                        .into_iter()
                        .map(|t_idx| self.mapping.transition(t_idx))
                        .collect(),
                    marking: self.mapping.encode(marking),
                }
            }
            CegarResult::Unsatisfiable { contradiction } => {
                Coverability::Uncoverable {
                    contradiction: contradiction.into_iter()
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
    use crate::marking::Marking;
    use crate::prelude::PetriNet;
    use crate::system::coverability::Coverability;

    #[test]
    fn coverability_initial_marking_covers() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        let net = b.build().unwrap();
        let sys = PetriNet::new(net, [(p0, 1), (p1, 0)]);

        let res = sys.analyze_coverability([(p0, 1)]);
        assert!(res.is_coverable());
        match res {
            Coverability::Coverable {
                marking,
                firing_sequence,
            } => {
                assert_eq!(marking, Marking::from([(p0, 1)]));
                assert_eq!(firing_sequence.len(), 0);
            }
            _ => panic!("expected InitialMarking proof"),
        }
    }
}
