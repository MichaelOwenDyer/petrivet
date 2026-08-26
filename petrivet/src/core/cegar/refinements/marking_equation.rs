use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::SmtSolver;
use crate::core::net::TransitionIdx;

/// This refinement encodes the state equation of the net into the SMT solver.
pub struct MarkingEquationRefinement;

impl MarkingEquationRefinement {
    pub fn encode_into<S: SmtSolver>(
        solver: &mut S,
        problem: &CegarProblem,
        place_terms: &[S::Int],
        transition_terms: &[S::Int],
    ) {
        for p_idx in problem.net.place_indices() {
            let initial_marking = problem.m0[p_idx];
            let net_effects: Vec<(TransitionIdx, i16)> = problem.net.transition_indices()
                .filter_map(|t_idx| {
                    let effect = problem.net.incidence_matrix.get_effect(t_idx, p_idx);
                    (effect != 0).then_some((t_idx, effect))
                })
                .collect();

            let m0_p = solver.mk_int(i64::from(initial_marking));
            let token_expression = if net_effects.is_empty() {
                // the place has no transitions affecting it, so the marking
                // is constant from the initial marking
                m0_p
            } else {
                // set the marking of the place to be equal to the initial marking
                // plus the sum of the effects of all transitions on the place.
                let transition_effects: Vec<S::Int> = net_effects
                    .iter()
                    .map(|&(t_idx, effect)| {
                        let firing_count = transition_terms[t_idx].clone();
                        let effect = solver.mk_int(i64::from(effect));
                        solver.mul([firing_count, effect])
                    })
                    .collect();
                let effect_sum = solver.add(transition_effects);
                solver.add([m0_p, effect_sum])
            };
            let constraint = solver.eq(&place_terms[p_idx], &token_expression);
            solver.assert_tracked(&constraint, IdxLemma::MarkingEquation {
                place: p_idx,
                initial_marking: problem.m0[p_idx],
                net_effects
            });
        }
    }
}
