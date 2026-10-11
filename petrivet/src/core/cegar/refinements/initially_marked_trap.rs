use crate::core::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::net::siphon_trap::{IdxTrap, maximal_trap_in};
use crate::core::solver::SmtSolver;
use crate::core::system::marking::IdxMarking;

/// Ensures that the SMT solver keeps all traps which were marked in the initial marking
/// also marked in its candidate solutions.
pub struct InitiallyMarkedTrapRule;

impl InitiallyMarkedTrapRule {
    pub fn check(
        problem: &CegarProblem,
        candidate_marking: &IdxMarking<u32>,
    ) -> Option<InitiallyMarkedTrapRefinement> {
        let trap = maximal_trap_in(problem.net, candidate_marking.support().complement());
        if trap.iter().any(|p| problem.m0[p] > 0) {
            Some(InitiallyMarkedTrapRefinement { trap })
        } else {
            None
        }
    }
}

/// A refinement that encodes an initially marked trap into the SMT solver:
/// the SMT solver must always put at least one token in the trap.
#[derive(Debug, Clone)]
pub struct InitiallyMarkedTrapRefinement {
    /// The trap (set of places represented as a bitset) which is marked in the initial marking.
    /// This set of places can never collectively have zero tokens in any reachable marking.
    pub trap: IdxTrap,
}

impl InitiallyMarkedTrapRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        place_terms: &[S::Int],
        callback: Option<&dyn Fn(IdxLemma)>,
    ) {
        let trap_sum = solver.add(self.trap.iter().map(|p| place_terms[p].clone()));
        let zero = solver.mk_int(0);
        let constraint = solver.gt(&trap_sum, &zero);
        let lemma = IdxLemma::InitiallyMarkedTrap(self.trap);
        if let Some(callback) = callback {
            callback(lemma.clone());
        }
        solver.assert_tracked(&constraint, lemma);
    }
}
