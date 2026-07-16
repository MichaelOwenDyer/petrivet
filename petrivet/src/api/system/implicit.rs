//! Detection of **implicit places**.
//!
//! An implicit place's token count is made redundant by a non-negative linear
//! combination of the *other* places, so it never uniquely disables a transition
//! and may be deleted without changing behaviour. This is the first, conservative
//! increment of structural reductions: it *detects* implicit places (with an exact
//! ℚ certificate) but does **not** rewrite the net.
//!
//! # Criterion (Silva–Colom implicit place)
//!
//! A place `p` is implicit for the system `(N, M₀)` if there is a rational vector
//! `y ≥ 0` over the places `q ≠ p` with, for all arcs weight 1 (`Pre[q][t] = [q∈•t]`):
//!
//! - **(C1) flow domination** — every transition `t`: `N[p][t] ≥ Σ_{q≠p} y_q·N[q][t]`;
//! - **(C2) enabling domination** — every `t ∈ p•`:
//!   `M₀[p] − Σ_{q≠p} y_q·M₀[q] + Σ_{q∈•t∖{p}} y_q ≥ 1`.
//!
//! Then `g(M) = M[p] − Σ_{q≠p} y_q·M[q]` is non-decreasing (C1), so it stays `≥
//! g(M₀)`; combined with (C2) this forces `M[p] ≥ 1` whenever `p`'s output
//! transition `t` is otherwise enabled — `p` is never the *sole* disabler of any
//! `t ∈ p•`. See
//! [`is_implicit_place_certificate`](crate::core::analysis::exact_matrix::is_implicit_place_certificate)
//! for the full soundness proof.
//!
//! # Soundness discipline (mirrors [`boundedness`](super::super::net::boundedness))
//!
//! An f64 LP only *suggests* the weights `y`; a positive verdict is returned **only**
//! after those weights are rationalized and re-verified EXACTLY over ℚ against this
//! system. A near-boundary float solution can therefore only fail verification
//! (→ abstain, `None`), never mint a false "implicit". Missing a genuinely-implicit
//! place (abstaining) is acceptable; flagging a non-implicit place is not.

use crate::core::analysis::rational::Rational;
use crate::core::analysis::{exact_matrix, semi_decision};
use crate::core::net::PlaceIdx;
use crate::net::{Net, Place};
use crate::prelude::PetriNet;

/// An exact certificate that a [`Place`] is **implicit** in a system `(N, M₀)`.
///
/// The certificate is the non-negative ℚ weight vector `y` of the Silva–Colom
/// criterion (see the [module docs](self)), carried as public `(Place, Rational)`
/// pairs (only the non-zero weights) so a caller can independently re-check it via
/// [`PetriNet::verify_implicit_place`]. The witness is meaningful **relative to the
/// initial marking** it was produced against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplicitPlaceWitness {
    /// The place proven implicit.
    place: Place,
    /// The non-negative weight `y_q > 0` placed on each *other* place `q` by the
    /// certificate. Places with weight zero are omitted. The subject `place` never
    /// appears here (its own coefficient in the criterion is the implicit `+1`).
    weights: Vec<(Place, Rational)>,
}

impl ImplicitPlaceWitness {
    /// The place this witness proves implicit.
    #[must_use]
    pub const fn place(&self) -> Place {
        self.place
    }

    /// The certificate weights `y_q > 0` on the other places, as public
    /// `(Place, Rational)` pairs. Re-checkable against the net; see
    /// [`PetriNet::verify_implicit_place`].
    #[must_use]
    pub fn weights(&self) -> &[(Place, Rational)] {
        &self.weights
    }
}

/// Rationalize an f64 weight suggestion (dense place-index order) at integer
/// `scale`, rounding each weight to the nearest integer, or `None` if any scaled
/// weight is not a finite, non-negative integer within a safe `i128` range. As in
/// [`boundedness`](super::super::net::boundedness), the result is only ever
/// *suggested* to the exact ℚ checker, so an over/under-rounded suggestion can only
/// fail exact verification, never mint a verdict.
fn rationalize_nonneg(weights_f64: &[f64], scale: i128) -> Option<Vec<Rational>> {
    #[allow(clippy::cast_precision_loss)]
    let scale_f = scale as f64;
    weights_f64
        .iter()
        .map(|&w| {
            let scaled = (w * scale_f).round();
            if !scaled.is_finite() || scaled < 0.0 || scaled.abs() > 1e18 {
                return None;
            }
            #[allow(clippy::cast_possible_truncation)]
            Some(Rational::from_int(scaled as i128))
        })
        .collect()
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Returns every place this procedure can **prove** implicit at the current
    /// (initial) marking, each with an exact ℚ [`ImplicitPlaceWitness`].
    ///
    /// Soundness-only: a place *absent* from the result is **not** thereby proven
    /// essential — the procedure abstains rather than guess (see
    /// [`is_implicit_place`](Self::is_implicit_place) for the exact criterion and
    /// its limits). Each certificate is independently established (removing one
    /// implicit place can make another essential, so the set is not jointly
    /// removable).
    #[must_use]
    pub fn implicit_places(&self) -> Vec<ImplicitPlaceWitness> {
        // Collect places first so the `places()` borrow ends before we re-borrow
        // `self` per place in `is_implicit_place`.
        self.places()
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|p| self.is_implicit_place(p))
            .collect()
    }

    /// Attempts to prove `place` **implicit** at the current (initial) marking,
    /// returning an exact ℚ [`ImplicitPlaceWitness`] when provable and `None`
    /// (abstain) otherwise.
    ///
    /// # What a returned witness proves
    ///
    /// `place` is implicit for `(N, M₀)`: it is never the sole place disabling one
    /// of its output transitions, so deleting `place` (and its arcs) leaves the
    /// fireable-sequence set — and the reachable markings projected onto the other
    /// places — unchanged. The criterion (C1)+(C2) and its soundness proof are
    /// documented on
    /// [`exact_matrix::is_implicit_place_certificate`](crate::core::analysis::exact_matrix::is_implicit_place_certificate).
    ///
    /// # What `None` does NOT prove
    ///
    /// `None` is an *abstention*, not a proof that `place` is essential: the
    /// criterion is sufficient, not necessary, and the f64 suggester is not
    /// complete. The verdict is **relative to the current initial marking**.
    ///
    /// # Method
    ///
    /// An f64 LP ([`semi_decision::find_implicit_place_weights`]) suggests weights;
    /// `Some` is returned only after they are rationalized (at a few integer
    /// scalings, recovering fractional optima) and re-verified EXACTLY over ℚ
    /// against this system. On exact-verification failure we abstain.
    #[must_use]
    pub fn is_implicit_place(&self, place: Place) -> Option<ImplicitPlaceWitness> {
        let p_idx = self.mapping.place_idx(place)?;
        let suggested =
            semi_decision::find_implicit_place_weights(&self.dense_net, p_idx, &self.marking)?;
        // Trust the suggestion only after an exact ℚ re-verification.
        [1_i128, 2, 3, 6].into_iter().find_map(|scale| {
            let weights = rationalize_nonneg(&suggested, scale)?;
            exact_matrix::is_implicit_place_certificate(
                &self.dense_net,
                p_idx,
                &self.marking,
                &weights,
            )
            .then(|| self.build_witness(place, p_idx, &weights))
        })
    }

    /// Re-checks an [`ImplicitPlaceWitness`] against this system, **exactly over ℚ**.
    ///
    /// Returns `true` iff the witness's weights still certify its place implicit at
    /// this system's current marking (via
    /// [`exact_matrix::is_implicit_place_certificate`](crate::core::analysis::exact_matrix::is_implicit_place_certificate)).
    /// A caller can use this to independently validate a witness — or to check
    /// whether a witness produced against one marking still holds against another.
    /// A witness referencing a place not in this net returns `false`.
    #[must_use]
    pub fn verify_implicit_place(&self, witness: &ImplicitPlaceWitness) -> bool {
        let Some(p_idx) = self.mapping.place_idx(witness.place) else {
            return false;
        };
        let mut weights = vec![Rational::ZERO; self.dense_net.place_count()];
        for &(q, w) in &witness.weights {
            let Some(q_idx) = self.mapping.place_idx(q) else {
                return false;
            };
            weights[q_idx] = w;
        }
        exact_matrix::is_implicit_place_certificate(&self.dense_net, p_idx, &self.marking, &weights)
    }

    /// Convert a verified dense weight vector into the public sparse witness (subject
    /// place dropped, zero weights omitted).
    fn build_witness(
        &self,
        place: Place,
        p_idx: PlaceIdx,
        weights: &[Rational],
    ) -> ImplicitPlaceWitness {
        let weights = weights
            .iter()
            .enumerate()
            .filter(|&(q, w)| q != p_idx && !w.is_zero())
            .map(|(q, &w)| (self.mapping.place(q), w))
            .collect();
        ImplicitPlaceWitness { place, weights }
    }
}

#[cfg(test)]
mod tests {
    use super::ImplicitPlaceWitness;
    use crate::builder::NetBuilder;
    use crate::net::{Net, Place};

    /// The "parallel place" net: `r` duplicates `a`. Transitions `t: {a,r} → b` and
    /// `u: {b} → {a,r}`. With `a` and `r` identically connected, `M[a] = M[r]` at
    /// every reachable marking once they start equal, so each of `a`, `r` is
    /// implicit (the other witnesses it) and the middle place `b` is essential.
    fn parallel_place_net() -> (Net, Place, Place, Place) {
        let mut nb = NetBuilder::new();
        let [pa, pb, pr] = nb.add_places();
        let [t_consume, t_produce] = nb.add_transitions();
        nb.add_arc((pa, t_consume));
        nb.add_arc((pr, t_consume));
        nb.add_arc((t_consume, pb));
        nb.add_arc((pb, t_produce));
        nb.add_arc((t_produce, pa));
        nb.add_arc((t_produce, pr));
        (nb.build().expect("valid net"), pa, pb, pr)
    }

    /// Positive: a genuinely redundant place is detected, and every returned witness
    /// re-verifies exactly over ℚ.
    #[test]
    fn parallel_place_detected_and_reverified() {
        let (net, a, _b, r) = parallel_place_net();
        let sys = net.with_initial_marking([(a, 1), (r, 1)]);

        let found = sys.implicit_places();
        assert!(!found.is_empty(), "a parallel/redundant place must be detected");
        for w in &found {
            assert!(sys.verify_implicit_place(w), "every returned witness must re-verify over ℚ");
        }

        // Specifically, r is implicit given this marking, and its witness re-checks.
        let wr = sys.is_implicit_place(r).expect("r is implicit under M₀ = {a:1, r:1}");
        assert_eq!(wr.place(), r);
        assert!(!wr.weights().is_empty(), "the certificate names the dominating place(s)");
        assert!(sys.verify_implicit_place(&wr));
    }

    /// Falsifiability: the essential middle place `b` is never flagged, and a
    /// tampered (empty-combination) witness for it is rejected by exact re-check.
    #[test]
    fn essential_place_is_not_implicit() {
        let (net, a, b, r) = parallel_place_net();
        let sys = net.with_initial_marking([(a, 1), (r, 1)]);

        assert!(
            sys.is_implicit_place(b).is_none(),
            "the constraining middle place must not be provable implicit"
        );
        assert!(
            !sys.implicit_places().iter().any(|w| w.place() == b),
            "b must never appear among the implicit places"
        );
        // A hand-forged witness claiming b implicit with an empty combination must
        // fail exact re-verification (this would panic the suite if the check were
        // unsound).
        let forged = ImplicitPlaceWitness { place: b, weights: Vec::new() };
        assert!(!sys.verify_implicit_place(&forged), "a bogus witness must be rejected");
    }

    /// Negative: the two-place cycle has no implicit place — each place is the sole
    /// input of its output transition, so the LP is infeasible and the detector
    /// abstains (empty / `None`), never fabricating a certificate.
    #[test]
    fn two_place_cycle_has_no_implicit_place() {
        let (net, p0, _t0, p1, _t1) = crate::api::system::tests::two_place_cycle();
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.implicit_places().is_empty(), "no place in a 2-cycle is implicit");
        assert!(sys.is_implicit_place(p0).is_none());
        assert!(sys.is_implicit_place(p1).is_none());
    }
}
