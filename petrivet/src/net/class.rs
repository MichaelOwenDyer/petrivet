use std::fmt;
use crate::{Place, Transition};
use crate::net::{PlaceMap, TransitionMap};
use super::sorted_set::SortedSet;

/// TODO: Add examples of each class of net with doctests asserting the classification correctly identifies the class.
/// Structural classification of a Petri net.
///
/// The classes form an inclusion hierarchy (each is a subclass of the next):
///
/// ```text
/// Circuit ⊂ S-net ⊂ Free-choice ⊂ AsymmetricChoice ⊂ Unrestricted
/// Circuit ⊂ T-net ⊂ Free-choice ⊂ AsymmetricChoice ⊂ Unrestricted
/// ```
///
/// `classify_net` returns the most specific class.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum NetClass {

    /// A net `N = (S, T, F)` is a **Circuit** if
    /// |•t| = |t•| = 1 for every t ∈ T and
    /// |•s| = |s•| = 1 for every s ∈ S.
    /// A system `(N, M<sub>0</sub>)` is a **Circuit system** if N is a circuit.
    /// Circuits represent the intersection of S-nets and T-nets,
    /// and are the most structurally restricted class of nets.
    ///
    /// Intuitively, a circuit is a single closed loop of places and transitions.
    /// This is the simplest class of Petri net, both structurally and behaviorally.
    /// In particular, the token count in the circuit is conserved,
    /// and the circuit is live iff the initial marking has a positive token count.
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
    ///
    /// ```
    /// use petrivet::class::NetClass;
    /// use petrivet::Net;
    /// let mut b = Net::builder();
    /// let [p1, p2, p3] = b.add_places();
    /// let [t1, t2, t3] = b.add_transitions();
    /// b.add_arcs((p1, t1, p2, t2, p3, t3, p1));
    /// let net = b.build().unwrap();
    /// assert!(net.class() == NetClass::Circuit);
    /// assert!(net.is_circuit());
    /// assert!(net.is_s_net());
    /// assert!(net.is_t_net());
    /// assert!(net.is_free_choice_net());
    /// assert!(net.is_asymmetric_choice_net());
    /// ```
    Circuit,

    /// A net `N = (S, T, F)` is an **S-net** (or **State Machine**) if
    /// `|•t| = |t•| = 1` for every transition `t ∈ T`.
    /// (N, M<sub>0</sub>) is an **S-system** if N is an S-net.
    ///
    /// In other words, a net is a state machine if each transition has
    /// exactly one input and one output place. It is therefore impossible
    /// to represent concurrency in a state machine; state machines can only
    /// model decisions (nondeterminism) [Murata III A]. todo cite
    ///
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
    /// - S-invariants of S-nets: // todo: apply optimization
    ///   Let N = (S, T, F) be an S-net. A vector I: S → Q is an S-invariant of N
    ///   iff I = (x, ..., x) for some x ∈ Q.
    ///
    /// # Example
    ///
    /// The following example encodes the state diagram of a vending machine which
    /// accepts 5-cent and 10-cent coins and sells 15-cent and 20-cent candy bars.
    /// For simplicity, the vending machine has a maximum balance of 20 cents and
    /// does not return change.
    ///
    /// Credit for this example goes to [Murata Figure 4]. // todo cite properly
    ///
    /// ```
    /// use petrivet::class::NetClass;
    /// use petrivet::Net;
    /// let mut b = Net::builder();
    /// let [bal_0, bal_5, bal_10, bal_15, bal_20] = b.add_places();
    /// let [bal_0_dep_5, bal_0_dep_10, bal_5_dep_5, bal_5_dep_10, bal_10_dep_5,
    ///     bal_10_dep_10, bal_15_dep_5, get_candy_for_15, get_candy_for_20] = b.add_transitions();
    /// b.add_arcs((bal_0, bal_0_dep_5, bal_5));
    /// b.add_arcs((bal_0, bal_0_dep_10, bal_10));
    /// b.add_arcs((bal_5, bal_5_dep_5, bal_10));
    /// b.add_arcs((bal_5, bal_5_dep_10, bal_15));
    /// b.add_arcs((bal_10, bal_10_dep_5, bal_15));
    /// b.add_arcs((bal_10, bal_10_dep_10, bal_20));
    /// b.add_arcs((bal_15, bal_15_dep_5, bal_20));
    /// b.add_arcs((bal_15, get_candy_for_15, bal_0));
    /// b.add_arcs((bal_20, get_candy_for_20, bal_0));
    /// let net = b.build().unwrap();
    /// assert!(net.class() == NetClass::SNet);
    /// assert!(!net.is_circuit());
    /// assert!(net.is_s_net());
    /// assert!(!net.is_t_net());
    /// assert!(net.is_free_choice_net());
    /// assert!(net.is_asymmetric_choice_net());
    /// ```
    SNet,

    /// A net `N = (S, T, F)` is a **T-net** (or **Marked Graph**) if
    /// `|•s| = |s•| = 1` for every place `s ∈ S`.
    /// `(N, M<sub>0</sub>)` is a **T-system** if N is a T-net.
    ///
    /// In other words, a net is a marked graph if each place has exactly
    /// one input and one output transition. It is therefore impossible to
    /// express decisions in a marked graph; marked graphs model purely
    /// deterministic concurrent systems [Murata VII]. TODO: cite
    ///
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
    ///
    /// ```
    /// use petrivet::class::NetClass;
    /// use petrivet::Net;
    /// let mut b = Net::builder();
    /// let [p1, p2, p3, p4, p5] = b.add_places();
    /// let [t1, t2, t3, t4] = b.add_transitions();
    /// b.add_arcs((t1, p1, t2, p3, t4));
    /// b.add_arcs((t1, p2, t3, p4, t4));
    /// b.add_arcs((t4, p5, t1));
    /// let net = b.build().unwrap();
    /// assert!(net.class() == NetClass::TNet);
    /// assert!(!net.is_circuit());
    /// assert!(!net.is_s_net());
    /// assert!(net.is_t_net());
    /// assert!(net.is_free_choice_net());
    /// assert!(net.is_asymmetric_choice_net());
    /// ```
    TNet,

    /// TODO: Cite Prof. Esparza
    /// A net `N = (S, T, F)` is an (extended) **Free-Choice Net**
    /// if `•t x s• ⊆ F` for every `s ∈ S` and `t ∈ T` such that `(s, t) ∈ F`.
    ///
    /// Alternative definitions:
    /// - for every two transitions `t1, t2 ∈ T`,
    ///   if `•t1 ∩ •t2 ≠ ∅` then `•t1 = •t2`.
    ///   In other words, if two transitions share any input place,
    ///   they must share all input places.
    ///
    /// - for every two places `s1, s2 ∈ S`,
    ///   if `s1• ∩ s2• ≠ ∅` then `s1• = s2•`.
    ///   In other words, if two places share any output transition,
    ///   they must share all output transitions.
    ///
    /// Free-choice nets can model both choice and concurrency, but with a key restriction to prevent
    /// "confusion": the difficult-to-analyze case where two transitions share some but not all input places,
    /// leading to complex interactions between choices and concurrency [Murata III B]. todo cite
    /// In a free-choice net, two transitions either share all input places or none.
    ///
    /// This enables various structural analysis techniques, most notably the
    /// Commoner's Liveness Theorem (citation needed) which is the last polynomial-time
    /// characterization of liveness for a non-trivial class of Petri nets.
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
    FreeChoice,

    /// TODO: write overview of asymmetric choice nets
    /// A net `N = (S, T, F)` is an **Asymmetric-choice Net** if for every two transitions t1, t2:
    /// if •t1 ∩ •t2 ≠ ∅ then •t1 ⊆ •t2 or •t2 ⊆ •t1.
    /// For every two places s1, s2: if s1• ∩ s2• ≠ ∅ then s1• ⊆ s2• or s2• ⊆ s1•.
    /// Asymmetric-choice nets allow one-sided resource sharing (e.g. a shared
    /// resource plus a private resource), but forbid symmetric conflicts.
    AsymmetricChoice,
    /// No structural restrictions.
    /// Can model arbitrary concurrency, choices, and conflicts.
    /// TODO: Write overview of unrestricted nets.
    Unrestricted,
}

impl fmt::Display for NetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetClass::Circuit => write!(f, "Circuit"),
            NetClass::SNet => write!(f, "S-net"),
            NetClass::TNet => write!(f, "T-net"),
            NetClass::FreeChoice => write!(f, "Free-choice"),
            NetClass::AsymmetricChoice => write!(f, "Asymmetric-choice"),
            NetClass::Unrestricted => write!(f, "Unrestricted"),
        }
    }
}

#[must_use]
pub fn classify(
    preset_t: &TransitionMap<SortedSet<Place>>,
    postset_t: &TransitionMap<SortedSet<Place>>,
    preset_p: &PlaceMap<SortedSet<Transition>>,
    postset_p: &PlaceMap<SortedSet<Transition>>,
) -> NetClass {
    let is_s = is_s_net(preset_t, postset_t);
    let is_t = is_t_net(preset_p, postset_p);
    match (is_s, is_t) {
        (true, true) => NetClass::Circuit,
        (true, false) => NetClass::SNet,
        (false, true) => NetClass::TNet,
        (false, false) if is_free_choice_net(postset_p, preset_t) => NetClass::FreeChoice,
        (false, false) if is_asymmetric_choice_net(postset_p) => NetClass::AsymmetricChoice,
        _ => NetClass::Unrestricted,
    }
}

/// S-net: |•t| = 1 and |t•| = 1 for every transition t.
fn is_s_net(transition_presets: &TransitionMap<SortedSet<Place>>, transition_postsets: &TransitionMap<SortedSet<Place>>) -> bool {
    transition_presets.values().all(|preset| preset.len() == 1) &&
    transition_postsets.values().all(|postset| postset.len() == 1)
}

/// T-net: |•p| = 1 and |p•| = 1 for every place p.
fn is_t_net(place_presets: &PlaceMap<SortedSet<Transition>>, place_postsets: &PlaceMap<SortedSet<Transition>>) -> bool {
    place_presets.values().all(|preset| preset.len() == 1) &&
    place_postsets.values().all(|postset| postset.len() == 1)
}

/// Free-choice: ∀ p1, p2: p1• ∩ p2• ≠ ∅ ⟹ p1• = p2•.
/// Equivalently: for every place p, all transitions in p• share the same preset.
fn is_free_choice_net(place_postsets: &PlaceMap<SortedSet<Transition>>, preset_t: &TransitionMap<SortedSet<Place>>) -> bool {
    place_postsets.iter().all(|(_p, postset)| {
        postset.array_windows().all(|&[t0, t1]| {
            preset_t[t0] == preset_t[t1]
        })
    })
}

/// Asymmetric-choice: ∀ p1, p2: p1• ∩ p2• ≠ ∅ ⟹ p1• ⊆ p2• ∨ p2• ⊆ p1•.
fn is_asymmetric_choice_net(place_postsets: &PlaceMap<SortedSet<Transition>>) -> bool {
    place_postsets.values().all(|a| {
        place_postsets.values().all(|b| {
            !a.intersects(b) || a.is_subset_of(b) || b.is_subset_of(a)
        })
    })
}
