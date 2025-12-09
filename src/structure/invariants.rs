use nalgebra::{DMatrix, Dyn, MatrixView, U1};
use crate::structure::Transition;

/// The incidence matrix of a net describes the net effect of firing each transition on the marking of each place.
/// It is a |S| x |T| matrix N where:
/// - N(s, t) = |t•(s)| - |•t(s)|
/// The rows correspond to places and the columns to transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidenceMatrix(pub(crate) DMatrix<i8>);

impl IncidenceMatrix {
    #[must_use]
    pub fn column(&self, transition: Transition) -> MatrixView<'_, i8, Dyn, U1, U1, Dyn> {
        self.0.column(transition.index)
    }
    #[must_use]
    pub fn row(&self, transition: Transition) -> MatrixView<'_, i8, U1, Dyn, U1, Dyn> {
        self.0.row(transition.index)
    }
}

/// An S-invariant of a Net is a weighted sum of places that remains constant
/// regardless of the transitions that occur. It is represented as a vector of `i8` values,
/// where each integer corresponds to a place in the Petri Net. It is validated to have
/// the same length as the number of places in the net that owns it.
/// Formally, an S-invariant is a vector I: S → Q such that I * N = 0, where N is the incidence matrix
/// of the Net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SInvariant(Box<[i8]>);

/// A T-invariant of a Net is a weighted sum of transitions that, when fired in sequence,
/// has no net effect on the marking of the Net. It is represented as a vector of `i8` values,
/// where each integer represents the number of times the corresponding transition is fired.
/// It is validated to have the same length as the number of transitions in the net that owns
/// it.
/// Formally, a T-invariant is a vector J: T → Q such that N * J = 0, where N is the incidence matrix
/// of the Net.
pub struct TInvariant(Box<[i8]>);