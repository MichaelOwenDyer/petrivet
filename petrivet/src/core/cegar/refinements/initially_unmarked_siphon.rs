use crate::core::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::net::siphon_trap::{IdxSiphon, maximal_siphon_in};
use crate::core::solver::SmtSolver;

/// Ensures that the SMT solver keeps the largest siphon which is unmarked in the initial marking
/// also unmarked in its candidate solutions.
/// Unlike the initially marked trap rule, this is a one-shot refinement:
/// there can only be one maximal unmarked siphon in the initial marking.
pub struct InitiallyUnmarkedSiphonRule;

impl InitiallyUnmarkedSiphonRule {
    pub fn check(problem: &CegarProblem) -> Option<InitiallyUnmarkedSiphonRefinement> {
        let siphon = maximal_siphon_in(problem.net, problem.m0.support().complement());
        if !siphon.is_empty() {
            Some(InitiallyUnmarkedSiphonRefinement { siphon })
        } else {
            None
        }
    }
}

/// A refinement that encodes an initially unmarked siphon into the SMT solver:
/// the SMT solver must always put zero tokens in the siphon.
#[derive(Debug, Clone)]
pub struct InitiallyUnmarkedSiphonRefinement {
    /// The siphon (set of places represented as a bitset) which is unmarked in the initial marking.
    /// This set of places can never contain any tokens in any reachable marking.
    pub siphon: IdxSiphon,
}

impl InitiallyUnmarkedSiphonRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        place_terms: &[S::Int],
        callback: Option<&dyn Fn(IdxLemma)>,
    ) {
        let siphon_sum = solver.add(self.siphon.iter().map(|p| place_terms[p].clone()));
        let zero = solver.mk_int(0);
        let constraint = solver.eq(&siphon_sum, &zero);
        let lemma = IdxLemma::InitiallyUnmarkedSiphon(self.siphon);
        if let Some(callback) = callback {
            callback(lemma.clone());
        }
        solver.assert_tracked(&constraint, lemma);
    }
}
