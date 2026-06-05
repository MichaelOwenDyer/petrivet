/// The liveness level of a [`Transition`] describes how often it
/// can be fired in a [`Petri net`](crate::PetriNet) `(N, M₀)`.
///
/// Liveness level of a transition from a given initial marking `M₀`,
/// following Murata 1989 §V-C.
///
/// The levels form a strict hierarchy: L4 ⊂ L3 ⊂ L2 ⊂ L1, and L0 means
/// the transition is dead (not even L1).
///
/// The liveness level of a Petri net is that of its *least* live transition.
///
/// References:
/// - [Murata 1989, Definition 5.1](crate::literature#definition-51--liveness-levels-l0l4)
/// - Petri Net Primer, §5.4 (liveness)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LivenessLevel {
    /// A [Transition] `t` is **L0-live** (or *dead*) if there exists no firing sequence
    /// from `M₀` which enables `t`.
    ///
    /// In other words, `t` can never fire.
    ///
    /// L0-live transitions are always *strictly* L0-live, in the sense that
    /// no transition can belong to another liveness class and also L0 at the same time.
    ///
    /// [Transition]: crate::net::Transition
    L0,
    /// A transition `t` is **L1-live** if it is *not* [`L0-live`](LivenessLevel::L0).
    ///
    /// In other words, there exists at least one firing sequence from `M₀` which enables `t`.
    ///
    /// Transitions which are [`L4`](LivenessLevel::L4), [`L3`](LivenessLevel::L3),
    /// or [`L2-live`](LivenessLevel::L2) are also L1-live.
    ///
    /// A transition which is L1-live but not L2-live is called *strictly* L1-live.
    L1,
    /// A transition `t` is **L2-live** if for any positive finite integer `k`,
    /// there exists a firing sequence from `M₀` which fires `t` `k` times.
    ///
    /// In other words, we can find a finite firing sequence which fires `t`
    /// any arbitrary number of times.
    ///
    /// Note that this does *not* imply the existence of an *infinite* firing sequence
    /// containing `t` infinitely many times, which is the definition of [`L3-liveness`](LivenessLevel::L3).
    ///
    /// For **bounded** Petri nets, any L2-live transition is also L3-live.
    /// This is because in a finite marking space the only way to fire a transition
    /// any arbitrary number of times is with a cycle, which immediately also enables
    /// the infinite firing sequence which spins around in that cycle forever.
    ///
    /// Transitions which are [`L4`](LivenessLevel::L4) or [`L3-live`](LivenessLevel::L3)
    /// are also L2-live.
    ///
    /// A transition which is L2-live but not L3-live is called *strictly* L2-live.
    /// Such transitions can only exist in the presence of
    /// [unboundedness](crate::boundedness::Boundedness::Unbounded).
    L2,
    /// A transition `t` is **L3-live** if there exists an *infinite* firing sequence
    /// from `M₀` which fires `t` infinitely many times.
    ///
    /// Transitions which are [`L4`](LivenessLevel::L4)-live are also L3-live.
    ///
    /// A transition which is L3-live but not L4-live is called *strictly* L3-live.
    L3,
    /// A transition `t` is **L4-live** (or just *live*) if it is [`L1`](LivenessLevel::L1)-live
    /// from *every* marking reachable from `M₀`.
    ///
    /// In other words, no matter which transitions we fire from `M₀`, and no matter which
    /// reachable marking `M` we end up in, there exists a firing sequence from `M` which enables `t`.
    /// It is impossible for `t` to become [`dead`](LivenessLevel::L0).
    L4,
}

impl LivenessLevel {
    /// Returns true if the transition is L0-live (dead).
    #[must_use]
    pub const fn is_l0_live(&self) -> bool {
        matches!(self, Self::L0)
    }

    /// Returns true if the transition is L0-live (dead).
    /// 
    /// This is a synonym for `is_l0_live`.
    #[must_use]
    pub const fn is_dead(&self) -> bool {
        self.is_l0_live()
    }

    /// Returns true if the transition is [`L1-live`](LivenessLevel::L1) or higher.
    #[must_use]
    pub const fn is_l1_live(&self) -> bool {
        matches!(self, Self::L1 | Self::L2 | Self::L3 | Self::L4)
    }

    /// Returns true if the transition is *strictly* L1-live, i.e. L1-live but not L2-live.
    #[must_use]
    pub const fn is_strictly_l1_live(&self) -> bool {
        matches!(self, Self::L1)
    }

    /// Returns true if the transition is [`L2-live`](LivenessLevel::L2) or higher.
    #[must_use]
    pub const fn is_l2_live(&self) -> bool {
        matches!(self, Self::L2 | Self::L3 | Self::L4)
    }

    /// Returns true if the transition is *strictly* L2-live, i.e. L2-live but not L3-live.
    #[must_use]
    pub const fn is_strictly_l2_live(&self) -> bool {
        matches!(self, Self::L2)
    }

    /// Returns true if the transition is [`L3-live`](LivenessLevel::L3) or higher.
    #[must_use]
    pub const fn is_l3_live(&self) -> bool {
        matches!(self, Self::L3 | Self::L4)
    }

    /// Returns true if the transition is *strictly* L3-live, i.e. L3-live but not L4-live.
    #[must_use]
    pub const fn is_strictly_l3_live(&self) -> bool {
        matches!(self, Self::L3)
    }

    /// Returns true if the transition is [`L4-live`](LivenessLevel::L4) (live).
    #[must_use]
    pub const fn is_l4_live(&self) -> bool {
        matches!(self, Self::L4)
    }

    /// Returns true if the transition is [`L4-live`](LivenessLevel::L4) (live).
    /// 
    /// This is a synonym for `is_l4_live`.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.is_l4_live()
    }
}

impl std::fmt::Display for LivenessLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L0 => write!(f, "L0-live"),
            Self::L1 => write!(f, "L1-live"),
            Self::L2 => write!(f, "L2-live"),
            Self::L3 => write!(f, "L3-live"),
            Self::L4 => write!(f, "L4-live"),
        }
    }
}