use crate::state_space::Omega;

/// Boundedness describes the maximum number of tokens that can
/// appear on a place in any reachable marking of a Petri net.
///
/// It is a fundamental property of Petri nets that has significant
/// implications for their behavior and the complexity of analyzing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Boundedness {
    /// A [`Place`] is bounded in a given [`PetriNet`] if there exists
    /// an upper limit on the number of tokens it will ever hold in any
    /// reachable marking in the Petri net's state space.
    ///
    /// A place is *k-bounded* if it is bounded and the upper limit on
    /// the number of tokens it can hold is less than or equal to `k`.
    ///
    /// A [`PetriNet`] is (k-)bounded if all of its places are (k-)bounded.
    ///
    /// Note that `k` might be a loose upper bound or not present at all;
    /// certain methods for computing boundedness may only be able to
    /// guarantee general boundedness without providing a specific `k`,
    /// or they may provide a `k` which is greater than the actual
    /// upper bound.
    Bounded(Option<K>),

    /// *Unboundedness* occurs when some [`Transition`] firing
    /// sequence can turn some reachable marking `M` into a
    /// marking `M'` where `M'` has at least as many tokens in
    /// all places as `M` and strictly more in at least one.
    ///
    /// Those places with strictly more tokens are unbounded:
    /// we can fire the same sequence of transitions from `M'`
    /// to reach a marking `M''`, which will have even more
    /// tokens on those places, and the firing sequence will
    /// be enabled in `M''` as well. The firing sequence will
    /// always be enabled due to the property of Petri nets
    /// known as *monotonicity*. We can therefore repeat the
    /// process indefinitely, generating markings with
    /// arbitrarily many tokens on any unbounded place.
    ///
    /// A Petri net is unbounded if it has one or more unbounded
    /// places. Unbounded Petri nets have an infinite state space,
    /// and are generally more difficult to analyze than bounded ones.
    Unbounded,
}

/// A type alias for the upper bound `k` in k-boundedness.
pub type K = usize;

impl Boundedness {
    /// Returns `true` if the place or Petri net is bounded.
    #[must_use]
    pub const fn is_bounded(self) -> bool {
        matches!(self, Boundedness::Bounded(_))
    }

    /// Returns `true` if the place or Petri net is known to be k-bounded
    /// for the given `k`. If we know that it is not k-bounded, or we
    /// cannot guarantee it to be k-bounded (due to loose bounds),
    /// this returns `false`.
    #[must_use]
    pub const fn is_k_bounded(self, k: K) -> bool {
        matches!(self, Boundedness::Bounded(Some(actual_k)) if actual_k <= k)
    }

    /// Returns `true` if the place or Petri net is unbounded.
    #[must_use]
    pub const fn is_unbounded(self) -> bool {
        matches!(self, Boundedness::Unbounded)
    }
}

impl From<u32> for Boundedness {
    fn from(value: u32) -> Self {
        Self::Bounded(Some(value as K))
    }
}

impl From<Omega> for Boundedness {
    fn from(value: Omega) -> Self {
        match value {
            Omega::Finite(n) => Self::Bounded(Some(n as K)),
            Omega::Unbounded => Self::Unbounded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Boundedness;

    #[test]
    fn test_ord() {
        assert!(Boundedness::Bounded(None) < Boundedness::Bounded(Some(0)));
        assert!(Boundedness::Bounded(Some(0)) < Boundedness::Bounded(Some(1)));
        assert!(Boundedness::Bounded(Some(1)) < Boundedness::Unbounded);
    }
}