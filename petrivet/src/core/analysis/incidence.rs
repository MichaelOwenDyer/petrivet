use crate::core::net::{DenseNet, PlaceIdx, TransitionIdx};

/// The incidence matrix N of a Petri net.
///
/// Stored as two separate matrices: the consume matrix C and the produce matrix P.
///
/// References:
/// - [Primer, Definition 4.1](crate::literature#definition-41--incidence-matrix)
/// - [Murata 1989, §IV-B](crate::literature#iv-b--incidence-matrix-and-state-equation) (uses the transposed convention; our N = Murata's Aᵀ)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidenceMatrix {
    /// The number of tokens consumed from place p by transition t.
    consume: Box<[u8]>,
    /// The number of tokens produced into place p by transition t.
    produce: Box<[u8]>,
    /// The number of places in the net.
    places: usize,
    /// The number of transitions in the net.
    transitions: usize,
}

impl IncidenceMatrix {
    /// Constructs the |P| × |T| incidence matrix for a given net.
    #[must_use]
    pub fn new(net: &DenseNet) -> Self {
        let rows = net.place_count();
        let cols = net.transition_count();
        let mut consume = vec![0; rows * cols].into_boxed_slice();
        let mut produce = vec![0; rows * cols].into_boxed_slice();
        
        for t in net.transition_indices() {
            for &p in &net.preset_t[t] {
                consume[p * cols + t] += 1;
            }
            for &p in &net.postset_t[t] {
                produce[p * cols + t] += 1;
            }
        }
        IncidenceMatrix {
            consume,
            produce,
            places: rows,
            transitions: cols,
        }
    }

    /// Constructs the |P| × |T| incidence matrix for a given net.
    #[must_use]
    pub fn from_preset_and_postset(
        place_count: usize,
        preset_t: &[Box<[PlaceIdx]>],
        postset_t: &[Box<[PlaceIdx]>],
    ) -> Self {
        let rows = place_count;
        let cols = preset_t.len();
        let mut consume = vec![0; rows * cols].into_boxed_slice();
        let mut produce = vec![0; rows * cols].into_boxed_slice();

        for t in 0..cols as TransitionIdx {
            for &p in &preset_t[t] {
                consume[p * cols + t] += 1;
            }
            for &p in &postset_t[t] {
                produce[p * cols + t] += 1;
            }
        }
        IncidenceMatrix {
            consume,
            produce,
            places: rows,
            transitions: cols,
        }
    }

    /// Returns the number of tokens transition `t` consumes from place `p`.
    #[must_use]
    pub fn get_consume(&self, transition: TransitionIdx, place: PlaceIdx) -> u8 {
        self.consume[place * self.transitions + transition]
    }

    /// Returns the number of tokens transition `t` produces to place `p`.
    #[must_use]
    pub fn get_produce(&self, transition: TransitionIdx, place: PlaceIdx) -> u8 {
        self.produce[place * self.transitions + transition]
    }

    /// Returns the net effect of transition `t` on place `p`,
    /// i.e., the number of tokens produced minus the number of tokens consumed.
    #[must_use]
    pub fn get_effect(&self, transition: TransitionIdx, place: PlaceIdx) -> i16 {
        i16::from(self.get_produce(transition, place)) - i16::from(self.get_consume(transition, place))
    }

    pub fn place_indices(&self) -> impl Iterator<Item = PlaceIdx> + '_ {
        0..self.places as PlaceIdx
    }

    pub fn transition_indices(&self) -> impl Iterator<Item = TransitionIdx> + '_ {
        0..self.transitions as TransitionIdx
    }
}
