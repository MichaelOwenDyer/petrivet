use crate::behavior::NormalMarking;
use crate::structure::Net;

/// A net N = (S, T, F) is a `Free-Choice Net` if •t x s• ⊆ F
/// for every s ∈ S and t ∈ T such that (s, t) ∈ F.
///
/// Alternative definitions:
/// - A net is free-choice if for every two transitions t1, t2 ∈ T,
///   if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
///   In other words, if two transitions share any input place,
///   they must share all input places.
///
/// - A net is free-choice if for every two places s1, s2 ∈ S,
///   if s1• ∩ s2• ≠ ∅ then s1• = s2•.
///   In other words, if two places share any output transition,
///   they must share all output transitions.
///
/// Technically this is an "extended free-choice net",
/// but the non-extended version is rarely used in practice
/// as it is more restrictive and does not have any
/// significant analytical advantages.
///
/// Free-choice nets can model both choice and concurrency,
/// but with a key restriction: contested resources (places with multiple output transitions)
/// must be consumed entirely by any transition they enable.
///
/// Commoner's Liveness Theorem:
/// A free-choice net (N, M<sub>0</sub>) is live iff every siphon of N
/// contains a trap marked at M<sub>0</sub>.
///
/// Boundedness Theorem:
/// (Heck's Boundedness Theorem)
/// Let (N, M<sub>0</sub>) be a live free-choice system.
/// Then (N, M<sub>0</sub>) is bounded iff every place of N belongs to an S-component.
/// An S-component is a subnet N' = (S', T', F') of a net N such that:
/// - N' is a strongly connected S-net.
/// - T' = •S' ∪ S'• (all transitions connected to places in S' are included in T').
///
/// Let (N, M<sub>0</sub>) be a live and bounded free-choice system and let s be a place of N.
/// We have max{M(s) | M is reachable} = min{M<sub>0</sub>(S') | S' is an S-component containing s}.
/// Intuitively, a place can only have as many tokens as the minimum number of tokens
/// in any S-component it belongs to.
/// If all places belong to some S-component, then the entire net is bounded.
///
/// Simultaneous Liveness and Boundedness Theorem:
/// A free-choice system (N, M<sub>0</sub>) is live and bounded iff
/// 1. N has a positive S-invariant
/// 2. N has a positive T-invariant
/// 3. The rank of the incidence matrix of N is equal to c - 1, where c is the number of clusters of N.
/// 4. Every proper siphon of N is marked at M<sub>0</sub>.
///
/// Reachability theorem:
/// Let (N, M<sub>0</sub>) be a live and bounded free-choice system.
/// A marking M is reachable from M<sub>0</sub> iff there exists X ∈ N^|T| such that:
/// - M = M<sub>0</sub> + N * X, where N is the incidence matrix of N
/// - (N<sub>U</sub>, M<sub>U</sub>) has no unmarked traps,
///   where U = {t ∈ T | X(t) = 0}, N<sub>U</sub> is the subnet induced by U,
///   and M<sub>U</sub> is the projection of M onto the places of N<sub>U</sub>.
///
/// This problem is decidable in polynomial time (!).
/// Given: a live, bounded, and cyclic free-choice system (N, M0) and a marking M
/// Decide: is M reachable?
///
/// A live and bounded free-choice system (N, M<sub>0</sub>) is cyclic iff
/// M<sub>0</sub> marks every proper trap of N.
///
/// Shortest sequence theorem:
/// Let (N, M<sub>0</sub>) be a b-bounded free-choice system and let M be a reachable marking.
/// Then there is a firing sequence M<sub>0</sub> <sup>σ</sup>→ M
/// such that `|σ| ≤ bn(n+1)(n+2)/6`, where n = |T| is the number of transitions of N.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeChoiceNet(pub(crate) Net);

/// A net N = (S, T, F) is a `T-net` if |•s| = |s•| = 1 for every place s ∈ S.
/// This means each place has exactly one input and one output transition.
/// T-nets can model concurrency and synchronization, but not choices.
/// (N, M<sub>0</sub>) is a `T-system` if N is a T-net.
/// This structural restriction implies several important properties:
///
/// - Fundamental property:
///
///   Notation: Let γ be a circuit of a net N and let M be a marking of N. We denote
///   by M(γ) the number if tokens of γ under M, that is, M(γ) = Σ<sub>s∈γ</sub> M(s).
///
///   Let γ be a circuit of a T-system (N, M<sub>0</sub>) and let M be a reachable marking.
///   Then M(γ) = M<sub>0</sub>(γ).
///   Intuitively, the number of tokens in each circuit is constant.
///
/// - Liveness theorem:
///   A T-system (N, M<sub>0</sub>) is live iff M<sub>0</sub>(γ) > 0 for every circuit γ of N.
///   Intuitively, a T-system is live iff every circuit contains at least one token.
///
/// - Boundedness theorem:
///   A live T-system (N, M<sub>0</sub>) is bounded iff N is strongly connected.
///   A place s of a live T-system (N,M<sub>0</sub>) is bounded iff it belongs to some circuit γ,
///   and b-bounded iff M<sub>0</sub>(γ) ≤ b.
///   More specifically, max{M(s) | M is reachable} = min{M<sub>0</sub>(γ) | γ contains s}.
///   Intuitively, a place can only have as many tokens as the minimum number of tokens in any
///   circuit it belongs to. If all places belong to some circuit, then the entire net is strongly
///   connected and thus bounded.
///
/// - Reachability theorem:
///   Let (N,M<sub>0</sub>) be a live T-system.
///   A marking M is reachable from M0 iff M<sub>0</sub> ∼ M.
///   For ordinary nets, reachability implies M<sub>0</sub> ∼ M,
///   but the converse is not true in general.
///   This is a very powerful result, as it allows to decide reachability by solving a system of
///   linear equations, as opposed to only disproving reachability when no solution exists.
///
/// - T-invariants of T-nets:
///   Let N = (S, T, F) be a T-net. A vector J: T → Q is a T-invariant of N
///   iff J = (x, ..., x) for some x ∈ Q.
///   Intuitively, firing all transitions the same number of times has no net effect on the marking.
///
/// - Let N be a strongly connected T-net. For every marking M<sub>0</sub> the following statements
///   are equivalent:
///   1. (N, M<sub>0</sub>) is live.
///   2. (N, M<sub>0</sub>) is deadlock-free.
///   3. (N, M<sub>0</sub>) has an infinite firing sequence.
///
/// - Genrich's theorem:
///   Let N be a strongly connected T-net with at least one place and one transition.
///   There exists a marking M<sub>0</sub> such that (N, M<sub>0</sub>) is live and 1-bounded.
///
/// - Let (N, M<sub>0</sub>) be a 1-bounded T-system (live or not).
///   For any two markings M<sub>1</sub> and M<sub>2</sub>, if M<sub>2</sub> is reachable from M<sub>1</sub>,
///   then it can be reached in at most n(n-1)/2 steps, where n = |T| is the number of transitions.
///
/// - Let (N, M<sub>0</sub>) be a b-bounded T-system (live or not).
///   For any marking M reachable from M<sub>0</sub>, there exists a firing sequence
///   M<sub>0</sub> <sup>σ</sup>→ M such that |σ| ≤ b * n(n-1)/2, where n = |T| is the number of transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TNet(pub(crate) Net);

/// A net N = (S, T, F) is an `S-net` if |•t| = |t•| = 1 for every transition t ∈ T.
/// This means each transition has exactly one input and one output place.
/// S-nets can model sequential processes and choices, but not concurrency.
/// (N, M<sub>0</sub>) is an `S-system` if N is an S-net.
/// This structural restriction implies several important properties:
/// - Fundamental property:
///   Let (N, M0) be an S-system with N = (S,T,F).
///   Then M<sub>0</sub>(S) = M(S) for every reachable marking M.
/// - Liveness theorem:
///   An S-system (N, M<sub>0</sub>) where N = (S, T, F) is live
///   iff N is strongly connected and M<sub>0</sub>(S) > 0.
/// - Boundedness theorem:
///   A live S-system (N, M<sub>0</sub>) where N = (S, T, F) is b-bounded
///   iff M<sub>0</sub>(S) ≤ b.
/// - Reachability theorem:
///   Let (N, M<sub>0</sub>) be a live S-system with N = (S, T, F)
///   and let M be a marking of N. M is reachable from M<sub>0</sub>
///   iff M(S) = M<sub>0</sub>(S).
/// - S-invariants of S-nets:
///   Let N = (S, T, F) be an S-net. A vector I: S → Q is an S-invariant of N
///   iff I = (x, ..., x) for some x ∈ Q.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SNet(pub(crate) Net);

impl SNet {
    #[must_use]
    pub fn is_live(&self, initial_marking: &NormalMarking) -> bool {
        self.0.is_strongly_connected() && !initial_marking.is_zero()
    }
}

/// A net N = (S, T, F) is a `circuit` iff it is both an S-net and a T-net,
/// i.e., |•t| = |t•| = 1 for every t ∈ T and |•s| = |s•| = 1 for every s ∈ S.
///
/// Liveness theorem:
/// A circuit (N, M<sub>0</sub>) is live iff M<sub>0</sub>(S) > 0.
///
/// Boundedness theorem:
/// A circuit (N, M<sub>0</sub>) is b-bounded iff M<sub>0</sub>(S) ≤ b.
///
/// Reachability theorem:
/// A marking M is reachable from M<sub>0</sub> in a circuit (N, M<sub>0</sub>)
/// iff M(S) = M<sub>0</sub>(S).
///
/// S-invariants and T-invariants of circuits:
/// Let N = (S, T, F) be a circuit. A vector I: S → Q is an S-invariant of N
/// iff I = (x, ..., x) for some x ∈ Q. Similarly, a vector J: T → Q is a T-invariant of N
/// iff J = (y, ..., y) for some y ∈ Q.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Circuit(pub(crate) Net);

/// Structural classification of Petri nets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureClass {
    /// The most general class of Petri nets, with no structural restrictions.
    Unrestricted(Net),
    /// Subclass of unrestricted nets where for every two transitions t1, t2 ∈ T,
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 = •t2.
    FreeChoiceNet(FreeChoiceNet),
    /// Subclass of free-choice nets where |•s| = |s•| = 1 for every place s ∈ S.
    TNet(TNet),
    /// Subclass of free-choice nets where |•t| = |t•| = 1 for every transition t ∈ T.
    SNet(SNet),
    /// A net fulfilling both the S-net and T-net properties.
    Circuit(Circuit),
}

impl StructureClass {
    #[must_use]
    pub fn into_inner(self) -> Net {
        match self {
            StructureClass::Unrestricted(net) => net,
            StructureClass::FreeChoiceNet(FreeChoiceNet(net)) => net,
            StructureClass::TNet(TNet(net)) => net,
            StructureClass::SNet(SNet(net)) => net,
            StructureClass::Circuit(Circuit(net)) => net,
        }
    }
    pub fn inner(&self) -> &Net {
        match self {
            StructureClass::Unrestricted(net) => net,
            StructureClass::FreeChoiceNet(FreeChoiceNet(net)) => net,
            StructureClass::TNet(TNet(net)) => net,
            StructureClass::SNet(SNet(net)) => net,
            StructureClass::Circuit(Circuit(net)) => net,
        }
    }
}

impl TryFrom<Net> for Circuit {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_circuit() {
            Ok(Circuit(net))
        } else {
            Err(net)
        }
    }
}

impl TryFrom<Net> for SNet {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_s_net() {
            Ok(SNet(net))
        } else {
            Err(net)
        }
    }
}

impl TryFrom<Net> for TNet {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_t_net() {
            Ok(TNet(net))
        } else {
            Err(net)
        }
    }
}

impl TryFrom<Net> for FreeChoiceNet {
    type Error = Net;

    fn try_from(net: Net) -> Result<Self, Self::Error> {
        if net.is_free_choice() {
            Ok(FreeChoiceNet(net))
        } else {
            Err(net)
        }
    }
}