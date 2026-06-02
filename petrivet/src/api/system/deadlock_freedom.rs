use crate::marking::Marking;
use crate::net::Net;
use crate::prelude::PetriNet;
use crate::state_space::ExplorationOrder;
use crate::system::chc::SiphonTrapPair;

/// A **deadlock** is a [`Marking`] which enables no [`Transition`](crate::net::Transition).
///
/// **Deadlock-freedom** is a property of a system (N, M₀)
/// which holds when no reachable marking is a deadlock.
///
/// Deadlock-freedom is a desirable property in many systems,
/// as it guarantees that the system can always make progress
/// and will never get stuck in a state where no actions are possible.
pub type Deadlock = Marking<u32>;

/// Evidence for a deadlock-freedom result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DeadlockAnalysisMethod {
    /// The Commoner/Hack criterion is fulfilled: every [`Siphon`](crate::api::system::chc::Siphon)
    /// contains a [`Trap`](crate::api::system::chc::Trap) marked in `M₀`.
    /// This is a sufficient condition for deadlock-freedom.
    /// 
    /// Provided are all minimal siphons and their maximal traps, all marked in `M₀`.
    CommonerHackCriterion(Box<[SiphonTrapPair]>),
    /// State space was fully explored and no deadlocks were found.
    Exploration,
    /// The net is an unbounded general net where the reachability graph is infinite;
    /// this library's current algorithms cannot decide deadlock-freedom here.
    Inconclusive,
}

/// Result of deadlock-freedom analysis.
///
/// `deadlocks` is always a valid list: empty if the system is deadlock-free,
/// populated with witnesses if deadlocks exist. When the structural
/// (siphon/trap) check proves freedom, no exploration is needed and
/// `deadlocks` is empty. When exploration is required, all reachable
/// deadlocks are returned with firing sequences.
#[derive(Debug, Clone)]
pub struct DeadlockAnalysis {
    /// All reachable deadlock markings with witness firing sequences.
    /// Empty if deadlock-free.
    pub deadlocks: Box<[Deadlock]>,
    /// How the result was obtained.
    pub evidence: DeadlockAnalysisMethod,
}

impl DeadlockAnalysis {
    /// Whether the system is deadlock-free.
    #[must_use]
    pub fn is_deadlock_free(&self) -> bool {
        self.deadlocks.is_empty()
    }
}

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
        if let Ok(chc) = self.commoner_hack_criterion() {
            return DeadlockAnalysis {
                deadlocks: Box::new([]),
                evidence: DeadlockAnalysisMethod::CommonerHackCriterion(chc),
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