use crate::core::analysis::incidence::IncidenceMatrix;
use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::{Satisfiability, SmtSolver};
use crate::core::marking::IdxMarking;
use crate::core::net::PlaceIdx;
use tap::TapOptional;
use crate::net::invariant::PInvariantKind;

/// Ensures that the SMT solver respects the P-Invariants (or sub-/sur-invariants) of the net.
pub struct PInvariantRule<S: SmtSolver> {
    /// The SMT solver instance used to encode and check the P-Invariant constraints.
    solver: S,
    /// The weight variables for each place in the net, representing the coefficients of the invariant.
    weights: Vec<S::Int>,
    /// Boolean variable indicating whether the invariant condition is being enforced
    /// (i.e., the weighted sum is always constant).
    is_invariant: S::Bool,
    /// Boolean variable indicating whether the subinvariant condition is being enforced
    /// (i.e., the weighted sum is non-increasing).
    is_subvariant: S::Bool,
    /// Boolean variable indicating whether the surinvariant condition is being enforced
    /// (i.e., the weighted sum is non-decreasing).
    is_survariant: S::Bool,
}

impl<S: SmtSolver> PInvariantRule<S> {
    pub fn new(incidence_matrix: &IncidenceMatrix) -> Self {
        let mut solver = S::default();

        let is_invariant = solver.mk_bool_var("is_invariant");
        let is_subvariant = solver.mk_bool_var("is_subvariant");
        let is_survariant = solver.mk_bool_var("is_survariant");

        let zero = solver.mk_int(0);
        let weights: Vec<S::Int> = incidence_matrix
            .place_indices()
            .map(|p_idx| solver.mk_int_var(&format!("p_idx_{p_idx}")))
            .collect();
        for weight in &weights {
            let weight_ge_zero = solver.ge(weight, &zero);
            solver.assert(&weight_ge_zero);
        }

        for t_idx in incidence_matrix.transition_indices() {
            let muls: Vec<S::Int> = incidence_matrix
                .place_indices()
                .zip(weights.iter())
                .filter_map(|(p_idx, weight)| {
                    let incidence = incidence_matrix.get_effect(t_idx, p_idx);
                    (incidence != 0).then(|| {
                        let incidence_term = solver.mk_int(i64::from(incidence));
                        solver.mul([weight.clone(), incidence_term])
                    })
                })
                .collect();
            if !muls.is_empty() {
                let sum = solver.add(muls);

                let eq_zero = solver.eq(&sum, &zero);
                let invariant_cond = solver.implies(&is_invariant, &eq_zero);
                solver.assert(&invariant_cond);
                
                let le_zero = solver.le(&sum, &zero);
                let subvariant_cond = solver.implies(&is_subvariant, &le_zero);
                solver.assert(&subvariant_cond);

                let ge_zero = solver.ge(&sum, &zero);
                let survariant_cond = solver.implies(&is_survariant, &ge_zero);
                solver.assert(&survariant_cond);
            }
        }

        solver.push();

        Self { solver, weights, is_invariant, is_subvariant, is_survariant }
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
        let target_lt = self.solver.lt(&target_dot, &m0_dot);
        let target_gt = self.solver.gt(&target_dot, &m0_dot);
        let target_neq = self.solver.or([target_lt.clone(), target_gt.clone()]);

        {
            // Try to find a violated invariant first.
            self.solver.push();

            let invariant_cond = self.solver.and([self.is_invariant.clone(), target_neq]);
            self.solver.assert(&invariant_cond);

            if self.solver.check() == Satisfiability::Sat {
                return self.extract_and_cleanup(problem, PInvariantKind::Invariant);
            }

            // Couldn't find anything.
            self.solver.pop();
        }
        {
            // That didn't work, so try to find a violated sub- or sur-invariant.
            self.solver.push();

            let subvariant_cond = self.solver.and([self.is_subvariant.clone(), target_gt]);
            let survariant_cond = self.solver.and([self.is_survariant.clone(), target_lt]);
            let either = self.solver.or([subvariant_cond, survariant_cond]);
            self.solver.assert(&either);

            if self.solver.check() == Satisfiability::Sat {
                let kind = match (
                    self.solver.eval_bool(&self.is_subvariant),
                    self.solver.eval_bool(&self.is_survariant)
                ) {
                    (Some(true), Some(false)) => PInvariantKind::Subinvariant,
                    (Some(false), Some(true)) => PInvariantKind::Surinvariant,
                    (sub, sur) => panic!("unexpected model: subvariant={sub:?}, survariant={sur:?}"),
                };
                return self.extract_and_cleanup(problem, kind);
            }

            // Couldn't find anything.
            self.solver.pop();
        }
        None
    }

    fn extract_and_cleanup(
        &mut self,
        problem: &CegarProblem,
        kind: PInvariantKind,
    ) -> Option<PInvariantRefinement> {
        let weights: Vec<(PlaceIdx, u32)> = self.weights.iter()
            .enumerate()
            .filter_map(|(p_idx, weight_term)| {
                let w = self.solver.eval_int(weight_term).tap_none(|| {
                    eprintln!("warning! no model assignment for weight at place {p_idx}");
                })?;
                (w > 0).then_some((p_idx, w))
            })
            .collect();

        // Pop the solver state to remove the assertions we added for this check,
        // so that the next check starts from a clean slate.
        self.solver.pop();

        if weights.is_empty() {
            return None;
        }

        let value: u32 = weights.iter().map(|&(p, w)| w * problem.m0[p]).sum();

        Some(PInvariantRefinement(IdxPInvariant {
            weights,
            value,
            kind,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct IdxPInvariant {
    /// The places and their weights that define the invariant.
    pub weights: Vec<(PlaceIdx, u32)>,
    /// The weighted sum of the initial marking's tokens over the invariant's places.
    pub value: u32,
    /// The kind of invariant: exact, subvariant, or survariant.
    pub kind: PInvariantKind,
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
        let m0_value = solver.mk_int(i64::from(p_invariant.value));
        let domain = match p_invariant.kind {
            PInvariantKind::Invariant => solver.eq(&weighted_sum, &m0_value),
            PInvariantKind::Subinvariant => solver.le(&weighted_sum, &m0_value),
            PInvariantKind::Surinvariant => solver.ge(&weighted_sum, &m0_value),
        };
        let lemma = IdxLemma::PInvariant(p_invariant);
        if let Some(callback) = callback {
            callback(lemma.clone());
        }
        solver.assert_tracked(&domain, lemma);
    }
}
