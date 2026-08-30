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
/// Uses the shrinking algorithm from the [Petri Net Primer, Algorithm 6.19](crate::literature#algorithm-619--maximal-siphontrap-in-a-subset):
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

/// Internal representation of [`SiphonTrapPair`](crate::system::siphon_trap::SiphonTrapPair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdxSiphonTrapPair {
    /// The minimal siphon (a set of places D with •D ⊆ D•).
    pub siphon: IdxSiphon,
    /// The maximal trap contained in this siphon (a set of places Q with Q• ⊆ •Q).
    /// Empty if no trap was found.
    pub trap: IdxTrap,
}

/// Internal representation of [`CommonerHackCriterionResult`](crate::system::siphon_trap::CommonerHackCriterionResult).
pub type CommonerHackCriterionResult = Result<(), IdxSiphonTrapPair>;

/// Checks the Commoner/Hack Criterion (CHC): every proper siphon contains
/// a trap that is marked under the given marking.
///
/// For free-choice nets, this is a necessary and sufficient condition for
/// liveness: [Murata, Theorem 12](crate::literature#theorem-12--commonerhack-criterion).
/// For asymmetric-choice nets it is sufficient but not necessary: [Murata, Theorem 15](crate::literature#theorem-15--liveness-of-asymmetric-choice-nets).
///
/// This is the key structural shortcut for proving liveness in free-choice nets
/// without exploring the full state space. This is significant because it runs
/// in polynomial time rather than exponential (in the number of reachable states).
///
/// For general nets, it is a sufficient condition for deadlock-freedom.
///
/// Instead of pre-enumerating all traps and checking containment, this
/// computes the maximal trap *inside* each siphon directly using the
/// shrinking algorithm. If the maximal trap is non-empty and marked,
/// the siphon satisfies the condition; if it is empty or unmarked,
/// no trap inside the siphon can be marked.
/// 
/// References:
/// - Oanea, Olivia, Harro Wimmel, and Karsten Wolf. 2010. “New Algorithms for Deciding the Siphon-Trap Property.” In Applications and Theory of Petri Nets, edited by Johan Lilius and Wojciech Penczek. Springer. https://doi.org/10.1007/978-3-642-13675-7_16.
/// - Murata, T. 1989. “Petri Nets: Properties, Analysis and Applications.” Proceedings of the IEEE 77 (4): 541–80. <https://doi.org/10.1109/5.24143>. Theorem 12
/// - Best, Eike, and Raymond Devillers. 2024. “Petri Net Primer.” Computer Science Foundations. Theorem 5.17
/// - Best, Eike, and Raymond Devillers. 2024. “Petri Net Primer.” Computer Science Foundations. Algorithm 6.19
pub fn commoner_hack_criterion<S: SmtSolver>(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
) -> CommonerHackCriterionResult {
    let mut solver = S::default();
    let place_count = net.place_count();

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

    // Now compute the maximal trap inside the siphon.
    // Start with the siphon itself as the initial candidate for the trap, and "iteratively" remove
    // places that violate the trap property Q• ⊆ •Q. This is done in a single SMT formula by
    // unrolling the loop for an upper bound of iterations (number of places in the net).
    let in_trap = (0..place_count).fold(in_siphon.clone(), |in_trap, _| {
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
        // no: CHC holds.
        Satisfiability::Unsat => Ok(()),
        // yes: return the siphon and unmarked trap found by the solver
        // (counterexample to the CHC).
        Satisfiability::Sat => {
            let mut siphon = PlaceIdxSet::none_of(place_count);
            let mut trap = PlaceIdxSet::none_of(place_count);

            for p_idx in net.place_indices() {
                if solver.eval_bool(&in_siphon[p_idx]).unwrap_or(false) {
                    siphon.add(p_idx);
                }
                if solver.eval_bool(&in_trap[p_idx]).unwrap_or(false) {
                    trap.add(p_idx);
                }
            }

            Err(IdxSiphonTrapPair { siphon, trap })
        }
    }
}
