//! Rackoff's a priori depth bound for the coverability problem.
//!
//! Rackoff proved that if a target marking is coverable at all, then it is
//! coverable by a firing sequence whose length is bounded by a function of
//! the number of places and the target alone — computable before any search
//! begins, and independent of the initial marking. This module computes that
//! bound.
//!
//! References:
//! - C. Rackoff, "The covering and boundedness problems for vector addition
//!   systems," *Theoretical Computer Science*, vol. 6, no. 2, pp. 223–231, 1978.
//! - [Esparza Lecture Notes, Theorem 3.2.9](crate::literature#theorem-329--rackoff-coverability-depth-bound)
//! - [Esparza Lecture Notes, Lemma 3.2.12](crate::literature#lemma-3212--length-of-shortest-i-covering-sequences)

use crate::core::marking::IdxMarking;
use crate::core::net::DenseNet;

/// Computes Rackoff's a priori upper bound on the length of a shortest
/// covering firing sequence for `target`, or `None` if the bound does not
/// fit in `u128`.
///
/// # Theorem
///
/// Let `k` be the number of places and `n = 1 + Σ_p target(p)`. If `target`
/// is coverable from a marking `M₀` — i.e. some firing sequence from `M₀`
/// reaches a marking `M′ ≥ target` — then some firing sequence of length at
/// most `f(k)` does, where
///
/// ```text
/// f(0) = 1,    f(i) = (n · f(i−1))^i + f(i−1)    for i ≥ 1.
/// ```
///
/// This is the recurrence of Lemma 3.2.12 in the Esparza Lecture Notes,
/// which also give the closed form `f(k) ≤ (2n)^((k+1)!)` (Theorem 3.2.9,
/// attributed to Rackoff 1978). This function returns the tighter `f(k)`.
/// The bound depends only on `k` and the target: it is valid for **every**
/// initial marking.
///
/// # Soundness contract
///
/// - **Positive direction (what the bound licenses):** searching for a
///   covering sequence deeper than `f(k)` is provably useless. Any search
///   may soundly stop extending a path once its length exceeds the bound.
/// - **Negative direction (what the bound does *not* license by itself):**
///   concluding `Uncoverable` from a depth-limited search is sound only if
///   the search provably enumerated *every* distinct marking reachable
///   within depth `f(k)` (de-duplication is admissible because a shortest
///   covering sequence never repeats a marking — see the proof sketch
///   below). This function only computes the bound; it decides nothing.
/// - **Abstention:** the bound is doubly exponential in `k`, so overflow is
///   the common case (already at 5 places for a unit target). On any
///   arithmetic overflow this function returns `None`, meaning "the bound is
///   not representable", never "no bound exists" and never a saturated
///   substitute value.
///
/// # Adaptation to standard firing semantics
///
/// The lecture notes prove Lemma 3.2.12 for *integer* Petri nets, where
/// transitions are never blocked. For nets with a place `p ∈ •t ∩ t•`, an
/// integer sequence that stays non-negative is not automatically a standard
/// firing sequence (`t` needs `p ≥ 1` even though its net effect on `p` is
/// 0), so the standard-semantics claim needs its own argument. The same
/// recurrence is valid for standard semantics on the plain (arc-weight-1)
/// nets represented by [`DenseNet`], by the following induction, which is
/// the reason this function may be used to cap searches over the *standard*
/// reachability graph.
///
/// For a place subset `I` with `|I| = i`, write `N|I` for the net restricted
/// to the places in `I` (transitions keep only their arcs into `I`; standard
/// enabledness checked on `I` only). Claim: for every `I` and every start
/// marking, if some firing sequence of `N|I` ends `≥ target` on `I`, then
/// one of length at most `f(i)` does. With `I` the full place set this is
/// the theorem. Induction on `i`, threshold `B = n · f(i−1)`:
///
/// - A *shortest* covering sequence of `N|I` never repeats a marking of
///   `N|I`: cutting the segment between two equal markings preserves
///   validity (enabledness in `N|I` depends only on the current `N|I`
///   marking) and preserves the final marking, contradicting minimality.
/// - Case 1: every visited `N|I` marking is `< B` on all places of `I`.
///   The markings are distinct points of `{0..B−1}^i`, so the length is at
///   most `B^i ≤ f(i)`.
/// - Case 2: otherwise, at the first position where some place `j ∈ I`
///   reaches `≥ B`, the prefix has length at most `B^i` (distinct markings
///   `< B` before it). From there, apply the induction hypothesis to
///   `I′ = I ∖ {j}` to get a tail of length at most `f(i−1)` covering the
///   target on `I′`, and lift it back to `N|I`: place `j` starts `≥ B` and
///   each firing consumes at most 1 token from it (plain net), so before
///   every tail step `j ≥ (n−1)·f(i−1) + 1 ≥ 1` (every firing stays
///   standard-enabled on `j`) and finally `j ≥ (n−1)·f(i−1) ≥ n−1 ≥
///   target(j)` (the target is covered on `j` as well). Total length at
///   most `B^i + f(i−1) = f(i)`.
///
/// For weighted nets the recurrence would need the maximum arc weight
/// factored into the threshold; [`DenseNet`] cannot represent weights, so
/// the weight-1 form is exact here.
///
/// # Degenerate cases
///
/// - An all-zero target yields a small positive bound; sound (the empty
///   sequence already covers), merely loose.
/// - `k = 0` returns `f(0) = 1`; sound for the same reason. (Empty nets are
///   rejected at build time, so this does not arise in practice.)
#[must_use]
pub fn coverability_depth_bound(net: &DenseNet, target: &IdxMarking<u32>) -> Option<u128> {
    debug_assert_eq!(net.place_count(), target.place_count());

    // n = 1 + Σ_p target(p). Checked arithmetic throughout: this module
    // must abstain (None) rather than ever return a too-small bound.
    let tokens = target
        .iter()
        .try_fold(0u128, |acc, &t| acc.checked_add(u128::from(t)))?;
    let n = tokens.checked_add(1)?;

    // f(0) = 1; f(i) = (n·f(i−1))^i + f(i−1).
    let mut f: u128 = 1;
    for i in 1..=net.place_count() {
        let exponent = u32::try_from(i).ok()?;
        f = n.checked_mul(f)?.checked_pow(exponent)?.checked_add(f)?;
    }
    Some(f)
}

#[cfg(test)]
mod tests {
    use crate::core::analysis::rackoff::coverability_depth_bound;
    use crate::prelude::{Net, NetBuilder};

    /// p0 -> t0 -> p1 -> t1 -> ... -> p(n-1), a chain with `n` places.
    fn chain(places: usize) -> Net {
        let mut b = NetBuilder::new();
        let ps: Vec<_> = (0..places).map(|_| b.add_place()).collect();
        for window in ps.windows(2) {
            let t = b.add_transition();
            b.add_arc((window[0], t));
            b.add_arc((t, window[1]));
        }
        if places == 1 {
            // Nets must have at least one transition to build.
            let t = b.add_transition();
            b.add_arc((ps[0], t));
        }
        b.build().unwrap()
    }

    #[test]
    fn one_place_unit_target() {
        // k = 1, target (1): n = 2.
        // f(1) = (2·1)^1 + 1 = 3.
        let net = chain(1);
        assert_eq!(coverability_depth_bound(&net.dense_net, &[1].into()), Some(3));
    }

    #[test]
    fn two_places_unit_target() {
        // k = 2, target (1,0): n = 2.
        // f(1) = (2·1)^1 + 1 = 3; f(2) = (2·3)^2 + 3 = 39.
        let net = chain(2);
        assert_eq!(coverability_depth_bound(&net.dense_net, &[1, 0].into()), Some(39));
    }

    #[test]
    fn two_places_ones_target() {
        // k = 2, target (1,1): n = 3.
        // f(1) = (3·1)^1 + 1 = 4; f(2) = (3·4)^2 + 4 = 148.
        let net = chain(2);
        assert_eq!(coverability_depth_bound(&net.dense_net, &[1, 1].into()), Some(148));
    }

    #[test]
    fn two_places_zero_target() {
        // k = 2, target (0,0): n = 1.
        // f(1) = (1·1)^1 + 1 = 2; f(2) = (1·2)^2 + 2 = 6.
        // Loose but sound: the empty sequence covers the zero target.
        let net = chain(2);
        assert_eq!(coverability_depth_bound(&net.dense_net, &[0, 0].into()), Some(6));
    }

    #[test]
    fn three_places_ones_target() {
        // k = 3, target (1,1,1): n = 4.
        // f(1) = (4·1)^1 + 1 = 5;
        // f(2) = (4·5)^2 + 5 = 405;
        // f(3) = (4·405)^3 + 405 = 1620^3 + 405 = 4_251_528_405.
        let net = chain(3);
        assert_eq!(
            coverability_depth_bound(&net.dense_net, &[1, 1, 1].into()),
            Some(4_251_528_405)
        );
    }

    #[test]
    fn four_places_computes_five_places_overflows() {
        // n = 2: f(3) = (2·39)^3 + 39 = 474_591; f(4) = 949_182^4 + 474_591
        // still fits u128 (949_182 < 2^20, so 949_182^4 < 2^80), but
        // f(5) = (2·f(4))^5 + f(4) exceeds 2^128. Overflow must abstain:
        // `None`, never a saturated value.
        let four = chain(4);
        assert!(coverability_depth_bound(&four.dense_net, &[1, 0, 0, 0].into()).is_some());
        let five = chain(5);
        assert_eq!(
            coverability_depth_bound(&five.dense_net, &[1, 0, 0, 0, 0].into()),
            None
        );
    }
}
