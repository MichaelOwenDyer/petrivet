use crate::core::net::idx_set::PlaceIdxSet;
use crate::core::net::{DenseNet, PlaceIdx};
use crate::core::solver::{Satisfiability, SmtSolver};
use crate::core::system::marking::IdxMarking;

/// Internal representation of a [`Siphon`](crate::net::siphon_trap::Siphon).
pub type IdxSiphon = PlaceIdxSet;

/// Internal representation of a [`Trap`](crate::net::siphon_trap::Trap).
pub type IdxTrap = PlaceIdxSet;

/// Computes the maximal siphon contained in a given set of places.
///
/// A siphon is a set of places D such that •D ⊆ D•: every transition that
/// produces into D also consumes from D. Once empty, it stays empty forever.
///
/// Uses the shrinking algorithm from the [Petri Net Primer, Algorithm 6.19]:
/// iteratively remove any place p where some transition t ∈ •p has no
/// input place in the current set. Runs in O(|S|² · |T|²).
#[must_use]
pub fn maximal_siphon_in(
    net: &DenseNet,
    mut siphon: PlaceIdxSet,
) -> IdxSiphon {
    let mut worklist: Vec<PlaceIdx> = siphon.iter().collect();
    while let Some(p_idx) = worklist.pop() {
        // Check if some t ∈ •p has no input place in the siphon. If so, p cannot be in the siphon.
        let should_remove = net.preset_p[p_idx]
            .iter()
            .any(|&t| net.preset_t[t].iter().all(|&q| !siphon[q]));
        if should_remove {
            siphon.remove(p_idx);
            for &t in &net.postset_p[p_idx] {
                for &q in &net.postset_t[t] {
                    if siphon[q] {
                        worklist.push(q);
                    }
                }
            }
        }
    }
    siphon
}

/// Computes the maximal trap contained in a given set of places.
///
/// A trap Q satisfies Q• ⊆ •Q: every transition that consumes from Q also
/// produces into Q. Once marked, a trap stays marked forever.
///
/// Uses the dual of the shrinking algorithm: iteratively remove any place p
/// where some transition t ∈ p• has no output place in the current set.
#[must_use]
pub fn maximal_trap_in(
    net: &DenseNet,
    mut trap: PlaceIdxSet,
) -> IdxTrap {
    let mut worklist: Vec<PlaceIdx> = trap.iter().collect();
    while let Some(p_idx) = worklist.pop() {
        // Check if some t ∈ p• has no output place in the trap. If so, p cannot be in the trap.
        let should_remove = net.postset_p[p_idx]
            .iter()
            .any(|&t| net.postset_t[t].iter().all(|&q| !trap[q]));
        if should_remove {
            trap.remove(p_idx);
            for &t in &net.preset_p[p_idx] {
                for &q in &net.preset_t[t] {
                    if trap[q] {
                        worklist.push(q);
                    }
                }
            }
        }
    }
    trap
}

pub enum ProperSiphonWithNoMarkedTrapResult {
    /// Found a proper siphon whose maximal trap is unmarked in the given marking.
    Found(IdxSiphon, IdxTrap),
    /// No proper siphon exists whose maximal trap is unmarked in the given marking.
    None,
    /// The solver could not determine whether such a siphon exists.
    Unknown,
}

/// Constructs an SMT formula to find a proper siphon whose maximal trap is unmarked
/// in the given marking.
///
/// Implementation Reference: [Oanea et al. 2010]
pub fn find_proper_siphon_with_no_marked_trap<S: SmtSolver>(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
    deadline: Option<std::time::Instant>,
) -> ProperSiphonWithNoMarkedTrapResult {
    // the formula has a term for every arc in every one of |P| rounds, so for a large net
    // building and asserting it is slow enough to need bounding by more than the final `check`
    S::with_deadline(deadline, || find_proper_siphon_with_no_marked_trap_unbounded::<S>(net, marking, deadline))
}

fn find_proper_siphon_with_no_marked_trap_unbounded<S: SmtSolver>(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
    deadline: Option<std::time::Instant>,
) -> ProperSiphonWithNoMarkedTrapResult {
    let mut solver = S::default();
    solver.set_deadline(deadline);

    // let solver choose which places to include in the siphon
    let in_siphon: Vec<S::Bool> = net
        .place_indices()
        .map(|p_idx| solver.mk_bool_var(&format!("p_idx_0_{p_idx}")))
        .collect();

    // siphon must contain at least one place
    let proper_siphon = solver.or(&in_siphon);
    solver.assert(&proper_siphon);

    // if a transition produces to a place in the siphon, then it
    // must also consume from the siphon (i.e., the siphon property •D ⊆ D•).
    for t_idx in net.transition_indices() {
        let produces_to_siphon = solver.or(
            &net.postset_t[t_idx]
                .iter()
                .map(|&p_idx| in_siphon[p_idx].clone())
                .collect::<Vec<_>>()
        );
        let consumes_from_siphon = solver.or(
            &net.preset_t[t_idx]
                .iter()
                .map(|&p_idx| in_siphon[p_idx].clone())
                .collect::<Vec<_>>()
        );
        let siphon_property = solver.implies(&produces_to_siphon, &consumes_from_siphon);
        solver.assert(&siphon_property);
    }

    // Now express the maximal trap inside the siphon as a logical combination of the existing free variables.
    // Start with the siphon itself as the initial candidate for the trap, and "iteratively" remove
    // places that violate the trap property Q• ⊆ •Q. This is done by unrolling the loop for a fixed
    // upper bound of iterations (number of places in the net).
    let mut in_trap = in_siphon.clone();
    for _ in 0..net.place_count() {
        // each round adds a term for every arc, so on a large net building the formula alone takes a while
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            return ProperSiphonWithNoMarkedTrapResult::Unknown;
        }
        in_trap = net.postset_p
            .iter()
            .zip(in_trap.iter())
            .map(|(consuming_transitions, currently_in_trap)| {
                let consuming_transitions_also_produce_to_trap = consuming_transitions
                    .iter()
                    .map(|&t_idx| solver.or(
                        &net.postset_t[t_idx]
                            .iter()
                            .map(|&p_idx| in_trap[p_idx].clone())
                            .collect::<Vec<_>>()
                    ))
                    .collect::<Vec<_>>();
                let trap_condition_continues_to_hold = solver.and(
                    &consuming_transitions_also_produce_to_trap
                );
                solver.and(&[currently_in_trap.clone(), trap_condition_continues_to_hold])
            })
            .collect::<Vec<_>>();
    }

    // look for an unmarked trap
    for p_idx in marking.support() {
        let not_in_trap = solver.not(&in_trap[p_idx]);
        solver.assert(&not_in_trap);
    }

    // is there a siphon whose maximal trap is unmarked?
    match solver.check() {
        Satisfiability::Unknown => ProperSiphonWithNoMarkedTrapResult::Unknown,
        Satisfiability::Unsat => ProperSiphonWithNoMarkedTrapResult::None,
        Satisfiability::Sat => {
            let mut siphon = PlaceIdxSet::none_of(net.place_count());
            let mut trap = PlaceIdxSet::none_of(net.place_count());

            for p_idx in net.place_indices() {
                if solver.eval_bool(&in_siphon[p_idx]).unwrap_or(false) {
                    siphon.add(p_idx);
                }
                if solver.eval_bool(&in_trap[p_idx]).unwrap_or(false) {
                    trap.add(p_idx);
                }
            }

            ProperSiphonWithNoMarkedTrapResult::Found(siphon, trap)
        }
    }
}
