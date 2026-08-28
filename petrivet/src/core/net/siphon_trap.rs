use crate::core::net::idx_set::PlaceIdxSet;
use crate::core::net::{DenseNet, PlaceIdx};
use crate::core::system::marking::IdxMarking;
use good_lp::Variable;
use std::collections::HashSet;

/// A siphon is a set of places D such that •D ⊆ D•.
///
/// In other words, every transition that produces to D also consumes from D.
/// This is significant because it means once a siphon is unmarked,
/// it can never be marked again (all transitions which could mark it are dead).
pub type IdxSiphon = PlaceIdxSet;

/// A trap is a set of places Q such that Q• ⊆ •Q.
///
/// In other words, every transition that consumes from Q also produces to Q.
/// This is significant because it means once a trap is marked, it can never be unmarked again.
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

/// Finds all minimal siphons of a net as sets of [`PlaceIdx`].
#[must_use]
pub fn minimal_siphons(net: &DenseNet) -> Box<[IdxSiphon]> {
    let mut results: Vec<PlaceIdxSet> = Vec::new();
    let mut stack: Vec<PlaceIdxSet> = vec![PlaceIdxSet::all_of(net.place_count())];
    let mut visited: ahash::HashSet<PlaceIdxSet> = HashSet::default();

    while let Some(candidate_set) = stack.pop() {
        let siphon = maximal_siphon_in(net, candidate_set);
        if siphon.is_empty() {
            continue;
        }

        if !visited.insert(siphon.clone()) {
            continue;
        }

        // Try excluding each place to find potentially smaller siphons.
        let mut is_minimal = true;
        for p_idx in siphon.place_indices() {
            let mut reduced = siphon.clone();
            reduced.remove(p_idx);
            if reduced.is_empty() {
                continue;
            }
            let smaller_siphon = maximal_siphon_in(net, reduced);
            if !smaller_siphon.is_empty() {
                is_minimal = false;
                stack.push(smaller_siphon);
            }
        }

        if is_minimal {
            let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
            if !dominated {
                results.retain(|existing| !siphon.is_subset(existing));
                results.push(siphon);
            }
        }
    }

    results.into_boxed_slice()
}

/// Finds all minimal traps of a net.
///
/// A trap is a set of places D where D• ⊆ •D: every transition that
/// takes tokens out of D also puts tokens into D. Once a token is present
/// in a trap, the trap can never become unmarked again.
#[must_use]
#[expect(unused)]
pub fn minimal_traps(net: &DenseNet) -> Box<[PlaceIdxSet]> {
    let mut results: Vec<PlaceIdxSet> = Vec::new();
    let mut stack = vec![PlaceIdxSet::all_of(net.place_count())];
    let mut visited: ahash::HashSet<PlaceIdxSet> = HashSet::default();

    while let Some(candidate_set) = stack.pop() {
        let trap = maximal_trap_in(net, candidate_set);
        if trap.is_empty() {
            continue;
        }

        if !visited.insert(trap.clone()) {
            continue;
        }

        let mut is_minimal = true;
        for p_idx in trap.place_indices() {
            let mut reduced = trap.clone();
            reduced.remove(p_idx);
            if reduced.is_empty() {
                continue;
            }
            let smaller_trap = maximal_trap_in(net, reduced);
            if !smaller_trap.is_empty() {
                is_minimal = false;
                stack.push(smaller_trap);
            }
        }

        if is_minimal {
            let dominated = results.iter().any(|existing| existing.is_subset(&trap));
            if !dominated {
                results.retain(|existing| !trap.is_subset(existing));
                results.push(trap);
            }
        }
    }

    results.into_boxed_slice()
}

/// Finds all minimal siphons using ILP enumeration.
///
/// Encodes the siphon property as binary constraints and iteratively
/// solves for minimum-cardinality siphons, adding no-good cuts to
/// exclude previously found solutions. Slower than the backtracking
/// approach for small nets but more systematic.
#[must_use]
#[expect(unused)]
pub fn minimal_siphons_ilp(net: &DenseNet) -> Box<[IdxSiphon]> {
    use good_lp::{Expression, ProblemVariables, Solution, SolverModel, constraint, variable};

    let mut results: Vec<IdxSiphon> = Vec::new();

    let mut vars = ProblemVariables::new();
    let place_selectors: Box<[Variable]> = net
        .place_indices()
        .map(|_| vars.add(variable().binary()))
        .collect();

    let mut constraints = Vec::new();

    let selected_count: Expression = place_selectors.iter().copied().sum();
    constraints.push(constraint!(selected_count >= 1.0));

    // Siphon property: x[p] ≤ Σ_{q ∈ •t} x[q]  for all p, t ∈ •p
    for p in net.place_indices() {
        for &t in &net.preset_p[p] {
            let sum_preset: Expression = net.preset_t[t].iter().map(|&q| place_selectors[q]).sum();
            constraints.push(constraint!(place_selectors[p] <= sum_preset));
        }
    }

    let objective: Expression = place_selectors.iter().copied().sum();
    while let Ok(solution) = vars
        .clone()
        .minimise(&objective)
        .using(good_lp::microlp)
        .with_all(constraints.clone())
        .solve()
    {
        let siphon: IdxSiphon = {
            let mut siphon = PlaceIdxSet::none_of(net.place_count());
            for p_idx in net.place_indices() {
                siphon.set(p_idx, solution.value(place_selectors[p_idx]) > 0.5);
            }
            siphon
        };

        if siphon.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
        if !dominated {
            results.retain(|existing| !siphon.is_subset(existing));
            results.push(siphon.clone());
        }

        let prev_sum: Expression = siphon.place_indices().map(|p| place_selectors[p]).sum();
        #[allow(clippy::cast_precision_loss)]
        constraints.push(constraint!(prev_sum <= siphon.size() as f64 - 1.0));
    }

    results.into_boxed_slice()
}

/// Finds all minimal traps using ILP enumeration.
#[must_use]
#[expect(unused)]
pub fn minimal_traps_ilp(net: &DenseNet) -> Box<[IdxTrap]> {
    use good_lp::{Expression, ProblemVariables, Solution, SolverModel, constraint, variable};

    let mut results: Vec<IdxTrap> = Vec::new();
    let mut no_good_sets: Vec<IdxTrap> = Vec::new();

    loop {
        let mut vars = ProblemVariables::new();
        let x: Vec<_> = net
            .place_indices()
            .map(|_| vars.add(variable().binary()))
            .collect();

        let mut constraints = Vec::new();
        let selected_places: Expression = x.iter().copied().sum();
        constraints.push(constraint!(selected_places.clone() >= 1.0));

        // Trap property: x[p] ≤ Σ_{q ∈ t•} x[q]  for all p, t ∈ p•
        for p in net.place_indices() {
            for &t in &net.postset_p[p] {
                let sum_postset: Expression = net.postset_t[t].iter().map(|&q| x[q]).sum();
                constraints.push(constraint!(x[p] <= sum_postset));
            }
        }

        for prev in &no_good_sets {
            let prev_sum: Expression = prev.place_indices().map(|p| x[p]).sum();
            #[allow(clippy::cast_precision_loss)]
            constraints.push(constraint!(prev_sum <= prev.size() as f64 - 1.0));
        }

        let Ok(solution) = vars
            .minimise(selected_places)
            .using(good_lp::microlp)
            .with_all(constraints)
            .solve()
        else {
            break;
        };

        let trap: IdxTrap = {
            let mut trap = PlaceIdxSet::none_of(net.place_count());
            for p_idx in net.place_indices() {
                trap.set(p_idx, solution.value(x[p_idx]) > 0.5);
            }
            trap
        };

        if trap.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&trap));
        if !dominated {
            results.retain(|existing| !trap.is_subset(existing));
            results.push(trap.clone());
        }
        no_good_sets.push(trap);
    }

    results.into_boxed_slice()
}

/// A minimal siphon and the maximal trap found within it,
/// and whether that trap is marked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiphonTrapPair {
    /// The minimal siphon (a set of places D with •D ⊆ D•).
    pub siphon: IdxSiphon,
    /// The maximal trap contained in this siphon (a set of places Q with Q• ⊆ •Q).
    /// Empty if no trap was found.
    pub trap: IdxTrap,
}

/// Result of the Commoner/Hack criterion check.
///
/// For free-choice nets, this criterion is both necessary and sufficient for
/// liveness: a free-choice system (N, M₀) is live if and only if every proper
/// siphon of N contains a trap that is marked under M₀.
///
/// For general nets, the condition is sufficient for deadlock-freedom but
/// not necessary: if every siphon contains a marked trap, the net is
/// deadlock-free, but the converse does not hold.
///
/// References:
/// - [Murata 1989, Theorem 12](crate::literature#theorem-12--commonerhack-criterion)
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc)
pub type CommonerHackCriterionResult = Result<Box<[SiphonTrapPair]>, SiphonTrapPair>;

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
/// - [Murata 1989, Theorem 12](crate::literature#theorem-12--commonerhack-criterion):
///   "A free-choice net (N, M₀) is live iff every siphon in N contains a marked trap."
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc) (Commoner/Hack Criterion)
/// - [Primer, Algorithm 6.19](crate::literature#algorithm-619--maximal-siphontrap-in-a-subset) (maximal siphon/trap in a subset)
pub fn commoner_hack_criterion(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
) -> CommonerHackCriterionResult {
    minimal_siphons(net)
        .into_iter()
        .try_fold(Vec::new(), |mut acc, siphon| {
            let trap = maximal_trap_in(net, siphon.clone());
            let trap_is_marked = !trap.is_empty() && trap.place_indices().any(|p_idx| marking[p_idx] > 0);
            if trap_is_marked {
                acc.push(SiphonTrapPair { siphon, trap });
                Ok(acc)
            } else {
                Err(SiphonTrapPair { siphon, trap })
            }
        })
        .map(Vec::into_boxed_slice)
}
