//! Integer linear algebra utilities.
//!
//! Provides fraction-free Gaussian elimination for computing the null space
//! of an integer matrix. Used internally for S-invariant and T-invariant
//! computation.

use std::iter;
use crate::net::IncidenceMatrix;

/// Computes an integer basis for the null space of the given matrix.
///
/// Returns a list of vectors x such that M · x = 0, where M is treated
/// as an `rows × cols` matrix. The vectors span the entire null space
/// over the rationals, scaled to integer entries with no common factor.
///
/// Uses fraction-free Gaussian elimination (Bareiss algorithm) to avoid
/// rational arithmetic entirely.
#[must_use]
pub fn integer_null_space(matrix: &IncidenceMatrix) -> Box<[Box<[i32]>]> {
    let rows = matrix.n_rows();
    let cols = matrix.n_cols();

    if rows == 0 || cols == 0 {
        return (0..cols).map(|i| {
            let mut v = vec![0; cols].into_boxed_slice();
            v[i] = 1;
            v
        }).collect();
    }

    // Work on a mutable copy in row-major order.
    let mut mat: Box<[Box<[i32]>]> = (0..rows)
        .map(|r| matrix.row(r).to_vec().into_boxed_slice())
        .collect();

    // Track which columns are pivot columns.
    let mut pivot_col = vec![None; rows];
    let mut pivot_row_for_col = vec![None; cols];
    let mut current_row = 0;

    for col in 0..cols {
        // Find a pivot in this column at or below current_row.
        let Some(pivot) = (current_row..rows).find(|&r| mat[r][col] != 0) else {
            continue;
        };

        // Swap pivot row into position.
        mat.swap(current_row, pivot);
        pivot_col[current_row] = Some(col);
        pivot_row_for_col[col] = Some(current_row);

        let pivot_val = mat[current_row][col];

        // Eliminate this column from all other rows.
        for row in 0..rows {
            if row == current_row {
                continue;
            }
            let factor = mat[row][col];
            if factor == 0 {
                continue;
            }
            let pivot_row_copy = mat[current_row].clone();
            for (val, pivot_entry) in iter::zip(&mut mat[row], pivot_row_copy) {
                *val = *val * pivot_val - factor * pivot_entry;
            }
            let g = row_gcd(&mat[row]);
            if g > 1 {
                for val in &mut mat[row] {
                    *val /= g;
                }
            }
        }

        current_row += 1;
    }

    // Free columns (not pivot columns) give null space vectors.
    (0..cols)
        .filter(|col| pivot_row_for_col[*col].is_none())
        .map(|free_col| {
            let mut bx = vec![0; cols].into_boxed_slice();
            bx[free_col] = 1;

            // For each pivot column, solve for its value.
            for row in 0..current_row {
                if let Some(pc) = pivot_col[row] {
                    let pivot_val = mat[row][pc];
                    let rhs = -mat[row][free_col];
                    // We need: pivot_val * vec[pc] + mat[row][fc] * vec[fc] = 0
                    // Since vec[fc] = 1: vec[pc] = -mat[row][fc] / pivot_val
                    // Scale everything to stay integer.
                    // Multiply all existing entries by |pivot_val|, set vec[pc] = rhs.
                    let abs_pivot = pivot_val.abs();
                    for (i, v) in bx.iter_mut().enumerate() {
                        if i != pc {
                            *v *= abs_pivot;
                        }
                    }
                    bx[pc] = if pivot_val > 0 { rhs } else { -rhs };
                }
            }

            // Reduce by GCD.
            let g = row_gcd(&bx);
            if g > 1 {
                for v in &mut bx {
                    *v /= g;
                }
            }

            if let Some(first_nonzero) = bx.iter().find(|&&v| v != 0)
                && *first_nonzero < 0
            {
                for v in &mut bx {
                    *v = -*v;
                }
            }

            bx
        })
        .collect()
}

/// GCD of absolute values of all elements in a row.
fn row_gcd(row: &[i32]) -> i32 {
    row.iter().copied().map(|x| x.unsigned_abs()).fold(0u32, gcd_u64) as i32
}

fn gcd_u64(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd_u64(b, a % b) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat_from_vecs(rows: &[&[i32]]) -> IncidenceMatrix {
        let n_rows = rows.len();
        let n_cols = if n_rows > 0 { rows[0].len() } else { 0 };
        let data: Vec<i32> = rows.iter().flat_map(|r| r.iter().copied()).collect();
        // Use the public constructor indirectly — build a net and get its matrix,
        // or construct directly for testing.
        IncidenceMatrix::from_raw(data, n_rows, n_cols)
    }

    fn verify_null_space(matrix: &IncidenceMatrix, basis: &[Box<[i32]>]) {
        for row in basis {
            for r in 0..matrix.n_rows() {
                let dot: i32 = (0..matrix.n_cols())
                    .map(|c| matrix.get(r, c) * row[c])
                    .sum();
                assert_eq!(dot, 0, "null space vector is not in kernel");
            }
        }
    }

    #[test]
    fn simple_cycle_invariant() {
        // Two-place cycle: t0 consumes p0, produces p1; t1 consumes p1, produces p0
        // N = [[-1, 1], [1, -1]]
        // Null space: [1, 1] (token conservation)
        let m = mat_from_vecs(&[&[-1, 1], &[1, -1]]);
        let basis = integer_null_space(&m);
        assert_eq!(basis.len(), 1);
        verify_null_space(&m, &basis);
        assert_eq!(basis[0].as_ref(), [1, 1]);
    }

    #[test]
    fn three_place_cycle() {
        // N = [[-1, 1, 0], [0, -1, 1], [1, 0, -1]]
        let m = mat_from_vecs(&[&[-1, 1, 0], &[0, -1, 1], &[1, 0, -1]]);
        let basis = integer_null_space(&m);
        assert_eq!(basis.len(), 1);
        verify_null_space(&m, &basis);
        // All components equal (token sum is conserved)
        assert_eq!(basis[0][0], basis[0][1]);
        assert_eq!(basis[0][1], basis[0][2]);
    }

    #[test]
    fn full_rank_no_null_space() {
        // Identity-like: [[1, 0], [0, 1]]
        let m = mat_from_vecs(&[&[1, 0], &[0, 1]]);
        let basis = integer_null_space(&m);
        assert!(basis.is_empty());
    }

    #[test]
    fn zero_matrix() {
        let m = mat_from_vecs(&[&[0, 0, 0], &[0, 0, 0]]);
        let basis = integer_null_space(&m);
        assert_eq!(basis.len(), 3);
        verify_null_space(&m, &basis);
    }

    #[test]
    fn empty_matrix() {
        let m = mat_from_vecs(&[]);
        let basis = integer_null_space(&m);
        assert!(basis.is_empty());
    }

    #[test]
    fn producer_net() {
        // t0: consumes p0, produces p0 and p1
        // N = [[0, 1]] (p0 change is 0, p1 gains 1)
        // Wait, for the net p0→t0→p0,p1: N = [[-1+1, 0+1]] = [[0, 1]]
        // Null space of [[0, 1]] is [1, 0]
        let m = mat_from_vecs(&[&[0, 1]]);
        let basis = integer_null_space(&m);
        assert_eq!(basis.len(), 1);
        verify_null_space(&m, &basis);
        assert_eq!(basis[0].as_ref(), [1, 0]);
    }

    #[test]
    fn mutex_s_invariants() {
        // Mutex net: 7 places, 6 transitions
        // S-invariants (null space of N, which is |T|×|P|) are |P|-dimensional
        // vectors y such that N · y = 0.
        // Known S-invariants: [1,1,1,0,0,0,0], [0,0,0,1,1,1,0], [0,0,1,0,0,1,1]
        use crate::net::builder::NetBuilder;

        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2,crit2] = b.add_places();
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
        let c = net.incidence_matrix();

        // S-invariants = null space of N (|T|×|P| → vectors of length |P|)
        let s_inv = integer_null_space(&c);
        assert_eq!(s_inv.len(), 3);
        verify_null_space(&c, &s_inv);

        // T-invariants = null space of N^T (|P|×|T| → vectors of length |T|)
        let ct = c.transpose();
        let t_inv = integer_null_space(&ct);
        assert_eq!(t_inv.len(), 2);
        verify_null_space(&ct, &t_inv);
    }
}
