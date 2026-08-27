use crate::net::Place;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PInvariantKind {
    /// The invariant is an equality: the weighted sum of the places is constant.
    Invariant,
    /// The invariant is a subinvariant: the weighted sum of the places is non-increasing.
    Subinvariant,
    /// The invariant is a surinvariant: the weighted sum of the places is non-decreasing.
    Surinvariant,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PInvariant {
    pub weights: Vec<(Place, u32)>,
    pub value: u32,
    pub kind: PInvariantKind,
}

impl Display for PInvariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, &(p_idx, weight)) in self.weights.iter().enumerate() {
            if i > 0 {
                write!(f, " + ")?;
            }
            if weight == 1 {
                write!(f, "{p_idx:?}")?;
            } else {
                write!(f, "{weight}*{p_idx:?}")?;
            }
        }
        match self.kind {
            PInvariantKind::Invariant => write!(f, " = {}", self.value),
            PInvariantKind::Subinvariant => write!(f, " <= {}", self.value),
            PInvariantKind::Surinvariant => write!(f, " >= {}", self.value),
        }
    }
}