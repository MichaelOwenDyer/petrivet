use crate::net::{Net, Place};

impl Net {
    /// Checks if the net is *structurally* bounded.
    /// This means that no initial marking on this net
    /// would create an unbounded system.
    #[must_use]
    pub fn is_structurally_bounded(&self) -> bool {
        self.dense_net.is_structurally_bounded()
    }

    /// Checks if a single place is structurally bounded.
    /// This means that there exists no initial marking
    /// which would cause this place to become unbounded.
    #[must_use]
    pub fn is_place_structurally_bounded(&self, place: Place) -> bool {
        self.mapping
            .place_idx(place)
            .is_some_and(|p_idx| self.dense_net.is_place_structurally_bounded(p_idx))
    }
}