use crate::{Node, Place, Transition};
use std::collections::{HashMap, HashSet, VecDeque};
use std::{fmt, iter};

/// TODO: Add examples of each class of net with doctests asserting the classification correctly identifies the class.
/// Structural classification of a Petri net.
///
/// The classes form an inclusion hierarchy (each is a subclass of the next):
///
/// ```text
/// Circuit ⊂ S-net ⊂ Free-choice ⊂ AsymmetricChoice ⊂ General
/// Circuit ⊂ T-net ⊂ Free-choice ⊂ AsymmetricChoice ⊂ General
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
    /// assert!(net.is_state_machine());
    /// assert!(net.is_marked_graph());
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
    ///
    ///   For S-nets, the marking equation is both necessary and sufficient:
    ///   `M'` is reachable from `M₀` if and only if every S-invariant is
    ///   preserved (`y · M' = y · M₀` for all S-invariants `y`). This is
    ///   equivalent to the LP marking equation being feasible.
    ///
    ///   This turns reachability, normally Ackermann-complete for general nets,
    ///   into a polynomial-time check for S-nets.
    /// - S-invariants of S-nets: // todo: implement this
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
    ///
    /// // Note how in an S-net,
    /// // every transition is just a simple bridge between two places.
    /// b.add_arcs((bal_0, bal_0_dep_5, bal_5));
    /// b.add_arcs((bal_0, bal_0_dep_10, bal_10));
    /// b.add_arcs((bal_5, bal_5_dep_5, bal_10));
    /// b.add_arcs((bal_5, bal_5_dep_10, bal_15));
    /// b.add_arcs((bal_10, bal_10_dep_5, bal_15));
    /// b.add_arcs((bal_10, bal_10_dep_10, bal_20));
    /// b.add_arcs((bal_15, bal_15_dep_5, bal_20));
    /// b.add_arcs((bal_15, get_candy_for_15, bal_0));
    /// b.add_arcs((bal_20, get_candy_for_20, bal_0));
    /// b.add_arcs((bal_20, get_candy_for_15, bal_5));
    /// let net = b.build().unwrap();
    /// assert!(net.class() == NetClass::StateMachine);
    /// assert!(!net.is_circuit());
    /// assert!(net.is_state_machine());
    /// assert!(!net.is_marked_graph());
    /// assert!(net.is_free_choice_net());
    /// assert!(net.is_asymmetric_choice_net());
    /// ```
    ///
    /// References:
    /// - [Murata 1989, Theorem 21](crate::literature#theorem-21--reachability-in-s-nets)
    /// - Lautenbach & Thiagarajan 1979 (original result)
    StateMachine,

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
    ///
    ///   In a T-net, every non-negative integer solution to the marking equation
    ///   `M' = M₀ + N · x` corresponds to a realizable firing sequence.
    ///
    ///   This turns reachability into an ILP feasibility check, which is
    ///   NP-complete but in practice hugely more efficient than the
    ///   Ackermann-complete reachability problem for general nets.
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
    /// assert!(net.class() == NetClass::MarkedGraph);
    /// assert!(!net.is_circuit());
    /// assert!(!net.is_state_machine());
    /// assert!(net.is_marked_graph());
    /// assert!(net.is_free_choice_net());
    /// assert!(net.is_asymmetric_choice_net());
    /// ```
    ///
    /// References:
    /// - [Murata 1989, Theorem 22](crate::literature#theorem-22--reachability-in-t-nets)
    MarkedGraph,

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

    /// A net `N = (S, T, F)` is an **Asymmetric-choice Net** (also known as a **Simple Net**)
    /// if for every two places `s1, s2 ∈ S`:
    /// if `s1• ∩ s2• ≠ ∅` then `s1• ⊆ s2•` or `s2• ⊆ s1•`.
    ///
    /// Asymmetric-choice nets allow one-sided resource sharing (e.g., a shared
    /// resource plus a private resource), but strictly forbid symmetric conflicts (symmetric confusion).
    ///
    /// Liveness theorem:
    /// An asymmetric-choice net `(N, M<sub>0</sub>)` is live if (but not only if) every siphon
    /// in `N` contains a marked trap at `M<sub>0</sub>`.
    /// Furthermore, an asymmetric-choice net `(N, M<sub>0</sub>)` is live iff it is *place-live*;
    /// that is, for each reachable marking `M` and each place `s ∈ S`, there exists a marking
    /// `M'` reachable from `M` such that `M'(s) > 0`.
    ///
    /// ```
    /// use petrivet::class::NetClass;
    /// use petrivet::Net;
    /// let mut b = Net::builder();
    /// let [p1, p2] = b.add_places();
    /// let [t1, t2] = b.add_transitions();
    /// // A shared resource p2 and a private resource p1.
    /// // p1• = {t1}, p2• = {t1, t2}. Since p1• ∩ p2• = {t1}, and p1• ⊆ p2•, this is an AC net.
    /// b.add_arcs((p1, t1));
    /// b.add_arcs((p2, t1));
    /// b.add_arcs((p2, t2));
    /// let net = b.build().unwrap();
    /// assert!(net.class() == NetClass::AsymmetricChoice);
    /// assert!(!net.is_free_choice_net());
    /// assert!(net.is_asymmetric_choice_net());
    /// ```
    AsymmetricChoice,

    /// No structural restrictions.
    /// Can model arbitrary concurrency, choices, and conflicts.
    ///
    /// A net falls into the **General** class if it does not satisfy the structural restrictions
    /// of any of the more restrictive classes (Circuit, S-Net, T-Net, Free-Choice, or
    /// Asymmetric-Choice). General Petri nets are fully expressive. While ordinary general Petri
    /// nets are strictly less powerful than Turing machines, they can still model highly complex
    /// distributed, asynchronous, and nondeterministic systems. (Adding inhibitor
    /// arcs or prioritizing transitions increases their modeling power to the level of Turing machines).
    ///
    /// Because they lack structural restrictions, analyzing general Petri nets often requires
    /// exhaustive state space exploration (e.g., constructing a reachability or coverability tree)
    /// or solving algebraic state equations, rather than relying purely on polynomial-time structural
    /// properties.
    ///
    /// Reachability theorem:
    /// The reachability problem for general Petri nets is decidable, but it has been shown to be
    /// [Ackermann-complete](https://en.wikipedia.org/wiki/Ackermann_function) [Czerwiński and Orlikowski 2021].
    ///
    /// ```
    /// use petrivet::class::NetClass;
    /// use petrivet::Net;
    /// let mut b = Net::builder();
    /// let [p1, p2] = b.add_places();
    /// let [t1, t2, t3] = b.add_transitions();
    ///
    /// // Create a symmetric confusion (which violates Asymmetric-Choice rules).
    /// // p1• = {t1, t2} and p2• = {t2, t3}
    /// // p1• ∩ p2• = {t2}. Neither is a subset of the other.
    /// b.add_arcs((p1, t1));
    /// b.add_arcs((p1, t2));
    /// b.add_arcs((p2, t2));
    /// b.add_arcs((p2, t3));
    /// let net = b.build().unwrap();
    /// assert!(net.class() == NetClass::General);
    /// assert!(!net.is_asymmetric_choice_net());
    /// ```
    General,
}

impl NetClass {
    /// Returns true if this class is `Circuit`.
    #[must_use]
    pub const fn is_circuit(&self) -> bool {
        matches!(self, Self::Circuit)
    }

    /// Returns true if this class is `SNet` or `Circuit`
    /// (since circuits are a subclass of S-nets).
    #[must_use]
    pub const fn is_state_machine(&self) -> bool {
        matches!(self, Self::StateMachine | Self::Circuit)
    }

    /// Returns true if this class is `TNet` or `Circuit`
    /// (since circuits are a subclass of T-nets).
    #[must_use]
    pub const fn is_marked_graph(&self) -> bool {
        matches!(self, Self::MarkedGraph | Self::Circuit)
    }

    /// Returns true if this class is `FreeChoice`, `StateMachine`, `MarkedGraph`, or `Circuit`
    /// (since all of these are subclasses of Free-Choice nets).
    #[must_use]
    pub const fn is_free_choice(&self) -> bool {
        matches!(self, Self::FreeChoice | Self::StateMachine | Self::MarkedGraph | Self::Circuit)
    }

    /// Returns true if this class is `AsymmetricChoice`, `FreeChoice`, `StateMachine`, `MarkedGraph`, or `Circuit`
    /// (since all of these are subclasses of Asymmetric-Choice nets).
    #[must_use]
    pub const fn is_asymmetric_choice(&self) -> bool {
        matches!(self, Self::AsymmetricChoice | Self::FreeChoice | Self::StateMachine | Self::MarkedGraph | Self::Circuit)
    }
}

impl fmt::Display for NetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetClass::Circuit => write!(f, "Circuit"),
            NetClass::StateMachine => write!(f, "S-net"),
            NetClass::MarkedGraph => write!(f, "T-net"),
            NetClass::FreeChoice => write!(f, "Free-choice net"),
            NetClass::AsymmetricChoice => write!(f, "Asymmetric-choice net"),
            NetClass::General => write!(f, "General net"),
        }
    }
}

/// Check if the net is strongly connected.
/// This is a prerequisite for classifying or building a net in this library.
/// It is expected that these maps are eagerly populated, symmetrically consistent,
/// and non-empty.
pub(crate) fn is_connected<S: std::hash::BuildHasher>(
    preset_t: &HashMap<Transition, HashSet<Place, S>, S>,
    postset_t: &HashMap<Transition, HashSet<Place, S>, S>,
    preset_p: &HashMap<Place, HashSet<Transition, S>, S>,
    postset_p: &HashMap<Place, HashSet<Transition, S>, S>,
) -> bool {
    let n_nodes = preset_p.len() + preset_t.len();
    if n_nodes > 0 {
        let mut visited_p = HashSet::new();
        let mut visited_t = HashSet::new();
        let mut queue = VecDeque::new();
        let first_place = preset_p.keys().next().expect("n_places > 0").to_owned();
        visited_p.insert(first_place);
        queue.push_back(Node::Place(first_place));
        while let Some(node) = queue.pop_front() {
            match node {
                Node::Place(p) => {
                    for &t in iter::chain(
                        preset_p.get(&p).expect("eagerly populated").iter(),
                        postset_p.get(&p).expect("eagerly populated").iter()
                    ) {
                        if visited_t.insert(t) {
                            queue.push_back(Node::Transition(t));
                        }
                    }
                }
                Node::Transition(t) => {
                    for &p in iter::chain(
                        preset_t.get(&t).expect("eagerly populated").iter(),
                        postset_t.get(&t).expect("eagerly populated").iter()
                    ) {
                        if visited_p.insert(p) {
                            queue.push_back(Node::Place(p));
                        }
                    }
                }
            }
        }
        if visited_p.len() + visited_t.len() != n_nodes {
            return false;
        }
    }
    true
}

/// Classify a net based on its structural properties.
/// Returns the most specific class of net that it belongs to.
/// If the net is not connected, returns None (since this library only supports connected nets).
/// It is expected that these maps are symmetrically consistent
/// (e.g. if `p` is in `preset_t[t]` then `t` is in `postset_p[p]`)
/// and eagerly populated for all nodes (e.g. if `t` has no preset,
/// then `preset_t[t]` is an empty set, not missing).
#[allow(clippy::similar_names)]
pub(crate) fn classify<S: std::hash::BuildHasher>(
    preset_t: &HashMap<Transition, HashSet<Place, S>, S>,
    postset_t: &HashMap<Transition, HashSet<Place, S>, S>,
    preset_p: &HashMap<Place, HashSet<Transition, S>, S>,
    postset_p: &HashMap<Place, HashSet<Transition, S>, S>
) -> Option<NetClass> {
    if !is_connected(preset_t, postset_t, preset_p, postset_p) {
        return None;
    }
    let is_s_net = is_s_net(preset_t, postset_t);
    let is_t_net = is_t_net(preset_p, postset_p);
    let class = match (is_s_net, is_t_net) {
        (true, true) => NetClass::Circuit,
        (true, false) => NetClass::StateMachine,
        (false, true) => NetClass::MarkedGraph,
        (false, false) if is_free_choice_net(postset_p, preset_t) => NetClass::FreeChoice,
        (false, false) if is_asymmetric_choice_net(postset_p) => NetClass::AsymmetricChoice,
        _ => NetClass::General,
    };
    Some(class)
}

/// S-net: |•t| = 1 and |t•| = 1 for every transition t.
fn is_s_net<S: std::hash::BuildHasher>(
    transition_presets: &HashMap<Transition, HashSet<Place, S>, S>,
    transition_postsets: &HashMap<Transition, HashSet<Place, S>, S>
) -> bool {
    transition_presets.values().all(|preset| preset.len() == 1)
        && transition_postsets.values().all(|postset| postset.len() == 1)
}

/// T-net: |•p| = 1 and |p•| = 1 for every place p.
fn is_t_net<S: std::hash::BuildHasher>(
    place_presets: &HashMap<Place, HashSet<Transition, S>, S>,
    place_postsets: &HashMap<Place, HashSet<Transition, S>, S>
) -> bool {
    place_presets.values().all(|preset| preset.len() == 1)
        && place_postsets.values().all(|postset| postset.len() == 1)
}

/// Free-choice: ∀ p1, p2: p1• ∩ p2• ≠ ∅ ⟹ p1• = p2•.
/// Equivalently: for every place p, all transitions in p• share the same preset.
fn is_free_choice_net<S: std::hash::BuildHasher>(
    postset_p: &HashMap<Place, HashSet<Transition, S>, S>,
    preset_t: &HashMap<Transition, HashSet<Place, S>, S>,
) -> bool {
    postset_p.values().all(|postset| {
        let mut iter = postset.iter().map(|t| preset_t.get(t).unwrap());
        iter.next().is_none_or(|first| iter.all(|preset| preset == first))
    })
}

/// Asymmetric-choice: ∀ p1, p2: p1• ∩ p2• ≠ ∅ ⟹ p1• ⊆ p2• ∨ p2• ⊆ p1•.
fn is_asymmetric_choice_net<S: std::hash::BuildHasher>(
    postset_p: &HashMap<Place, HashSet<Transition, S>, S>,
) -> bool {
    postset_p.values().all(|a| {
        postset_p.values().all(|b| {
            a.is_disjoint(b) || a.is_subset(b) || b.is_subset(a)
        })
    })
}
