use crate::core::marking::IdxMarking;
use crate::core::{DenseNet, PlaceIdx};
use good_lp::Variable;
use std::collections::HashSet;

/// Computes the maximal siphon contained in a given set of places.
///
/// A siphon is a set of places D such that •D ⊆ D•: every transition that
/// produces into D also consumes from D. Once empty, it stays empty forever.
///
/// Uses the shrinking algorithm from the [Petri Net Primer, Algorithm 6.19](crate::literature#algorithm-619--maximal-siphontrap-in-a-subset):
/// iteratively remove any place p where some transition t ∈ •p has no
/// input place in the current set. Runs in O(|S|² · |T|²).
#[must_use]
pub fn maximal_siphon_in<S: std::hash::BuildHasher>(
    net: &DenseNet,
    mut subset: HashSet<PlaceIdx, S>,
) -> HashSet<PlaceIdx, S> {
    loop {
        let mut removed = false;
        let to_remove: Vec<PlaceIdx> = subset.iter().copied().filter(|&p| {
            // Check if some t ∈ •p has no input place in D.
            net.preset_p[p].iter().any(|&t| {
                // t ∈ •p. For the siphon property, we need t ∈ D•,
                // i.e. t consumes from some place in D.
                // If it doesn't, then p cannot be in the siphon.
                net.preset_t[t].iter().all(|q| !subset.contains(q))
            })
        }).collect();
        for p in to_remove {
            subset.remove(&p);
            removed = true;
        }
        if !removed {
            break;
        }
    }
    subset
}

/// Computes the maximal trap contained in a given set of places.
///
/// A trap Q satisfies Q• ⊆ •Q: every transition that consumes from Q also
/// produces into Q. Once marked, a trap stays marked forever.
///
/// Uses the dual of the shrinking algorithm: iteratively remove any place p
/// where some transition t ∈ p• has no output place in the current set.
#[must_use]
pub fn maximal_trap_in<S: std::hash::BuildHasher>(
    net: &DenseNet,
    mut maximal_trap: HashSet<PlaceIdx, S>
) -> HashSet<PlaceIdx, S> {
    loop {
        let mut removed = false;
        let to_remove: Vec<PlaceIdx> = maximal_trap
            .iter()
            .filter(|&&p| {
                // Check if some t ∈ p• has no output place in Q.
                // p• = transitions that consume from p = postset_p(p)
                net.postset_p[p].iter().any(|&t| {
                    // t ∈ p•. For the trap property, we need t ∈ •Q,
                    // i.e. t produces into some place in Q.
                    // t• = postset_t(t) = output places of t.
                    !net.postset_t[t].iter().any(|r| maximal_trap.contains(r))
                })
            })
            .copied()
            .collect();
        for p in to_remove {
            maximal_trap.remove(&p);
            removed = true;
        }
        if !removed {
            break;
        }
    }
    maximal_trap
}

/// Finds all minimal siphons of a net as sets of [`PlaceIdx`].
#[must_use]
pub fn minimal_siphons(net: &DenseNet) -> Box<[HashSet<PlaceIdx>]> {
    let mut results: Vec<HashSet<PlaceIdx>> = Vec::new();
    let mut stack: Vec<HashSet<PlaceIdx>> = vec![net.place_indices().collect()];
    let mut visited: HashSet<Vec<PlaceIdx>> = HashSet::new();

    while let Some(candidate_set) = stack.pop() {
        let siphon = maximal_siphon_in(net, candidate_set);
        if siphon.is_empty() {
            continue;
        }
        let mut key: Vec<PlaceIdx> = siphon.iter().copied().collect();
        key.sort_unstable();
        if !visited.insert(key) {
            continue;
        }

        // Try excluding each place to find potentially smaller siphons.
        let mut is_minimal = true;
        for &p in &siphon {
            let mut reduced = siphon.clone();
            reduced.remove(&p);
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
pub fn minimal_traps(net: &DenseNet) -> Box<[HashSet<PlaceIdx>]> {
    let all_places: HashSet<PlaceIdx> = net.place_indices().collect();
    let mut results: Vec<HashSet<PlaceIdx>> = Vec::new();
    let mut stack: Vec<HashSet<PlaceIdx>> = vec![all_places];
    let mut visited: HashSet<Vec<PlaceIdx>> = HashSet::new();

    while let Some(candidate_set) = stack.pop() {
        let trap = maximal_trap_in(net, candidate_set);
        if trap.is_empty() {
            continue;
        }

        let mut key: Vec<PlaceIdx> = trap.iter().copied().collect();
        key.sort_unstable();
        if !visited.insert(key) {
            continue;
        }

        let mut is_minimal = true;
        for &p in &trap {
            let mut reduced = trap.clone();
            reduced.remove(&p);
            if reduced.is_empty() {
                continue;
            }
            let sub = maximal_trap_in(net, reduced);
            if !sub.is_empty() {
                is_minimal = false;
                stack.push(sub);
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
pub fn minimal_siphons_ilp(
    net: &DenseNet
) -> Box<[HashSet<PlaceIdx>]> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    if net.place_count() == 0 {
        return Box::new([]);
    }

    let mut results: Vec<HashSet<PlaceIdx>> = Vec::new();

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
            let sum_preset: Expression = net
                .preset_t[t]
                .iter()
                .map(|&q| place_selectors[q])
                .sum();
            constraints.push(constraint!(place_selectors[p] <= sum_preset));
        }
    }

    let objective: Expression = place_selectors.iter().copied().sum();
    while let Ok(solution) = vars.clone()
        .minimise(&objective)
        .using(good_lp::microlp)
        .with_all(constraints.clone())
        .solve() {

        let siphon: HashSet<PlaceIdx> = net
            .place_indices()
            .filter(|&p| solution.value(place_selectors[p]) > 0.5)
            .collect();

        if siphon.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
        if !dominated {
            results.retain(|existing| !siphon.is_subset(existing));
            results.push(siphon.clone());
        }

        let prev_sum: Expression = siphon.iter().map(|&p| place_selectors[p]).sum();
        #[allow(clippy::cast_precision_loss)]
        constraints.push(constraint!(prev_sum <= siphon.len() as f64 - 1.0));
    }

    results.into_boxed_slice()
}

/// Finds all minimal traps using ILP enumeration.
#[must_use]
#[expect(unused)]
pub fn minimal_traps_ilp(net: &DenseNet) -> Box<[HashSet<PlaceIdx>]> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    if net.place_count() == 0 {
        return Box::new([]);
    }

    let mut results: Vec<HashSet<PlaceIdx>> = Vec::new();
    let mut no_good_sets: Vec<HashSet<PlaceIdx>> = Vec::new();

    loop {
        let mut vars = ProblemVariables::new();
        let x: Box<[Variable]> = net
            .place_indices()
            .map(|_| vars.add(variable().binary()))
            .collect();

        let mut constraints = Vec::new();
        let selected_places: Expression = x.iter().copied().sum();
        constraints.push(constraint!(selected_places.clone() >= 1.0));

        // Trap property: x[p] ≤ Σ_{q ∈ t•} x[q]  for all p, t ∈ p•
        for p in net.place_indices() {
            for &t in &net.postset_p[p] {
                let sum_postset: Expression = net
                    .postset_t[t]
                    .iter()
                    .map(|&q| x[q])
                    .sum();
                constraints.push(constraint!(x[p] <= sum_postset));
            }
        }

        for prev in &no_good_sets {
            let prev_sum: Expression = prev.iter().map(|&p| x[p]).sum();
            #[allow(clippy::cast_precision_loss)]
            constraints.push(constraint!(prev_sum <= prev.len() as f64 - 1.0));
        }

        let Ok(solution) = vars
            .minimise(selected_places)
            .using(good_lp::microlp)
            .with_all(constraints)
            .solve() else { break };

        let trap: HashSet<PlaceIdx> = net
            .place_indices()
            .filter(|&p| solution.value(x[p]) > 0.5)
            .collect();

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

/// Checks the Commoner/Hack Criterion (CHC): every proper siphon contains
/// a trap that is marked under the given marking.
///
/// For free-choice nets, this is a necessary and sufficient condition for
/// liveness — [Murata, Theorem 12](crate::literature#theorem-12--commonerhack-criterion).
/// For asymmetric-choice nets it is sufficient but not necessary
/// — [Murata, Theorem 15](crate::literature#theorem-15--liveness-of-asymmetric-choice-nets).
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
/// # Examples
///
/// ```
/// use petrivet::api::builder::NetBuilder;
/// use petrivet::api::marking::IdxMarking;
/// use petrivet::analysis::structural::{minimal_siphons, commoner_hack_criterion_inner};
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap();
///
/// let m0 = IdxMarking::from([1u32, 0]);
/// // With a token, the siphon {p0, p1} contains a marked trap → live
/// assert!(commoner_hack_criterion_inner(&net, &m0).is_satisfied());
///
/// // Without tokens, the trap is unmarked → not live
/// let m_empty = IdxMarking::from([0u32, 0]);
/// assert!(!commoner_hack_criterion_inner(&net, &m_empty).is_satisfied());
/// ```
///
/// References:
/// - [Murata 1989, Theorem 12](crate::literature#theorem-12--commonerhack-criterion):
///   "A free-choice net (N, M₀) is live iff every siphon in N contains a marked trap."
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc) (Commoner/Hack Criterion)
/// - [Primer, Algorithm 6.19](crate::literature#algorithm-619--maximal-siphontrap-in-a-subset) (maximal siphon/trap in a subset)
pub fn commoner_hack_criterion(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
) -> impl Iterator<Item = (HashSet<PlaceIdx>, HashSet<PlaceIdx>, bool)> {
    minimal_siphons(net)
        .into_iter()
        .map(|siphon| {
            let trap = maximal_trap_in(net, siphon.clone());
            let trap_is_marked = !trap.is_empty() && trap.iter().any(|&p| marking[p] > 0);
            (siphon, trap, trap_is_marked)
        })
}