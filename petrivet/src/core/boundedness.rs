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
    /// Note that `k` might be a loose upper bound: `k` might not
    /// be the smallest integer for which the place is k-bounded,
    /// depending on the method used to determine it.
    Bounded(K),

    /// *Unboundedness* describes the situation where there is
    /// no upper limit on the number of tokens that can appear
    /// on a [Place](crate::net::Place) in a reachable marking
    /// of a [Petri net](crate::system::PetriNet).
    ///
    /// It occurs if and only if a transition firing sequence
    /// turns some marking `M` into a different marking
    /// `M'` where `M'` has at least as many tokens in all
    /// places as `M` and strictly more in at least one.
    ///
    /// Those places with strictly more tokens are unbounded:
    /// we can fire the same sequence of transitions from `M'`
    /// to reach a marking `M''`, which will have even more
    /// tokens on those places, and the firing sequence will
    /// again be enabled in `M''`. The firing sequence will
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
    pub const fn is_bounded(&self) -> bool {
        matches!(self, &Boundedness::Bounded(_))
    }

    /// Returns `true` if the place or Petri net is known to be k-bounded
    /// for the given `k`. If we know that it is not k-bounded, or we
    /// cannot guarantee it to be k-bounded (due to loose bounds),
    /// this returns `false`.
    #[must_use]
    pub const fn is_k_bounded(&self, k: K) -> bool {
        matches!(self, &Boundedness::Bounded(bound) if bound <= k)
    }

    /// Returns `true` if the place or Petri net is safe (1-bounded).
    #[must_use]
    pub const fn is_safe(&self) -> bool {
        self.is_k_bounded(1)
    }

    /// Returns `true` if the place or Petri net is unbounded.
    #[must_use]
    pub const fn is_unbounded(&self) -> bool {
        matches!(self, Boundedness::Unbounded)
    }
}

impl std::fmt::Display for Boundedness {
    /// Formats `Bounded(k)` as `bounded (k)` and `Unbounded` as `unbounded`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bounded(k) => write!(f, "bounded ({k})"),
            Self::Unbounded => write!(f, "unbounded"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Boundedness;

    #[test]
    fn test_ord() {
        assert!(Boundedness::Bounded(0) < Boundedness::Bounded(1));
        assert!(Boundedness::Bounded(usize::MAX) < Boundedness::Unbounded);
    }

    #[test]
    fn display() {
        assert_eq!(Boundedness::Bounded(3).to_string(), "bounded (3)");
        assert_eq!(Boundedness::Unbounded.to_string(), "unbounded");
    }
}