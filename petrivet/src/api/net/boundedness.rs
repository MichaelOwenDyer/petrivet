use crate::core::analysis::rational::Rational;
use crate::core::analysis::{exact_matrix, semi_decision};
use crate::net::{Net, Place};

/// Rationalize an `f64` place-weight suggestion (dense place-index order) at the
/// integer `scale`, rounding each weight to the nearest integer. Returns `None` if
/// any scaled weight is not a finite integer at least `min_each` within the safe
/// `i128` range. The result is only ever *suggested* to the exact checker, so an
/// over- or under-rounded suggestion can only fail the exact verification, never
/// mint a verdict.
fn rationalize(weights_f64: &[f64], scale: i128, min_each: f64) -> Option<Vec<Rational>> {
    #[allow(clippy::cast_precision_loss)]
    let scale_f = scale as f64;
    weights_f64
        .iter()
        .map(|&w| {
            let scaled = (w * scale_f).round();
            if !scaled.is_finite() || scaled < min_each || scaled.abs() > 1e18 {
                return None;
            }
            #[allow(clippy::cast_possible_truncation)]
            Some(Rational::from_int(scaled as i128))
        })
        .collect()
}

impl Net {
    /// Checks if the net is *structurally* bounded.
    /// This means that no initial marking on this net
    /// would create an unbounded system.
    ///
    /// The positive verdict rests on an **exact** positive place sub-invariant,
    /// never the bare `f64` LP. The f64 LP
    /// ([`find_positive_place_subvariant`](semi_decision::find_positive_place_subvariant))
    /// only *suggests* a weight vector `y`; `true` is returned only after the
    /// exact checker re-verifies `y > 0` and `yᵀ·C ≤ 0` over ℚ against this net,
    /// so a near-boundary `yᵀ·C` that the float simplex rounds to `≤ 0` cannot mint
    /// a false `true`. On exact-validation failure we return `false` (the safe
    /// direction for a positive structural verdict — this is a marking-free query,
    /// so there is no coverability-graph fallback). The small scalings recover a
    /// fractional LP optimum (e.g. `1/2`) as an exact integer multiple.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        let Some(weights_f64) = semi_decision::find_positive_place_subvariant(&self.dense_net)
        else {
            return false;
        };
        [1, 2, 3, 6].into_iter().any(|scale| {
            rationalize(&weights_f64, scale, 1.0).is_some_and(|weights| {
                exact_matrix::is_positive_place_subinvariant(&self.dense_net, &weights)
            })
        })
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    ///
    /// As with [`is_structurally_bounded`](Self::is_structurally_bounded), the
    /// positive verdict rests on an **exact** semi-positive place sub-invariant:
    /// the f64 LP suggests `y`, and `true` is returned only after the exact checker
    /// re-verifies `y ≥ 0`, `y[place] > 0`, and `yᵀ·C ≤ 0` over ℚ. On exact-
    /// validation failure we return `false`.
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
        // The semi-positive cert admits weight 0 off the witness place (`min_each =
        // 0`); the exact checker independently enforces `y[place] > 0`.
        [1, 2, 3, 6].into_iter().any(|scale| {
            rationalize(&weights_f64, scale, 0.0).is_some_and(|weights| {
                exact_matrix::is_semipositive_place_subinvariant(&self.dense_net, &weights, p_idx)
            })
        })
    }
}