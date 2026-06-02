use crate::core::analysis::siphon_trap;
use crate::core::mapping::DenseMapping;
use crate::model::{CommonerHackCriterionResult, DeadlockAnalysis, DeadlockAnalysisMethod, SiphonTrapPair};
use crate::net::Net;
use crate::prelude::PetriNet;
use crate::state_space::ExplorationOrder;

impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether the system is deadlock-free: no reachable marking has zero
    /// enabled transitions.
    ///
    /// This is a convenience method which delegates to
    /// [`analyze_deadlock_freedom`](Self::analyze_deadlock_freedom)
    /// and throws away the witnesses and evidence.
    /// For detailed analysis, call the latter method directly
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.analyze_deadlock_freedom().is_deadlock_free()
    }

    /// Returns true if state space enumeration encounters any deadlock marking.
    pub fn has_reachable_deadlock_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .find(|m| self.dense_net.is_deadlock(m))
            .is_some()
    }

    /// Checks the Commoner/Hack criterion, which is fulfilled when all siphons in the system
    /// contain a trap marked at the initial marking.
    /// This is a necessary and sufficient condition for liveness in free-choice nets,
    /// and a sufficient condition for deadlock-freedom in general nets.
    pub fn commoner_hack_criterion(&self) -> CommonerHackCriterionResult {
        fn to_api(mapping: &DenseMapping, pair: siphon_trap::SiphonTrapPair) -> SiphonTrapPair {
            SiphonTrapPair {
                siphon: pair.siphon.into_iter().map(|p_idx| mapping.place(p_idx)).collect(),
                trap: pair.trap.into_iter().map(|p_idx| mapping.place(p_idx)).collect(),
            }
        }

        siphon_trap::commoner_hack_criterion(&self.dense_net, &self.marking)
            .map(|siphon_trap_pairs| {
                siphon_trap_pairs.into_iter().map(|pair| to_api(&self.mapping, pair)).collect()
            })
            .map_err(|counterexample| {
                to_api(&self.mapping, counterexample)
            })
    }

    /// Analyzes deadlock-freedom and returns deadlock witnesses with evidence.
    ///
    /// Strategy:
    /// 1. Siphon/trap check (Commoner criterion): if every siphon contains
    ///    a marked trap, the system is deadlock-free (no exploration needed).
    /// 2. If the structural check is inconclusive, escalates to state space
    ///    exploration (CG → RG) and reports all reachable deadlocks with
    ///    firing sequences.
    #[must_use]
    pub fn analyze_deadlock_freedom(&self) -> DeadlockAnalysis {
        if let chc = self.commoner_hack_criterion()
            && chc.is_ok() {
            return DeadlockAnalysis {
                deadlocks: Box::new([]),
                evidence: DeadlockAnalysisMethod::CommonerTheorem(chc),
            };
        }

        match self.try_build_reachability_graph() {
            Ok(rg) => {
                let deadlocks = rg.deadlocks().collect();
                DeadlockAnalysis {
                    deadlocks,
                    evidence: DeadlockAnalysisMethod::Exploration,
                }
            }
            Err(_cg) => {
                // TODO: deadlock-freedom for unbounded nets is currently inconclusive rather than attempting infinite exploration.
                DeadlockAnalysis {
                    deadlocks: Box::new([]),
                    evidence: DeadlockAnalysisMethod::Inconclusive,
                }
            }
        }
    }
}