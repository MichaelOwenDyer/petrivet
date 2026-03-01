#![allow(clippy::cast_precision_loss)]
//! Structural analysis: invariants, siphons, and traps.
//!
//! These properties depend only on the net topology, not on any particular
//! marking. They provide necessary conditions for behavioral properties like
//! boundedness and liveness.
//!
//! # Overview
//!
//! | Technique | What it tells you | Typical application |
//! |---|---|---|
//! | S-invariants | Token conservation laws | Verify resource counts are preserved |
//! | T-invariants | Reproducible firing sequences | Verify cyclic workflows complete |
//! | Siphons | Sets that can empty permanently | Diagnose potential deadlocks |
//! | Traps | Sets that stay marked forever | Prove liveness via Commoner's theorem |
//!
//! # Example
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::analysis::structural;
//!
//! let mut b = NetBuilder::new();
//! let [p0, p1] = b.add_places();
//! let [t0, t1] = b.add_transitions();
//! b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! let net = b.build().unwrap();
//!
//! let inv = structural::compute_invariants(&net);
//! // One S-invariant: tokens are conserved (p0 + p1 = const)
//! assert_eq!(inv.s_invariants.len(), 1);
//! assert!(inv.is_covered_by_s_invariants(net.n_places()));
//!
//! // One minimal siphon = one minimal trap = {p0, p1}
//! let siphons = structural::minimal_siphons(&net);
//! assert_eq!(siphons.len(), 1);
//! ```

use crate::analysis::math::integer_null_space;
use crate::marking::Marking;
use crate::net::{Net, Place, Transition};
use std::collections::HashSet;
use std::fmt;

/// The incidence matrix N of a Petri net.
///
/// A |P| × |T| matrix stored in row-major order (Primer convention).
/// Entry N\[p\]\[t\] is the net token change at place p when transition t fires:
/// +1 if t produces to p, -1 if t consumes from p, 0 otherwise.
///
/// With this convention the state equation reads M' = M₀ + N · x directly,
/// where x is the |T|×1 firing count vector.
///
/// References:
/// - Petri Net Primer (Best & Devillers), Definition 4.1
/// - Murata 1989, §IV-B (uses the transposed convention; our N = Murata's Aᵀ)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidenceMatrix {
    data: Vec<i32>,
    rows: usize,
    cols: usize,
}

impl IncidenceMatrix {
    /// Constructs the |P| × |T| incidence matrix for a given net.
    #[must_use]
    pub fn new(net: &Net) -> Self {
        let rows = net.n_places();
        let cols = net.n_transitions();
        let mut data = vec![0; rows * cols];
        for t in net.transitions() {
            for &p in net.preset_t(t) {
                data[p.idx * cols + t.idx] -= 1;
            }
            for &p in net.postset_t(t) {
                data[p.idx * cols + t.idx] += 1;
            }
        }
        IncidenceMatrix { data, rows, cols }
    }

    /// Constructs an incidence matrix from raw data in row-major order.
    #[must_use]
    pub fn from_raw(data: Vec<i32>, rows: usize, cols: usize) -> Self {
        debug_assert_eq!(data.len(), rows * cols);
        Self { data, rows, cols }
    }

    /// Number of rows (places).
    #[must_use]
    pub fn n_rows(&self) -> usize {
        self.rows
    }

    /// Number of columns (transitions).
    #[must_use]
    pub fn n_cols(&self) -> usize {
        self.cols
    }

    /// Entry at (row, col) = N\[place\]\[transition\].
    #[must_use]
    pub fn get(&self, row: Place, col: Transition) -> i32 {
        self.data[row.idx * self.cols + col.idx]
    }

    /// Row slice for a given place.
    #[must_use]
    pub fn row(&self, p: Place) -> &[i32] {
        let start = p.idx * self.cols;
        &self.data[start..start + self.cols]
    }

    /// Returns a column vector (extracting one transition across all places).
    #[must_use]
    pub fn col(&self, t: Transition) -> Vec<i32> {
        (0..self.rows).map(|p| self.data[p * self.cols + t.idx]).collect()
    }

    /// Returns the transpose (|T| × |P| matrix).
    #[must_use]
    pub fn transpose(&self) -> Self {
        let mut data = vec![0; self.rows * self.cols];
        for r in 0..self.rows {
            for c in 0..self.cols {
                data[c * self.rows + r] = self.data[r * self.cols + c];
            }
        }
        Self { data, rows: self.cols, cols: self.rows }
    }
}

impl fmt::Display for IncidenceMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for idx in 0..self.rows {
            write!(f, "[")?;
            for (t, val) in self.row(Place { idx }).iter().enumerate() {
                if t > 0 { write!(f, ", ")?; }
                write!(f, "{val:>3}")?;
            }
            writeln!(f, "]")?;
        }
        Ok(())
    }
}

/// Result of structural invariant analysis on a net.
#[derive(Debug, Clone)]
pub struct Invariants {
    /// Basis vectors for the place invariants of the net.
    /// A place invariant is a vector I ∈ ℤ^|P| such that Nᵀ · I = 0.
    /// Each vector has length |P|. An S-invariant y satisfies y^T · N = 0,
    /// meaning the weighted token sum y · m is conserved across all firings.
    pub s_invariants: Box<[Box<[i32]>]>,

    /// Basis vectors for the transition invariants of the net.
    /// Each inner slice has length |T|. A T-invariant x satisfies N · x = 0,
    /// meaning firing the multiset of transitions x has no net effect on the marking.
    pub t_invariants: Box<[Box<[i32]>]>,
}

impl Invariants {
    /// Whether every place is covered by a non-negative S-invariant.
    ///
    /// A place p is covered if there exists a non-negative linear combination
    /// of the S-invariant basis that yields a vector with a strictly positive
    /// entry at p (and non-negative everywhere). Checked via LP.
    ///
    /// If true, the net is **conservative**: there exists a positive
    /// weight vector y > 0 such that the weighted token sum y · M is the
    /// same for every reachable marking M. Conservativeness implies
    /// structural boundedness, but is strictly stronger.
    ///
    /// **Property hierarchy** (each implies the next):
    /// 1. S-invariant coverage (this check) → **conservativeness**
    /// 2. Conservativeness → **structural boundedness**
    ///    (every initial marking yields a bounded system)
    /// 3. Structural boundedness → **behavioral boundedness** under any M₀
    ///
    /// The structural boundedness LP in
    /// [`is_structurally_bounded`](super::semi_decision::is_structurally_bounded)
    /// checks condition 2 directly (finds y > 0 with yᵀ · N ≤ 0, which
    /// allows *non-strict* inequality — a weaker requirement). A net can
    /// be structurally bounded without being conservative if some places
    /// have no positive S-invariant but are still bounded due to the net
    /// topology constraining token flow.
    ///
    /// References:
    /// - Murata 1989, §VII: S-invariant coverage and conservativeness
    /// - Petri Net Primer, Proposition 4.12 (structural boundedness via LP)
    #[must_use]
    pub fn is_covered_by_s_invariants(&self, n_places: usize) -> bool {
        Self::is_covered(&self.s_invariants, n_places)
    }

    /// Whether every transition is covered by a non-negative T-invariant.
    #[must_use]
    pub fn is_covered_by_t_invariants(&self, n_transitions: usize) -> bool {
        Self::is_covered(&self.t_invariants, n_transitions)
    }

    /// Single-LP coverage check: does there exist a non-negative linear
    /// combination of the basis vectors that is strictly positive at every
    /// index? Solves one LP with k variables and n constraints instead of
    /// n separate LPs.
    ///
    /// Formulation: find λ₁..λₖ (unrestricted in sign) such that for every
    /// index j, Σᵢ λᵢ · basis[i][j] ≥ 1. Lambda values must be unrestricted
    /// because a positive vector in the null space may require negative
    /// coefficients over the integer basis produced by Bareiss elimination.
    fn is_covered(basis: &[Box<[i32]>], n: usize) -> bool {
        use good_lp::{constraint, variable, Expression, ProblemVariables, SolverModel};

        if basis.is_empty() {
            return n == 0;
        }

        let mut vars = ProblemVariables::new();
        let lambda: Vec<_> = (0..basis.len())
            .map(|_| vars.add(variable()))
            .collect();

        let constraints = (0..n)
            .map(|j| {
                let val: Expression = lambda.iter()
                    .enumerate()
                    .map(|(i, &l)| f64::from(basis[i][j]) * l)
                    .sum();
                constraint!(val >= 1.0)
            });

        vars
            .minimise(Expression::from(0.0))
            .using(good_lp::microlp)
            .with_all(constraints)
            .solve()
            .is_ok()
    }
}

/// Computes the S-invariants and T-invariants of a net.
///
/// With incidence matrix N = |P|×|T|:
/// - S-invariants: vectors y ∈ ℤ^|P| satisfying Nᵀ · y = 0.
///   These are place invariants: the weighted token sum y · M is conserved
///   across all transition firings.
/// - T-invariants: vectors x ∈ ℤ^|T| satisfying N · x = 0.
///   These are transition invariants: firing the multiset x returns the
///   marking to its original value.
///
/// S-invariants encode conservation laws (e.g. "idle + busy = const" in a
/// resource model). T-invariants identify reproducible workflows (e.g. a
/// complete manufacturing cycle that returns the system to its original state).
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::analysis::structural::compute_invariants;
///
/// // Mutex net: 7 places, 6 transitions
/// let mut b = NetBuilder::new();
/// let [idle1, wait1, crit1] = b.add_places();
/// let [idle2, wait2, crit2] = b.add_places();
/// let mutex = b.add_place();
/// let [req1, enter1, exit1] = b.add_transitions();
/// let [req2, enter2, exit2] = b.add_transitions();
/// b.add_arc((idle1, req1)); b.add_arc((req1, wait1));
/// b.add_arc((wait1, enter1)); b.add_arc((enter1, crit1));
/// b.add_arc((crit1, exit1)); b.add_arc((exit1, idle1));
/// b.add_arc((idle2, req2)); b.add_arc((req2, wait2));
/// b.add_arc((wait2, enter2)); b.add_arc((enter2, crit2));
/// b.add_arc((crit2, exit2)); b.add_arc((exit2, idle2));
/// b.add_arc((mutex, enter1)); b.add_arc((exit1, mutex));
/// b.add_arc((mutex, enter2)); b.add_arc((exit2, mutex));
/// let net = b.build().unwrap();
///
/// let inv = compute_invariants(&net);
/// // 3 S-invariants: idle1+wait1+crit1, idle2+wait2+crit2, crit1+crit2+mutex
/// assert_eq!(inv.s_invariants.len(), 3);
/// assert!(inv.is_covered_by_s_invariants(net.n_places()));
/// // 2 T-invariants: complete cycle for each process
/// assert_eq!(inv.t_invariants.len(), 2);
/// ```
///
/// References:
/// - Murata 1989, §VII (invariant analysis)
/// - Petri Net Primer, §4.3 (S-invariants and T-invariants)
#[must_use]
pub fn compute_invariants(net: &Net) -> Invariants {
    let n = net.incidence_matrix();
    // S-invariants: null space of Nᵀ (|T|×|P| matrix → |P|-dimensional vectors),
    let nt = n.transpose();
    let s_invariants = integer_null_space(&nt);
    // T-invariants: null space of N (|P|×|T| matrix → |T|-dimensional vectors)
    let t_invariants = integer_null_space(&n);
    Invariants { s_invariants, t_invariants }
}

/// Computes the maximal siphon contained in a given set of places.
///
/// A siphon is a set of places D such that •D ⊆ D•: every transition that
/// produces into D also consumes from D. Once empty, it stays empty forever.
///
/// Uses the shrinking algorithm from the Petri Net Primer (Algorithm 6.19):
/// iteratively remove any place p where some transition t ∈ •p has no
/// input place in the current set. Runs in O(|S|² · |T|²).
#[must_use]
pub fn maximal_siphon_in(
    net: &Net,
    subset: &HashSet<Place>
) -> HashSet<Place> {
    let mut d: HashSet<Place> = subset.clone();
    loop {
        let mut removed = false;
        let to_remove: Vec<Place> = d.iter().copied().filter(|&p| {
            // Check if some t ∈ •p has no input place in D.
            net.preset_p(p).iter().any(|&t| {
                // t ∈ •p. For the siphon property, we need t ∈ D•,
                // i.e. t consumes from some place in D.
                // If it doesn't, then p cannot be in the siphon.
                net.preset_t(t).iter().all(|&q| !d.contains(&q))
            })
        }).collect();
        for p in to_remove {
            d.remove(&p);
            removed = true;
        }
        if !removed {
            break;
        }
    }
    d
}

/// Finds all minimal siphons of a net using backtracking.
///
/// A siphon is a set of places D where •D ⊆ D•: every transition that
/// outputs into D also has an input from D. Once all places in a siphon
/// become empty, they stay empty forever — a potential deadlock cause.
///
/// Starts by computing the maximal siphon (all places), then recursively
/// tries excluding each place to find smaller siphons. Results are filtered
/// to keep only minimal ones.
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::analysis::structural::minimal_siphons;
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap();
///
/// let siphons = minimal_siphons(&net);
/// // In a cycle, {p0, p1} is the only minimal siphon
/// assert_eq!(siphons.len(), 1);
/// assert_eq!(siphons[0].len(), 2);
/// ```
#[must_use]
pub fn minimal_siphons(net: &Net) -> Vec<HashSet<Place>> {
    let all_places: HashSet<Place> = net.places().collect();
    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut stack: Vec<HashSet<Place>> = vec![all_places];
    let mut visited: HashSet<Vec<usize>> = HashSet::new();

    while let Some(candidate_set) = stack.pop() {
        let siphon = maximal_siphon_in(net, &candidate_set);
        if siphon.is_empty() {
            continue;
        }

        let mut key: Vec<usize> = siphon.iter().map(|p| p.idx).collect();
        key.sort_unstable();
        if !visited.insert(key) {
            continue;
        }

        // Try excluding each place to find potentially smaller siphons.
        let mut is_minimal = true;
        for &p in &siphon {
            let mut reduced = siphon.clone();
            reduced.remove(&p);
            if reduced.is_empty() {
                continue;
            }
            let sub = maximal_siphon_in(net, &reduced);
            if !sub.is_empty() {
                is_minimal = false;
                stack.push(reduced);
            }
        }

        if is_minimal {
            let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
            if !dominated {
                results.retain(|existing| !siphon.is_subset(existing));
                results.push(siphon);
            }
        }
    }

    results
}

/// Computes the maximal trap contained in a given set of places.
///
/// A trap Q satisfies Q• ⊆ •Q: every transition that consumes from Q also
/// produces into Q. Once marked, a trap stays marked forever.
///
/// Uses the dual of the shrinking algorithm: iteratively remove any place p
/// where some transition t ∈ p• has no output place in the current set.
#[must_use]
pub fn maximal_trap_in(
    net: &Net,
    subset: &HashSet<Place>
) -> HashSet<Place> {
    let mut maximal_trap = subset.clone();
    loop {
        let mut removed = false;
        let to_remove: Vec<Place> = maximal_trap
            .iter()
            .filter(|&&p| {
                // Check if some t ∈ p• has no output place in Q.
                // p• = transitions that consume from p = postset_p(p)
                net.postset_p(p).iter().any(|&t| {
                    // t ∈ p•. For the trap property, we need t ∈ •Q,
                    // i.e. t produces into some place in Q.
                    // t• = postset_t(t) = output places of t.
                    !net.postset_t(t).iter().any(|&r| maximal_trap.contains(&r))
                })
            })
            .copied()
            .collect();
        for p in to_remove {
            maximal_trap.remove(&p);
            removed = true;
        }
        if !removed {
            break;
        }
    }
    maximal_trap
}

/// Finds all minimal traps of a net using backtracking.
#[must_use]
pub fn minimal_traps(net: &Net) -> Vec<HashSet<Place>> {
    let all_places: HashSet<Place> = net.places().collect();
    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut stack: Vec<HashSet<Place>> = vec![all_places];
    let mut visited: HashSet<Vec<usize>> = HashSet::new();

    while let Some(candidate_set) = stack.pop() {
        let trap = maximal_trap_in(net, &candidate_set);
        if trap.is_empty() {
            continue;
        }

        let mut key: Vec<usize> = trap.iter().map(|p| p.idx).collect();
        key.sort_unstable();
        if !visited.insert(key) {
            continue;
        }

        let mut is_minimal = true;
        for &p in &trap {
            let mut reduced = trap.clone();
            reduced.remove(&p);
            if reduced.is_empty() {
                continue;
            }
            let sub = maximal_trap_in(net, &reduced);
            if !sub.is_empty() {
                is_minimal = false;
                stack.push(reduced);
            }
        }

        if is_minimal {
            let dominated = results.iter().any(|existing| existing.is_subset(&trap));
            if !dominated {
                results.retain(|existing| !trap.is_subset(existing));
                results.push(trap);
            }
        }
    }

    results
}

/// Finds all minimal siphons using ILP enumeration.
///
/// Encodes the siphon property as binary constraints and iteratively
/// solves for minimum-cardinality siphons, adding no-good cuts to
/// exclude previously found solutions. Slower than the backtracking
/// approach for small nets but more systematic.
#[must_use]
pub fn minimal_siphons_ilp(net: &Net) -> Vec<HashSet<Place>> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    if net.n_places() == 0 {
        return Vec::new();
    }

    let mut results: Vec<HashSet<Place>> = Vec::new();

    let mut vars = ProblemVariables::new();
    let x: Vec<_> = net
        .places()
        .map(|_| vars.add(variable().binary()))
        .collect();

    let mut constraints = Vec::new();

    // At least one place must be in the siphon.
    let at_least_one: Expression = x.iter().sum();
    constraints.push(constraint!(at_least_one >= 1.0));

    // Siphon property: for each place p and each transition t ∈ •p,
    // if p is in the siphon then at least one input place of t is too.
    // Encoding: x[p] ≤ Σ_{q ∈ •t} x[q]  for all p, t ∈ •p
    for p in net.places() {
        for &t in net.preset_p(p) {
            let sum_preset: Expression = net
                .preset_t(t)
                .iter()
                .map(|&q| x[q.idx])
                .sum();
            constraints.push(constraint!(x[p.idx] <= sum_preset));
        }
    }

    let objective: Expression = x.iter().copied().sum();
    loop {
        let Ok(solution) = vars.clone()
            .minimise(&objective)
            .using(good_lp::microlp)
            .with_all(constraints.clone())
            .solve() else { break };

        let siphon: HashSet<Place> = net
            .places() // x[p] > 0.5 => binary variable is 1, p is in the siphon
            .filter(|p| solution.value(x[p.idx]) > 0.5)
            .collect();

        // Should not happen since we require at least one place >= 1
        if siphon.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
        if !dominated {
            results.retain(|existing| !siphon.is_subset(existing));
            results.push(siphon.clone());
        }

        // the sum of those exact variables must be ≤ |siphon| - 1 to exclude this siphon and all supersets
        let prev_sum: Expression = siphon.iter().map(|&p| x[p.idx]).sum();
        constraints.push(constraint!(prev_sum <= siphon.len() as f64 - 1.0));
    }

    results
}

/// Finds all minimal traps using ILP enumeration.
#[must_use]
pub fn minimal_traps_ilp(net: &Net) -> Vec<HashSet<Place>> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    let n_p = net.n_places();
    if n_p == 0 {
        return Vec::new();
    }

    let mut results: Vec<HashSet<Place>> = Vec::new();
    let mut nogood_sets: Vec<HashSet<Place>> = Vec::new();

    loop {
        let mut vars = ProblemVariables::new();
        let x: Vec<_> = (0..n_p)
            .map(|_| vars.add(variable().binary()))
            .collect();

        let objective: Expression = x.iter().copied().sum();

        let mut constraints = Vec::new();
        let at_least_one: Expression = x.iter().copied().sum();
        constraints.push(constraint!(at_least_one >= 1.0));

        // Trap property: for each place p and each transition t ∈ p•,
        // if p is in the trap then at least one output place of t is too.
        // Encoding: x[p] ≤ Σ_{q ∈ t•} x[q]  for all p, t ∈ p•
        for p in net.places() {
            for &t in net.postset_p(p) {
                let sum_postset: Expression = net.postset_t(t).iter()
                    .map(|&q| x[q.idx])
                    .sum();
                constraints.push(constraint!(x[p.idx] <= sum_postset));
            }
        }

        for prev in &nogood_sets {
            let prev_sum: Expression = prev.iter().map(|&p| x[p.idx]).sum();
            constraints.push(constraint!(prev_sum <= (prev.len() as f64 - 1.0)));
        }

        let Ok(solution) = vars
            .minimise(objective)
            .using(good_lp::microlp)
            .with_all(constraints)
            .solve() else { break };

        let trap: HashSet<Place> = (0..n_p)
            .filter(|&i| solution.value(x[i]) > 0.5)
            .map(|i| Place { idx: i })
            .collect();

        if trap.is_empty() {
            break;
        }

        let dominated = results.iter().any(|existing| existing.is_subset(&trap));
        if !dominated {
            results.retain(|existing| !trap.is_subset(existing));
            results.push(trap.clone());
        }
        nogood_sets.push(trap);
    }

    results
}

/// Checks the Commoner/Hack Criterion (CHC): every proper siphon contains
/// a trap that is marked under the given marking.
///
/// For free-choice nets, this is a necessary and sufficient condition for
/// liveness (Commoner's theorem / Theorem 12 in Murata 1989). For
/// asymmetric-choice nets it is sufficient but not necessary.
///
/// This is the key structural shortcut for proving liveness in free-choice nets
/// without exploring the full state space. This is significant because it runs
/// in polynomial time rather than exponential (in the number of reachable states).
///
/// For general nets, it is a sufficient condition for deadlock-freedom.
///
/// Instead of pre-enumerating all traps and checking containment, this
/// computes the maximal trap *inside* each siphon directly using the
/// shrinking algorithm. If the maximal trap is non-empty and marked,
/// the siphon satisfies the condition; if it is empty or unmarked,
/// no trap inside the siphon can be marked.
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::marking::Marking;
/// use petrivet::analysis::structural::{minimal_siphons, every_siphon_contains_marked_trap};
///
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap();
///
/// let siphons = minimal_siphons(&net);
/// let m0 = Marking::from([1u32, 0]);
/// // With a token, the siphon {p0, p1} contains a marked trap → live
/// assert!(every_siphon_contains_marked_trap(&net, &m0, &siphons));
///
/// // Without tokens, the trap is unmarked → not live
/// let m_empty = Marking::from([0u32, 0]);
/// assert!(!every_siphon_contains_marked_trap(&net, &m_empty, &siphons));
/// ```
///
/// References:
/// - Murata 1989, Theorem 12: "A free-choice net (N, M₀) is live iff
///   every siphon in N contains a marked trap."
/// - Petri Net Primer, Theorem 5.17 (Commoner/Hack Criterion)
/// - Petri Net Primer, Algorithm 6.19 (maximal siphon/trap in a subset)
#[must_use]
pub fn every_siphon_contains_marked_trap(
    net: &Net,
    marking: &Marking,
    siphons: &[HashSet<Place>],
) -> bool {
    siphons.iter().all(|siphon| {
        let maximal_trap = maximal_trap_in(net, siphon);
        !maximal_trap.is_empty() && maximal_trap.iter().any(|&p| marking[p] > 0)
    })
}

/// An S-component of a Petri net: a strongly connected subnet where every
/// transition has exactly one input place and one output place within the
/// component. S-components represent sequential cycles of token flow.
///
/// Key theorems involving S-components:
/// - If every place belongs to an S-component, the net is **conservative**
///   (and therefore structurally bounded).
/// - For live free-choice nets, boundedness ⟺ S-component coverage
///   (Heck's theorem).
///
/// References:
/// - Murata 1989, §VI-C (S-components and conservativeness)
/// - Petri Net Primer, Definition 5.9 and Theorem 5.22
#[derive(Debug, Clone)]
pub struct SComponent {
    pub places: HashSet<Place>,
    pub transitions: HashSet<Transition>,
}

/// A T-component of a Petri net: a strongly connected subnet where every
/// place has exactly one input transition and one output transition within
/// the component. T-components represent minimal cycles of concurrent firing.
///
/// Key theorem: if every transition belongs to a T-component, the net is
/// **repetitive** (every transition can participate in some T-invariant).
///
/// References:
/// - Murata 1989, §VI-C (T-components and repetitiveness)
/// - Petri Net Primer, Definition 5.9 and Theorem 5.23
#[derive(Debug, Clone)]
pub struct TComponent {
    pub places: HashSet<Place>,
    pub transitions: HashSet<Transition>,
}

/// Finds all S-components of a net.
///
/// An S-component is a strongly connected subnet where every transition has
/// exactly one input and one output place within the subnet. Found by
/// examining the support of each S-invariant basis vector: the places with
/// non-zero weight, plus the transitions connecting them, form a candidate.
///
/// For well-structured nets (especially free-choice), the S-invariant basis
/// directly yields the S-components. For general nets, this finds all
/// S-components that correspond to S-invariant supports.
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::analysis::structural;
///
/// // Mutex net: 3 S-components (one per process cycle + mutex cycle)
/// let mut b = NetBuilder::new();
/// let [idle1, wait1, crit1] = b.add_places();
/// let [idle2, wait2, crit2] = b.add_places();
/// let mutex = b.add_place();
/// let [req1, enter1, exit1] = b.add_transitions();
/// let [req2, enter2, exit2] = b.add_transitions();
/// b.add_arc((idle1, req1)); b.add_arc((req1, wait1));
/// b.add_arc((wait1, enter1)); b.add_arc((enter1, crit1));
/// b.add_arc((crit1, exit1)); b.add_arc((exit1, idle1));
/// b.add_arc((idle2, req2)); b.add_arc((req2, wait2));
/// b.add_arc((wait2, enter2)); b.add_arc((enter2, crit2));
/// b.add_arc((crit2, exit2)); b.add_arc((exit2, idle2));
/// b.add_arc((mutex, enter1)); b.add_arc((exit1, mutex));
/// b.add_arc((mutex, enter2)); b.add_arc((exit2, mutex));
/// let net = b.build().unwrap();
///
/// let components = structural::s_components(&net);
/// assert_eq!(components.len(), 3);
///
/// // Every place is covered by at least one S-component
/// assert!(structural::is_covered_by_s_components(&net, &components));
/// ```
#[must_use]
pub fn s_components(net: &Net) -> Vec<SComponent> {
    let inv = compute_invariants(net);
    if inv.s_invariants.is_empty() {
        return Vec::new();
    }

    let mut components = Vec::new();
    let mut found_supports: HashSet<Vec<usize>> = HashSet::new();

    // A place can belong to multiple S-components (e.g. crit1 in both the
    // process cycle and the mutex cycle). Query every place to find all
    // distinct components.
    for p in net.places() {
        let Some(support) = find_nonneg_invariant_support(&inv.s_invariants, p.index(), net.n_places())
        else {
            continue;
        };

        let mut key: Vec<usize> = support.iter().copied().collect();
        key.sort_unstable();
        if !found_supports.insert(key) {
            continue;
        }

        let places: HashSet<Place> = support.into_iter().map(Place::from_index).collect();

        let transitions: HashSet<Transition> = net
            .transitions()
            .filter(|&t| {
                let pre_count = net.preset_t(t).iter().filter(|p| places.contains(p)).count();
                let post_count = net.postset_t(t).iter().filter(|p| places.contains(p)).count();
                pre_count == 1 && post_count == 1
            })
            .collect();

        if transitions.is_empty() {
            continue;
        }

        if is_subnet_strongly_connected(net, &places, &transitions) {
            components.push(SComponent { places, transitions });
        }
    }

    components
}

/// Finds a non-negative linear combination of `basis` vectors that is
/// strictly positive at `target_idx` with minimal total weight (encouraging
/// small support). Returns the support as a set of indices, or `None` if
/// no non-negative combination covers the target.
fn find_nonneg_invariant_support(
    basis: &[Box<[i32]>],
    target_idx: usize,
    dimension: usize,
) -> Option<HashSet<usize>> {
    use good_lp::{constraint, variable, Expression, ProblemVariables, Solution, SolverModel};

    let mut vars = ProblemVariables::new();
    let lambda: Vec<_> = (0..basis.len())
        .map(|_| vars.add(variable()))
        .collect();

    let mut constraints = Vec::new();

    let y_exprs: Vec<Expression> = (0..dimension)
        .map(|i| {
            lambda
                .iter()
                .enumerate()
                .map(|(j, &l)| f64::from(basis[j][i]) * l)
                .sum()
        })
        .collect();

    for (i, y_i) in y_exprs.iter().enumerate() {
        constraints.push(constraint!(y_i.clone() >= 0.0));
        if i == target_idx {
            constraints.push(constraint!(y_i.clone() >= 1.0));
        }
    }

    let objective: Expression = y_exprs.iter().cloned().sum();

    let solution = vars
        .minimise(objective)
        .using(good_lp::microlp)
        .with_all(constraints)
        .solve()
        .ok()?;

    let lambda_vals: Vec<f64> = lambda.iter().map(|&l| solution.value(l)).collect();
    let support: HashSet<usize> = (0..dimension)
        .filter(|&i| {
            let y_i: f64 = lambda_vals
                .iter()
                .enumerate()
                .map(|(j, &lj)| lj * f64::from(basis[j][i]))
                .sum();
            y_i > 0.5
        })
        .collect();

    if support.is_empty() { None } else { Some(support) }
}

/// Finds all T-components of a net.
///
/// A T-component is a strongly connected subnet where every place has exactly
/// one input and one output transition within the subnet. Found by examining
/// the support of each T-invariant basis vector.
///
/// # Examples
///
/// ```
/// use petrivet::net::builder::NetBuilder;
/// use petrivet::analysis::structural;
///
/// // Simple cycle: the whole net is one T-component
/// let mut b = NetBuilder::new();
/// let [p0, p1] = b.add_places();
/// let [t0, t1] = b.add_transitions();
/// b.add_arc((p0, t0)); b.add_arc((t0, p1));
/// b.add_arc((p1, t1)); b.add_arc((t1, p0));
/// let net = b.build().unwrap();
///
/// let components = structural::t_components(&net);
/// assert_eq!(components.len(), 1);
/// assert!(structural::is_covered_by_t_components(&net, &components));
/// ```
#[must_use]
pub fn t_components(net: &Net) -> Vec<TComponent> {
    let inv = compute_invariants(net);
    if inv.t_invariants.is_empty() {
        return Vec::new();
    }

    let mut components = Vec::new();

    for t in net.transitions() {
        let Some(support) = find_nonneg_invariant_support(
            &inv.t_invariants,
            t.index(),
            net.n_transitions(),
        ) else {
            continue;
        };

        let transitions: HashSet<Transition> = support.into_iter().map(Transition::from_index).collect();

        if components.iter().any(|c: &TComponent| c.transitions == transitions) {
            continue;
        }

        let places: HashSet<Place> = net
            .places()
            .filter(|&p| {
                let pre_count = net.preset_p(p).iter().filter(|t| transitions.contains(t)).count();
                let post_count = net.postset_p(p).iter().filter(|t| transitions.contains(t)).count();
                pre_count == 1 && post_count == 1
            })
            .collect();

        if places.is_empty() {
            continue;
        }

        if is_subnet_strongly_connected(net, &places, &transitions) {
            components.push(TComponent { places, transitions });
        }
    }

    components
}

/// Whether every place in the net belongs to at least one S-component.
///
/// S-component coverage implies conservativeness (and thus structural
/// boundedness). For live free-choice nets, this is also a necessary
/// condition for boundedness (Heck's theorem).
///
/// References:
/// - Murata 1989, Theorem 14
/// - Petri Net Primer, Theorem 5.22
#[must_use]
pub fn is_covered_by_s_components(net: &Net, components: &[SComponent]) -> bool {
    net.places().all(|p| components.iter().any(|c| c.places.contains(&p)))
}

/// Whether every transition in the net belongs to at least one T-component.
///
/// T-component coverage implies repetitiveness: for every transition, there
/// exists a T-invariant whose support includes that transition.
///
/// References:
/// - Murata 1989, Theorem 15
/// - Petri Net Primer, Theorem 5.23
#[must_use]
pub fn is_covered_by_t_components(net: &Net, components: &[TComponent]) -> bool {
    net.transitions().all(|t| components.iter().any(|c| c.transitions.contains(&t)))
}

/// Checks strong connectivity of a subnet induced by a set of places and transitions.
fn is_subnet_strongly_connected(
    net: &Net,
    places: &HashSet<Place>,
    transitions: &HashSet<Transition>,
) -> bool {
    use petgraph::graph::NodeIndex;

    let n_nodes = places.len() + transitions.len();
    if n_nodes <= 1 {
        return true;
    }

    let mut graph = petgraph::Graph::<(), ()>::with_capacity(n_nodes, n_nodes * 2);
    let mut p_map: std::collections::HashMap<Place, NodeIndex> = std::collections::HashMap::new();
    let mut t_map: std::collections::HashMap<Transition, NodeIndex> = std::collections::HashMap::new();

    for &p in places {
        p_map.insert(p, graph.add_node(()));
    }
    for &t in transitions {
        t_map.insert(t, graph.add_node(()));
    }

    for &t in transitions {
        for &p in net.preset_t(t) {
            if let Some(&p_idx) = p_map.get(&p) {
                graph.add_edge(p_idx, t_map[&t], ());
            }
        }
        for &p in net.postset_t(t) {
            if let Some(&p_idx) = p_map.get(&p) {
                graph.add_edge(t_map[&t], p_idx, ());
            }
        }
    }

    petgraph::algo::kosaraju_scc(&graph).len() == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::builder::NetBuilder;
    use crate::net::class::NetClass;

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
    fn cycle_invariants() {
        let net = two_place_cycle();
        let inv = compute_invariants(&net);
        assert_eq!(inv.s_invariants.len(), 1);
        assert_eq!(inv.t_invariants.len(), 1);
        // S-invariant: equal weights on both places (token conservation)
        assert_eq!(inv.s_invariants[0][0], inv.s_invariants[0][1]);
        // T-invariant: equal firing counts (full cycle)
        assert_eq!(inv.t_invariants[0][0], inv.t_invariants[0][1]);
        assert!(inv.is_covered_by_s_invariants(net.n_places()));
        assert!(inv.is_covered_by_t_invariants(net.n_transitions()));
    }

    #[test]
    fn cycle_siphons_and_traps() {
        let net = two_place_cycle();
        let siphons = minimal_siphons(&net);
        let traps = minimal_traps(&net);
        // In a cycle, the entire set of places is both the only minimal
        // siphon and the only minimal trap.
        assert_eq!(siphons.len(), 1);
        assert_eq!(traps.len(), 1);
        let all_places: HashSet<Place> = net.places().collect();
        assert_eq!(siphons[0], all_places);
        assert_eq!(traps[0], all_places);
    }

    #[test]
    fn mutex_structural_analysis() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap();
        assert_eq!(net.class(), NetClass::AsymmetricChoice);
        let inv = compute_invariants(&net);

        // 3 S-invariants for mutex net (7 places, rank 4 → dim 3)
        assert_eq!(inv.s_invariants.len(), 3);
        assert!(inv.is_covered_by_s_invariants(net.n_places()));

        // 2 T-invariants (6 transitions, rank 4 → dim 2)
        assert_eq!(inv.t_invariants.len(), 2);
        assert!(inv.is_covered_by_t_invariants(net.n_transitions()));
    }

    #[test]
    fn commoner_liveness_mutex() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap();
        let marking = Marking::from([1u32, 0, 0, 1, 0, 0, 1]);
        let siphons = minimal_siphons(&net);

        assert!(every_siphon_contains_marked_trap(&net, &marking, &siphons));
    }

    #[test]
    fn producer_consumer_not_fully_covered() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((t0, p0));
        b.add_arc((p0, t1));
        b.add_arc((t1, p1));
        b.add_arc((p1, t0));
        let net = b.build().unwrap();
        let inv = compute_invariants(&net);
        assert_eq!(inv.s_invariants.len(), 1);
    }

    #[test]
    fn maximal_siphon_shrinks_correctly() {
        // Simple choice net: p0 -> t0 -> p1, p0 -> t1 -> p2
        // {p0} is a siphon (•{p0} = ∅ ⊆ {p0}• trivially).
        // {p1} is not a siphon (•{p1} = {t0}, t0 ∉ {p1}•).
        // {p0, p1} is a siphon.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        let net = b.build().unwrap();

        let all: HashSet<Place> = net.places().collect();
        let max = maximal_siphon_in(&net, &all);
        // The maximal siphon in {p0, p1, p2} should shrink:
        // p1: •p1 = {t0}, •t0 = {p0} which is in the set → p1 OK
        // p2: •p2 = {t1}, •t1 = {p0} which is in the set → p2 OK
        // p0: •p0 = ∅ → vacuously OK (no transitions to violate)
        // So the maximal siphon is {p0, p1, p2}.
        assert_eq!(max, all);

        // Now try just {p1, p2}: should shrink to empty.
        let subset: HashSet<Place> = [p1, p2].into_iter().collect();
        let max2 = maximal_siphon_in(&net, &subset);
        assert!(max2.is_empty());
    }

    #[test]
    fn maximal_trap_shrinks_correctly() {
        // Same choice net: p0 -> t0 -> p1, p0 -> t1 -> p2
        // {p1, p2} is a trap: p1• = ∅, p2• = ∅, so Q• = ∅ ⊆ •Q trivially.
        // {p0} is not a trap: p0• = {t0, t1}, t0 ∉ •{p0}, t1 ∉ •{p0}.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        let net = b.build().unwrap();

        let all: HashSet<Place> = net.places().collect();
        let max = maximal_trap_in(&net, &all);
        // p0: p0• = {t0, t1}. t0• = {p1} in set → OK for t0. t1• = {p2} in set → OK.
        // p1: p1• = ∅ → vacuously OK.
        // p2: p2• = ∅ → vacuously OK.
        assert_eq!(max, all);
    }

    #[test]
    fn ilp_siphons_match_backtracking() {
        let net = two_place_cycle();
        let bt = minimal_siphons(&net);
        let ilp = minimal_siphons_ilp(&net);
        assert_eq!(bt.len(), ilp.len());
        for s in &bt {
            assert!(ilp.contains(s), "backtracking siphon not found by ILP");
        }
    }

    #[test]
    fn ilp_traps_match_backtracking() {
        let net = two_place_cycle();
        let bt = minimal_traps(&net);
        let ilp = minimal_traps_ilp(&net);
        assert_eq!(bt.len(), ilp.len());
        for t in &bt {
            assert!(ilp.contains(t), "backtracking trap not found by ILP");
        }
    }

    #[test]
    fn ilp_siphons_mutex() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap();
        let bt = minimal_siphons(&net);
        let ilp = minimal_siphons_ilp(&net);
        assert_eq!(bt.len(), ilp.len());
        for s in &bt {
            assert!(ilp.contains(s), "backtracking siphon {s:?} not found by ILP");
        }
    }

    #[test]
    fn commoner_fails_for_dead_net() {
        // Two-place cycle with zero initial marking — not live.
        let net = two_place_cycle();
        let marking = Marking::from([0u32, 0]);
        let siphons = minimal_siphons(&net);
        // The only siphon is {p0, p1}; its maximal trap is also {p0, p1},
        // but the trap is unmarked (both places have 0 tokens).
        assert!(!every_siphon_contains_marked_trap(&net, &marking, &siphons));
    }

    #[test]
    fn cycle_s_and_t_components() {
        let net = two_place_cycle();
        let s_comps = s_components(&net);
        let t_comps = t_components(&net);
        // A circuit is both one S-component and one T-component
        assert_eq!(s_comps.len(), 1);
        assert_eq!(t_comps.len(), 1);
        assert_eq!(s_comps[0].places.len(), 2);
        assert_eq!(s_comps[0].transitions.len(), 2);
        assert_eq!(t_comps[0].places.len(), 2);
        assert_eq!(t_comps[0].transitions.len(), 2);
        assert!(is_covered_by_s_components(&net, &s_comps));
        assert!(is_covered_by_t_components(&net, &t_comps));
    }

    #[test]
    fn mutex_s_components() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap();
        let s_comps = s_components(&net);

        // 3 S-components: process 1 cycle, process 2 cycle, mutex cycle
        assert_eq!(s_comps.len(), 3);
        assert!(is_covered_by_s_components(&net, &s_comps));

        // Each process cycle should contain 3 places
        let proc1: HashSet<Place> = [idle1, wait1, crit1].into_iter().collect();
        let proc2: HashSet<Place> = [idle2, wait2, crit2].into_iter().collect();
        assert!(s_comps.iter().any(|c| c.places == proc1));
        assert!(s_comps.iter().any(|c| c.places == proc2));
        // Mutex cycle contains mutex + crit1 + crit2
        assert!(s_comps.iter().any(|c| c.places.contains(&mutex)));
    }

    #[test]
    fn mutex_t_components() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().unwrap();
        let t_comps = t_components(&net);

        // 2 T-components: one per process (req, enter, exit)
        assert_eq!(t_comps.len(), 2);
        assert!(is_covered_by_t_components(&net, &t_comps));

        let proc1_t: HashSet<Transition> = [t_req1, t_enter1, t_exit1].into_iter().collect();
        let proc2_t: HashSet<Transition> = [t_req2, t_enter2, t_exit2].into_iter().collect();
        assert!(t_comps.iter().any(|c| c.transitions == proc1_t));
        assert!(t_comps.iter().any(|c| c.transitions == proc2_t));
    }

    #[test]
    fn choice_net_no_s_component_coverage() {
        // p0 -> t0 -> p1, p0 -> t1 -> p2 (choice, not a cycle)
        // S-invariant support includes both branches but they don't form
        // a strongly connected S-component covering all places.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        let net = b.build().unwrap();

        let s_comps = s_components(&net);
        // No S-components: the net has no cycles
        assert!(!is_covered_by_s_components(&net, &s_comps));
    }

    #[test]
    fn fc_net_not_live_siphon_without_marked_trap() {
        // Free-choice net with an absorbing branch:
        //   p0 -> t0 -> p1 (dead end: p1 has no output transitions)
        //   p0 -> t1 -> p2 -> t2 -> p0 (cycle)
        //
        // This is free-choice: •t0 = •t1 = {p0}.
        // If t0 fires, token goes to p1 and never returns.
        // Siphon {p0, p2}: •D = {t2, t1} ⊆ D• = {t0, t1, t2}. ✓
        // Maximal trap in {p0, p2}:
        //   p0: p0• = {t0, t1}. t0• = {p1} ∉ D → p0 removed.
        //   p2: p2• = {t2}. t2• = {p0} ∉ remaining → p2 removed.
        //   Maximal trap = ∅ → no marked trap → not live.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        b.add_arc((p2, t2)); b.add_arc((t2, p0));
        let net = b.build().unwrap();
        assert!(net.is_free_choice());

        let marking = Marking::from([1u32, 0, 0]);
        let siphons = minimal_siphons(&net);

        let emptiable: HashSet<Place> = [p0, p2].into_iter().collect();
        assert!(siphons.contains(&emptiable), "should find siphon {{p0, p2}}");

        assert!(!every_siphon_contains_marked_trap(&net, &marking, &siphons));
    }
}
