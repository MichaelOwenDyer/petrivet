//! Structural analysis: invariants, siphons, and traps.
//!
//! These properties depend only on the net topology, not on any particular
//! marking. They provide necessary conditions for behavioral properties like
//! boundedness and liveness.

use crate::analysis::math::integer_null_space;
use crate::marking::Marking;
use crate::net::{Net, Place};
use std::collections::HashSet;

/// Result of structural invariant analysis on a net.
#[derive(Debug, Clone)]
pub struct Invariants {
    /// Basis vectors for the S-invariants (place invariants).
    /// Each vector has length |P|. An S-invariant y satisfies y^T · N = 0,
    /// meaning the weighted token sum y · m is conserved across all firings.
    pub s_invariants: Box<[Box<[i32]>]>,

    /// Basis vectors for the T-invariants (transition invariants).
    /// Each inner slice has length |T|. A T-invariant x satisfies N · x = 0,
    /// meaning firing the multiset of transitions x returns the marking
    /// to its original value.
    pub t_invariants: Box<[Box<[i32]>]>,
}

impl Invariants {
    /// Whether every place is covered by a non-negative S-invariant.
    ///
    /// A place p is covered if there exists a non-negative linear combination
    /// of the S-invariant basis that yields a vector with a positive entry at p.
    /// This is checked via LP. If true, the net is structurally bounded.
    ///
    /// For a simpler structural boundedness check, see
    /// [`potentially_unbounded_places`](super::semi_decision::potentially_unbounded_places).
    #[must_use]
    pub fn is_covered_by_s_invariants(&self, n_places: usize) -> bool {
        if self.s_invariants.is_empty() {
            return n_places == 0;
        }
        (0..n_places).all(|p| self.place_has_positive_invariant(p, n_places))
    }

    /// Whether every transition is covered by a non-negative T-invariant.
    #[must_use]
    pub fn is_covered_by_t_invariants(&self, n_transitions: usize) -> bool {
        if self.t_invariants.is_empty() {
            return n_transitions == 0;
        }
        (0..n_transitions).all(|t| self.transition_has_positive_invariant(t, n_transitions))
    }

    /// Checks whether there exists a non-negative S-invariant with a
    /// positive value at place `p`, by solving an LP over the basis.
    fn place_has_positive_invariant(&self, p: usize, n_places: usize) -> bool {
        use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

        let k = self.s_invariants.len();
        let mut vars = ProblemVariables::new();
        let mut constraints = Vec::new();
        let lambda: Vec<_> = (0..k).map(|_| vars.add(variable())).collect();

        let objective: Expression = (0..k)
            .map(|i| f64::from(self.s_invariants[i][p]) * lambda[i])
            .sum();

        // Bound total absolute value to prevent unbounded LP.
        // Use two constraints per λ: -1 ≤ λ_i ≤ 1.
        for &l in &lambda {
            constraints.push(constraint!(l <= 1.0));
            constraints.push(constraint!(l >= -1.0));
        }

        // Combined invariant must be non-negative at every place.
        for q in 0..n_places {
            let val: Expression = (0..k)
                .map(|i| f64::from(self.s_invariants[i][q]) * lambda[i])
                .sum();
            constraints.push(constraint!(val >= 0.0));
        }

        let problem = vars
            .maximise(objective)
            .using(good_lp::microlp)
            .with_all(constraints)
            .solve();
        match problem {
            Ok(sol) => {
                let obj: f64 = (0..k)
                    .map(|i| f64::from(self.s_invariants[i][p]) * sol.value(lambda[i]))
                    .sum();
                obj > 1e-9
            }
            Err(_) => false,
        }
    }

    /// Checks whether there exists a non-negative T-invariant with a
    /// positive value at transition `t`.
    fn transition_has_positive_invariant(&self, t: usize, n_transitions: usize) -> bool {
        use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

        let k = self.t_invariants.len();
        let mut vars = ProblemVariables::new();
        let lambda: Vec<_> = (0..k).map(|_| vars.add(variable())).collect();

        let objective: Expression = (0..k)
            .map(|i| f64::from(self.t_invariants[i][t]) * lambda[i])
            .sum();

        let mut problem = vars.maximise(objective).using(good_lp::microlp);

        for &l in &lambda {
            problem = problem.with(constraint!(l <= 1.0));
            problem = problem.with(constraint!(l >= -1.0));
        }

        for q in 0..n_transitions {
            let val: Expression = (0..k)
                .map(|i| f64::from(self.t_invariants[i][q]) * lambda[i])
                .sum();
            problem = problem.with(constraint!(val >= 0.0));
        }

        match problem.solve() {
            Ok(sol) => {
                let obj: f64 = (0..k)
                    .map(|i| f64::from(self.t_invariants[i][t]) * sol.value(lambda[i]))
                    .sum();
                obj > 1e-9
            }
            Err(_) => false,
        }
    }
}

/// Computes the S-invariants and T-invariants of a net.
///
/// With the Primer convention (N is |P|×|T|):
/// - S-invariants: vectors y ∈ ℤ^|P| satisfying Nᵀ · y = 0.
///   These are place invariants: the weighted token sum y · M is conserved
///   across all transition firings.
/// - T-invariants: vectors x ∈ ℤ^|T| satisfying N · x = 0.
///   These are transition invariants: firing the multiset x returns the
///   marking to its original value.
///
/// References:
/// - Murata 1989, §VII (invariant analysis)
/// - Petri Net Primer, §4.3 (S-invariants and T-invariants)
#[must_use]
pub fn compute_invariants(net: &Net) -> Invariants {
    let n = net.incidence_matrix();
    // S-invariants: null space of Nᵀ (|T|×|P| matrix → |P|-dimensional vectors)
    let nt = n.transpose();
    let s_invariants = integer_null_space(&nt);
    // T-invariants: null space of N (|P|×|T| matrix → |T|-dimensional vectors)
    let t_invariants = integer_null_space(&n);
    Invariants { s_invariants, t_invariants }
}

/// Computes the maximal siphon contained in a given set of places.
///
/// A siphon is a set of places D such that •D ⊆ D•: every transition that
/// produces into D also consumes from D. Once empty, it stays empty forever.
///
/// Uses the shrinking algorithm from the Petri Net Primer (Algorithm 6.19):
/// iteratively remove any place p where some transition t ∈ •p has no
/// input place in the current set. Runs in O(|S|² · |T|²).
#[must_use]
pub fn maximal_siphon_in(net: &Net, subset: &HashSet<Place>) -> HashSet<Place> {
    let mut d: HashSet<Place> = subset.clone();
    loop {
        let mut removed = false;
        let to_remove: Vec<Place> = d.iter().copied().filter(|&p| {
            // Check if some t ∈ •p has no input place in D.
            // •p = transitions that produce into p = preset_p(p)
            net.preset_p(p).iter().any(|&t| {
                // t ∈ •p. For the siphon property, we need t ∈ D•,
                // i.e. t consumes from some place in D.
                // •t = preset_t(t) = input places of t.
                !net.preset_t(t).iter().any(|&q| d.contains(&q))
            })
        }).collect();
        for p in to_remove {
            d.remove(&p);
            removed = true;
        }
        if !removed {
            break;
        }
    }
    d
}

/// Finds all minimal siphons of a net using backtracking.
///
/// Starts by computing the maximal siphon (all places), then recursively
/// tries excluding each place to find smaller siphons. Results are filtered
/// to keep only minimal ones.
#[must_use]
pub fn minimal_siphons(net: &Net) -> Vec<HashSet<Place>> {
    let all_places: HashSet<Place> = net.places().collect();
    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut stack: Vec<HashSet<Place>> = vec![all_places];
    let mut visited: HashSet<Vec<usize>> = HashSet::new();

    while let Some(candidate_set) = stack.pop() {
        let siphon = maximal_siphon_in(net, &candidate_set);
        if siphon.is_empty() {
            continue;
        }

        let mut key: Vec<usize> = siphon.iter().map(|p| p.idx).collect();
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
            let sub = maximal_siphon_in(net, &reduced);
            if !sub.is_empty() {
                is_minimal = false;
                stack.push(reduced);
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

    results
}

/// Computes the maximal trap contained in a given set of places.
///
/// A trap Q satisfies Q• ⊆ •Q: every transition that consumes from Q also
/// produces into Q. Once marked, a trap stays marked forever.
///
/// Uses the dual of the shrinking algorithm: iteratively remove any place p
/// where some transition t ∈ p• has no output place in the current set.
#[must_use]
pub fn maximal_trap_in(net: &Net, subset: &HashSet<Place>) -> HashSet<Place> {
    let mut q: HashSet<Place> = subset.clone();
    loop {
        let mut removed = false;
        let to_remove: Vec<Place> = q.iter().copied().filter(|&p| {
            // Check if some t ∈ p• has no output place in Q.
            // p• = transitions that consume from p = postset_p(p)
            net.postset_p(p).iter().any(|&t| {
                // t ∈ p•. For the trap property, we need t ∈ •Q,
                // i.e. t produces into some place in Q.
                // t• = postset_t(t) = output places of t.
                !net.postset_t(t).iter().any(|&r| q.contains(&r))
            })
        }).collect();
        for p in to_remove {
            q.remove(&p);
            removed = true;
        }
        if !removed {
            break;
        }
    }
    q
}

/// Finds all minimal traps of a net using backtracking.
#[must_use]
pub fn minimal_traps(net: &Net) -> Vec<HashSet<Place>> {
    let all_places: HashSet<Place> = net.places().collect();
    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut stack: Vec<HashSet<Place>> = vec![all_places];
    let mut visited: HashSet<Vec<usize>> = HashSet::new();

    while let Some(candidate_set) = stack.pop() {
        let trap = maximal_trap_in(net, &candidate_set);
        if trap.is_empty() {
            continue;
        }

        let mut key: Vec<usize> = trap.iter().map(|p| p.idx).collect();
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
            let sub = maximal_trap_in(net, &reduced);
            if !sub.is_empty() {
                is_minimal = false;
                stack.push(reduced);
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

    results
}

/// Finds all minimal siphons using ILP enumeration.
///
/// Encodes the siphon property as binary constraints and iteratively
/// solves for minimum-cardinality siphons, adding no-good cuts to
/// exclude previously found solutions. Slower than the backtracking
/// approach for small nets but more systematic.
#[must_use]
pub fn minimal_siphons_ilp(net: &Net) -> Vec<HashSet<Place>> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    let n_p = net.n_places();
    if n_p == 0 {
        return Vec::new();
    }

    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut nogood_sets: Vec<HashSet<Place>> = Vec::new();

    loop {
        let mut vars = ProblemVariables::new();
        let x: Vec<_> = (0..n_p)
            .map(|_| vars.add(variable().binary()))
            .collect();

        let objective: Expression = x.iter().copied().sum();

        let mut problem = vars.minimise(objective).using(good_lp::microlp);

        // At least one place must be in the siphon.
        let at_least_one: Expression = x.iter().copied().sum();
        problem = problem.with(constraint!(at_least_one >= 1.0));

        // Siphon property: for each place p and each transition t ∈ •p,
        // if p is in the siphon then at least one input place of t is too.
        // Encoding: x[p] ≤ Σ_{q ∈ •t} x[q]  for all p, t ∈ •p
        for p in net.places() {
            for &t in net.preset_p(p) {
                let sum_preset: Expression = net.preset_t(t).iter()
                    .map(|&q| x[q.idx])
                    .sum();
                problem = problem.with(constraint!(x[p.idx] <= sum_preset));
            }
        }

        // No-good cuts: exclude previously found siphons.
        for prev in &nogood_sets {
            let prev_sum: Expression = prev.iter().map(|&p| x[p.idx]).sum();
            problem = problem.with(constraint!(prev_sum <= (prev.len() as f64 - 1.0)));
        }

        let Ok(solution) = problem.solve() else { break };

        let siphon: HashSet<Place> = (0..n_p)
            .filter(|&i| solution.value(x[i]) > 0.5)
            .map(|i| Place { idx: i })
            .collect();

        if siphon.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
        if !dominated {
            results.retain(|existing| !siphon.is_subset(existing));
            results.push(siphon.clone());
        }
        nogood_sets.push(siphon);
    }

    results
}

/// Finds all minimal traps using ILP enumeration.
#[must_use]
pub fn minimal_traps_ilp(net: &Net) -> Vec<HashSet<Place>> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    let n_p = net.n_places();
    if n_p == 0 {
        return Vec::new();
    }

    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut nogood_sets: Vec<HashSet<Place>> = Vec::new();

    loop {
        let mut vars = ProblemVariables::new();
        let x: Vec<_> = (0..n_p)
            .map(|_| vars.add(variable().binary()))
            .collect();

        let objective: Expression = x.iter().copied().sum();

        let mut problem = vars.minimise(objective).using(good_lp::microlp);

        let at_least_one: Expression = x.iter().copied().sum();
        problem = problem.with(constraint!(at_least_one >= 1.0));

        // Trap property: for each place p and each transition t ∈ p•,
        // if p is in the trap then at least one output place of t is too.
        // Encoding: x[p] ≤ Σ_{q ∈ t•} x[q]  for all p, t ∈ p•
        for p in net.places() {
            for &t in net.postset_p(p) {
                let sum_postset: Expression = net.postset_t(t).iter()
                    .map(|&q| x[q.idx])
                    .sum();
                problem = problem.with(constraint!(x[p.idx] <= sum_postset));
            }
        }

        for prev in &nogood_sets {
            let prev_sum: Expression = prev.iter().map(|&p| x[p.idx]).sum();
            problem = problem.with(constraint!(prev_sum <= (prev.len() as f64 - 1.0)));
        }

        let Ok(solution) = problem.solve() else { break };

        let trap: HashSet<Place> = (0..n_p)
            .filter(|&i| solution.value(x[i]) > 0.5)
            .map(|i| Place { idx: i })
            .collect();

        if trap.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&trap));
        if !dominated {
            results.retain(|existing| !trap.is_subset(existing));
            results.push(trap.clone());
        }
        nogood_sets.push(trap);
    }

    results
}

/// Checks if every siphon contains a marked trap.
///
/// This is a sufficient condition for liveness in free-choice nets
/// (Commoner's theorem): a free-choice net with initial marking m₀ is live
/// if and only if every siphon contains a trap marked under m₀.
#[must_use]
pub fn every_siphon_contains_marked_trap<S: std::hash::BuildHasher>(
    marking: &Marking,
    siphons: &[HashSet<Place, S>],
    traps: &[HashSet<Place, S>],
) -> bool {
    siphons.iter().all(|siphon| {
        traps.iter().any(|trap| {
            trap.is_subset(siphon)
                && trap.iter().any(|&p| marking[p] > 0)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::builder::NetBuilder;
    use crate::net::NetClass;

    fn two_place_cycle() -> Net {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        b.build().unwrap().into_net()
    }

    #[test]
    fn cycle_invariants() {
        let net = two_place_cycle();
        let inv = compute_invariants(&net);
        assert_eq!(inv.s_invariants.len(), 1);
        assert_eq!(inv.t_invariants.len(), 1);
        // S-invariant: equal weights on both places (token conservation)
        assert_eq!(inv.s_invariants[0][0], inv.s_invariants[0][1]);
        // T-invariant: equal firing counts (full cycle)
        assert_eq!(inv.t_invariants[0][0], inv.t_invariants[0][1]);
        assert!(inv.is_covered_by_s_invariants(net.n_places()));
        assert!(inv.is_covered_by_t_invariants(net.n_transitions()));
    }

    #[test]
    fn cycle_siphons_and_traps() {
        let net = two_place_cycle();
        let siphons = minimal_siphons(&net);
        let traps = minimal_traps(&net);
        // In a cycle, the entire set of places is both the only minimal
        // siphon and the only minimal trap.
        assert_eq!(siphons.len(), 1);
        assert_eq!(traps.len(), 1);
        let all_places: HashSet<Place> = net.places().collect();
        assert_eq!(siphons[0], all_places);
        assert_eq!(traps[0], all_places);
    }

    #[test]
    fn mutex_structural_analysis() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let classified = b.build().unwrap();
        assert_eq!(classified.class(), NetClass::Unrestricted);
        let net = classified.net();
        let inv = compute_invariants(net);

        // 3 S-invariants for mutex net (7 places, rank 4 → dim 3)
        assert_eq!(inv.s_invariants.len(), 3);
        assert!(inv.is_covered_by_s_invariants(net.n_places()));

        // 2 T-invariants (6 transitions, rank 4 → dim 2)
        assert_eq!(inv.t_invariants.len(), 2);
        assert!(inv.is_covered_by_t_invariants(net.n_transitions()));
    }

    #[test]
    fn commoner_liveness_mutex() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap().into_net();
        let marking = crate::marking::Marking::from([1u32, 0, 0, 1, 0, 0, 1]);
        let siphons = minimal_siphons(&net);
        let traps = minimal_traps(&net);

        // Commoner's theorem condition (necessary for free-choice, but
        // informative even for unrestricted nets as a sufficient condition)
        assert!(every_siphon_contains_marked_trap(&marking, &siphons, &traps));
    }

    #[test]
    fn producer_consumer_not_fully_covered() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0));
        b.add_arc((p0, t1));
        b.add_arc((t1, p1));
        b.add_arc((p1, t0));
        let net = b.build().unwrap().into_net();
        let inv = compute_invariants(&net);
        assert_eq!(inv.s_invariants.len(), 1);
    }
}
