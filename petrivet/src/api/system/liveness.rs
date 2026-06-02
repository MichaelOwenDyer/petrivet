use crate::liveness::LivenessLevel;
use crate::model::{LivenessAnalysis, LivenessMethod};
use crate::net::Net;
use crate::prelude::PetriNet;

impl<N: AsRef<Net>> PetriNet<N> {
    /// Whether the system is live (L4): every transition can fire from
    /// every reachable marking (possibly after further firings).
    ///
    /// Delegates to [`analyze_liveness`](Self::analyze_liveness).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.analyze_liveness().global_level().is_live()
    }

    /// Analyzes liveness and returns per-transition levels with evidence.
    ///
    /// Strategy (ascending cost):
    /// 1. **S-nets**: SCC decomposition of the place graph. Polynomial.
    ///    Sink SCCs → L4, non-sink SCCs → L3, inter-SCC → L1.
    /// 2. **T-nets**: SCC decomposition of the transition graph. Polynomial.
    ///    Every transition is L0 or L4 (circuit token invariance).
    /// 3. **Free-choice nets**: Commoner's theorem (structural).
    ///    If the criterion holds, all transitions are L4.
    /// 4. **General**: CG → RG → SCC analysis (exponential worst-case).
    #[must_use]
    pub fn analyze_liveness(&self) -> LivenessAnalysis {
        // TODO: Optimize for state machines and marked graphs
        //  by analyzing SCCs of the appropriate graph

        if self.class().is_free_choice()
            && let chc = self.commoner_hack_criterion()
            && chc.is_ok() {
            return LivenessAnalysis {
                levels: self.transitions().zip(std::iter::repeat(LivenessLevel::L4)).collect(),
                method: LivenessMethod::FreeChoice(chc),
            };
        }

        match self.try_build_reachability_graph() {
            Ok(rg) => rg.transition_liveness(),
            Err(_cg) => {
                // TODO: liveness for unbounded nets
                LivenessAnalysis {
                    levels: self.transitions().zip(std::iter::repeat(LivenessLevel::L0)).collect(),
                    method: LivenessMethod::Inconclusive,
                }
            }
        }
    }
}