//! M4 · B0 (scalar half) — an exact-rational scalar with a documented overflow
//! policy.
//!
//! A Petri net's linear theory is defined over ℚ; checkable algebraic
//! certificates (the Farkas dual, P/T-semiflows, exact rank — M5) cannot be
//! produced over `f64` (§1, §4.1: a floating feasibility result near a constraint
//! boundary can be wrong, and rank is *discontinuous* in the entries, so rounding
//! is unsound). This module supplies the scalar `S` the exact matrix layer (M5)
//! is built on.
//!
//! # The representation decision (the program's first engineering decision)
//!
//! `Rational` is a normalized `i128 / i128`. The design (§4, §4.1) names three
//! candidate representations and their trade-offs:
//!
//! - `i64 / i64` — fast, but overflows on large MCC instances. **Rejected:** the
//!   marking-equation coefficients are small, but Bareiss intermediate
//!   determinants grow (Hadamard bound), and silent `i64` wrap is a soundness
//!   hole.
//! - **`i128` with overflow *detection* — chosen.** Every arithmetic operation is
//!   `checked_*` and returns [`Result`]: an overflow is a *detected error*
//!   ([`RationalOverflow`]), **never a silent wrap**. The doctrine's
//!   floating-point-is-a-soundness-concern stance applies equally to integer
//!   wrap: a wrong magnitude is a wrong certificate. `i128` carries the Bareiss
//!   intermediates for the corpus's net sizes; when it does not, the operation
//!   *fails loudly* and the caller abstains (`Inconclusive`) rather than
//!   fabricating.
//! - A bignum dependency (`num-rational`) — unbounded, but a new dependency and a
//!   per-op allocation cost. **Deferred:** recorded as the promotion path if the
//!   corpus ever overflows `i128` in practice (a measured question, not assumed).
//!   The `checked_*` API is exactly the seam a future `i128 → bignum` promotion
//!   slots into without changing call sites.
//!
//! This decision gates F1/M5 and affects the cost of every downstream proof; it
//! is recorded here and in `foundational-design.md` §F4·M4 so it is
//! re-derivable.
//!
//! # Invariants (held by construction)
//!
//! Every `Rational` is **normalized**: `gcd(|num|, den) == 1`, `den > 0`, and the
//! canonical zero is `0/1`. Normalization makes `Eq`/`Ord`/`Hash`
//! **representation-independent**: `2/4` and `1/2` are the *same value and the
//! same bits*, because the only constructor normalizes. This is the property the
//! M4 `[PROP]` "value-equality independent of representation" gate rests on.

// `dead_code`: M5's exact matrix layer now consumes the core `Rational` surface
// (construction, the checked arithmetic, ordering), but a few convenience members
// (`From<i32>`, `denominator`, `Display`) are exercised only by tests / reserved
// for the M6+ deciders and the C6 certificate rendering. They are kept as the
// type's complete, documented surface rather than trimmed and re-added; the
// narrow allow says so without masking a genuinely-unused *core* method.
#![allow(dead_code)]

use std::cmp::Ordering;
use std::fmt;

/// An exact rational number `num / den`, always stored in lowest terms with a
/// positive denominator.
///
/// Arithmetic is **checked**: `checked_add`/`sub`/`mul`/`div` return
/// `Result<Rational, RationalOverflow>` and never wrap. There is deliberately no
/// `impl Add`/`Mul` (an operator that can silently overflow `i128` would
/// re-introduce the very soundness hole this type closes); callers thread the
/// `Result`, so an overflow forces an honest abstention at the decision site.
#[derive(Debug, Clone, Copy)]
pub struct Rational {
    /// Numerator. Sign of the value lives here (the denominator is always > 0).
    num: i128,
    /// Denominator. Invariant: `den > 0` and `gcd(|num|, den) == 1`.
    den: i128,
}

/// An exact-arithmetic operation overflowed `i128`.
///
/// This is the *detected* alternative to a silent wrap. A decider that hits it
/// must abstain (`Verdict::Inconclusive`), never fabricate a verdict from a
/// wrapped magnitude. (The `i128 → bignum` promotion path, if a measured corpus
/// need arises, replaces the `checked_*` internals without changing this
/// signature.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalOverflow;

impl fmt::Display for RationalOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact-rational arithmetic overflowed i128 (abstain, do not fabricate)")
    }
}

impl std::error::Error for RationalOverflow {}

/// Greatest common divisor of two non-negative `i128`s (Euclid), `gcd(0, 0) = 0`.
/// Operates on magnitudes; callers pass `|num|` and `den`.
const fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

impl Rational {
    /// Zero, `0/1`.
    pub const ZERO: Self = Self { num: 0, den: 1 };
    /// One, `1/1`.
    pub const ONE: Self = Self { num: 1, den: 1 };

    /// Construct a normalized rational `num / den`.
    ///
    /// # Errors
    /// Returns [`RationalOverflow`] if `den == 0` (no rational), or if
    /// normalization would overflow `i128` (only possible at `i128::MIN`, whose
    /// negation is not representable).
    pub const fn new(num: i128, den: i128) -> Result<Self, RationalOverflow> {
        if den == 0 {
            return Err(RationalOverflow); // not a rational; treated as a hard failure
        }
        // i128::MIN has no positive magnitude, so it cannot be normalized safely.
        if num == i128::MIN || den == i128::MIN {
            return Err(RationalOverflow);
        }
        // Make the denominator positive by moving any sign to the numerator.
        let (mut num, mut den) = if den < 0 { (-num, -den) } else { (num, den) };
        let g = gcd(num.abs(), den);
        if g != 0 {
            num /= g;
            den /= g;
        }
        Ok(Self { num, den })
    }

    /// An integer as a rational, `n / 1`. Cannot overflow (the denominator is 1).
    ///
    /// # Panics
    ///
    /// Panics on `i128::MIN`: its magnitude `|i128::MIN|` is not representable, so
    /// it is the one value `new` refuses (its negation/abs would overflow). Making
    /// `from_int` *infallible* while *silently* admitting `i128::MIN` would bypass
    /// that guard and leave an instance on which a later `checked_neg`/normalization
    /// behaves inconsistently with the rest of the type — so `from_int` upholds the
    /// same invariant `new` does, loudly. In-tree callers only ever pass small
    /// integers (place-count and token-difference magnitudes), so this is
    /// unreachable in practice; the assertion is the uniform guard the minor (M·minor)
    /// records, not a runtime hazard. A caller that genuinely needs to admit
    /// arbitrary `i128` (including `MIN`) uses [`Rational::new`], which returns a
    /// `Result`.
    #[must_use]
    pub const fn from_int(n: i128) -> Self {
        assert!(n != i128::MIN, "Rational::from_int(i128::MIN) is unrepresentable; use Rational::new");
        Self { num: n, den: 1 }
    }

    /// The numerator (in lowest terms, sign-carrying).
    #[must_use]
    pub const fn numerator(self) -> i128 {
        self.num
    }

    /// The denominator (in lowest terms, always positive).
    #[must_use]
    pub const fn denominator(self) -> i128 {
        self.den
    }

    /// Whether this is exactly zero. Exact because of normalization (`0/1`).
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.num == 0
    }

    /// The additive inverse `-self`.
    ///
    /// # Errors
    /// Returns [`RationalOverflow`] only at the unrepresentable `i128::MIN`
    /// numerator (which `new` already refuses to construct, so this is reachable
    /// only via a corrupted instance).
    pub const fn checked_neg(self) -> Result<Self, RationalOverflow> {
        match self.num.checked_neg() {
            Some(num) => Ok(Self { num, den: self.den }),
            None => Err(RationalOverflow),
        }
    }

    /// Exact addition `self + other`.
    ///
    /// `a/b + c/d = (a·d + c·b) / (b·d)`, then normalized. Every multiply and the
    /// add is `checked`; an `i128` overflow is reported, never wrapped.
    ///
    /// # Errors
    /// Returns [`RationalOverflow`] if any intermediate overflows `i128`.
    pub const fn checked_add(self, other: Self) -> Result<Self, RationalOverflow> {
        let Some(ad) = self.num.checked_mul(other.den) else { return Err(RationalOverflow) };
        let Some(cb) = other.num.checked_mul(self.den) else { return Err(RationalOverflow) };
        let Some(num) = ad.checked_add(cb) else { return Err(RationalOverflow) };
        let Some(den) = self.den.checked_mul(other.den) else { return Err(RationalOverflow) };
        Self::new(num, den)
    }

    /// Exact subtraction `self - other`.
    ///
    /// # Errors
    /// Returns [`RationalOverflow`] if any intermediate overflows `i128`.
    pub const fn checked_sub(self, other: Self) -> Result<Self, RationalOverflow> {
        let Some(ad) = self.num.checked_mul(other.den) else { return Err(RationalOverflow) };
        let Some(cb) = other.num.checked_mul(self.den) else { return Err(RationalOverflow) };
        let Some(num) = ad.checked_sub(cb) else { return Err(RationalOverflow) };
        let Some(den) = self.den.checked_mul(other.den) else { return Err(RationalOverflow) };
        Self::new(num, den)
    }

    /// Exact multiplication `self · other`.
    ///
    /// # Errors
    /// Returns [`RationalOverflow`] if any intermediate overflows `i128`.
    pub const fn checked_mul(self, other: Self) -> Result<Self, RationalOverflow> {
        let Some(num) = self.num.checked_mul(other.num) else { return Err(RationalOverflow) };
        let Some(den) = self.den.checked_mul(other.den) else { return Err(RationalOverflow) };
        Self::new(num, den)
    }

    /// Exact division `self / other`.
    ///
    /// # Errors
    /// Returns [`RationalOverflow`] if `other` is zero (division by zero) or if
    /// any intermediate overflows `i128`.
    pub const fn checked_div(self, other: Self) -> Result<Self, RationalOverflow> {
        if other.num == 0 {
            return Err(RationalOverflow); // division by zero is a hard failure
        }
        let Some(num) = self.num.checked_mul(other.den) else { return Err(RationalOverflow) };
        let Some(den) = self.den.checked_mul(other.num) else { return Err(RationalOverflow) };
        Self::new(num, den)
    }
}

// --- Value equality and ordering, exact and representation-independent. ---
//
// Because every `Rational` is normalized at construction, two equal values have
// identical (num, den), so structural `Eq`/`Hash`/`Ord` on the fields are exact.
// We implement them by hand (rather than derive) to document that they are exact
// and to compare ordering via the sign of (a·d − c·b) without constructing an
// intermediate `Rational` (avoiding a spurious overflow path on comparison).

impl PartialEq for Rational {
    fn eq(&self, other: &Self) -> bool {
        // Normalized ⇒ field equality is value equality.
        self.num == other.num && self.den == other.den
    }
}

impl Eq for Rational {}

impl std::hash::Hash for Rational {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.num.hash(state);
        self.den.hash(state);
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare a/b vs c/d. The fast path is sign(a·d − c·b) with both
        // denominators positive; if the cross-products fit `i128` it is exact.
        if let (Some(ad), Some(cb)) =
            (self.num.checked_mul(other.den), other.num.checked_mul(self.den))
        {
            return ad.cmp(&cb);
        }
        // Overflow on the cross-multiply: fall back to an **exact**, overflow-free
        // continued-fraction comparison (the Euclidean / Stern–Brocot descent).
        // This keeps `Rational` exact even at magnitudes that overflow the
        // cross-product — no `f64` ever touches an ordering decision.
        cmp_continued_fraction(self.num, self.den, other.num, other.den)
    }
}

/// Exact, overflow-free comparison of `a/b` and `c/d` (both denominators `> 0`)
/// by continued-fraction descent: compare integer parts, then recurse on the
/// *reciprocals* of the fractional remainders, reversing the outcome (since
/// `x < y ⇔ 1/x > 1/y` for positive fractions). Each step only floor-divides and
/// takes a remainder — no multiplication of operands — so it cannot overflow.
fn cmp_continued_fraction(a: i128, b: i128, c: i128, d: i128) -> Ordering {
    // Integer part (floor) and non-negative remainder of each.
    let (qa, ra) = (a.div_euclid(b), a.rem_euclid(b));
    let (qc, rc) = (c.div_euclid(d), c.rem_euclid(d));
    if qa != qc {
        return qa.cmp(&qc);
    }
    // Integer parts agree; compare the fractional parts ra/b vs rc/d.
    match (ra == 0, rc == 0) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,    // a is an integer here, c has a fraction
        (false, true) => Ordering::Greater, // a has a fraction, c is an integer
        // Both have a positive fraction: ra/b vs rc/d  ⇔  reverse(b/ra vs d/rc).
        (false, false) => cmp_continued_fraction(b, ra, d, rc).reverse(),
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl From<i32> for Rational {
    fn from(n: i32) -> Self {
        Self::from_int(i128::from(n))
    }
}

#[cfg(test)]
mod tests {
    use super::{Rational, RationalOverflow};

    /// A small representative domain of rationals for the field-axiom enumeration.
    /// Includes zero, one, negatives, and non-trivial denominators. The M4
    /// `[PROP]` gates are discharged by exhaustive enumeration over this finite
    /// domain (the petrivet `[PROP]` convention — no `proptest` dependency; see
    /// foundational-design §F3″ on the discharge convention).
    fn domain() -> Vec<Rational> {
        let mut v = Vec::new();
        for num in -4_i128..=4 {
            for den in 1_i128..=4 {
                v.push(Rational::new(num, den).expect("small rational"));
            }
        }
        v
    }

    /// `[PROP]` — value-equality is independent of representation: the only
    /// constructor normalizes, so equal values are identical bits.
    #[test]
    fn equality_is_representation_independent() {
        assert_eq!(Rational::new(2, 4).unwrap(), Rational::new(1, 2).unwrap());
        assert_eq!(Rational::new(-3, -6).unwrap(), Rational::new(1, 2).unwrap());
        assert_eq!(Rational::new(0, 5).unwrap(), Rational::ZERO);
        assert_eq!(Rational::new(6, 3).unwrap(), Rational::from_int(2));
        // Sign always lives in the numerator.
        let r = Rational::new(1, -2).unwrap();
        assert_eq!(r.numerator(), -1);
        assert_eq!(r.denominator(), 2);
    }

    /// `[PROP]` — additive identity and inverse: `a + 0 == a` and `a + (−a) == 0`
    /// exactly, over the whole domain.
    #[test]
    fn additive_identity_and_inverse() {
        for a in domain() {
            assert_eq!(a.checked_add(Rational::ZERO).unwrap(), a, "a + 0 == a");
            let neg_a = a.checked_neg().unwrap();
            assert_eq!(a.checked_add(neg_a).unwrap(), Rational::ZERO, "a + (-a) == 0");
        }
    }

    /// `[PROP]` — multiplicative identity and inverse: `a · 1 == a`, and for
    /// non-zero `a`, `a · a⁻¹ == 1` exactly.
    #[test]
    fn multiplicative_identity_and_inverse() {
        for a in domain() {
            assert_eq!(a.checked_mul(Rational::ONE).unwrap(), a, "a · 1 == a");
            if !a.is_zero() {
                let inv = Rational::ONE.checked_div(a).unwrap();
                assert_eq!(a.checked_mul(inv).unwrap(), Rational::ONE, "a · a⁻¹ == 1");
            }
        }
    }

    /// `[PROP]` — commutativity of `+` and `·` over the domain.
    #[test]
    fn commutativity() {
        for a in domain() {
            for b in domain() {
                assert_eq!(
                    a.checked_add(b).unwrap(),
                    b.checked_add(a).unwrap(),
                    "a + b == b + a"
                );
                assert_eq!(
                    a.checked_mul(b).unwrap(),
                    b.checked_mul(a).unwrap(),
                    "a · b == b · a"
                );
            }
        }
    }

    /// `[PROP]` — associativity of `+` and `·` over the domain.
    #[test]
    fn associativity() {
        for a in domain() {
            for b in domain() {
                for c in domain() {
                    assert_eq!(
                        a.checked_add(b).unwrap().checked_add(c).unwrap(),
                        a.checked_add(b.checked_add(c).unwrap()).unwrap(),
                        "(a + b) + c == a + (b + c)"
                    );
                    assert_eq!(
                        a.checked_mul(b).unwrap().checked_mul(c).unwrap(),
                        a.checked_mul(b.checked_mul(c).unwrap()).unwrap(),
                        "(a · b) · c == a · (b · c)"
                    );
                }
            }
        }
    }

    /// `[PROP]` — distributivity: `a · (b + c) == a·b + a·c` over the domain.
    #[test]
    fn distributivity() {
        for a in domain() {
            for b in domain() {
                for c in domain() {
                    let lhs = a.checked_mul(b.checked_add(c).unwrap()).unwrap();
                    let rhs = a
                        .checked_mul(b)
                        .unwrap()
                        .checked_add(a.checked_mul(c).unwrap())
                        .unwrap();
                    assert_eq!(lhs, rhs, "a · (b + c) == a·b + a·c");
                }
            }
        }
    }

    /// `[PROP]` — subtraction is addition of the inverse: `a − b == a + (−b)`.
    #[test]
    fn subtraction_is_add_of_negation() {
        for a in domain() {
            for b in domain() {
                assert_eq!(
                    a.checked_sub(b).unwrap(),
                    a.checked_add(b.checked_neg().unwrap()).unwrap(),
                );
            }
        }
    }

    /// `[PROP]` — ordering is exact and total over the domain, and consistent
    /// with subtraction's sign: `a.cmp(&b) == sign(a − b)`.
    #[test]
    fn ordering_is_exact_and_consistent_with_subtraction() {
        for a in domain() {
            for b in domain() {
                // a < b  ⇔  a − b < 0, so cmp(a, b) is the sign of (a − b).
                let diff = a.checked_sub(b).unwrap();
                let by_sign = diff.numerator().cmp(&0);
                assert_eq!(a.cmp(&b), by_sign, "a vs b agrees with sign(a - b)");
            }
        }
        // A few exact spot checks across denominators.
        assert!(Rational::new(1, 3).unwrap() < Rational::new(1, 2).unwrap());
        assert!(Rational::new(-1, 2).unwrap() < Rational::ZERO);
        assert!(Rational::new(2, 1).unwrap() > Rational::new(3, 2).unwrap());
    }

    /// The ordering fast path (cross-multiply) and the overflow-free
    /// continued-fraction fallback agree, *and* the fallback is genuinely
    /// exercised: choose magnitudes whose cross-product `a·d` overflows `i128`
    /// while the values are still distinguishable. This proves no `f64` ever
    /// decides an ordering — the comparison stays exact at every magnitude.
    #[test]
    fn ordering_is_exact_even_when_the_cross_product_overflows() {
        // a = MAX/2, c = MAX/3 — both irreducible (MAX = 2^127−1 is odd and not
        // divisible by 3). The cross-product a.num·c.den = MAX·3 overflows i128,
        // forcing the continued-fraction fallback. The true order is MAX/2 > MAX/3.
        let m = i128::MAX;
        let a = Rational::new(m, 2).unwrap();
        let c = Rational::new(m, 3).unwrap();
        // Confirm the cross-multiply fast path WOULD overflow (so the fallback runs).
        assert!(
            a.numerator().checked_mul(c.denominator()).is_none()
                || c.numerator().checked_mul(a.denominator()).is_none(),
            "the test must actually exercise the overflow fallback"
        );
        assert!(a > c, "continued-fraction comparison must order MAX/2 > MAX/3 exactly");
        assert!(c < a);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);

        // Two large equal values compare Equal through the fallback as well.
        let big1 = Rational::new(m, 7).unwrap();
        let big2 = Rational::new(m, 7).unwrap();
        assert_eq!(big1.cmp(&big2), std::cmp::Ordering::Equal);

        // A large negative vs a large positive: sign dominates, still exact.
        let neg = Rational::new(-m, 5).unwrap();
        let pos = Rational::new(m, 5).unwrap();
        assert!(neg < pos);
    }

    /// `[PROP]` / policy — overflow is **detected, never silently wrapped**.
    #[test]
    fn overflow_is_detected_not_wrapped() {
        let big = Rational::from_int(i128::MAX);
        // i128::MAX + i128::MAX overflows: must be an Err, not a wrapped value.
        assert_eq!(big.checked_add(big), Err(RationalOverflow));
        // i128::MAX · 2 overflows.
        assert_eq!(big.checked_mul(Rational::from_int(2)), Err(RationalOverflow));
        // A denominator product that overflows.
        let tiny = Rational::new(1, i128::MAX).unwrap();
        assert_eq!(tiny.checked_mul(tiny), Err(RationalOverflow));
    }

    /// Division by zero and `new` with a zero denominator are hard failures, not
    /// fabricated values.
    #[test]
    fn division_by_zero_and_zero_denominator_fail() {
        assert_eq!(Rational::new(1, 0), Err(RationalOverflow));
        let a = Rational::from_int(3);
        assert_eq!(a.checked_div(Rational::ZERO), Err(RationalOverflow));
    }

    /// `i128::MIN` cannot be normalized (its magnitude is not representable), so
    /// `new` refuses it — better a detected failure than an arithmetic panic.
    #[test]
    fn i128_min_is_refused_by_new() {
        assert_eq!(Rational::new(i128::MIN, 1), Err(RationalOverflow));
        assert_eq!(Rational::new(1, i128::MIN), Err(RationalOverflow));
    }

    /// M·minor — `from_int` upholds the same `i128::MIN` guard `new` does, rather
    /// than silently bypassing it to construct an un-normalizable instance. The
    /// guard is a panic (the documented precondition) because `from_int` is
    /// infallible by contract and no in-tree caller passes `i128::MIN`.
    #[test]
    #[should_panic(expected = "unrepresentable")]
    fn from_int_min_is_guarded() {
        let _ = Rational::from_int(i128::MIN);
    }

    /// `from_int` is unaffected for every other value, including `i128::MIN + 1`
    /// (the smallest representable) and `i128::MAX`.
    #[test]
    fn from_int_admits_every_representable_integer() {
        assert_eq!(Rational::from_int(i128::MIN + 1).numerator(), i128::MIN + 1);
        assert_eq!(Rational::from_int(i128::MAX).numerator(), i128::MAX);
        assert_eq!(Rational::from_int(0), Rational::ZERO);
    }
}
