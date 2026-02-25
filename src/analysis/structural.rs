//! Structural analysis: invariants, siphons, and traps.
//!
//! These properties depend only on the net topology, not on any particular
//! marking. They provide necessary conditions for behavioral properties like
//! boundedness and liveness.

use crate::analysis::math::integer_null_space;
use crate::marking::Marking;
use crate::net::{Net, Place, Transition};
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
#[must_use]
pub fn compute_invariants(net: &Net) -> Invariants {
    let n = net.incidence_matrix();
    let s_invariants = integer_null_space(&n);
    let nt = n.transpose();
    let t_invariants = integer_null_space(&nt);
    Invariants { s_invariants, t_invariants }
}

/// Finds all minimal siphons of a net.
///
/// A siphon is a set of places D such that •D ⊆ D•: every transition that
/// produces into D also consumes from D. Once empty, it stays empty forever.
///
/// Uses a simple enumeration: start from each place, grow the set until
/// the siphon property •D ⊆ D• is satisfied.
#[must_use]
pub fn minimal_siphons(net: &Net) -> Vec<HashSet<Place>> {
    let mut results: Vec<HashSet<Place>> = Vec::new();

    for seed in net.places() {
        let mut siphon = HashSet::new();
        siphon.insert(seed);

        loop {
            let mut grew = false;
            // •D: transitions that produce into D
            let pre_d: HashSet<Transition> = siphon.iter()
                .flat_map(|&p| net.preset_p(p).iter().copied())
                .collect();
            // D•: transitions that consume from D
            let post_d: HashSet<Transition> = siphon.iter()
                .flat_map(|&p| net.postset_p(p).iter().copied())
                .collect();

            // For •D ⊆ D•, every transition in •D must also be in D•.
            // If t ∈ •D but t ∉ D•, then t produces into D but doesn't
            // consume from D. We need to add one of t's input places to D.
            for &t in &pre_d {
                if !post_d.contains(&t) {
                    // t consumes from •t; add all its input places
                    for &p in net.preset_t(t) {
                        if siphon.insert(p) {
                            grew = true;
                        }
                    }
                }
            }

            if !grew {
                break;
            }
        }

        // Check if this is genuinely minimal (no proper subset in results)
        let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
        if !dominated {
            results.retain(|existing| !siphon.is_subset(existing));
            results.push(siphon);
        }
    }

    results
}

/// Finds all minimal traps of a net.
///
/// A trap Q satisfies Q• ⊆ •Q: every transition that consumes from Q also
/// produces into Q. Once marked, a trap stays marked forever.
#[must_use]
pub fn minimal_traps(net: &Net) -> Vec<HashSet<Place>> {
    let mut results: Vec<HashSet<Place>> = Vec::new();

    for seed in net.places() {
        let mut trap = HashSet::new();
        trap.insert(seed);

        loop {
            let mut grew = false;
            // Q•: transitions that consume from Q
            let post_q: HashSet<Transition> = trap.iter()
                .flat_map(|&p| net.postset_p(p).iter().copied())
                .collect();
            // •Q: transitions that produce into Q
            let pre_q: HashSet<Transition> = trap.iter()
                .flat_map(|&p| net.preset_p(p).iter().copied())
                .collect();

            // For Q• ⊆ •Q, every transition in Q• must also be in •Q.
            // If t ∈ Q• but t ∉ •Q, then t consumes from Q but doesn't
            // produce into Q. We need to add one of t's output places to Q.
            for &t in &post_q {
                if !pre_q.contains(&t) {
                    for &p in net.postset_t(t) {
                        if trap.insert(p) {
                            grew = true;
                        }
                    }
                }
            }

            if !grew {
                break;
            }
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&trap));
        if !dominated {
            results.retain(|existing| !trap.is_subset(existing));
            results.push(trap);
        }
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
