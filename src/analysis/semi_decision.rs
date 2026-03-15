//! Semi-decision procedures using LP/ILP formulations.
//!
//! These encode the marking equation and related checks as linear programs
//! solved via `good_lp` with the `microlp` backend.
//!
//! The marking equation m₀ + N · x = m' (N is the |P|×|T| incidence matrix,
//! x is a firing count vector) is a necessary condition for reachability:
//! if no non-negative integer solution x exists, then m' is definitely not
//! reachable from m₀. If a solution exists, reachability is possible but
//! not guaranteed (the equation ignores firing order).
//!
//! These checks are extremely fast compared to full state space exploration
//! and serve as early-out filters. In hardware verification and protocol
//! analysis, they can quickly rule out impossible states without building
//! the (potentially enormous) reachability graph.
//!
//! # Example
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::marking::Marking;
//! use petrivet::analysis::semi_decision::find_marking_equation_rational_solution;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! let net = b.build().unwrap();
//!
//! let m0 = Marking::from([1u32, 0]);
//!
//! // Can we reach (0, 1)? LP says: feasible (and it truly is)
//! assert!(find_marking_equation_rational_solution(&net, &m0, &Marking::from([0u32, 1])).is_some());
//!
//! // Can we reach (2, 0)? LP says: infeasible (conservation law violated)
//! assert!(find_marking_equation_rational_solution(&net, &m0, &Marking::from([2u32, 0])).is_none());
//! ```

use crate::marking::Marking;
use crate::net::{Net, Place};
use good_lp::{
    constraint, variable, Expression, ProblemVariables, Solution,
    SolverModel, Variable,
};

/// Checks the marking equation M = M₀ + N · x for a non-negative rational solution x,
/// where N: |P|×|T| is the incidence matrix of the net.
/// This is a necessary condition for M to be reachable from M₀, but not sufficient.
/// The feasibility of this LP is logically equivalent to `M ~ M₀`
/// (agreement on all place invariants).
///
/// Note that this LP tries to find a _rational_ solution,
/// which is faster to solve than the integer version but may yield spurious solutions
/// that are not actually realizable (e.g. firing a transition 0.5 times).
/// For a stronger check, see [`find_marking_equation_integer_solution`].
///
/// References:
/// - [Murata 1989, §IV-B](crate::literature#iv-b--incidence-matrix-and-state-equation):
///   "a nonnegative integer solution x must exist" is a necessary reachability condition.
/// - [Primer, Proposition 4.3](crate::literature#proposition-43--state-equation)
///   (state equation as necessary condition)
#[must_use]
pub fn find_marking_equation_rational_solution(
    net: &Net,
    initial: &Marking,
    target: &Marking,
) -> Option<Box<[f64]>> {
    let incidence = net.incidence_matrix();

    let mut variables = ProblemVariables::new();
    let rational_firing_counts: Vec<Variable> = net
        .transitions()
        .map(|_| variables.add(variable().min(0.0)))
        .collect();

    // Minimize total firing count (arbitrary objective; we only care about feasibility).
    let objective: Expression = rational_firing_counts.iter().copied().sum();

    // Constraint: m₀[p] + Σ_t N[p][t] · x[t] = m'[p]  for each place p
    let constraints = net
        .places()
        .map(|p| {
            let lhs: Expression = net
                .transitions()
                .map(|t| incidence.get(p, t) as f64 * rational_firing_counts[t.idx])
                .sum();
            let rhs = f64::from(target[p]) - f64::from(initial[p]);
            constraint!(lhs == rhs)
        });

    variables
        .minimise(objective)
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .map_or(None, |solution| {
            let rational_solution: Box<[f64]> = rational_firing_counts
                .into_iter()
                .map(|v| solution.value(v))
                .collect();
            Some(rational_solution)
        })
}

/// Checks the marking equation using ILP (integer linear programming).
///
/// This is a stronger necessary condition than [`find_marking_equation_rational_solution`]:
/// it searches for a non-negative **integer** firing count vector x such that
/// `m₀ + N · x = m'`. If no integer solution exists, the target marking is
/// definitely unreachable (even if a rational LP solution existed).
///
/// More expensive than the LP variant due to branch-and-bound, but can
/// rule out spurious LP solutions that have no integer counterpart.
///
/// # Panics
/// - If the ILP solver returns an error other than infeasibility (e.g. unboundedness, numerical issues).
///
/// References:
/// - [Murata 1989, §IV-B](crate::literature#iv-b--incidence-matrix-and-state-equation): the firing count vector must be a non-negative integer
#[must_use]
pub fn find_marking_equation_integer_solution(
    net: &Net,
    initial: &Marking,
    target: &Marking,
) -> Option<Box<[u32]>> {
    let mut variables = ProblemVariables::new();
    let integer_firing_counts: Vec<Variable> = net
        .transitions()
        .map(|_| variables.add(variable().integer().min(0)))
        .collect();

    let objective: Expression = integer_firing_counts.iter().copied().sum();

    let incidence = net.incidence_matrix();
    let constraints = net
        .places()
        .map(|p| {
            let lhs: Expression = net
                .transitions()
                .map(|t| incidence.get(p, t) as f64 * integer_firing_counts[t.idx])
                .sum();
            let rhs = f64::from(target[p]) - f64::from(initial[p]);
            constraint!(lhs == rhs)
        });

    variables
        .minimise(objective)
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .map_or(None, |solution| {
            let integer_solution: Box<[u32]> = integer_firing_counts.iter()
                .map(|&v| solution.value(v).round() as u32)
                .collect();
            Some(integer_solution)
        })
}

/// Checks a *covering* variant of the marking equation: is there a
/// reachable marking m' such that m'\[p\] >= threshold\[p\] for each place?
///
/// Unlike [`find_marking_equation_rational_solution`], this uses inequality constraints
/// (`>=`) rather than equality, so it asks whether *any* marking at least
/// as large as `threshold` is reachable.
///
/// This is useful for checking whether a transition can ever be enabled:
/// set `threshold[p] = 1` for each input place and `0` elsewhere.
///
/// Still a necessary condition only (LP relaxation of the marking equation).
#[must_use]
pub fn find_covering_equation_rational_solution(
    net: &Net,
    initial: &Marking,
    threshold: &Marking,
) -> Option<Box<[f64]>> {
    let mut variables = ProblemVariables::new();
    let parikh_vector: Vec<Variable> = net
        .transitions()
        .map(|_| variables.add(variable().min(0.0)))
        .collect();

    let incidence = net.incidence_matrix();
    let constraints = net
        .places()
        .map(|p| {
            let change: Expression = net
                .transitions()
                .map(|t| f64::from(incidence.get(p, t)) * parikh_vector[t.idx])
                .sum();
            let m0_p = f64::from(initial[p]);
            let thresh = f64::from(threshold[p]);
            // m₀[p] + Σ_t N[p][t] · x[t] >= threshold[p]
            constraint!(change >= thresh - m0_p)
        });

    let objective: Expression = parikh_vector.iter().copied().sum();
    variables
        .minimise(objective)
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .map_or(None, |solution| {
            let firing_counts: Box<[f64]> = parikh_vector
                .iter()
                .map(|&v| solution.value(v))
                .collect();
            Some(firing_counts)
        })
}

/// Finds a non-negative integer solution to the covering equation:
/// does there exist x ∈ ℕ^{|T|} such that `m₀ + N · x >= threshold`?
///
/// This is a stronger necessary condition than [`find_covering_equation_rational_solution`].
/// If no integer solution exists, the target is definitely not coverable.
///
/// References:
/// - [Primer, Proposition 4.3](crate::literature#proposition-43--state-equation) (state equation is a necessary condition)
/// - [Murata 1989, §IV-B](crate::literature#iv-b--incidence-matrix-and-state-equation) (firing count vector must be integer)
#[must_use]
pub fn find_covering_equation_integer_solution(
    net: &Net,
    initial: &Marking,
    threshold: &Marking,
) -> Option<Box<[u32]>> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, SolverModel, Variable};

    let mut variables = ProblemVariables::new();
    let parikh_vector: Vec<Variable> = net
        .transitions()
        .map(|_| variables.add(variable().integer().min(0)))
        .collect();

    let incidence = net.incidence_matrix();
    let constraints = net
        .places()
        .map(|p| {
            let change: Expression = net
                .transitions()
                .map(|t| incidence.get(p, t) as f64 * parikh_vector[t.idx])
                .sum();
            let m0_p = f64::from(initial[p]);
            let thresh = f64::from(threshold[p]);
            // m₀[p] + Σ_t N[p][t] · x[t] >= threshold[p]
            constraint!(change >= thresh - m0_p)
        });

    let objective: Expression = parikh_vector.iter().copied().sum();
    variables
        .minimise(objective)
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .map_or(None, |solution| {
            let firing_counts: Box<[u32]> = parikh_vector
                .iter()
                .map(|&v| solution.value(v).round() as u32)
                .collect();
            Some(firing_counts)
        })
}

/// Checks structural boundedness: is the net bounded for every possible
/// initial marking?
///
/// Finds a positive S-sub-invariant y >> 0 such that yᵀ · N ≤ 0 (non-strict).
/// Equivalently, for each transition t: Σ_p N\[p\]\[t\] · y\[p\] ≤ 0.
///
/// This is weaker than *conservativeness* (which requires yᵀ · N = 0,
/// i.e. S-invariant coverage). A structurally bounded net has the property
/// that the weighted token sum y · M can only decrease or stay the same
/// across firings, guaranteeing boundedness under any initial marking.
///
/// **Property hierarchy** (each implies the next):
/// 1. S-invariant coverage → conservativeness (see [`Invariants::is_covered_by_s_invariants`](super::structural::Invariants::is_covered_by_s_invariants))
/// 2. Structural boundedness (this check) → bounded for every M₀
///
/// References:
/// - [Murata 1989, Table 5](crate::literature#table-5--structural-boundedness): structural boundedness ⟺ ∃y > 0, Ay ≤ 0
/// - [Primer, Proposition 4.12](crate::literature#proposition-412--structural-boundedness-via-lp)
///
/// Checks structural boundedness and returns the weight vector if feasible.
///
/// Finds y > 0 such that yᵀ · N ≤ 0 (each component ≥ 1). If feasible,
/// returns the weight vector y. Given a specific initial marking M₀,
/// per-place upper bounds can be derived: M\[p\] ≤ ⌊(y·M₀) / y\[p\]⌋.
#[must_use]
pub fn find_positive_place_subvariant(net: &Net) -> Option<Box<[f64]>> {
    if net.place_count() == 0 {
        return Some(Box::new([]));
    }

    let mut variables = ProblemVariables::new();
    let place_weights: Vec<Variable> = net
        .places()
        .map(|_| variables.add(variable().min(1.0)))
        .collect();

    let incidence = net.incidence_matrix();
    let constraints = net
        .transitions()
        .map(|t| {
            let token_delta: Expression = net.places()
                .map(|p| f64::from(incidence.get(p, t)) * place_weights[p.idx])
                .sum();
            constraint!(token_delta <= 0.0)
        });

    variables
        .minimise(Expression::from(0))
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .ok()
        .map(|solution| {
            place_weights
                .iter()
                .map(|&v| solution.value(v))
                .collect::<Vec<_>>()
                .into_boxed_slice()
        })
}

#[must_use]
pub fn is_structurally_bounded(net: &Net) -> bool {
    find_positive_place_subvariant(net).is_some()
}

/// Checks whether a single place is structurally bounded (bounded under
/// every possible initial marking).
///
/// Tries to find a semi-positive weighting with `place` in its support
/// (`y[place] ≥ 1`) and `yᵀ · N ≤ 0`, demonstrating that the weighted
/// token count of that place cannot increase no matter what transitions fire,
/// thus guaranteeing its boundedness.
///
/// For a stronger check of the entire net, see [`is_structurally_bounded`].
///
/// Feasible → place is structurally bounded; Infeasible → structurally
/// unbounded (there exists an initial marking under which it is unbounded).
#[must_use]
pub fn find_place_subvariant_covering(net: &Net, place: Place) -> Option<Box<[f64]>> {
    let mut variables = ProblemVariables::new();
    let place_weights: Vec<Variable> = net.places()
        .map(|p| {
            if p == place {
                variables.add(variable().min(1.0))
            } else {
                variables.add(variable().min(0.0))
            }
        })
        .collect();

    // we are looking for a region of the net containing the target place
    // which the firing of any transition cannot increase the weighted token count of that region.
    let incidence = net.incidence_matrix();
    let constraints = net
        .transitions()
        .map(|t| {
            let token_delta: Expression = net
                .places()
                .map(|p| f64::from(incidence.get(p, t)) * place_weights[p.idx])
                .sum();
            constraint!(token_delta <= 0.0)
        });

    variables // objective doesn't matter; we only care about feasibility
        .minimise(Expression::from(0))
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .map_or(None, |solution| {
            let weights: Box<[f64]> = place_weights
                .iter()
                .map(|&v| solution.value(v))
                .collect();
            Some(weights)
        })
}

/// Exact reachability decision for S-nets (every transition has exactly
/// one input and one output place).
///
/// In an S-net, every transition moves exactly one token from its input
/// place to its output place. The total token count is therefore invariant
/// under all firings. More generally, each S-invariant is preserved.
///
/// For S-nets, the marking equation is both necessary and sufficient:
/// `M'` is reachable from `M₀` if and only if every S-invariant is
/// preserved (`y · M' = y · M₀` for all S-invariants `y`). This is
/// equivalent to the LP marking equation being feasible.
///
/// This turns reachability, normally Ackermann-complete for general nets,
/// into a polynomial-time check for S-nets.
///
/// # Panics
///
/// Debug-asserts that the net is actually an S-net.
///
/// References:
/// - [Murata 1989, Theorem 21](crate::literature#theorem-21--reachability-in-s-nets): for S-nets, the marking equation is
///   necessary and sufficient for reachability.
/// - Lautenbach & Thiagarajan 1979 (original result)
#[must_use]
pub fn is_reachable_s_net(net: &Net, initial: &Marking, target: &Marking) -> bool {
    debug_assert!(net.is_s_net(), "is_reachable_s_net called on non-S-net");
    find_marking_equation_rational_solution(net, initial, target).is_some()
}

/// Exact reachability decision for T-nets (every place has exactly one
/// input and one output transition).
///
/// In a T-net, every non-negative integer solution to the marking equation
/// `M' = M₀ + N · x` corresponds to a realizable firing sequence. This
/// means the ILP marking equation is both necessary and sufficient for
/// reachability.
///
/// This turns reachability into an ILP feasibility check, which is
/// NP-complete in general but efficient for the small instances typical
/// of Petri net analysis.
///
/// # Panics
///
/// Debug-asserts that the net is actually a T-net.
///
/// References:
/// - [Murata 1989, Theorem 22](crate::literature#theorem-22--reachability-in-t-nets): for T-nets, a non-negative integer solution
///   to the marking equation is necessary and sufficient for reachability.
#[must_use]
pub fn is_reachable_t_net(net: &Net, initial: &Marking, target: &Marking) -> bool {
    debug_assert!(net.is_t_net(), "is_reachable_t_net called on non-T-net");
    find_marking_equation_integer_solution(net, initial, target).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::builder::NetBuilder;

    fn two_place_cycle() -> Net {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        b.build().unwrap()    }

    #[test]
    fn reachable_marking_feasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let m1 = Marking::from([0u32, 1]);
        let result = find_marking_equation_rational_solution(&net, &m0, &m1);
        assert!(result.is_some());
    }

    #[test]
    fn unreachable_marking_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        // [2, 0] requires creating a token - not possible in a conservative net
        let m1 = Marking::from([2u32, 0]);
        let result = find_marking_equation_rational_solution(&net, &m0, &m1);
        assert!(result.is_none());
    }

    #[test]
    fn cycle_structurally_bounded() {
        let net = two_place_cycle();
        assert!(is_structurally_bounded(&net), "cycle should be structurally bounded");
    }

    #[test]
    fn producer_structurally_bounded() {
        // t0 consumes p1, produces p0; t1 consumes p0, produces p1.
        // This is a cycle, so it IS structurally bounded.
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0));
        b.add_arc((p0, t1));
        b.add_arc((t1, p1));
        b.add_arc((p1, t0));
        let net = b.build().unwrap();
        assert!(is_structurally_bounded(&net), "producer net should be proven bounded");
    }

    #[test]
    fn source_transition_not_structurally_bounded() {
        // t0 produces into p0 with no input - a true source transition.
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let t0 = b.add_transition();
        b.add_arc((t0, p0));
        let net = b.build().unwrap();
        assert!(!is_structurally_bounded(&net));
        assert!(find_place_subvariant_covering(&net, p0).is_none());
    }

    #[test]
    fn nonuniform_weights_structurally_bounded() {
        // t0: p0 → {p1, p2}   (1 input, 2 outputs)  C[t0] = [-1, +1, +1]
        // t1: {p1, p2} → p0   (2 inputs, 1 output)   C[t1] = [+1, -1, -1]
        //
        // Uniform y=(1,1,1) fails: C[t0]·y = 1 > 0.
        // But y=(2,1,1) works: C[t0]·y = 0, C[t1]·y = 0.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((t0, p2));
        b.add_arc((p1, t1));
        b.add_arc((p2, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap();
        assert!(is_structurally_bounded(&net));
    }

    #[test]
    fn marking_equation_identity() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = find_marking_equation_rational_solution(&net, &m0, &m0);
        assert!(result.is_some());
    }

    #[test]
    fn marking_equation_round_trip() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = find_marking_equation_rational_solution(&net, &m0, &m0);
        assert!(result.is_some());
        if let Some(x) = &result {
            assert!(x.iter().all(|&v| v >= -1e-9));
        }
    }

    #[test]
    fn ilp_reachable_marking_feasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let target = Marking::from([0u32, 1]);
        let result = find_marking_equation_integer_solution(&net, &m0, &target);
        assert!(result.is_some());
    }

    #[test]
    fn ilp_unreachable_marking_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let target = Marking::from([2u32, 0]);
        let result = find_marking_equation_integer_solution(&net, &m0, &target);
        assert!(result.is_none());
    }

    #[test]
    fn ilp_identity() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = find_marking_equation_integer_solution(&net, &m0, &m0);
        assert!(result.is_some());
    }

    #[test]
    fn covering_equation_feasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let threshold = Marking::from([0u32, 1]);
        let result = find_covering_equation_rational_solution(&net, &m0, &threshold);
        assert!(result.is_some());
    }

    #[test]
    fn covering_equation_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let threshold = Marking::from([2u32, 0]);
        let result = find_covering_equation_rational_solution(&net, &m0, &threshold);
        assert!(result.is_none());
    }

    #[test]
    fn s_net_reachability_positive() {
        // Two-place cycle is an S-net (circuit, actually)
        let net = two_place_cycle();
        assert!(net.is_s_net());
        let m0 = Marking::from([1u32, 0]);
        // (0,1) is reachable: token moves from p0 to p1
        assert!(is_reachable_s_net(&net, &m0, &Marking::from([0u32, 1])));
        // Identity is always reachable
        assert!(is_reachable_s_net(&net, &m0, &m0));
    }

    #[test]
    fn s_net_reachability_negative() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        // Token sum mismatch: 1 ≠ 2
        assert!(!is_reachable_s_net(&net, &m0, &Marking::from([2u32, 0])));
        // Token sum mismatch: 1 ≠ 0
        assert!(!is_reachable_s_net(&net, &m0, &Marking::from([0u32, 0])));
    }

    #[test]
    fn s_net_reachability_chain() {
        // Non-cyclic S-net: p0 → t0 → p1 → t1 → p2 (chain, not cycle)
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p2));
        let net = b.build().unwrap();
        assert!(net.is_s_net());

        let m0 = Marking::from([1u32, 0, 0]);
        // (0, 0, 1) reachable: token flows down the chain
        assert!(is_reachable_s_net(&net, &m0, &Marking::from([0u32, 0, 1])));
        // (0, 1, 0) reachable: token stops at p1
        assert!(is_reachable_s_net(&net, &m0, &Marking::from([0u32, 1, 0])));
        // (1, 1, 0) NOT reachable: token sum 1 ≠ 2
        assert!(!is_reachable_s_net(&net, &m0, &Marking::from([1u32, 1, 0])));
    }

    fn t_net_sync() -> Net {
        // T-net: two places feed into one transition, which feeds back
        //   t0: {p0, p1} → p2
        //   t1: p2 → {p0, p1}
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((p1, t0)); b.add_arc((t0, p2));
        b.add_arc((p2, t1)); b.add_arc((t1, p0)); b.add_arc((t1, p1));
        b.build().unwrap()
    }

    #[test]
    fn t_net_reachability_positive() {
        let net = t_net_sync();
        assert!(net.is_t_net());
        let m0 = Marking::from([1u32, 1, 0]);
        // Fire t0: (1,1,0) → (0,0,1)
        assert!(is_reachable_t_net(&net, &m0, &Marking::from([0u32, 0, 1])));
        // Fire t0 then t1: back to (1,1,0)
        assert!(is_reachable_t_net(&net, &m0, &m0));
    }

    #[test]
    fn t_net_reachability_negative() {
        let net = t_net_sync();
        let m0 = Marking::from([1u32, 1, 0]);
        // (1,0,0): violates marking equation (no integer solution)
        assert!(!is_reachable_t_net(&net, &m0, &Marking::from([1u32, 0, 0])));
        // (2,2,0): would need negative firings of t0
        assert!(!is_reachable_t_net(&net, &m0, &Marking::from([2u32, 2, 0])));
    }
}
