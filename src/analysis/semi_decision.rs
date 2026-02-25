//! Semi-decision procedures using LP/ILP formulations.
//!
//! These encode the marking equation and related checks as linear programs
//! solved via `good_lp` with the `microlp` backend.
//!
//! The marking equation m₀ + N^T · x = m' is a necessary condition for
//! reachability: if no non-negative integer solution x exists, then m' is
//! definitely not reachable from m₀. If a solution exists, reachability is
//! possible but not guaranteed (the equation ignores firing order).

use crate::marking::Marking;
use crate::net::{Net, Place};
use good_lp::{
    constraint, variable, Expression, ProblemVariables, Solution,
    SolverModel, Variable,
};

/// Result of a marking-equation feasibility check.
#[derive(Debug, Clone)]
pub enum MarkingEquationResult {
    /// No solution exists — the target marking is definitely unreachable.
    Infeasible,
    /// A non-negative rational solution exists. The firing count vector
    /// is returned. This is necessary but not sufficient for reachability.
    FeasibleRational(Vec<f64>),
    /// A non-negative integer solution exists. Stronger evidence of
    /// reachability, but still not a proof (firing order may not exist).
    FeasibleInteger(Vec<f64>),
}

impl MarkingEquationResult {
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

/// Checks the marking equation m₀ + N^T · x = m' for feasibility.
///
/// This is a necessary condition for reachability: if infeasible, then m'
/// is definitely not reachable from m₀. Uses LP relaxation first (rational
/// solution), which is faster than ILP.
///
/// The incidence matrix N is |T|×|P|, so N^T is |P|×|T|. The equation
/// for each place p is:  `m₀[p] + Σ_t N[t][p] · x[t] = m'[p]`
#[must_use]
pub fn check_marking_equation(
    net: &Net,
    initial: &Marking,
    target: &Marking,
) -> MarkingEquationResult {
    let incidence = net.incidence_matrix();

    let mut variables = ProblemVariables::new();
    let mut constraints = Vec::new();
    let x: Vec<Variable> = (0..net.n_transitions())
        .map(|_| variables.add(variable().min(0.0)))
        .collect();

    // Objective: minimize total firings (any feasible solution suffices,
    // but minimizing keeps numbers small).
    let objective: Expression = x.iter().copied().sum();

    // Constraint: m₀[p] + Σ_t N[t][p] * x[t] = m'[p]  for each place p
    for p in 0..net.n_places() {
        let lhs: Expression = (0..net.n_transitions())
            .map(|t| incidence.get(t, p) as f64 * x[t])
            .sum();
        let rhs = f64::from(target[Place { idx: p }]) - f64::from(initial[Place { idx: p }]);
        constraints.push(constraint!(lhs == rhs));
    }

    let result = variables
        .minimise(objective)
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve();
    match result {
        Ok(solution) => {
            let firing_counts: Vec<f64> = x.iter().map(|&v| solution.value(v)).collect();
            MarkingEquationResult::FeasibleRational(firing_counts)
        }
        Err(_) => MarkingEquationResult::Infeasible,
    }
}

/// Checks structural boundedness: is the net bounded for every possible
/// initial marking?
///
/// Uses the primal LP from Murata (Theorem 29) / Petri Net Primer (Proposition 4.12):
/// find a positive S-subvariant y >> 0 with C · y ≤ 0.
///
/// Variables: `y[p] ≥ 1` for each place (`y ≥ 1` encodes `y >> 0`)
/// Constraints: `Σ_p C[t,p] · y[p] ≤ 0` for each transition `t`
/// Feasible → structurally bounded; Infeasible → not structurally bounded
#[must_use]
pub fn is_structurally_bounded(net: &Net) -> bool {
    let mut variables = ProblemVariables::new();
    let y: Vec<Variable> = net.places()
        .map(|_| variables.add(variable().min(1.0)))
        .collect();

    let incidence = net.incidence_matrix();
    let constraints = net.transitions().map(|t| {
        let weighted_change: Expression = net.places()
            .map(|p| f64::from(incidence.get(t.idx, p.idx)) * y[p.idx])
            .sum();
        constraint!(weighted_change <= 0.0)
    });

    variables
        .minimise(Expression::from(0)) // no real objective, just a feasibility check
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .is_ok()
}

/// Checks whether a single place is structurally bounded (bounded under
/// every possible initial marking).
///
/// Uses the primal LP: find `y ≥ 0` with `y[place] ≥ 1` and `C · y ≤ 0`.
/// This is weaker than whole-net structural boundedness (which requires
/// y >> 0), since only the target place needs a strictly positive weight.
///
/// Feasible → place is structurally bounded; Infeasible → structurally
/// unbounded (there exists an initial marking under which it is unbounded).
#[must_use]
pub fn is_place_structurally_bounded(net: &Net, place: Place) -> bool {
    let mut variables = ProblemVariables::new();
    let y: Vec<Variable> = net.places()
        .map(|p| {
            if p == place {
                variables.add(variable().min(1.0))
            } else {
                variables.add(variable().min(0.0))
            }
        })
        .collect();

    let incidence = net.incidence_matrix();
    let constraints = net.transitions().map(|t| {
        let weighted_change: Expression = net.places()
            .map(|p| f64::from(incidence.get(t.idx, p.idx)) * y[p.idx])
            .sum();
        constraint!(weighted_change <= 0.0)
    });

    variables
        .minimise(Expression::from(0)) // no real objective, just a feasibility check
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
        let result = check_marking_equation(&net, &m0, &m1);
        assert!(result.is_feasible());
    }

    #[test]
    fn unreachable_marking_infeasible() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        // [2, 0] requires creating a token — not possible in a conservative net
        let m1 = Marking::from([2u32, 0]);
        let result = check_marking_equation(&net, &m0, &m1);
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
        let result = check_marking_equation(&net, &m0, &m0);
        assert!(result.is_feasible());
    }

    #[test]
    fn marking_equation_round_trip() {
        let net = two_place_cycle();
        let m0 = Marking::from([1u32, 0]);
        let result = check_marking_equation(&net, &m0, &m0);
        assert!(result.is_feasible());
        if let MarkingEquationResult::FeasibleRational(x) = &result {
            assert!(x.iter().all(|&v| v >= -1e-9));
        }
    }
}
