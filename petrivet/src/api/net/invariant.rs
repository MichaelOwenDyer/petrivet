use crate::net::Place;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PInvariant {
    pub weights: Vec<(Place, u32)>,
    pub value: u32,
}