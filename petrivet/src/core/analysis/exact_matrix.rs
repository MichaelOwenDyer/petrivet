//! Exact linear algebra over [`Rational`] — the layer the checkable algebraic
//! certificates (Farkas duals, P/T-semiflows, exact rank) are built on.
//!
//! The incidence matrix `C` of a Petri net (|P| × |T|, `C[p][t]` = net token
//! change at `p` when `t` fires) carries the net's linear theory. Over `f64` that
//! theory is unsound: rank is *discontinuous* in the entries, and a
//! near-boundary LP feasibility result can be wrong. This module computes the
//! theory **exactly** over [`Rational`], so a verdict it produces is a
//! certificate, not a heuristic.
//!
//! # What it provides
//!
//! - [`Matrix::rank`] — the exact rank.
//! - [`Matrix::right_kernel`] — a basis of `{x : C·x = 0}` (the **T-semiflows**,
//!   over ℚ).
//! - [`Matrix::left_kernel`] — a basis of `{y : yᵀ·C = 0}` (the **P-semiflows**,
//!   over ℚ), via the right kernel of the transpose.
//! - [`Matrix::solve`] — a particular rational solution to `C·x = b`, or `None`
//!   if `b ∉ col(C)` (the system is infeasible over ℚ).
//! - [`Matrix::farkas_certificate`] — on an infeasible equality system
//!   `C·x = b`, an **exact** dual `y` with `yᵀ·C = 0` and `yᵀ·b ≠ 0`: a
//!   separating P-invariant, re-checkable by one exact dot product. This is the
//!   object the float LP computes and discards (`reachability.rs`), and the one
//!   that closes the silent false-`Unreachable` hole.
//!
//! # Exactness, and the Bareiss note
//!
//! Elimination is plain Gaussian elimination over `Rational`. Because `Rational`
//! is exact and **overflow-detecting**, the result is exact and any magnitude
//! blow-up is *caught* (returned as `None`/overflow), never silently wrong — the
//! property the verdicts built on this layer require. The fraction-free
//! **Bareiss** schedule (which keeps intermediate *integer* determinants
//! polynomially bounded by the Hadamard inequality) is a drop-in refinement of
//! the elimination order that changes neither the results nor the interface.
//! It is recorded as a follow-on: the requirement here is *exactness*, which
//! the `Rational` reduction delivers; making it *fast at scale* is the Bareiss
//! schedule, deferred until a measured cost need (the same discipline as
//! choosing `i128` over a bignum dependency).

// `dead_code`: several matrix primitives (`from_rows`, `from_ints`, `rank`,
// `mul_vector`, …) are built ahead of their consumers — the planned cluster/
// rank-theorem and S/T-component deciders — and are exercised by this module's
// tests now. Remove this allow as each gains a non-test consumer.
#![allow(dead_code)]

use super::rational::Rational;

/// A dense matrix over [`Rational`], row-major, `rows × cols`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    /// Row-major entries, length `rows * cols`.
    data: Vec<Rational>,
}

impl Matrix {
    /// A `rows × cols` zero matrix.
    #[must_use]
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![Rational::ZERO; rows * cols],
        }
    }

    /// Build a matrix from a row-major iterator of `Rational`s. Panics if the
    /// length is not `rows * cols` (a programming error, not runtime input).
    #[must_use]
    pub fn from_rows(rows: usize, cols: usize, data: Vec<Rational>) -> Self {
        assert_eq!(data.len(), rows * cols, "matrix data length must be rows*cols");
        Self { rows, cols, data }
    }

    /// Build from integer entries (the incidence matrix is integer).
    #[must_use]
    pub fn from_ints(rows: usize, cols: usize, entries: &[i128]) -> Self {
        assert_eq!(entries.len(), rows * cols, "entry length must be rows*cols");
        Self {
            rows,
            cols,
            data: entries.iter().map(|&n| Rational::from_int(n)).collect(),
        }
    }

    /// Number of rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// Entry at `(r, c)`.
    #[must_use]
    pub fn get(&self, r: usize, c: usize) -> Rational {
        self.data[r * self.cols + c]
    }

    /// Set entry `(r, c)`.
    fn set(&mut self, r: usize, c: usize, v: Rational) {
        self.data[r * self.cols + c] = v;
    }

    /// The transpose.
    #[must_use]
    pub fn transpose(&self) -> Self {
        let mut t = Self::zeros(self.cols, self.rows);
        for r in 0..self.rows {
            for c in 0..self.cols {
                t.set(c, r, self.get(r, c));
            }
        }
        t
    }

    /// Reduce `self` to row-echelon form over ℚ, returning the echelon matrix and
    /// the list of pivot columns (in row order). The reduced matrix is in
    /// *reduced* row-echelon form (each pivot is 1 and is the only non-zero entry
    /// in its column), so it serves both rank and kernel extraction.
    ///
    /// Returns `None` if any exact-arithmetic step overflows `i128` — the honest
    /// "cannot decide exactly" signal (a caller abstains rather than fabricates).
    fn reduced_row_echelon(&self) -> Option<(Self, Vec<usize>)> {
        let mut m = self.clone();
        let mut pivots = Vec::new();
        let mut pivot_row = 0;
        for col in 0..m.cols {
            if pivot_row >= m.rows {
                break;
            }
            // Find a row at or below pivot_row with a non-zero entry in `col`.
            let Some(sel) = (pivot_row..m.rows).find(|&r| !m.get(r, col).is_zero()) else {
                continue; // free column
            };
            m.swap_rows(pivot_row, sel);
            // Scale the pivot row so the pivot becomes 1.
            let pivot = m.get(pivot_row, col);
            for c in 0..m.cols {
                let v = m.get(pivot_row, c).checked_div(pivot).ok()?;
                m.set(pivot_row, c, v);
            }
            // Eliminate the pivot column from every other row.
            for r in 0..m.rows {
                if r == pivot_row {
                    continue;
                }
                let factor = m.get(r, col);
                if factor.is_zero() {
                    continue;
                }
                for c in 0..m.cols {
                    // row_r[c] -= factor * pivot_row[c]
                    let sub = factor.checked_mul(m.get(pivot_row, c)).ok()?;
                    let v = m.get(r, c).checked_sub(sub).ok()?;
                    m.set(r, c, v);
                }
            }
            pivots.push(col);
            pivot_row += 1;
        }
        Some((m, pivots))
    }

    fn swap_rows(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        for c in 0..self.cols {
            let tmp = self.get(a, c);
            self.set(a, c, self.get(b, c));
            self.set(b, c, tmp);
        }
    }

    /// The exact rank of the matrix, or `None` on exact-arithmetic overflow.
    #[must_use]
    pub fn rank(&self) -> Option<usize> {
        self.reduced_row_echelon().map(|(_, pivots)| pivots.len())
    }

    /// A basis of the **right kernel** `{x ∈ ℚ^cols : C·x = 0}` — the T-semiflows
    /// of an incidence matrix. Each basis vector has length `cols`. Returns the
    /// empty vector when the kernel is trivial (`{0}`), or `None` on overflow.
    #[must_use]
    pub fn right_kernel(&self) -> Option<Vec<Vec<Rational>>> {
        let (rref, pivots) = self.reduced_row_echelon()?;
        let pivot_set: std::collections::BTreeSet<usize> = pivots.iter().copied().collect();
        // Map each pivot column to the row that pivots it (row i pivots pivots[i]).
        let mut basis = Vec::new();
        for free_col in 0..self.cols {
            if pivot_set.contains(&free_col) {
                continue;
            }
            // Build the kernel vector setting this free variable to 1, the other
            // free variables to 0, and each pivot variable to the back-substituted
            // value −rref[pivot_row][free_col].
            let mut v = vec![Rational::ZERO; self.cols];
            v[free_col] = Rational::ONE;
            for (row, &pcol) in pivots.iter().enumerate() {
                // pivot variable x_pcol = − rref[row][free_col]
                v[pcol] = rref.get(row, free_col).checked_neg().ok()?;
            }
            basis.push(v);
        }
        Some(basis)
    }

    /// A basis of the **left kernel** `{y ∈ ℚ^rows : yᵀ·C = 0}` — the P-semiflows
    /// of an incidence matrix. Computed as the right kernel of the transpose.
    #[must_use]
    pub fn left_kernel(&self) -> Option<Vec<Vec<Rational>>> {
        self.transpose().right_kernel()
    }

    /// A particular rational solution `x` to `C·x = b` (here `self == C`,
    /// `b.len() == rows`), or `None` if the system is **infeasible over ℚ**
    /// (`b ∉ col(C)`), or on exact-arithmetic overflow (which is also reported as
    /// `None` — the conservative answer; the caller must not read `None` as
    /// "infeasible" without the [`Self::farkas_certificate`] exact witness).
    ///
    /// To distinguish infeasible from overflow, callers use
    /// [`Self::farkas_certificate`], which returns the *witness* of infeasibility
    /// (and `None` if the system is in fact feasible).
    #[must_use]
    pub fn solve(&self, b: &[Rational]) -> Option<Vec<Rational>> {
        assert_eq!(b.len(), self.rows, "b length must equal rows");
        // Augment [C | b] and reduce.
        let mut aug = Matrix::zeros(self.rows, self.cols + 1);
        for (r, &br) in b.iter().enumerate() {
            for c in 0..self.cols {
                aug.set(r, c, self.get(r, c));
            }
            aug.set(r, self.cols, br);
        }
        let (rref, pivots) = aug.reduced_row_echelon()?;
        // Infeasible iff the augmented column is a pivot column (a row 0…0 | k≠0).
        if pivots.contains(&self.cols) {
            return None;
        }
        // Back-substitute: free variables 0, pivot variable = rref[row][last].
        let mut x = vec![Rational::ZERO; self.cols];
        for (row, &pcol) in pivots.iter().enumerate() {
            if pcol < self.cols {
                x[pcol] = rref.get(row, self.cols);
            }
        }
        Some(x)
    }

    /// On an **infeasible** equality system `C·x = b` (here `self == C`,
    /// `b.len() == rows`), return an exact Farkas dual `y` with `yᵀ·C = 0` and
    /// `yᵀ·b ≠ 0` — a separating P-invariant, re-checkable by one exact dot
    /// product. Returns `None` if the system is **feasible** over ℚ (no separating
    /// invariant exists), or on exact-arithmetic overflow.
    ///
    /// This is the object that makes a negative reachability verdict a
    /// *certificate* rather than the absence of an f64 solution: the verdict
    /// returns only after `y·C = 0 ∧ y·(m'−m₀) ≠ 0` holds in exact arithmetic.
    ///
    /// # Method
    ///
    /// `b ∈ col(C)` over ℚ iff every left-kernel vector of `C` (every `y` with
    /// `yᵀ·C = 0`) also annihilates `b` (`yᵀ·b = 0`). So the system is
    /// **infeasible** exactly when some `y ∈ left_kernel(C)` has `yᵀ·b ≠ 0`, and
    /// that `y` *is* the separating P-invariant. We scan the left-kernel basis and
    /// return the first such `y`; if none separates, the system is feasible and we
    /// return `None`. (This is the standard duality: `b ∉ col(C) ⇔ ∃y ⊥ col(C),
    /// y·b ≠ 0` — the rows orthogonal to the column space that `b` is not
    /// orthogonal to.)
    #[must_use]
    pub fn farkas_certificate(&self, b: &[Rational]) -> Option<Vec<Rational>> {
        assert_eq!(b.len(), self.rows, "b length must equal rows");
        let left = self.left_kernel()?;
        for y in left {
            // yᵀ·b
            let mut dot = Rational::ZERO;
            for (yi, &bi) in y.iter().zip(b.iter()) {
                let prod = yi.checked_mul(bi).ok()?;
                dot = dot.checked_add(prod).ok()?;
            }
            if !dot.is_zero() {
                // Re-verify yᵀ·C = 0 exactly before returning. The basis is
                // constructed to satisfy it, but the certificate's contract is an
                // exact check, so we discharge it here too (defence in depth).
                if self.left_annihilates(&y)? {
                    return Some(y);
                }
            }
        }
        None
    }

    /// Whether `yᵀ · self == 0` exactly (every column's dot with `y` is zero).
    /// `y.len()` must equal `rows`. Returns `None` on overflow.
    fn left_annihilates(&self, y: &[Rational]) -> Option<bool> {
        for c in 0..self.cols {
            let mut dot = Rational::ZERO;
            for (r, &yi) in y.iter().enumerate() {
                let prod = yi.checked_mul(self.get(r, c)).ok()?;
                dot = dot.checked_add(prod).ok()?;
            }
            if !dot.is_zero() {
                return Some(false);
            }
        }
        Some(true)
    }

    /// Compute `C · x` (here `self == C`, `x.len() == cols`), returning a vector
    /// of length `rows`, or `None` on overflow. A convenience for checking a
    /// returned kernel/solution exactly.
    #[must_use]
    pub fn mul_vector(&self, x: &[Rational]) -> Option<Vec<Rational>> {
        assert_eq!(x.len(), self.cols, "x length must equal cols");
        let mut out = vec![Rational::ZERO; self.rows];
        for (r, slot) in out.iter_mut().enumerate() {
            let mut acc = Rational::ZERO;
            for (c, &xc) in x.iter().enumerate() {
                let prod = self.get(r, c).checked_mul(xc).ok()?;
                acc = acc.checked_add(prod).ok()?;
            }
            *slot = acc;
        }
        Some(out)
    }
}

/// Build the exact incidence matrix `C` (|P| × |T|) of a net from its
/// per-transition pre/post sets — all arcs weight-1, so `C[p][t]` is
/// `+1` if `p ∈ t•`, `−1` if `p ∈ •t`, `0` otherwise (a self-loop nets to 0).
/// This mirrors [`IncidenceMatrix`](super::incidence::IncidenceMatrix) but over
/// `Rational`, for the exact layer.
#[must_use]
pub fn incidence_over_rationals(net: &crate::core::net::DenseNet) -> Matrix {
    let p = net.place_count();
    let t = net.transition_count();
    let mut m = Matrix::zeros(p, t);
    for ti in net.transition_indices() {
        for &pi in &net.preset_t[ti] {
            // C[pi][ti] -= 1
            let v = m.get(pi, ti).checked_sub(Rational::ONE).expect("unit arc cannot overflow");
            m.set(pi, ti, v);
        }
        for &pi in &net.postset_t[ti] {
            let v = m.get(pi, ti).checked_add(Rational::ONE).expect("unit arc cannot overflow");
            m.set(pi, ti, v);
        }
    }
    m
}

/// The exact-arithmetic outcome of the marking-equation **equality** system
/// `C·x = m' − m₀` over ℚ (sign-free). It re-derives the infeasibility that a
/// floating-point LP can only *suggest*, and supplies the exact witness when it
/// confirms it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkingEquationExact {
    /// The equality system is **feasible** over ℚ (`m'−m₀ ∈ col(C)`). A negative
    /// reachability verdict must **not** be returned on the strength of an `f64`
    /// "infeasible" here — escalate to the integer/state-space path. (A spurious
    /// `f64` infeasibility at a degenerate vertex lands here, and is thereby
    /// caught instead of becoming a false `Unreachable`.)
    Feasible,
    /// The equality system is **infeasible** over ℚ, exactly certified by the
    /// separating P-invariant `y`: `yᵀ·C = 0 ∧ yᵀ·(m'−m₀) ≠ 0`. A negative
    /// reachability verdict is sound here, carrying `y` as its certificate.
    Infeasible { separating_invariant: Vec<Rational> },
    /// Exact arithmetic overflowed `i128` (the net is too large for the `i128`
    /// kernel). The honest "cannot decide exactly" outcome: the caller must not
    /// fabricate a verdict — escalate, do not return `Unreachable`.
    Overflowed,
}

/// Re-derive, over ℚ, whether the marking-equation equality system
/// `C·x = m' − m₀` is feasible, and if not, produce the exact separating
/// P-invariant. `initial` and `target` are dense markings in place-index order.
///
/// This is sign-free (it ignores the `x ≥ 0` firing-count constraint), so an
/// `Infeasible` result is a *sufficient* exact certificate of unreachability:
/// `m'−m₀ ∉ col(C)` ⇒ no firing-count vector (integer, non-negative, or
/// otherwise) solves the equation. A `Feasible` result does **not** confirm
/// reachability (it ignores signs and firing order) — it only forbids returning
/// the cheap `Unreachable(MarkingEquationNoRationalSolution)` verdict, closing
/// the silent false-`Unreachable` hole.
#[must_use]
pub fn marking_equation_exact(
    net: &crate::core::net::DenseNet,
    initial: &crate::core::marking::IdxMarking<u32>,
    target: &crate::core::marking::IdxMarking<u32>,
) -> MarkingEquationExact {
    let c = incidence_over_rationals(net);
    // b = target − initial, place-index order.
    let init = initial.as_ref();
    let targ = target.as_ref();
    debug_assert_eq!(init.len(), c.rows());
    debug_assert_eq!(targ.len(), c.rows());
    let mut b = Vec::with_capacity(c.rows());
    for p in 0..c.rows() {
        let diff = i128::from(targ[p]) - i128::from(init[p]);
        b.push(Rational::from_int(diff));
    }
    // Feasible iff b ∈ col(C). `solve` returns None on both infeasible AND
    // overflow, so disambiguate via `farkas_certificate` (which returns the
    // witness on genuine infeasibility, None on feasible-or-overflow).
    match c.farkas_certificate(&b) {
        Some(y) => MarkingEquationExact::Infeasible { separating_invariant: y },
        // No separating invariant: either genuinely feasible (an exact solution
        // exists) or the i128 kernel overflowed (cannot decide → escalate, never
        // fabricate).
        None if c.solve(&b).is_some() => MarkingEquationExact::Feasible,
        None => MarkingEquationExact::Overflowed,
    }
}

/// The exact-arithmetic outcome of an attempt to *certify uncoverability* via a
/// place sub-invariant. This is the covering analogue of
/// [`MarkingEquationExact`]: a sound, *sufficient*, exact certificate of
/// uncoverability, so the `Uncoverable` verdict never rests on a solver merely
/// failing to find a solution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoveringInvariantExact {
    /// An exact, non-negative place sub-invariant `y` proves the target is
    /// **uncoverable**: `y ≥ 0`, `yᵀ·C ≤ 0`, and `yᵀ·target > yᵀ·M₀`. The weighted
    /// token count `y·M` cannot increase across any firing (`yᵀ·C ≤ 0`), so no
    /// reachable `M` can reach the higher weight `target` requires — a fortiori
    /// none can *cover* it. Re-checkable by one exact dot product per column.
    Uncoverable { sub_invariant: Vec<Rational> },
    /// No such sub-invariant was found among the exact P-semiflow basis (and its
    /// negations). This does **not** prove coverability — it only forbids the
    /// cheap structural `Uncoverable` verdict; the caller must escalate to the
    /// exact coverability graph, never fabricate.
    NotCertified,
    /// Exact arithmetic overflowed `i128` while computing the P-semiflow basis. The
    /// honest "cannot decide exactly" outcome — escalate, never fabricate.
    Overflowed,
}

/// Try to certify, over ℚ, that `target` is **uncoverable** from `initial` using a
/// non-negative place sub-invariant (a conservation/decreasing law). `initial` and
/// `target` are dense markings in place-index order.
///
/// # Soundness
///
/// If `y ≥ 0` satisfies `yᵀ·C ≤ 0` and `yᵀ·target > yᵀ·initial`, then `target` is
/// uncoverable: every reachable marking `M` has `y·M ≤ y·initial` (the weighted
/// count never increases), while covering `target` would require `y·M ≥ y·target >
/// y·initial`. This is a *sufficient* condition; absence of such a `y` (the
/// `NotCertified` arm) proves nothing and must escalate.
///
/// # Method (exact, no `f64`)
///
/// The P-semiflows `{y : yᵀ·C = 0}` are the left kernel of `C`. Every P-semiflow
/// is in particular a sub-invariant (`yᵀ·C = 0 ≤ 0`). We scan the exact left-kernel
/// basis and each basis vector's negation for one that is non-negative and
/// separates `target` from `initial`. (Scanning the basis ± is sound but not
/// complete — a separating sub-invariant that is only a *non-trivial* non-negative
/// combination is missed; that case escalates to the coverability graph, which is
/// exact. A complete treatment — exact covering-LP duality over ℚ — is a recorded
/// follow-on; soundness here does not depend on completeness.)
#[must_use]
pub fn covering_invariant_exact(
    net: &crate::core::net::DenseNet,
    initial: &crate::core::marking::IdxMarking<u32>,
    target: &crate::core::marking::IdxMarking<u32>,
) -> CoveringInvariantExact {
    let c = incidence_over_rationals(net);
    let init = initial.as_ref();
    let targ = target.as_ref();
    debug_assert_eq!(init.len(), c.rows());
    debug_assert_eq!(targ.len(), c.rows());
    let Some(basis) = c.left_kernel() else {
        return CoveringInvariantExact::Overflowed;
    };
    // `b[p] = target[p] − initial[p]`; a separating y has yᵀ·b > 0.
    let mut b = Vec::with_capacity(c.rows());
    for p in 0..c.rows() {
        b.push(Rational::from_int(i128::from(targ[p]) - i128::from(init[p])));
    }
    // Try each basis vector and its negation: accept the first that is
    // non-negative everywhere and has yᵀ·b > 0 (strictly separating).
    for y in basis {
        for signed in [Some(y.clone()), negate_vector(&y)] {
            let Some(signed) = signed else { continue };
            if !signed.iter().all(|v| *v >= Rational::ZERO) {
                continue; // y must be ≥ 0 to be a covering sub-invariant
            }
            // yᵀ·b, exactly.
            let mut dot = Rational::ZERO;
            let mut overflowed = false;
            for (yi, bi) in signed.iter().zip(b.iter()) {
                let Ok(acc) = yi.checked_mul(*bi).and_then(|p| dot.checked_add(p)) else {
                    overflowed = true;
                    break;
                };
                dot = acc;
            }
            if overflowed {
                continue;
            }
            if dot > Rational::ZERO {
                // y ∈ ker(Cᵀ) ⇒ yᵀ·C = 0 ≤ 0 (a sub-invariant), y ≥ 0, yᵀ·b > 0:
                // a sound exact uncoverability certificate.
                return CoveringInvariantExact::Uncoverable { sub_invariant: signed };
            }
        }
    }
    CoveringInvariantExact::NotCertified
}

/// Negate every entry of a rational vector, or `None` on overflow (only at the
/// unrepresentable `i128::MIN` numerator, which `Rational::new` already refuses).
fn negate_vector(v: &[Rational]) -> Option<Vec<Rational>> {
    v.iter().map(|x| x.checked_neg().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        incidence_over_rationals, marking_equation_exact, Matrix, MarkingEquationExact,
    };
    use crate::core::analysis::rational::Rational;
    use crate::prelude::NetBuilder;

    fn r(n: i128) -> Rational {
        Rational::from_int(n)
    }

    fn rat(n: i128, d: i128) -> Rational {
        Rational::new(n, d).unwrap()
    }

    /// `[PROP]` — rank–nullity: for an m×n matrix, `rank + dim(right_kernel) == n`.
    #[test]
    fn rank_nullity_holds() {
        // A 2×3 matrix of rank 2: nullity must be 1.
        // [1 0 2]
        // [0 1 3]
        let m = Matrix::from_ints(2, 3, &[1, 0, 2, 0, 1, 3]);
        let rank = m.rank().unwrap();
        let kernel = m.right_kernel().unwrap();
        assert_eq!(rank, 2);
        assert_eq!(rank + kernel.len(), m.cols(), "rank + nullity == cols");
        assert_eq!(kernel.len(), 1);
    }

    /// `[PROP]` — `C·k == 0` exactly for every kernel basis vector.
    #[test]
    fn kernel_vectors_are_annihilated_exactly() {
        let m = Matrix::from_ints(2, 3, &[1, 0, 2, 0, 1, 3]);
        for k in m.right_kernel().unwrap() {
            let prod = m.mul_vector(&k).unwrap();
            assert!(prod.iter().all(|x| x.is_zero()), "C·k must be exactly 0");
        }
    }

    /// `[PROP]` — `yᵀ·C == 0` exactly for every left-kernel basis vector.
    #[test]
    fn left_kernel_vectors_annihilate_columns_exactly() {
        // A 3×2 matrix of rank 2 → left kernel dim 1.
        // [1 0]
        // [0 1]
        // [2 3]
        let m = Matrix::from_ints(3, 2, &[1, 0, 0, 1, 2, 3]);
        let left = m.left_kernel().unwrap();
        assert_eq!(left.len(), 1, "left nullity = rows − rank = 3 − 2 = 1");
        for y in left {
            // yᵀ·C: each column dotted with y is zero.
            for c in 0..m.cols() {
                let mut dot = Rational::ZERO;
                for (rr, &yi) in y.iter().enumerate() {
                    dot = dot.checked_add(yi.checked_mul(m.get(rr, c)).unwrap()).unwrap();
                }
                assert!(dot.is_zero(), "yᵀ·C must be exactly 0");
            }
        }
    }

    /// `solve` returns an exact particular solution when feasible.
    #[test]
    fn solve_returns_exact_particular_solution() {
        // C = [1 0; 0 1], b = [3; 4] → x = [3; 4].
        let c = Matrix::from_ints(2, 2, &[1, 0, 0, 1]);
        let x = c.solve(&[r(3), r(4)]).unwrap();
        assert_eq!(x, vec![r(3), r(4)]);
        // Verify C·x == b exactly.
        assert_eq!(c.mul_vector(&x).unwrap(), vec![r(3), r(4)]);

        // A fractional exact solution: C = [2 0; 0 3], b = [1; 1] → x = [1/2; 1/3].
        let c2 = Matrix::from_ints(2, 2, &[2, 0, 0, 3]);
        let x2 = c2.solve(&[r(1), r(1)]).unwrap();
        assert_eq!(x2, vec![rat(1, 2), rat(1, 3)]);
    }

    /// `[PROP]` — Farkas duality is exact: on an infeasible equality system the
    /// returned `y` satisfies `yᵀ·C == 0` and `yᵀ·b ≠ 0`, both exactly; on a
    /// feasible system no certificate is returned.
    #[test]
    fn farkas_certificate_is_exact_on_infeasible_and_absent_on_feasible() {
        // Infeasible: C = [1; 1] (2×1), b = [1; 2]. col(C) = span{(1,1)}; (1,2)
        // is not in it. A separating y with yᵀ·C = 0 is (1, −1): yᵀ·C = 1−1 = 0,
        // yᵀ·b = 1−2 = −1 ≠ 0.
        let c = Matrix::from_ints(2, 1, &[1, 1]);
        let b = vec![r(1), r(2)];
        // No rational solution exists.
        assert!(c.solve(&b).is_none(), "the system is infeasible over ℚ");
        let y = c.farkas_certificate(&b).expect("an exact separating invariant exists");
        // yᵀ·C == 0 exactly.
        let mut yc = Rational::ZERO;
        for (rr, &yi) in y.iter().enumerate() {
            yc = yc.checked_add(yi.checked_mul(c.get(rr, 0)).unwrap()).unwrap();
        }
        assert!(yc.is_zero(), "yᵀ·C must be exactly 0");
        // yᵀ·b ≠ 0 exactly.
        let mut yb = Rational::ZERO;
        for (yi, &bi) in y.iter().zip(b.iter()) {
            yb = yb.checked_add(yi.checked_mul(bi).unwrap()).unwrap();
        }
        assert!(!yb.is_zero(), "yᵀ·b must be non-zero (the separation)");

        // Feasible: same C, b = [3; 3] ∈ col(C). No certificate.
        let feasible_b = vec![r(3), r(3)];
        assert!(c.solve(&feasible_b).is_some(), "feasible: a solution exists");
        assert!(
            c.farkas_certificate(&feasible_b).is_none(),
            "a feasible system has no separating invariant"
        );
    }

    /// `marking_equation_exact` reports `Infeasible` with an exact separating
    /// invariant for a token-non-conserving target on a conservative net, and
    /// `Feasible` for a reachable target — so a genuinely-reachable target is
    /// never reported infeasible.
    #[test]
    fn marking_equation_exact_guard_separates_infeasible_from_feasible() {
        // The two-place cycle conserves tokens (P-semiflow (1,1)). Target
        // {p0:2} has token sum 2 ≠ 1 = sum(m0={p0:1}): the equality system is
        // infeasible, and (1,1) separates it (yᵀ·C=0, yᵀ·b = 2−1 = 1 ≠ 0... here
        // b = target−m0 = (1, −1) in some order; the separating invariant exists).
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap();
        let m0 = net.mapping.decode([(p0, 1)].into());

        // Infeasible target: a different token sum.
        let bad = net.mapping.decode([(p0, 2)].into());
        match marking_equation_exact(&net.dense_net, &m0, &bad) {
            MarkingEquationExact::Infeasible { separating_invariant } => {
                // Verify the certificate exactly: yᵀ·C = 0.
                let c = incidence_over_rationals(&net.dense_net);
                for t in 0..c.cols() {
                    let mut dot = Rational::ZERO;
                    for (p, &yi) in separating_invariant.iter().enumerate() {
                        dot = dot
                            .checked_add(yi.checked_mul(c.get(p, t)).unwrap())
                            .unwrap();
                    }
                    assert!(dot.is_zero(), "separating invariant must satisfy yᵀ·C = 0");
                }
            }
            other => panic!("expected Infeasible with a certificate, got {other:?}"),
        }

        // Feasible target: (p1,1) is reachable, same token sum — must be Feasible,
        // NOT infeasible (a reachable target must never
        // reported infeasible).
        let good = net.mapping.decode([(p1, 1)].into());
        assert_eq!(
            marking_equation_exact(&net.dense_net, &m0, &good),
            MarkingEquationExact::Feasible,
            "a reachable target's equality system must be exactly feasible"
        );
    }

    /// The exact incidence matrix matches the i32 `IncidenceMatrix` entry-wise,
    /// and a P-semiflow of a conservative net (the two-place cycle) is in the
    /// left kernel: y = (1, 1) gives yᵀ·C = 0 (token conservation).
    #[test]
    fn incidence_over_rationals_matches_and_yields_p_semiflow() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap();

        let exact = incidence_over_rationals(&net.dense_net);
        let i32mat = net.dense_net.incidence_matrix();
        for p in 0..exact.rows() {
            for t in 0..exact.cols() {
                assert_eq!(
                    exact.get(p, t),
                    Rational::from_int(i128::from(i32mat.get(p, t))),
                    "exact incidence must match the i32 incidence at ({p}, {t})"
                );
            }
        }

        // The cycle conserves tokens: the all-ones P-weighting is a P-semiflow.
        let left = exact.left_kernel().unwrap();
        assert!(!left.is_empty(), "a conservative net has a non-trivial P-semiflow");
        // Every column dotted with each basis vector is zero.
        for y in &left {
            for t in 0..exact.cols() {
                let mut dot = Rational::ZERO;
                for (p, &yi) in y.iter().enumerate() {
                    dot = dot.checked_add(yi.checked_mul(exact.get(p, t)).unwrap()).unwrap();
                }
                assert!(dot.is_zero());
            }
        }
    }
}
