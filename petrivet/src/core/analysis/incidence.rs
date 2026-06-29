use crate::core::net::{DenseNet, PlaceIdx, TransitionIdx};

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
/// - [Primer, Definition 4.1](crate::literature#definition-41--incidence-matrix)
/// - [Murata 1989, §IV-B](crate::literature#iv-b--incidence-matrix-and-state-equation) (uses the transposed convention; our N = Murata's Aᵀ)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidenceMatrix {
    data: Box<[i32]>,
    places: usize,
    transitions: usize,
}

impl IncidenceMatrix {
    /// Constructs the |P| × |T| incidence matrix for a given net.
    #[must_use]
    pub fn new(net: &DenseNet) -> Self {
        let rows = net.place_count();
        let cols = net.transition_count();
        let mut data = vec![0; rows * cols].into_boxed_slice();
        for t in net.transition_indices() {
            for &p in &net.preset_t[t] {
                data[p * cols + t] -= 1;
            }
            for &p in &net.postset_t[t] {
                data[p * cols + t] += 1;
            }
        }
        IncidenceMatrix {
            data,
            places: rows,
            transitions: cols,
        }
    }

    /// Entry at (row, col) = N\[place\]\[transition\].
    #[must_use]
    pub fn get(&self, place: PlaceIdx, transition: TransitionIdx) -> i32 {
        self.data[place * self.transitions + transition]
    }

    pub fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
        0..self.places as PlaceIdx
    }

    pub fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
        0..self.transitions as TransitionIdx
    }
}