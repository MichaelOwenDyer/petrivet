use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::SmtSolver;
use crate::core::marking::IdxMarking;
use crate::core::net::PlaceIdx;
use crate::core::siphon_trap::IdxTrap;
use fixedbitset::FixedBitSet;

/// Ensures that the SMT solver keeps all traps which were marked in m0
/// also marked in the candidate solution.
pub struct InitiallyMarkedTrapRule;

impl InitiallyMarkedTrapRule {
    pub fn check(
        problem: &CegarProblem,
        candidate_marking: &IdxMarking<u32>,
    ) -> Option<InitiallyMarkedTrapRefinement> {
        let mut trap: FixedBitSet = FixedBitSet::with_capacity(problem.net.place_count());
        for p_idx in problem.net.place_indices() {
            trap.set(p_idx, candidate_marking[p_idx] == 0);
        }

        let mut worklist: Vec<PlaceIdx> = problem.net.place_indices().filter(|&p| trap[p]).collect();

        while let Some(p_idx) = worklist.pop() {
            let violates = problem.net.postset_p[p_idx]
                .iter()
                .any(|&t| problem.net.postset_t[t].iter().all(|&q| !trap[q]));
            if violates {
                trap.remove(p_idx);
                for &t in &problem.net.preset_p[p_idx] {
                    for &q in &problem.net.preset_t[t] {
                        if trap[q] {
                            worklist.push(q);
                        }
                    }
                }
            }
        }

        if trap.ones().any(|p| problem.m0[p] > 0) {
            Some(InitiallyMarkedTrapRefinement { trap })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct InitiallyMarkedTrapRefinement {
    /// The trap (set of places represented as a bitset) which is marked in the initial marking
    /// but not in the candidate marking.
    /// This set of places can never collectively have zero tokens in any reachable marking,
    /// which contradicts the candidate marking.
    pub trap: IdxTrap,
}

impl InitiallyMarkedTrapRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        place_terms: &[S::Int]
    ) {
        let trap_sum = solver.add(self.trap.ones().map(|p| place_terms[p].clone()));
        let zero = solver.mk_int(0);
        let constraint = solver.gt(&trap_sum, &zero);
        solver.assert_tracked(&constraint, IdxLemma::InitiallyMarkedTrap(self.trap));
    }
}
