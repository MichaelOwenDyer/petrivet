use crate::core::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::net::PlaceIdx;
use crate::core::net::incidence::IdxIncidenceMatrix;
use crate::core::solver::{Satisfiability, SmtSolver};
use crate::core::system::marking::IdxMarking;
use crate::net::p_invariant::PInvariantKind;
use tap::TapOptional;

/// Ensures that the SMT solver respects the P-Invariants of the net.
pub struct PInvariantRule<S: SmtSolver> {
    /// The SMT solver instance used to encode and check the P-Invariant constraints.
    solver: S,
    /// The weight variables for each place in the net, which the SMT solver will assign values to
    /// in order to find a P-Invariant.
    weight_terms: Vec<S::Int>,
    /// Boolean variable indicating whether the invariant condition is being enforced
    /// (i.e., the weighted sum is always constant).
    is_invariant: S::Bool,
    /// Boolean variable indicating whether the subinvariant condition is being enforced
    /// (i.e., the weighted sum is non-increasing).
    is_subinvariant: S::Bool,
    /// Boolean variable indicating whether the surinvariant condition is being enforced
    /// (i.e., the weighted sum is non-decreasing).
    is_surinvariant: S::Bool,
}

impl<S: SmtSolver> PInvariantRule<S> {
    pub fn new(incidence_matrix: &IdxIncidenceMatrix) -> Self {
        let mut solver = S::default();

        let is_invariant = solver.mk_bool_var("invariant");
        let is_subinvariant = solver.mk_bool_var("subinvariant");
        let is_surinvariant = solver.mk_bool_var("surinvariant");

        let zero = solver.mk_int(0);
        let weight_terms: Vec<S::Int> = incidence_matrix
            .place_indices()
            .map(|p_idx| solver.mk_int_var(&format!("p_idx_{p_idx}")))
            .collect();
        for weight in &weight_terms {
            let weight_ge_zero = solver.ge(weight, &zero);
            solver.assert(&weight_ge_zero);
        }

        for t_idx in incidence_matrix.transition_indices() {
            let weighted_effects: Vec<S::Int> = incidence_matrix
                .place_indices()
                .zip(weight_terms.iter())
                .filter_map(|(p_idx, weight)| {
                    let incidence = incidence_matrix.get_effect(t_idx, p_idx);
                    (incidence != 0).then(|| {
                        let incidence_term = solver.mk_int(i64::from(incidence));
                        solver.mul([weight.clone(), incidence_term])
                    })
                })
                .collect();
            if !weighted_effects.is_empty() {
                let effect = solver.add(weighted_effects);
                let effect_eq_zero = solver.eq(&effect, &zero);
                let effect_le_zero = solver.le(&effect, &zero);
                let effect_ge_zero = solver.ge(&effect, &zero);
                let seek_invariant_when_enabled = solver.implies(&is_invariant, &effect_eq_zero);
                let seek_subinvariant_when_enabled = solver.implies(&is_subinvariant, &effect_le_zero);
                let seek_surinvariant_when_enabled = solver.implies(&is_surinvariant, &effect_ge_zero);
                solver.assert(&seek_invariant_when_enabled);
                solver.assert(&seek_subinvariant_when_enabled);
                solver.assert(&seek_surinvariant_when_enabled);
            }
        }

        Self { solver, weight_terms, is_invariant, is_subinvariant, is_surinvariant }
    }
}

/// The weighted sum of `weights` over `marking`'s places, or `zero` if every place has either
/// a zero weight or a zero token count (`solver.add` must not be called with an empty collection).
fn dot_product<S: SmtSolver>(
    solver: &mut S,
    weights: &[S::Int],
    marking: &IdxMarking<u32>,
) -> S::Int {
    let weighted_tokens: Vec<S::Int> = weights
        .iter()
        .zip(marking.iter())
        .filter(|&(_, &tokens)| tokens > 0)
        .map(|(weight, &tokens)| {
            let tokens_term = solver.mk_int(i64::from(tokens));
            solver.mul([weight.clone(), tokens_term])
        })
        .collect();
    if weighted_tokens.is_empty() {
        solver.mk_int(0)
    } else {
        solver.add(weighted_tokens)
    }
}

impl<S: SmtSolver> PInvariantRule<S> {
    /// Checks whether the given candidate marking violates any P-Invariant of the net.
    pub fn check(
        &mut self,
        problem: &CegarProblem,
        candidate: &IdxMarking<u32>,
    ) -> Option<PInvariantRefinement> {
        let m0_value = dot_product(&mut self.solver, &self.weight_terms, problem.m0);
        let target_value = dot_product(&mut self.solver, &self.weight_terms, candidate);

        let target_lt = self.solver.lt(&target_value, &m0_value);
        let target_gt = self.solver.gt(&target_value, &m0_value);
        let target_neq = self.solver.or(&[target_lt.clone(), target_gt.clone()]);

        // Try to find a violated exact invariant first.
        {
            self.solver.push();

            // Seek a constant weighted sum in the net, such that...
            self.solver.assert(&self.is_invariant);
            // ...the value of the target is not equal to the value of the initial marking.
            self.solver.assert(&target_neq);

            if self.solver.check() == Satisfiability::Sat {
                // Found one
                return self.extract_and_cleanup(problem, PInvariantKind::Invariant);
            }

            // Couldn't find anything.
            self.solver.pop();
        }
        // That didn't work, so try to find a violated subinvariant or surinvariant.
        {
            self.solver.push();

            // Seek a non-increasing weighted sum in the net such that the value of the target
            // is greater than the value of the initial marking.
            let subinvariant_cond = self.solver.and(&[self.is_subinvariant.clone(), target_gt]);
            // Seek a non-decreasing weighted sum in the net such that the value of the target
            // is less than the value of the initial marking.
            let surinvariant_cond = self.solver.and(&[self.is_surinvariant.clone(), target_lt]);
            let either = self.solver.or(&[subinvariant_cond, surinvariant_cond]);
            self.solver.assert(&either);

            if self.solver.check() == Satisfiability::Sat {
                let kind = match (
                    self.solver.eval_bool(&self.is_subinvariant),
                    self.solver.eval_bool(&self.is_surinvariant)
                ) {
                    // we expect exactly one of these to be true, but not both.
                    (Some(true), Some(false)) => PInvariantKind::Subinvariant,
                    (Some(false), Some(true)) => PInvariantKind::Surinvariant,
                    (sub, sur) => panic!("unexpected model: subinvariant={sub:?}, surinvariant={sur:?}"),
                };
                return self.extract_and_cleanup(problem, kind);
            }

            // Couldn't find anything.
            self.solver.pop();
        }
        None
    }

    /// Extracts the P-Invariant from the solver's model and cleans up the solver state.
    fn extract_and_cleanup(
        &mut self,
        problem: &CegarProblem,
        kind: PInvariantKind,
    ) -> Option<PInvariantRefinement> {
        let weights: Vec<(PlaceIdx, u32)> = self.weight_terms.iter()
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
            // not expected: all zero weights would be equal to 0 for every target and initial marking,
            // so the solver should not have returned SAT in the first place.
            eprintln!("warning! no non-zero weights found in P-Invariant model");
            return None;
        }

        // The value of the invariant in the initial marking defines its domain for all reachable markings.
        let value: u32 = weights.iter().map(|&(p, w)| w * problem.m0[p]).sum();

        Some(PInvariantRefinement(IdxPInvariant {
            weights,
            value,
            kind,
        }))
    }
}

/// A P-Invariant is a weighted sum of places which is either constant, non-increasing, or non-decreasing,
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdxPInvariant {
    /// The places and their weights that define the invariant.
    pub weights: Vec<(PlaceIdx, u32)>,
    /// The weighted sum of the initial marking's tokens over the invariant's places.
    pub value: u32,
    /// The kind of invariant: exact, subinvariant, or surinvariant.
    pub kind: PInvariantKind,
}

/// A refinement that encodes a P-Invariant into the SMT solver.
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
