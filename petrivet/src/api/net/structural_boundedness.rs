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

#[cfg(test)]
mod tests {
    use crate::prelude::NetBuilder;

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, p0, _t0, _p1, _t1) = crate::system::tests::two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_bounded());
    }

    #[test]
    fn unbounded_not_structurally_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        assert!(!net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(!sys.is_bounded());
    }
}
