//! A Petri net system: net + marking, with simulation and behavioral analysis.
//!
//! `System<N>` pairs a net structure with a mutable marking, providing methods
//! to simulate (check enablement, fire transitions) and analyze behavior
//! (boundedness, liveness, deadness).
//!
//! # Quick start
//!
//! ```
//! use petrivet::net::builder::NetBuilder;
//! use petrivet::system::System;
//!
//! // Build a simple producer-consumer net
//! let mut b = NetBuilder::new();
//! let [idle, busy] = b.add_places();
//! let [start, finish] = b.add_transitions();
//! b.add_arc((idle, start));
//! b.add_arc((start, busy));
//! b.add_arc((busy, finish));
//! b.add_arc((finish, idle));
//! let net = b.build().expect("valid net");
//!
//! let mut sys = System::new(net, [1, 0]);
//!
//! // Simulation
//! assert!(sys.is_enabled(start));
//! sys.try_fire(start).unwrap();
//! assert_eq!(sys.marking().iter().collect::<Vec<_>>(), vec![&0, &1]);
//!
//! // Behavioral analysis
//! sys.reset();
//! assert!(sys.is_bounded());
//! assert!(sys.is_live());
//! assert!(!sys.is_dead(start));
//! ```
//!
//! # Firing patterns
//!
//! Three patterns for firing transitions:
//!
//! ```
//! # use petrivet::net::builder::NetBuilder;
//! # use petrivet::system::System;
//! # let mut b = NetBuilder::new();
//! # let [p0, p1] = b.add_places();
//! # let [t0, t1] = b.add_transitions();
//! # b.add_arc((p0, t0)); b.add_arc((t0, p1));
//! # b.add_arc((p1, t1)); b.add_arc((t1, p0));
//! # let net = b.build().unwrap();
//! # let mut sys = System::new(net, [1, 0]);
//! // 1. I know which transition — just try it
//! sys.try_fire(t0).unwrap();
//!
//! // 2. I need to choose from the enabled set — zero redundant checks
//! sys.choose_and_fire(|enabled| enabled.first());
//!
//! // 3. Fire anything, I don't care which
//! sys.fire_any();
//! ```

use crate::analysis;
use crate::marking::Marking;
use crate::net::{Net, Transition};
use crate::state_space::CoverabilityGraph;
use crate::state_space::ExplorationOrder;
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;

/// Liveness level of a transition, following Murata 1989 §V-C.
///
/// The levels form a strict hierarchy: L4 ⊂ L3 ⊂ L2 ⊂ L1, and L0 means
/// the transition is dead (not even L1).
///
/// For **bounded** nets, L2 and L3 coincide: if a transition can fire
/// arbitrarily many times, the finite state space forces a cycle, making it
/// fire infinitely often. We still distinguish them in the enum for
/// theoretical completeness, but bounded-net analysis reports L3 when both
/// L2 and L3 hold.
///
/// References:
/// - Murata 1989, Definition 5.1 (liveness levels L0–L4)
/// - Petri Net Primer, §5.4 (liveness)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LivenessLevel {
    /// L0 — Dead: the transition never fires in any firing sequence from M₀.
    Dead,
    /// L1 — Potentially firable: the transition fires at least once in some
    /// firing sequence from M₀.
    L1,
    /// L2 — For any positive integer k, there exists a firing sequence from
    /// M₀ in which t fires at least k times. (Equivalent to L3 for bounded nets.)
    L2,
    /// L3 — Weakly live: there exists an infinite firing sequence from M₀ in
    /// which t appears infinitely often. (Equivalent to L2 for bounded nets.)
    L3,
    /// L4 — Live: for every marking M reachable from M₀, there exists a
    /// firing sequence from M that includes t.
    L4,
}

/// A Petri net system: a net N paired with a mutable marking.
///
/// `N` can be any type that provides access to a [`&Net`] via [`AsRef<Net>`]:
/// `Net` (owned), `&Net` (borrowed), `Rc<Net>`, `Arc<Net>`, etc.
/// This lets callers choose the ownership strategy that fits their use case.
///
/// The initial marking is stored for reference and [`reset`](System::reset).
/// The current marking evolves as transitions fire.
#[derive(Debug, Clone)]
pub struct System<N: AsRef<Net>> {
    net: N,
    initial_marking: Marking,
    marking: Marking,
}

impl<N: AsRef<Net>> System<N> {
    /// Creates a new system from a net and initial marking.
    ///
    /// Accepts anything that converts to `Marking`.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if the marking length doesn't match the number of
    /// places in the net.
    #[must_use]
    pub fn new(net: N, initial_marking: impl Into<Marking>) -> Self {
        let initial_marking = initial_marking.into();
        debug_assert_eq!(
            initial_marking.len(),
            net.as_ref().n_places(),
            "marking length ({}) must equal number of places ({})",
            initial_marking.len(),
            net.as_ref().n_places(),
        );
        let marking = initial_marking.clone();
        Self { net, initial_marking, marking }
    }

    /// Returns a reference to the underlying net.
    #[must_use]
    pub fn net(&self) -> &N {
        &self.net
    }

    /// Returns the initial marking.
    #[must_use]
    pub fn initial_marking(&self) -> &Marking {
        &self.initial_marking
    }

    /// Returns the current marking.
    #[must_use]
    pub fn marking(&self) -> &Marking {
        &self.marking
    }

    /// Resets the current marking to the initial marking.
    pub fn reset(&mut self) {
        self.marking = self.initial_marking.clone();
    }

    /// Consumes the system and returns (`net`, `initial_marking`, `current_marking`).
    #[must_use]
    pub fn into_parts(self) -> (N, Marking, Marking) {
        (self.net, self.initial_marking, self.marking)
    }

    /// Whether a transition is enabled under the current marking.
    ///
    /// A transition t is enabled if every input place p in its preset has
    /// at least one token.
    #[must_use]
    pub fn is_enabled(&self, t: Transition) -> bool {
        let net = self.net.as_ref();
        net.preset_t(t).iter().all(|&p| self.marking[p] >= 1)
    }

    /// Returns the set of currently enabled transitions.
    ///
    /// This is a read-only query. To fire one of these, use [`try_fire`](Self::try_fire)
    /// or [`choose_and_fire`](Self::choose_and_fire).
    #[must_use]
    pub fn enabled_transitions(&self) -> Vec<Transition> {
        let net = self.net.as_ref();
        net.transitions().filter(|&t| self.is_enabled(t)).collect()
    }

    /// Whether the system is in a deadlock state (no transitions are enabled).
    #[must_use]
    pub fn is_deadlocked(&self) -> bool {
        let net = self.net.as_ref();
        net.transitions().all(|t| !self.is_enabled(t))
    }

    /// Check-and-fire a specific transition.
    ///
    /// Returns `Ok(())` if the transition was enabled and has been fired.
    /// # Errors
    /// Returns `Err(FireError::NotEnabled)` if it was not enabled.
    pub fn try_fire(&mut self, t: Transition) -> Result<(), FireError> {
        if self.is_enabled(t) {
            self.fire_unchecked(t);
            Ok(())
        } else {
            Err(FireError::NotEnabled(t))
        }
    }

    /// Fire any single enabled transition.
    ///
    /// Returns the transition that was fired, or `None` if no transition is
    /// enabled (deadlock).
    pub fn fire_any(&mut self) -> Option<Transition> {
        let net = self.net.as_ref();
        let t = net.transitions().find(|&t| self.is_enabled(t))?;
        self.fire_unchecked(t);
        Some(t)
    }

    /// Compute the enabled set, let the caller choose one, and fire it.
    ///
    /// The closure receives an [`EnabledSet`] and returns an
    /// [`EnabledTransition`] proof token for the chosen transition. The token
    /// cannot be fabricated (private fields), duplicated (not Copy/Clone), or
    /// stashed outside the closure (higher-ranked lifetime). This makes the
    /// subsequent fire infallible with zero redundant enablement checks.
    ///
    /// Returns the fired transition, or `None` if the closure chose not to fire
    /// (or no transitions were enabled).
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// let mut sys = System::new(net, [1, 0]);
    ///
    /// // Pick the first enabled transition
    /// let fired = sys.choose_and_fire(|enabled| enabled.first());
    /// assert_eq!(fired, Some(t0));
    ///
    /// // Pick a specific transition (t1 is now enabled since marking is [0,1])
    /// let fired = sys.choose_and_fire(|enabled| {
    ///     enabled.iter().find(|et| *et == t1)
    /// });
    /// assert_eq!(fired, Some(t1));
    /// ```
    pub fn choose_and_fire<F>(&mut self, choose: F) -> Option<Transition>
    where
        F: for<'a> FnOnce(EnabledSet<'a>) -> Option<EnabledTransition<'a>>,
    {
        let enabled = self.enabled_transitions();
        let set = EnabledSet(enabled, PhantomData);
        let chosen = choose(set)?;
        let t = chosen.0;
        self.fire_unchecked(t);
        Some(t)
    }

    /// Fire a transition without checking enablement.
    ///
    /// The caller must guarantee the transition is enabled. Underflow will
    /// panic in debug mode and wrap in release mode.
    fn fire_unchecked(&mut self, t: Transition) {
        let net = self.net.as_ref();
        for &p in net.preset_t(t) {
            self.marking[p] -= 1;
        }
        for &p in net.postset_t(t) {
            self.marking[p] += 1;
        }
    }
}

/// Proof that a transition was found enabled in the current marking.
///
/// Cannot be constructed outside this module (private fields), cannot be
/// copied or cloned, and cannot escape the [`choose_and_fire`](System::choose_and_fire)
/// closure (higher-ranked lifetime bound).
pub struct EnabledTransition<'a>(Transition, PhantomData<&'a ()>);

impl std::ops::Deref for EnabledTransition<'_> {
    type Target = Transition;
    fn deref(&self) -> &Transition {
        &self.0
    }
}

impl PartialEq<Transition> for EnabledTransition<'_> {
    fn eq(&self, other: &Transition) -> bool {
        self.0 == *other
    }
}

impl PartialEq<EnabledTransition<'_>> for Transition {
    fn eq(&self, other: &EnabledTransition<'_>) -> bool {
        *self == other.0
    }
}

impl fmt::Debug for EnabledTransition<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnabledTransition({})", self.idx)
    }
}

/// The set of transitions enabled in a specific marking.
///
/// Only exists inside the [`choose_and_fire`](System::choose_and_fire) closure.
pub struct EnabledSet<'a>(Vec<Transition>, PhantomData<&'a ()>);

impl<'a> EnabledSet<'a> {
    /// Returns the first enabled transition, if any.
    #[must_use]
    pub fn first(&self) -> Option<EnabledTransition<'a>> {
        self.0.first().map(|&t| EnabledTransition(t, PhantomData))
    }

    /// Returns the enabled transition at the given index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<EnabledTransition<'a>> {
        self.0.get(index).map(|&t| EnabledTransition(t, PhantomData))
    }

    /// Iterator over enabled transitions as proof tokens.
    pub fn iter(&self) -> impl Iterator<Item = EnabledTransition<'a>> + '_ {
        self.0.iter().map(|&t| EnabledTransition(t, PhantomData))
    }

    /// Number of enabled transitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no transitions are enabled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for EnabledSet<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EnabledSet").field(&self.0).finish()
    }
}

/// Error returned when attempting to fire a transition that is not enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FireError {
    /// The transition is not enabled under the current marking.
    NotEnabled(Transition),
}

impl fmt::Display for FireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FireError::NotEnabled(t) => write!(f, "transition {t} is not enabled"),
        }
    }
}

impl std::error::Error for FireError {}

impl<N: AsRef<Net>> System<N> {

    /// Checks if the system is bounded under its initial marking.
    ///
    /// A system is bounded if the number of tokens on every place remains
    /// finite across all reachable markings. Useful for verifying that
    /// buffers, queues, or resource pools cannot overflow.
    ///
    /// Strategy:
    /// 1. First tries structural boundedness (LP) — if true, we're done.
    /// 2. Otherwise, builds the coverability graph (always terminates) and
    ///    checks whether any omega values appear.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    ///
    /// // A cycle is bounded: tokens just move around
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// assert!(System::new(net, [1, 0]).is_bounded());
    ///
    /// // A source transition makes the net unbounded
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p0)); b.add_arc((t0, p1));
    /// let net = b.build().unwrap();
    /// assert!(!System::new(net, [1, 0]).is_bounded());
    /// ```
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        if self.net.as_ref().is_structurally_bounded() {
            return true;
        }
        CoverabilityGraph::new(self, ExplorationOrder::BreadthFirst)
            .iter()
            .all(|step| !step.is_new || step.marking.is_finite())
    }

    /// Checks whether transition `t` is dead (L0): it can never fire from
    /// any marking reachable from the initial marking.
    ///
    /// A dead transition represents an operation that is structurally
    /// present but can never execute — often indicating a design error
    /// (unreachable code path, impossible precondition).
    ///
    /// Strategy:
    /// 1. Covering equation check — if no firing vector can produce a
    ///    marking with at least 1 token on every input place of `t`,
    ///    then `t` is definitely dead.
    /// 2. Coverability graph — builds the full coverability graph and
    ///    checks whether any enabling marking for `t` is coverable.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    ///
    /// // With tokens, both transitions are alive
    /// let sys = System::new(net.clone(), [1, 0]);
    /// assert!(!sys.is_dead(t0));
    /// assert!(!sys.is_dead(t1));
    ///
    /// // Without tokens, both transitions are dead
    /// let sys = System::new(net, [0, 0]);
    /// assert!(sys.is_dead(t0));
    /// assert!(sys.is_dead(t1));
    /// ```
    #[must_use]
    pub fn is_dead(&self, t: Transition) -> bool {
        let net = self.net.as_ref();
        let preset = net.preset_t(t);
        if preset.is_empty() {
            return false; // empty preset means t is trivially L4
        }

        let mut threshold_tokens = vec![0u32; net.n_places()];
        for &p in preset {
            threshold_tokens[p.idx] = 1;
        }
        let threshold = Marking::from(threshold_tokens);

        let me_result = analysis::semi_decision::check_covering_equation(
            net,
            &self.initial_marking,
            &threshold,
        );
        if me_result.is_infeasible() {
            return true;
        }

        !CoverabilityGraph::new(self, ExplorationOrder::BreadthFirst)
            .iter()
            .any(|step| step.transition == t)
    }

    /// Checks if the system is deadlock-free: no reachable marking is a
    /// deadlock (no enabled transitions).
    ///
    /// This is checked structurally via the siphon-trap condition:
    /// every siphon must contain a marked trap. If this condition holds, the
    /// system can never reach a marking where all transitions are disabled.
    ///
    /// This is more commonly known as a liveness condition for free-choice nets,
    /// but it also implies deadlock-freedom for general nets.
    pub fn is_deadlock_free(&self) -> bool {
        analysis::structural::every_siphon_contains_marked_trap(
            self.net.as_ref(),
            &self.marking,
            &analysis::structural::minimal_siphons(self.net.as_ref()),
        )
    }

    /// Checks if the system is quasi-live (L1): every transition can fire
    /// in at least one reachable marking.
    ///
    /// Quasi-liveness means every operation in the system is at least
    /// potentially reachable. A non-quasi-live system has dead code.
    ///
    /// Strategy: builds the coverability graph and checks that every
    /// transition appears on at least one edge.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    ///
    /// assert!(System::new(net.clone(), [1, 0]).is_quasi_live());
    /// assert!(!System::new(net, [0, 0]).is_quasi_live());  // no tokens → nothing fires
    /// ```
    #[must_use]
    pub fn is_quasi_live(&self) -> bool {
        let net = self.net.as_ref();
        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);

        let graph = cg.core().graph();
        let mut fired: HashSet<Transition> = HashSet::new();
        for edge in graph.edge_references() {
            fired.insert(*edge.weight());
        }
        net.transitions().all(|t| fired.contains(&t))
    }

    /// Checks if the system is live (L4): every transition can fire from
    /// every reachable marking (possibly after further firings).
    ///
    /// This is the strongest liveness property — it guarantees the system
    /// can never reach a state where some operation becomes permanently
    /// impossible. In manufacturing, liveness means no workstation can
    /// become permanently idle; in protocols, no message type is ever
    /// permanently blocked.
    ///
    /// For free-choice nets, uses Commoner's theorem (structural check).
    /// Otherwise falls back to state space exploration.
    ///
    /// **Note**: For unbounded non-free-choice nets, this builds the
    /// coverability graph which may be an over-approximation — the result
    /// is sound but may conservatively return `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    ///
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    ///
    /// // A cycle with tokens is live
    /// assert!(System::new(net.clone(), [1, 0]).is_live());
    ///
    /// // Same net with no tokens is not live
    /// assert!(!System::new(net, [0, 0]).is_live());
    /// ```
    #[must_use]
    pub fn is_live(&self) -> bool {
        let net = self.net.as_ref();

        if net.is_free_choice() {
            let siphons = analysis::structural::minimal_siphons(net);
            return analysis::structural::every_siphon_contains_marked_trap(
                net,
                &self.initial_marking,
                &siphons,
            );
        }

        self.is_live_by_exploration()
    }

    /// Full state-space liveness check via SCC analysis on the reachability
    /// graph. Delegates to [`ReachabilityGraph::is_live`].
    ///
    /// Returns `false` conservatively if the system is unbounded (cannot
    /// build a finite reachability graph).
    fn is_live_by_exploration(&self) -> bool {
        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        let Ok(rg) = cg.into_reachability_graph() else { return false };
        rg.is_live()
    }

    /// Checks whether `target` is reachable from the initial marking.
    ///
    /// Automatically dispatches to the best available algorithm based on
    /// the net's structural class:
    ///
    /// - **S-nets** (incl. circuits): token conservation makes the marking
    ///   equation both necessary and sufficient — reachability reduces to a
    ///   single LP solve (polynomial time).
    /// - **T-nets**: every non-negative integer solution to
    ///   the marking equation corresponds to a realizable firing sequence —
    ///   reachability reduces to ILP feasibility.
    /// - **General nets**: uses the marking equation as a fast necessary
    ///   condition filter (LP, then ILP), falling back to coverability or
    ///   reachability graph exploration if the equation is feasible.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::System;
    /// use petrivet::marking::Marking;
    ///
    /// // S-net cycle: reachability decided in polynomial time
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    /// let sys = System::new(net, [1, 0]);
    ///
    /// assert!(sys.is_reachable(&Marking::from([0u32, 1])));
    /// assert!(!sys.is_reachable(&Marking::from([2u32, 0])));
    /// ```
    ///
    /// References:
    /// - Murata 1989, Theorem 21 (S-net reachability)
    /// - Murata 1989, Theorem 22 (T-net reachability)
    #[must_use]
    pub fn is_reachable(&self, target: &Marking) -> bool {
        let net = self.net.as_ref();

        if net.is_s_net() {
            if net.is_strongly_connected() {
                return target.iter().sum::<u32>() == self.initial_marking.iter().sum::<u32>();
            }
            return analysis::semi_decision::is_reachable_s_net(
                net, &self.initial_marking, target,
            );
        }

        if net.is_t_net() {
            return analysis::semi_decision::is_reachable_t_net(
                net, &self.initial_marking, target,
            );
        }

        if analysis::semi_decision::find_marking_equation_rational_solution(
            net, &self.initial_marking, target,
        ).is_infeasible() {
            return false;
        }

        if analysis::semi_decision::find_marking_equation_integer_solution(
            net, &self.initial_marking, target,
        ).is_infeasible() {
            return false;
        }

        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        if let Ok(rg) = cg.into_reachability_graph() {
            rg.is_reachable(target)
        } else {
            unimplemented!("reachability for unbounded general nets")
        }
    }

    /// Computes liveness levels for all transitions.
    ///
    /// Returns a [`LivenessLevel`] for each transition, classifying it from
    /// `Dead` (L0) through `L4` (live). This provides a detailed picture of
    /// which parts of a system are healthy and which are degraded.
    ///
    /// Builds the coverability graph. If bounded, promotes to a
    /// reachability graph for exact SCC-based analysis. Returns `None`
    /// if the system is unbounded and no structural shortcut applies.
    ///
    /// # Examples
    ///
    /// ```
    /// use petrivet::net::builder::NetBuilder;
    /// use petrivet::system::{System, LivenessLevel};
    ///
    /// // A simple cycle: both transitions are L4 (live)
    /// let mut b = NetBuilder::new();
    /// let [p0, p1] = b.add_places();
    /// let [t0, t1] = b.add_transitions();
    /// b.add_arc((p0, t0)); b.add_arc((t0, p1));
    /// b.add_arc((p1, t1)); b.add_arc((t1, p0));
    /// let net = b.build().unwrap();
    ///
    /// let sys = System::new(net, [1, 0]);
    /// let levels = sys.liveness_levels().expect("bounded net");
    /// assert_eq!(levels[t0.index()], LivenessLevel::L4);
    /// assert_eq!(levels[t1.index()], LivenessLevel::L4);
    /// ```
    #[must_use]
    pub fn liveness_levels(&self) -> Option<Box<[LivenessLevel]>> {
        let cg = CoverabilityGraph::build(self, ExplorationOrder::BreadthFirst);
        let rg = cg.into_reachability_graph().ok()?;
        Some(rg.liveness_levels())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marking::Marking;
    use crate::net::builder::NetBuilder;
    /// Shorthand for creating a `Marking<u32>` in tests.
    fn m(val: impl Into<Marking>) -> Marking { val.into() }

    /// Builds a simple two-place cycle: p0 -> t0 -> p1 -> t1 -> p0
    fn two_place_cycle() -> (Net, Transition, Transition) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().expect("valid net");
        (net, t0, t1)
    }

    #[test]
    fn basic_firing() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        assert_eq!(sys.marking(), m([1, 0]));
        assert!(sys.is_enabled(t0));
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.marking(), m([0, 1]));
    }

    #[test]
    fn try_fire_not_enabled() {
        let (net, _t0, t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        assert!(sys.try_fire(t1).is_err());
    }

    #[test]
    fn fire_any_deadlock() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [0, 0]);
        assert!(sys.is_deadlocked());
        assert!(sys.fire_any().is_none());
    }

    #[test]
    fn fire_any_success() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        assert!(!sys.is_deadlocked());
        let fired = sys.fire_any();
        assert!(fired.is_some());
        assert_eq!(sys.marking(), m([0, 1]));
    }

    #[test]
    fn choose_and_fire_first() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        let fired = sys.choose_and_fire(|enabled| enabled.first());
        assert_eq!(fired, Some(t0));
        assert_eq!(sys.marking(), m([0, 1]));
    }

    #[test]
    fn choose_and_fire_specific() {
        let (net, _t0, t1) = two_place_cycle();
        let mut sys = System::new(net, [0, 1]);
        let fired = sys.choose_and_fire(|enabled| {
            enabled.iter().find(|et| *et == t1)
        });
        assert_eq!(fired, Some(t1));
        assert_eq!(sys.marking(), m([1, 0]));
    }

    #[test]
    fn choose_and_fire_none_enabled() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [0, 0]);
        let fired = sys.choose_and_fire(|enabled: EnabledSet<'_>| enabled.first());
        assert_eq!(fired, None);
    }

    #[test]
    fn choose_and_fire_user_declines() {
        let (net, _t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        let fired = sys.choose_and_fire(|_enabled| None);
        assert_eq!(fired, None);
        assert_eq!(sys.marking(), m([1, 0]));
    }

    #[test]
    fn reset() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        sys.try_fire(t0).unwrap();
        assert_eq!(sys.marking(), m([0, 1]));
        sys.reset();
        assert_eq!(sys.marking(), m([1, 0]));
    }

    #[test]
    fn enabled_transitions_query() {
        let (net, t0, t1) = two_place_cycle();
        let sys = System::new(net, [1, 1]);
        let enabled = sys.enabled_transitions();
        assert!(enabled.contains(&t0));
        assert!(enabled.contains(&t1));
    }

    #[test]
    fn into_parts() {
        let (net, t0, _t1) = two_place_cycle();
        let mut sys = System::new(net, [1, 0]);
        sys.try_fire(t0).unwrap();
        let (_net, initial, current) = sys.into_parts();
        assert_eq!(initial, m([1, 0]));
        assert_eq!(current, m([0, 1]));
    }

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, _, _) = two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_bounded());
    }

    #[test]
    fn cycle_is_live() {
        let (net, _, _) = two_place_cycle();
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_live());
    }

    #[test]
    fn cycle_is_quasi_live() {
        let (net, _, _) = two_place_cycle();
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_quasi_live());
    }

    #[test]
    fn deadlocked_cycle_not_quasi_live() {
        let (net, _, _) = two_place_cycle();
        let sys = System::new(net, [0, 0]);
        assert!(!sys.is_quasi_live());
    }

    #[test]
    fn deadlocked_cycle_not_live() {
        let (net, _, _) = two_place_cycle();
        let sys = System::new(net, [0, 0]);
        assert!(!sys.is_live());
    }

    #[test]
    fn dead_transition_detection() {
        let (net, t0, t1) = two_place_cycle();
        // With [0, 0], both transitions are dead (never fireable)
        let sys = System::new(net, [0, 0]);
        assert!(sys.is_dead(t0));
        assert!(sys.is_dead(t1));
    }

    #[test]
    fn alive_transitions_not_dead() {
        let (net, t0, t1) = two_place_cycle();
        let sys = System::new(net, [1, 0]);
        assert!(!sys.is_dead(t0));
        assert!(!sys.is_dead(t1));
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
        let sys = System::new(net, [1, 0]);
        assert!(!sys.is_bounded());
    }

    #[test]
    fn s_net_reachability_dispatches() {
        let (net, _, _) = two_place_cycle();
        assert!(net.is_s_net());
        let sys = System::new(net, [1, 0]);
        assert!(sys.is_reachable(&m([0, 1])));
        assert!(sys.is_reachable(&m([1, 0])));
        assert!(!sys.is_reachable(&m([2, 0])));
        assert!(!sys.is_reachable(&m([0, 0])));
    }

    #[test]
    fn t_net_reachability_dispatches() {
        // T-net: {p0, p1} → t0 → p2 → t1 → {p0, p1}
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((p1, t0)); b.add_arc((t0, p2));
        b.add_arc((p2, t1)); b.add_arc((t1, p0)); b.add_arc((t1, p1));
        let net = b.build().unwrap();
        assert!(net.is_t_net());
        let sys = System::new(net, [1, 1, 0]);
        assert!(sys.is_reachable(&m([0, 0, 1])));
        assert!(sys.is_reachable(&m([1, 1, 0])));
        assert!(!sys.is_reachable(&m([1, 0, 0])));
    }

    #[test]
    fn general_net_reachability_fallback() {
        // Free-choice net (not S-net, not T-net): falls back to exploration
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p0, t1)); b.add_arc((t1, p2));
        b.add_arc((p1, t2)); b.add_arc((t2, p0));
        b.add_arc((p2, t2)); b.add_arc((t2, p0));
        let net = b.build().unwrap();
        assert!(!net.is_s_net());
        assert!(!net.is_t_net());
        let sys = System::new(net, [1, 0, 0]);
        assert!(sys.is_reachable(&m([0, 1, 0])));
        assert!(sys.is_reachable(&m([1, 0, 0])));
    }

    #[test]
    fn mutex_is_live_and_bounded() {
        let mut b = NetBuilder::new();
        let [idle1, wait1, crit1] = b.add_places();
        let [idle2, wait2, crit2] = b.add_places();
        let mutex = b.add_place();
        let [t_req1, t_enter1, t_exit1] = b.add_transitions();
        let [t_req2, t_enter2, t_exit2] = b.add_transitions();

        b.add_arc((idle1, t_req1)); b.add_arc((t_req1, wait1));
        b.add_arc((wait1, t_enter1)); b.add_arc((t_enter1, crit1));
        b.add_arc((crit1, t_exit1)); b.add_arc((t_exit1, idle1));
        b.add_arc((idle2, t_req2)); b.add_arc((t_req2, wait2));
        b.add_arc((wait2, t_enter2)); b.add_arc((t_enter2, crit2));
        b.add_arc((crit2, t_exit2)); b.add_arc((t_exit2, idle2));
        b.add_arc((mutex, t_enter1)); b.add_arc((t_exit1, mutex));
        b.add_arc((mutex, t_enter2)); b.add_arc((t_exit2, mutex));

        let net = b.build().expect("valid net");
        let sys = System::new(net, [1u32, 0, 0, 1, 0, 0, 1]);
        assert!(sys.is_bounded());
        assert!(sys.is_quasi_live());
        assert!(sys.is_live());
    }
}
