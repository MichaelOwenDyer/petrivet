use crate::core::analysis::incidence::IncidenceMatrix;
use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::{Satisfiability, SmtSolver};
use crate::core::marking::IdxMarking;
use crate::core::net::PlaceIdx;
use tap::TapOptional;

/// Ensures that the SMT solver respects the P-Invariants of the net.
pub struct PInvariantRule<S: SmtSolver> {
    solver: S,
    weights: Vec<S::Int>,
}

impl<S: SmtSolver> PInvariantRule<S> {
    pub fn new(incidence_matrix: &IncidenceMatrix) -> Self {
        let mut solver = S::default();

        let weights: Vec<S::Int> = incidence_matrix
            .place_indices()
            .map(|p_idx| solver.mk_int_var(&format!("p{p_idx}")))
            .collect();

        let zero = solver.mk_int(0);

        for weight in &weights {
            let ge_zero = solver.ge(weight, &zero);
            solver.assert(&ge_zero);
        }

        for t_idx in incidence_matrix.transition_indices() {
            let muls: Vec<S::Int> = incidence_matrix
                .place_indices()
                .zip(weights.iter())
                .filter_map(|(p_idx, weight)| {
                    let incidence = incidence_matrix.get_effect(t_idx, p_idx);
                    if incidence != 0 {
                        let incidence_term = solver.mk_int(i64::from(incidence));
                        Some(solver.mul([weight.clone(), incidence_term]))
                    } else {
                        None
                    }
                })
                .collect();
            if !muls.is_empty() {
                let sum = solver.add(muls);
                let eq_zero = solver.eq(&sum, &zero);
                solver.assert(&eq_zero);
            }
        }

        // Save the current state of the solver so that we can later return to this point.
        solver.push();

        Self { solver, weights }
    }
}

/// The weighted sum of `weights` over `marking`'s places, or `zero` if every place has either
/// a zero weight or a zero token count (`solver.add` must not be called with an empty collection).
fn dot<S: SmtSolver>(
    solver: &mut S,
    weights: &[S::Int],
    marking: &IdxMarking<u32>,
) -> S::Int {
    let muls: Vec<S::Int> = weights
        .iter()
        .zip(marking.iter())
        .filter(|&(_, &tokens)| tokens > 0)
        .map(|(weight, &tokens)| {
            let tokens_term = solver.mk_int(i64::from(tokens));
            solver.mul([weight.clone(), tokens_term])
        })
        .collect();
    if muls.is_empty() {
        solver.mk_int(0)
    } else {
        solver.add(muls)
    }
}

impl<S: SmtSolver> PInvariantRule<S> {
    /// A weighted token sum across a subset of the net's places
    /// that is invariant under the net's transitions.
    /// If there exists some P-Invariant that has a different value
    /// in the initial marking than in the candidate marking,
    /// then we know it cannot be a valid solution.
    ///
    /// This search is a self-contained SMT query, entirely decoupled from the main CEGAR
    /// solver's incremental state, so it's driven by its own fresh instance of the same
    /// backend `S` rather than sharing the caller's solver.
    pub fn check(
        &mut self,
        problem: &CegarProblem,
        candidate: &IdxMarking<u32>,
    ) -> Option<PInvariantRefinement> {
        let m0_dot = dot(&mut self.solver, &self.weights, problem.m0);
        let target_dot = dot(&mut self.solver, &self.weights, candidate);

        // m0_dot != target_dot
        let lt = self.solver.lt(&m0_dot, &target_dot);
        let gt = self.solver.gt(&m0_dot, &target_dot);
        let distinct = self.solver.or([lt, gt]);
        self.solver.assert(&distinct);

        if self.solver.check() != Satisfiability::Sat {
            self.solver.pop();
            return None;
        }

        let invariant: Vec<(PlaceIdx, u32)> = self.weights.iter()
            .enumerate()
            .filter_map(|(p_idx, weight_term)| {
                let w = self.solver.eval_int(weight_term).tap_none(|| {
                    eprintln!("warning! no model assignment for weight at place {p_idx}");
                })?;
                (w > 0).then_some((p_idx, w))
            })
            .collect();

        self.solver.pop();

        if invariant.is_empty() {
            return None;
        }

        let value: u32 = invariant.iter().map(|&(p, w)| w * problem.m0[p]).sum();

        Some(PInvariantRefinement(IdxPInvariant {
            weights: invariant,
            value,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct IdxPInvariant {
    pub weights: Vec<(PlaceIdx, u32)>,
    pub value: u32,
}

#[derive(Debug, Clone)]
pub struct PInvariantRefinement(pub IdxPInvariant);

impl PInvariantRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        place_terms: &[S::Int],
        callback: Option<&dyn Fn(IdxLemma)>,
    ) {
        let Self(p_invariant) = self;
        let weighted_places: Vec<S::Int> = p_invariant
            .weights
            .iter()
            .map(|&(p_idx, weight)| {
                if weight > 1 {
                    let weight_term = solver.mk_int(i64::from(weight));
                    solver.mul([place_terms[p_idx].clone(), weight_term])
                } else {
                    place_terms[p_idx].clone()
                }
            })
            .collect();
        let weighted_sum = solver.add(weighted_places);
        let value = solver.mk_int(i64::from(p_invariant.value));
        let eq = solver.eq(&weighted_sum, &value);
        if let Some(callback) = callback {
            callback(IdxLemma::PInvariant(p_invariant.clone()));
        }
        solver.assert_tracked(&eq, IdxLemma::PInvariant(p_invariant));
    }
}
