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
    mut places: PlaceIdxSet,
) -> IdxSiphon {
    loop {
        let to_remove: Vec<PlaceIdx> = places
            .place_indices()
            .filter(|&p| {
                // Check if some t ∈ •p has no input place in D.
                net.preset_p[p].iter().any(|&t_idx| {
                    // t ∈ •p. For the siphon property, we need t ∈ D•,
                    // i.e. t consumes from some place in D.
                    // If it doesn't, then p cannot be in the siphon.
                    net.preset_t[t_idx]
                        .iter()
                        .all(|&p_idx| !places.contains(p_idx))
                })
            })
            .collect();
        if to_remove.is_empty() {
            break;
        }
        for p_idx in to_remove {
            places.remove(p_idx);
        }
    }
    places
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
    mut places: PlaceIdxSet,
) -> IdxTrap {
    loop {
        let to_remove: Vec<PlaceIdx> = places
            .place_indices()
            .filter(|&p_idx| {
                // Check if some t ∈ p• has no output place in Q.
                // p• = transitions that consume from p = postset_p(p)
                net.postset_p[p_idx].iter().any(|&t| {
                    // t ∈ p•. For the trap property, we need t ∈ •Q,
                    // i.e. t produces into some place in Q.
                    // t• = postset_t(t) = output places of t.
                    !net.postset_t[t].iter().any(|&p_idx| places.contains(p_idx))
                })
            })
            .collect();
        if to_remove.is_empty() {
            break;
        }
        for p_idx in to_remove {
            places.remove(p_idx);
        }
    }
    places
}

/// Constructs an SMT formula to find a proper siphon whose maximal trap is unmarked
/// in the given marking.
///
/// Implementation Reference: [Oanea et al. 2010]
pub fn find_proper_siphon_with_no_marked_trap<S: SmtSolver>(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
) -> Option<(IdxSiphon, IdxTrap)> {
    let mut solver = S::default();

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
    let in_trap = (0..net.place_count()).fold(in_siphon.clone(), |in_trap, _| {
        net.postset_p
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
            .collect::<Vec<_>>()
    });

    // look for an unmarked trap
    for p_idx in marking.support() {
        let not_in_trap = solver.not(&in_trap[p_idx]);
        solver.assert(&not_in_trap);
    }

    // is there a siphon whose maximal trap is unmarked?
    match solver.check() {
        Satisfiability::Unsat => None,
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

            Some((siphon, trap))
        }
    }
}
