use crate::core::analysis::incidence::IncidenceMatrix;
use crate::core::marking::IdxMarking;
use crate::core::net::{DenseNet, PlaceIdx};
use oxiz::core::TermKind;
use oxiz::solver::Model;
use oxiz::{Solver, SolverResult, TermId, TermManager};
use tap::TapOptional;

pub fn find_candidate_covering_parikh_vector(
    net: &DenseNet,
    m0: &IdxMarking<u32>,
    target: &IdxMarking<u32>,
) -> Option<Vec<u32>> {
    let mut solver = Solver::new();
    let mut tm = TermManager::new();

    let place_terms: Vec<_> = net.place_indices()
        .map(|p_idx| tm.mk_var(&format!("p{p_idx}"), tm.sorts.int_sort))
        .collect();

    for (&place, &target_tokens) in place_terms.iter().zip(target.iter()) {
        let lower = tm.mk_int(target_tokens);
        let ge = tm.mk_ge(place, lower);
        solver.assert(ge, &mut tm);
    }

    let zero = tm.mk_int(0u32);
    let incidence_matrix = net.incidence_matrix();
    let mut transition_terms: Option<Vec<_>> = None;

    let extract_u32 = |model: &Model, tm: &TermManager, term_id: TermId| {
        let evaluated_term = model.get(term_id)
            .tap_none(|| eprintln!("warning! model did not contain {term_id:?}. model: {model:?}"))?;
        match &tm.get(evaluated_term)
            .tap_none(|| eprintln!("warning! evaluation of {term_id:?} failed, term manager did not contain {evaluated_term:?}. model: {model:?}"))?
            .kind
        {
            TermKind::IntConst(n) =>
                u32::try_from(n).ok().tap_none(|| eprintln!("warning! {evaluated_term:?} is not a valid u32: {n}")),
            TermKind::RealConst(r) if *r.denom() == 1 =>
                u32::try_from(*r.numer()).ok().tap_none(|| eprintln!("warning! real constant {r} does not fit in u32")),
            TermKind::RealConst(r) => {
                eprintln!("warning! {term_id:?} evaluated to non-integer real {r}");
                None
            }
            _ => {
                eprintln!("warning! {term_id:?} evaluated to {evaluated_term:?} which is not a numeric constant. model: {model:?}");
                None
            }
        }
    };

    loop {
        let result = solver.check(&mut tm);
        if result == SolverResult::Sat && let Some(model) = solver.model() {
            let candidate_solution: IdxMarking<u32> = place_terms.iter().copied()
                .map(|place_term| extract_u32(model, &tm, place_term))
                .collect::<Option<_>>()?;

            // priority list of techniques to rule out candidate solutions:
            match &transition_terms {
                // #1: check for violated S-invariant
                _ if let Some(invariant) = find_violated_s_invariant(&incidence_matrix, m0, &candidate_solution) => {
                    let constant_tokens: u32 = invariant.iter().map(|&(p, w)| w * m0[p]).sum();
                    let weighted_places: Vec<_> = invariant.iter()
                        .map(|&(p, w)| {
                            let weight = tm.mk_int(w);
                            tm.mk_mul([place_terms[p], weight])
                        })
                        .collect();
                    let lhs = tm.mk_add(weighted_places);
                    let rhs = tm.mk_int(constant_tokens);
                    solver.assert(tm.mk_eq(lhs, rhs), &mut tm);
                },
                // #2: check for trap which was marked in M₀ but is not marked anymore (impossible)
                _ if let Some(trap) = find_violated_initially_marked_trap(net, m0, &candidate_solution) => {
                    let trap_sum = tm.mk_add(trap.iter().map(|&p| place_terms[p]));
                    solver.assert(tm.mk_gt(trap_sum, zero), &mut tm);
                }
                // #3: add new transition variables and enforce the state equation (only once)
                None => {
                    let t_terms: Vec<_> = net.transition_indices()
                        .map(|t_idx| {
                            let var = tm.mk_var(&format!("t{t_idx}"), tm.sorts.int_sort);
                            solver.assert(tm.mk_ge(var, zero), &mut tm);
                            var
                        })
                        .collect();
                    for p in net.place_indices() {
                        let m0_p = tm.mk_int(m0[p]);
                        let effect_terms: Vec<_> = net.transition_indices()
                            .filter_map(|t| {
                                let n = incidence_matrix.get(p, t);
                                if n != 0 {
                                    let n_term = tm.mk_int(n);
                                    Some(tm.mk_mul([t_terms[t], n_term]))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        let rhs = if effect_terms.is_empty() {
                            m0_p
                        } else {
                            let effect_sum = tm.mk_add(effect_terms);
                            tm.mk_add([m0_p, effect_sum])
                        };
                        solver.assert(tm.mk_eq(place_terms[p], rhs), &mut tm);
                    }
                    // we won't reach this match arm a second time
                    transition_terms = Some(t_terms);
                }
                // out of options: extract the candidate parikh vector and return it for final verification
                Some(transition_terms) => {
                    let parikh: Vec<u32> = transition_terms.iter()
                        .copied()
                        .map(|transition_term| extract_u32(model, &tm, transition_term))
                        .collect::<Option<_>>()?;
                    return Some(parikh);
                }
            }
        } else {
            if result != SolverResult::Unsat {
                eprintln!("warning! unexpected oxiz solver state!\nDebug info:\n{solver:#?}\n{tm:#?}");
            }
            return None;
        }
    }
}

fn find_violated_s_invariant(
    incidence_matrix: &IncidenceMatrix,
    m0: &IdxMarking<u32>,
    candidate: &IdxMarking<u32>,
) -> Option<Vec<(PlaceIdx, u32)>> {
    let mut solver = Solver::new();
    let mut tm = TermManager::new();

    let weights: Vec<_> = incidence_matrix.place_indices()
        .map(|p_idx| tm.mk_var(&format!("p{p_idx}"), tm.sorts.int_sort))
        .collect();

    let zero = tm.mk_int(0u32);

    for &weight in &weights {
        solver.assert(tm.mk_ge(weight, zero), &mut tm);
    }

    for t_idx in incidence_matrix.transition_indices() {
        let muls: Vec<_> = incidence_matrix.place_indices()
            .zip(weights.iter().copied())
            .filter_map(|(p_idx, weight)| {
                let incidence = incidence_matrix.get(p_idx, t_idx);
                if incidence != 0 {
                    let incidence_term = tm.mk_int(incidence);
                    Some(tm.mk_mul([weight, incidence_term]))
                } else {
                    None
                }
            })
            .collect();
        if !muls.is_empty() {
            let sum = tm.mk_add(muls);
            solver.assert(tm.mk_eq(sum, zero), &mut tm);
        }
    }

    let dot = |marking: &IdxMarking<u32>, tm: &mut TermManager| {
        let muls: Vec<_> = weights.iter().copied()
            .zip(marking.iter().copied())
            .filter(|&(_, tokens)| tokens > 0)
            .map(|(weight, tokens)| {
                let tokens_term = tm.mk_int(tokens);
                tm.mk_mul([weight, tokens_term])
            })
            .collect();
        if muls.is_empty() { zero } else { tm.mk_add(muls) }
    };

    let m0_dot = dot(m0, &mut tm);
    let target_dot = dot(candidate, &mut tm);

    solver.assert(tm.mk_distinct([m0_dot, target_dot]), &mut tm);

    if solver.check(&mut tm) == SolverResult::Sat
        && let Some(model) = solver.model()
    {
        let invariant: Vec<(PlaceIdx, u32)> = incidence_matrix
            .place_indices()
            .zip(weights.iter().copied())
            .filter_map(|(p_idx, weight_term)| {
                let val_id = model.get(weight_term)
                    .tap_none(|| eprintln!("warning! no model assignment for weight {weight_term:?}"))?;
                if let TermKind::IntConst(n) = &tm.get(val_id)?.kind {
                    let w = u32::try_from(n).ok().tap_none(|| eprintln!("warning! weight {val_id:?} is not a valid u32: {n}"))?;
                    (w > 0).then_some((p_idx, w))
                } else {
                    eprintln!("warning! weight {weight_term:?} has unexpected kind: {:?}", tm.get(val_id).map(|t| &t.kind));
                    None
                }
            })
            .collect();
        (!invariant.is_empty()).then_some(invariant)
    } else {
        None
    }
}

fn find_violated_initially_marked_trap(
    net: &DenseNet,
    m0: &IdxMarking<u32>,
    candidate: &IdxMarking<u32>,
) -> Option<Vec<PlaceIdx>> {
    let mut in_trap: Vec<bool> = (0..net.place_count())
        .map(|p| candidate[p] == 0)
        .collect();

    let mut worklist: Vec<PlaceIdx> = (0..net.place_count())
        .filter(|&p| in_trap[p])
        .collect();

    while let Some(p) = worklist.pop() {
        if !in_trap[p] {
            continue;
        }
        let violates = net.postset_p[p]
            .iter()
            .any(|&t| net.postset_t[t].iter().all(|&q| !in_trap[q]));
        if violates {
            in_trap[p] = false;
            for &t in &net.preset_p[p] {
                for &q in &net.preset_t[t] {
                    if in_trap[q] {
                        worklist.push(q);
                    }
                }
            }
        }
    }

    let trap: Vec<PlaceIdx> = (0..net.place_count()).filter(|&p| in_trap[p]).collect();
    trap.iter().any(|&p| m0[p] > 0).then_some(trap)
}
