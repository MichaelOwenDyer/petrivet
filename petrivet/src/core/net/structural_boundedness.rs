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
//! use petrivet::system::marking::Marking;
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::PetriNet;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arcs((p0, t0, p1, t1, p0));
//! let net = b.build().unwrap();
//! let sys = PetriNet::new(net, [(p0, 1)]);
//!
//! // Can we reach (0, 1)? The marking equation says: feasible
//! let result = sys.analyze_reachability(&Marking::from([(p1, 1)]));
//! assert!(result.is_reachable());
//!
//! // Can we reach (2, 0)? Conservation law violated, definitely not
//! let result = sys.analyze_reachability(&Marking::from([(p0, 2)]));
//! assert!(!result.is_reachable());
//! ```

use crate::core::net::{DenseNet, PlaceIdx};
use good_lp::{
    Expression, ProblemVariables, Solution, SolverModel, Variable, constraint,
    variable,
};

/// Checks structural boundedness: is the net bounded for every possible
/// initial marking?
///
/// Finds a positive S-sub-invariant `y >> 0` such that `yᵀ · N ≤ 0` (non-strict).
/// Equivalently, for each transition t: `Σ_p N[p][t] · y[p] ≤ 0`.
///
/// This is weaker than *conservativeness* (which requires `yᵀ · N = 0`,
/// i.e. S-invariant coverage). A structurally bounded net has the property
/// that the weighted token sum y · M can only decrease or stay the same
/// across firings, guaranteeing boundedness under any initial marking.
///
/// **Property hierarchy** (each implies the next):
/// 1. S-invariant coverage → conservativeness (see [`Invariants::is_covered_by_s_invariants`](crate::analysis::Invariants::is_covered_by_s_invariants))
/// 2. Structural boundedness (this check) → bounded for every M₀
///
/// References:
/// - [Murata 1989, Table 5](crate::literature#table-5--structural-boundedness): structural boundedness ⟺ ∃y > 0, Ay ≤ 0
/// - [Primer, Proposition 4.12](crate::literature#proposition-412--structural-boundedness-via-lp)
///
/// Checks structural boundedness and returns the weight vector if feasible.
///
/// Finds `y > 0` such that `yᵀ · N ≤ 0` (each component ≥ 1). If feasible,
/// returns the weight vector y. Given a specific initial marking M₀,
/// per-place upper bounds can be derived: `M[p] ≤ ⌊(y·M₀) / y[p]⌋`.
#[must_use]
pub fn find_positive_place_subinvariant(net: &DenseNet) -> Option<Box<[f64]>> {
    find_place_subinvariant(net, |_| true)
}

/// Checks whether a set of places is structurally bounded
/// (bounded under every possible initial marking).
///
/// Tries to find a semi-positive weighting with `place` in its support
/// (`y[place] ≥ 1`) and `yᵀ · N ≤ 0`, demonstrating that the weighted
/// token count of that place cannot increase no matter what transitions fire,
/// thus guaranteeing its boundedness.
///
/// For a stronger check of the entire net, see [`find_positive_place_subinvariant`].
///
/// Feasible → place is structurally bounded; Infeasible → structurally
/// unbounded (there exists an initial marking under which it is unbounded).
#[must_use]
pub fn find_place_subinvariant<F: FnMut(&PlaceIdx) -> bool>(
    net: &DenseNet,
    mut in_support: F,
) -> Option<Box<[f64]>> {
    let mut variables = ProblemVariables::new();
    let place_weights: Box<[Variable]> = net
        .place_indices()
        .map(|p| {
            if in_support(&p) {
                variables.add(variable().min(1.0))
            } else {
                variables.add(variable().min(0.0))
            }
        })
        .collect();

    let incidence = net.incidence_matrix();
    let constraints = net.transition_indices().map(|t| {
        let token_delta: Expression = net
            .place_indices()
            .map(|p| f64::from(incidence.get_effect(t, p)) * place_weights[p])
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
                .into_iter()
                .map(|v| solution.value(v))
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use crate::core::net::structural_boundedness::*;
    use crate::net::{Net, builder::NetBuilder};

    fn two_place_cycle() -> Net {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p0));
        b.build().unwrap()
    }

    #[test]
    fn cycle_structurally_bounded() {
        let net = two_place_cycle();
        assert!(
            find_positive_place_subinvariant(&net.dense_net).is_some(),
            "cycle should be structurally bounded"
        );
    }

    #[test]
    fn producer_structurally_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0));
        b.add_arc((p0, t1));
        b.add_arc((t1, p1));
        b.add_arc((p1, t0));
        let net = b.build().unwrap().dense_net;
        assert!(
            find_positive_place_subinvariant(&net).is_some(),
            "producer net should be proven bounded"
        );
    }

    #[test]
    fn source_transition_not_structurally_bounded() {
        let mut b = NetBuilder::new();
        let p0 = b.add_place();
        let t0 = b.add_transition();
        b.add_arc((t0, p0));
        let net = b.build().unwrap();
        let p0 = net.mapping.place_idx(p0).expect("place in built net");
        assert!(find_positive_place_subinvariant(&net.dense_net).is_none());
        assert!(find_place_subinvariant(&net.dense_net, |&idx| idx == p0).is_none());
    }

    #[test]
    fn nonuniform_weights_structurally_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((t0, p2));
        b.add_arc((p1, t1));
        b.add_arc((p2, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap().dense_net;
        assert!(find_positive_place_subinvariant(&net).is_some());
    }
}
