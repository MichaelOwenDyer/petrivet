use std::fmt::Display;
use crate::net::Place;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PInvariant {
    pub weights: Vec<(Place, u32)>,
    pub value: u32,
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
        write!(f, " = {}", self.value)
    }
}