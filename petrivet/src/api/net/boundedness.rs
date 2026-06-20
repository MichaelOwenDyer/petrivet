use crate::core::analysis::rational::Rational;
use crate::core::analysis::semi_decision;
use crate::marking::Marking;
use crate::model::{
    BoundednessSubinvariantCert, Certificate, PlaceBoundednessSubinvariantCert, Query,
};
use crate::net::{Net, Place};
use ahash::HashMap;

/// Rationalize an `f64` place-weight suggestion (in dense place-index order) into
/// a `HashMap<Place, Rational>` under the scaling `scale`, rounding each weight to
/// the nearest integer. Returns `None` if any scaled weight is not a finite,
/// representable integer in the admitted range, or if the LP vector length does
/// not match the place count.
///
/// The result is only ever *suggested* to an exact checker; an over- or
/// under-rounded suggestion can only fail the exact `check`, never mint a verdict.
/// Weights below `min_each` are rejected (the whole-net cert needs `≥ 1` on every
/// place; the per-place cert admits `0` elsewhere, so it passes `min_each = 0`).
fn rationalize_suggestion(
    places: &[Place],
    weights_f64: &[f64],
    scale: f64,
    min_each: f64,
) -> Option<HashMap<Place, Rational>> {
    if places.len() != weights_f64.len() {
        return None;
    }
    let mut weights = HashMap::default();
    for (&p, &w) in places.iter().zip(weights_f64.iter()) {
        let scaled = (w * scale).round();
        if !scaled.is_finite() || scaled < min_each || scaled > 1e18 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        weights.insert(p, Rational::from_int(scaled as i128));
    }
    Some(weights)
}

impl Net {
    /// Checks if the net is *structurally* bounded.
    /// This means that no initial marking on this net
    /// would create an unbounded system.
    ///
    /// The positive verdict rests on an **exact** positive place sub-invariant
    /// certificate ([`BoundednessSubinvariantCert`]), never the bare `f64` LP: the
    /// `f64` LP (`find_positive_place_subvariant`) may *suggest* a weight vector
    /// `y`, but the verdict returns `true` only after the exact checker re-verifies
    /// `y > 0` and `yᵀ·C ≤ 0` over ℚ against this net (a near-boundary `yᵀ·C` that
    /// the float simplex rounds to ≤ 0 cannot mint a false `true`). On exact-
    /// validation failure we return `false` — the honest *not certified* (this is a
    /// structural, marking-free query, so no coverability-graph fallback is
    /// available; abstaining to `false` is the safe direction for a positive
    /// verdict). See foundational-design §4.6.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        let Some(weights_f64) = semi_decision::find_positive_place_subvariant(&self.dense_net)
        else {
            return false;
        };
        let places: Vec<Place> = self.places().collect();
        // A whole-net positive sub-invariant needs every weight ≥ 1. Try a few
        // small scalings so a fractional LP optimum (e.g. 1/2) rounds to an exact
        // integer multiple.
        for scale in [1.0_f64, 2.0, 3.0, 6.0] {
            let Some(weights) = rationalize_suggestion(&places, &weights_f64, scale, 1.0) else {
                continue;
            };
            let cert = BoundednessSubinvariantCert { weights };
            if cert.check(self, &Marking::default(), &Query::Trivial) {
                return true;
            }
        }
        false
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    ///
    /// As with [`Net::is_structurally_bounded`], the positive verdict rests on an
    /// **exact** semi-positive place sub-invariant certificate
    /// ([`PlaceBoundednessSubinvariantCert`]): the `f64` LP
    /// (`find_semipositive_place_subvariant`) suggests `y`, and the verdict returns
    /// `true` only after the exact checker re-verifies `y ≥ 0`, `y[place] ≥ 1`, and
    /// `yᵀ·C ≤ 0` over ℚ. On exact-validation failure we return `false` (not
    /// certified; the safe direction for a positive structural verdict).
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: Place) -> bool {
        let Some(p_idx) = self.mapping.place_idx(place) else {
            return false;
        };
        let Some(weights_f64) =
            semi_decision::find_semipositive_place_subvariant(&self.dense_net, |&p| p == p_idx)
        else {
            return false;
        };
        let places: Vec<Place> = self.places().collect();
        // A semi-positive cert admits weight 0 off the witness place (`min_each =
        // 0`); the checker independently enforces `y[place] ≥ 1`. The scalings
        // recover a fractional witness-place weight.
        for scale in [1.0_f64, 2.0, 3.0, 6.0] {
            let Some(weights) = rationalize_suggestion(&places, &weights_f64, scale, 0.0) else {
                continue;
            };
            let cert = PlaceBoundednessSubinvariantCert {
                place,
                weights,
            };
            if cert.check(self, &Marking::default(), &Query::Trivial) {
                return true;
            }
        }
        false
    }
}
