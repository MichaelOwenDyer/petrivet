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
//! use petrivet::analysis::semi_decision::is_marking_equation_feasible_rational;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! let net = b.build().unwrap().into_net();
//!
//! let m0 = Marking::from([1u32, 0]);
//!
//! // Can we reach (0, 1)? LP says: feasible (and it truly is)
//! assert!(is_marking_equation_feasible_rational(&net, &m0, &Marking::from([0u32, 1])).is_feasible());
//!
//! // Can we reach (2, 0)? LP says: infeasible (conservation law violated)
//! assert!(is_marking_equation_feasible_rational(&net, &m0, &Marking::from([2u32, 0])).is_infeasible());
//! ```

use crate::marking::Marking;
use crate::net::{Net, Place};
use good_lp::{
    constraint, variable, Expression, ProblemVariables, Solution,
    SolverModel, Variable,
};

/// Result of a marking-equation feasibility check.
#[derive(Debug, Clone)]
pub enum Feasibility<T> {
    /// No solution exists — the target marking is definitely unreachable.
    Infeasible,
    /// A non-negative solution exists. The firing count vector is returned.
    /// This is necessary but not sufficient for reachability.
    Feasible(Vec<T>),
}

impl<T> From<good_lp::ResolutionError> for Feasibility<T> {
    fn from(err: good_lp::ResolutionError) -> Self {
        match err {
            good_lp::ResolutionError::Infeasible => Self::Infeasible,
            _ => panic!("unexpected LP error: {err}"),
        }
    }
}

impl<T> Feasibility<T> {
    /// Whether any feasible solution was found.
    #[must_use]
    pub fn is_feasible(&self) -> bool {
        !matches!(self, Self::Infeasible)
    }

    /// Whether the target is definitely unreachable.
    #[must_use]
    pub fn is_infeasible(&self) -> bool {
        matches!(self, Self::Infeasible)
    }
}

/// Checks the marking equation M = M₀ + N · x for a non-negative rational solution x,
/// where N: |P|×|T| is the incidence matrix of the net.
/// This is a necessary condition for M to be reachable from M₀, but not sufficient.
/// The feasibility of this LP is logically equivalent to `M ~ M₀`
/// (agreement on all place invariants).
///
/// Note that this LP tries to find a _rational_ solution,
/// which is faster to solve than the integer version but may yield spurious solutions
/// that are not actually realizable (e.g. firing a transition 0.5 times).
/// For a stronger check, see [`is_marking_equation_feasible_integer`].
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::marking::Marking;
/// use petrivet::analysis::semi_decision::{is_marking_equation_feasible_rational, Feasibility};
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap().into_net();
///
/// let m0 = Marking::from([1u32, 0]);
/// let result = is_marking_equation_feasible_rational(&net, &m0, &Marking::from([0u32, 1]));
/// assert!(result.is_feasible());
///
/// // Inspect the firing count vector
/// if let Feasibility::Feasible(rational_solution) = result {
///     assert!(rational_solution.iter().all(|&v| v >= -1e-9)); // non-negative
/// }
/// ```
///
/// References:
/// - Murata 1989, §IV-B: "a nonnegative integer solution x must exist"
///   is a necessary reachability condition.
/// - Petri Net Primer, Proposition 4.3 (state equation as necessary condition)
#[must_use]
pub fn is_marking_equation_feasible_rational(
    net: impl AsRef<Net>,
    initial: &Marking,
    target: &Marking,
) -> Feasibility<f64> {
    let net = net.as_ref();
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
        .map_or_else(Feasibility::from, |solution| {
            let rational_solution: Vec<f64> = rational_firing_counts
                .into_iter()
                .map(|v| solution.value(v))
                .collect();
            Feasibility::Feasible(rational_solution)
        })
}

/// Checks the marking equation using ILP (integer linear programming).
///
/// This is a stronger necessary condition than [`is_marking_equation_feasible_rational`]:
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
/// - Murata 1989, §IV-B: the firing count vector must be a non-negative integer
#[must_use]
pub fn is_marking_equation_feasible_integer(
    net: impl AsRef<Net>,
    initial: &Marking,
    target: &Marking,
) -> Feasibility<u32> {
    let net = net.as_ref();

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
        .map_or_else(Feasibility::from, |solution| {
            let integer_solution: Vec<u32> = integer_firing_counts.iter()
                .map(|&v| solution.value(v).round() as u32)
                .collect();
            Feasibility::Feasible(integer_solution)
        })
}

/// Checks a *covering* variant of the marking equation: is there a
/// reachable marking m' such that m'[p] >= threshold[p] for each place?
///
/// Unlike [`is_marking_equation_feasible_rational`], this uses inequality constraints
/// (`>=`) rather than equality, so it asks whether *any* marking at least
/// as large as `threshold` is reachable.
///
/// This is useful for checking whether a transition can ever be enabled:
/// set `threshold[p] = 1` for each input place and `0` elsewhere.
///
/// Still a necessary condition only (LP relaxation of the marking equation).
#[must_use]
pub fn check_covering_equation(
    net: impl AsRef<Net>,
    initial: &Marking,
    threshold: &Marking,
) -> Feasibility<f64> {
    let net = net.as_ref();

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
        .map_or_else(Feasibility::from, |solution| {
            let firing_counts: Vec<f64> = parikh_vector
                .iter()
                .map(|&v| solution.value(v))
                .collect();
            Feasibility::Feasible(firing_counts)
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
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::analysis::semi_decision::is_structurally_bounded;
///
/// // A cycle is structurally bounded
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// assert!(is_structurally_bounded(&b.build().unwrap().into_net()));
///
/// // A source transition (produces without consuming) is NOT bounded
/// let mut b = NetBuilder::new();
/// let p = b.add_place();
/// let t = b.add_transition();
/// b.add_arc((t, p));
/// assert!(!is_structurally_bounded(&b.build().unwrap().into_net()));
/// ```
///
/// References:
/// - Murata 1989, Table 5: structural boundedness ⟺ ∃y > 0, Ay ≤ 0
/// - Petri Net Primer, Proposition 4.12
#[must_use]
pub fn is_structurally_bounded(net: &Net) -> bool {
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

    variables // objective doesn't matter; we only care about feasibility
        .minimise(Expression::from(0))
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .is_ok()
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
pub fn is_place_structurally_bounded(net: &Net, place: Place) -> bool {
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
        .is_ok()
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
        b.build().unwrap().into_net()
    }

    #[test]
    fn reachable_marking_feasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let m1 = Marking::from([0u32, 1]);
        let result = is_marking_equation_feasible_rational(&net, &m0, &m1);
        assert!(result.is_feasible());
    }

    #[test]
    fn unreachable_marking_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        // [2, 0] requires creating a token — not possible in a conservative net
        let m1 = Marking::from([2u32, 0]);
        let result = is_marking_equation_feasible_rational(&net, &m0, &m1);
        assert!(result.is_infeasible());
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
        let net = b.build().unwrap().into_net();
        assert!(is_structurally_bounded(&net), "producer net should be proven bounded");
    }

    #[test]
    fn source_transition_not_structurally_bounded() {
        // t0 produces into p0 with no input — a true source transition.
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let t0 = b.add_transition();
        b.add_arc((t0, p0));
        let net = b.build().unwrap().into_net();
        assert!(!is_structurally_bounded(&net));
        assert!(!is_place_structurally_bounded(&net, p0));
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
        let net = b.build().unwrap().into_net();
        assert!(is_structurally_bounded(&net));
    }

    #[test]
    fn marking_equation_identity() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = is_marking_equation_feasible_rational(&net, &m0, &m0);
        assert!(result.is_feasible());
    }

    #[test]
    fn marking_equation_round_trip() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = is_marking_equation_feasible_rational(&net, &m0, &m0);
        assert!(result.is_feasible());
        if let Feasibility::Feasible(x) = &result {
            assert!(x.iter().all(|&v| v >= -1e-9));
        }
    }

    #[test]
    fn ilp_reachable_marking_feasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let target = Marking::from([0u32, 1]);
        let result = is_marking_equation_feasible_integer(&net, &m0, &target);
        assert!(result.is_feasible());
        assert!(matches!(result, Feasibility::Feasible(_)));
    }

    #[test]
    fn ilp_unreachable_marking_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let target = Marking::from([2u32, 0]);
        let result = is_marking_equation_feasible_integer(&net, &m0, &target);
        assert!(result.is_infeasible());
    }

    #[test]
    fn ilp_identity() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = is_marking_equation_feasible_integer(&net, &m0, &m0);
        assert!(result.is_feasible());
    }

    #[test]
    fn covering_equation_feasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let threshold = Marking::from([0u32, 1]);
        let result = check_covering_equation(&net, &m0, &threshold);
        assert!(result.is_feasible());
    }

    #[test]
    fn covering_equation_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let threshold = Marking::from([2u32, 0]);
        let result = check_covering_equation(&net, &m0, &threshold);
        assert!(result.is_infeasible());
    }
}
