//! Data-Interpreted Petri Net (DIPN) implementation for simulation.
//!
//! Based on Hussain et al. (2023) - "Discovering a data interpreted petri net model
//! of industrial control systems for anomaly detection."
//!
//! A DIPN extends a standard Petri net with:
//! - **Guards**: Predicates on transitions that must evaluate to `true` for the transition
//!   to be enabled (in addition to the standard token requirements).
//! - **Actions**: Callbacks that execute when a transition fires.
//!
//! This implementation focuses solely on simulation capabilities:
//! - Detecting enabled transitions
//! - Firing transitions manually
//!
//! # Example
//!
//! ```
//! use petrivet::dipn::{NetBuilder, Marking};
//!
//! // Build a simple net with a guarded transition
//! let mut builder = NetBuilder::new();
//! let [p0, p1] = builder.add_places();
//! let t0 = builder.add_transition()
//!     .guard(|| true)  // Always enabled when tokens available
//!     .action(|| println!("Transition fired!"))
//!     .build();
//! builder.add_arc((p0, t0));
//! builder.add_arc((t0, p1));
//!
//! let net = builder.build().unwrap();
//!
//! // Create initial marking: one token in p0
//! let mut marking = Marking::new(net.n_places());
//! marking[p0] = 1;
//!
//! // Check enabled transitions
//! let enabled: Vec<_> = net.enabled_transitions(&marking).collect();
//! assert_eq!(enabled.len(), 1);
//!
//! // Fire the transition
//! net.fire(&mut marking, t0).unwrap();
//! assert_eq!(marking[p0], 0);
//! assert_eq!(marking[p1], 1);
//! ```

use std::fmt;
use std::time::Duration;
use rand::RngExt;

/// Index type used for places and transitions.
pub type Index = usize;

/// Token count type.
pub type Tokens = i32;

/// Describes the timing behavior of a transition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TransitionTiming {
    /// The transition fires instantaneously.
    #[default]
    Atomic,
    /// The transition takes a fixed amount of time.
    Fixed(Duration),
    /// The transition takes a random amount of time sampled uniformly from `[min, max]`.
    Range { min: Duration, max: Duration },
}

/// A place in the Petri net.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub index: Index,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.index)
    }
}

/// A transition in the Petri net.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transition {
    pub index: Index,
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.index)
    }
}

/// A marking represents the current state of the net: token counts for each place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marking(Box<[Tokens]>);

impl Marking {
    /// Creates a new marking with all places having zero tokens.
    #[must_use]
    pub fn new(n_places: usize) -> Self {
        Self(vec![0; n_places].into_boxed_slice())
    }

    /// Creates a marking from a vector of token counts.
    #[must_use]
    pub fn from_vec(tokens: Vec<Tokens>) -> Self {
        Self(tokens.into_boxed_slice())
    }

    /// Returns the number of places.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the marking has zero places.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over token counts.
    pub fn iter(&self) -> impl Iterator<Item = &Tokens> {
        self.0.iter()
    }
}

impl std::ops::Index<Place> for Marking {
    type Output = Tokens;

    fn index(&self, place: Place) -> &Self::Output {
        &self.0[place.index]
    }
}

impl std::ops::IndexMut<Place> for Marking {
    fn index_mut(&mut self, place: Place) -> &mut Self::Output {
        &mut self.0[place.index]
    }
}

impl<const N: usize> From<[Tokens; N]> for Marking {
    fn from(tokens: [Tokens; N]) -> Self {
        Self(tokens.into())
    }
}

impl From<Vec<Tokens>> for Marking {
    fn from(tokens: Vec<Tokens>) -> Self {
        Self(tokens.into_boxed_slice())
    }
}

impl fmt::Display for Marking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, &count) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{count}")?;
        }
        write!(f, ")")
    }
}

/// Type alias for guard functions.
/// A guard returns `true` if the transition is allowed to fire
/// (in addition to having sufficient tokens).
pub type Guard = Box<dyn Fn() -> bool>;

/// Type alias for action functions.
/// An action is executed when the transition fires.
pub type Action = Box<dyn Fn()>;

/// Internal representation of a transition with its guard and action.
struct TransitionData {
    /// Input places and their arc weights (tokens consumed).
    inputs: Box<[(Place, Tokens)]>,
    /// Output places and their arc weights (tokens produced).
    outputs: Box<[(Place, Tokens)]>,
    /// Guard predicate (if None, always returns true).
    guard: Option<Guard>,
    /// Action callback (if None, does nothing).
    action: Option<Action>,
    /// Timing behavior of this transition.
    timing: TransitionTiming,
}

/// Error type for firing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FireError {
    /// The transition is not enabled (insufficient tokens or guard returned false).
    NotEnabled,
    /// The transition index is out of bounds.
    InvalidTransition,
}

impl fmt::Display for FireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FireError::NotEnabled => write!(f, "transition is not enabled"),
            FireError::InvalidTransition => write!(f, "invalid transition index"),
        }
    }
}

impl std::error::Error for FireError {}

/// A Data-Interpreted Petri Net (DIPN).
///
/// Supports simulation via:
/// - [`enabled_transitions`](Net::enabled_transitions): Returns an iterator of enabled transitions.
/// - [`fire`](Net::fire): Fires a specific transition.
/// - [`is_enabled`](Net::is_enabled): Checks if a specific transition is enabled.
pub struct Net {
    n_places: Index,
    transitions: Box<[TransitionData]>,
}

impl Net {
    /// Returns the number of places in the net.
    #[must_use]
    pub fn n_places(&self) -> Index {
        self.n_places
    }

    /// Returns the number of transitions in the net.
    #[must_use]
    pub fn n_transitions(&self) -> Index {
        self.transitions.len()
    }

    /// Returns an iterator over all places.
    pub fn places(&self) -> impl Iterator<Item = Place> {
        (0..self.n_places).map(|index| Place { index })
    }

    /// Returns an iterator over all transitions.
    pub fn transitions(&self) -> impl Iterator<Item = Transition> {
        (0..self.transitions.len()).map(|index| Transition { index })
    }

    /// Checks if a transition is enabled in the given marking.
    ///
    /// A transition is enabled if:
    /// 1. All input places have sufficient tokens.
    /// 2. The guard (if any) returns `true`.
    #[must_use]
    pub fn is_enabled(&self, marking: &Marking, transition: Transition) -> bool {
        let Some(data) = self.transitions.get(transition.index) else {
            return false;
        };

        // Check token requirements
        for &(place, weight) in &data.inputs {
            if marking[place] < weight {
                return false;
            }
        }

        // Check guard
        data.guard.as_ref().is_none_or(|guard| guard())
    }

    /// Returns an iterator over all transitions that are currently enabled.
    pub fn enabled_transitions<'a>(
        &'a self,
        marking: &'a Marking,
    ) -> impl Iterator<Item = Transition> + 'a {
        self.transitions().filter(move |&t| self.is_enabled(marking, t))
    }

    /// Returns the timing behavior of a transition.
    ///
    /// # Panics
    ///
    /// Panics if the transition index is out of bounds.
    #[must_use]
    pub fn timing(&self, transition: Transition) -> &TransitionTiming {
        &self.transitions[transition.index].timing
    }

    /// Fires a transition, updating the marking and executing the action.
    ///
    /// Returns the [`TransitionTiming`] associated with the fired transition.
    ///
    /// # Errors
    ///
    /// Returns [`FireError::NotEnabled`] if the transition is not enabled.
    /// Returns [`FireError::InvalidTransition`] if the transition index is invalid.
    pub fn fire(
        &self,
        marking: &mut Marking,
        transition: Transition,
    ) -> Result<Duration, FireError> {
        let data = self
            .transitions
            .get(transition.index)
            .ok_or(FireError::InvalidTransition)?;

        if !self.is_enabled(marking, transition) {
            return Err(FireError::NotEnabled);
        }

        for &(place, weight) in &data.inputs {
            marking[place] -= weight;
        }

        for &(place, weight) in &data.outputs {
            marking[place] += weight;
        }

        if let Some(action) = &data.action {
            action();
        }

        match &data.timing {
            TransitionTiming::Atomic => Ok(Duration::ZERO),
            TransitionTiming::Fixed(d) => Ok(*d),
            TransitionTiming::Range { min, max } => {
                let secs = rand::rng().random_range(min.as_secs_f64()..=max.as_secs_f64());
                Ok(Duration::from_secs_f64(secs))
            }
        }
    }

    /// Fires a transition without checking if it's enabled.
    ///
    /// Returns the [`TransitionTiming`] associated with the fired transition.
    ///
    /// # Safety
    ///
    /// The caller must ensure the transition is enabled. Firing a non-enabled
    /// transition may result in negative token counts.
    ///
    /// # Panics
    ///
    /// Panics if the transition index is out of bounds.
    pub fn fire_unchecked(&self, marking: &mut Marking, transition: Transition) -> &TransitionTiming {
        let data = &self.transitions[transition.index];

        for &(place, weight) in &data.inputs {
            marking[place] -= weight;
        }

        for &(place, weight) in &data.outputs {
            marking[place] += weight;
        }

        if let Some(action) = &data.action {
            action();
        }

        &data.timing
    }

    /// Returns the input places of a transition with their arc weights.
    pub fn inputs(&self, transition: Transition) -> impl Iterator<Item = (Place, Tokens)> + '_ {
        self.transitions
            .get(transition.index)
            .into_iter()
            .flat_map(|data| data.inputs.iter().copied())
    }

    /// Returns the output places of a transition with their arc weights.
    pub fn outputs(&self, transition: Transition) -> impl Iterator<Item = (Place, Tokens)> + '_ {
        self.transitions
            .get(transition.index)
            .into_iter()
            .flat_map(|data| data.outputs.iter().copied())
    }
}

impl fmt::Debug for Net {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Net")
            .field("n_places", &self.n_places)
            .field("n_transitions", &self.transitions.len())
            .finish_non_exhaustive()
    }
}

/// Builder for constructing a DIPN.
#[derive(Default)]
pub struct NetBuilder {
    n_places: Index,
    n_transitions: Index,
    guards: Vec<Option<Guard>>,
    actions: Vec<Option<Action>>,
    timings: Vec<TransitionTiming>,
    arcs: Vec<ArcSpec>,
}

/// Specifies an arc during construction.
enum ArcSpec {
    /// Arc from place to transition with weight.
    PlaceToTransition(Place, Transition, Tokens),
    /// Arc from transition to place with weight.
    TransitionToPlace(Transition, Place, Tokens),
}

/// Builder for a single transition, allowing fluent configuration of guard and action.
pub struct TransitionBuilder<'a> {
    builder: &'a mut NetBuilder,
    index: Index,
}

impl TransitionBuilder<'_> {
    /// Sets the guard for this transition.
    ///
    /// The guard is a predicate that must return `true` for the transition
    /// to be enabled (in addition to having sufficient tokens).
    #[must_use]
    pub fn guard<F>(self, guard: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.builder.guards[self.index] = Some(Box::new(guard));
        self
    }

    /// Sets the action for this transition.
    ///
    /// The action is executed when the transition fires.
    #[must_use]
    pub fn action<F>(self, action: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.builder.actions[self.index] = Some(Box::new(action));
        self
    }

    /// Sets the timing behavior for this transition.
    ///
    /// Defaults to [`TransitionTiming::Atomic`] if not set.
    #[must_use]
    pub fn timing(self, timing: TransitionTiming) -> Self {
        self.builder.timings[self.index] = timing;
        self
    }

    /// Returns the transition handle for use in arc construction.
    #[must_use]
    pub fn build(self) -> Transition {
        Transition { index: self.index }
    }
}

/// Error type for building a net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// An arc references a non-existent place.
    InvalidPlace(Place),
    /// An arc references a non-existent transition.
    InvalidTransition(Transition),
    /// A transition has no input or output arcs.
    DisconnectedTransition(Transition),
    /// No transitions defined.
    NoTransitions,
    /// No places defined.
    NoPlaces,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::InvalidPlace(p) => write!(f, "arc references non-existent place {p}"),
            BuildError::InvalidTransition(t) => {
                write!(f, "arc references non-existent transition {t}")
            }
            BuildError::DisconnectedTransition(t) => {
                write!(f, "transition {t} has no input or output arcs")
            }
            BuildError::NoTransitions => write!(f, "net has no transitions"),
            BuildError::NoPlaces => write!(f, "net has no places"),
        }
    }
}

impl std::error::Error for BuildError {}

impl NetBuilder {
    /// Creates a new empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new place to the net.
    pub fn add_place(&mut self) -> Place {
        let place = Place {
            index: self.n_places,
        };
        self.n_places += 1;
        place
    }

    /// Adds N places to the net.
    pub fn add_places<const N: usize>(&mut self) -> [Place; N] {
        std::array::from_fn(|_| self.add_place())
    }

    /// Adds a new transition to the net, returning a builder for it.
    ///
    /// Use the returned [`TransitionBuilder`] to set the guard and action,
    /// then call [`build()`](TransitionBuilder::build) to get the transition handle.
    pub fn add_transition(&mut self) -> TransitionBuilder<'_> {
        let index = self.n_transitions;
        self.n_transitions += 1;
        self.guards.push(None);
        self.actions.push(None);
        self.timings.push(TransitionTiming::default());
        TransitionBuilder {
            builder: self,
            index,
        }
    }

    /// Adds a new transition without guard or action, returning its handle directly.
    pub fn add_simple_transition(&mut self) -> Transition {
        let index = self.n_transitions;
        self.n_transitions += 1;
        self.guards.push(None);
        self.actions.push(None);
        self.timings.push(TransitionTiming::default());
        Transition { index }
    }

    /// Sets the guard for an existing transition.
    pub fn set_guard<F>(&mut self, transition: Transition, guard: F)
    where
        F: Fn() -> bool + 'static,
    {
        if transition.index < self.guards.len() {
            self.guards[transition.index] = Some(Box::new(guard));
        }
    }

    /// Sets the action for an existing transition.
    pub fn set_action<F>(&mut self, transition: Transition, action: F)
    where
        F: Fn() + 'static,
    {
        if transition.index < self.actions.len() {
            self.actions[transition.index] = Some(Box::new(action));
        }
    }

    /// Adds an arc from a place to a transition with weight 1.
    pub fn add_arc<A: Into<Arc>>(&mut self, arc: A) {
        let arc = arc.into();
        match arc {
            Arc::PlaceToTransition(p, t) => {
                self.arcs.push(ArcSpec::PlaceToTransition(p, t, 1));
            }
            Arc::TransitionToPlace(t, p) => {
                self.arcs.push(ArcSpec::TransitionToPlace(t, p, 1));
            }
        }
    }

    /// Adds an arc with a specific weight.
    pub fn add_weighted_arc<A: Into<Arc>>(&mut self, arc: A, weight: Tokens) {
        let arc = arc.into();
        match arc {
            Arc::PlaceToTransition(p, t) => {
                self.arcs.push(ArcSpec::PlaceToTransition(p, t, weight));
            }
            Arc::TransitionToPlace(t, p) => {
                self.arcs.push(ArcSpec::TransitionToPlace(t, p, weight));
            }
        }
    }

    /// Builds the net, consuming the builder.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The net has no places or transitions.
    /// - An arc references a non-existent place or transition.
    /// - A transition has no input or output arcs.
    pub fn build(self) -> Result<Net, BuildError> {
        if self.n_places == 0 {
            return Err(BuildError::NoPlaces);
        }
        if self.n_transitions == 0 {
            return Err(BuildError::NoTransitions);
        }

        // Collect arcs per transition
        let mut inputs: Vec<Vec<(Place, Tokens)>> = vec![Vec::new(); self.n_transitions];
        let mut outputs: Vec<Vec<(Place, Tokens)>> = vec![Vec::new(); self.n_transitions];

        for arc in self.arcs {
            match arc {
                ArcSpec::PlaceToTransition(p, t, w) => {
                    if p.index >= self.n_places {
                        return Err(BuildError::InvalidPlace(p));
                    }
                    if t.index >= self.n_transitions {
                        return Err(BuildError::InvalidTransition(t));
                    }
                    inputs[t.index].push((p, w));
                }
                ArcSpec::TransitionToPlace(t, p, w) => {
                    if t.index >= self.n_transitions {
                        return Err(BuildError::InvalidTransition(t));
                    }
                    if p.index >= self.n_places {
                        return Err(BuildError::InvalidPlace(p));
                    }
                    outputs[t.index].push((p, w));
                }
            }
        }

        // Check that all transitions have at least one arc
        for (i, (inp, out)) in Iterator::zip(inputs.iter(), outputs.iter()).enumerate() {
            if inp.is_empty() && out.is_empty() {
                return Err(BuildError::DisconnectedTransition(Transition { index: i }));
            }
        }

        // Build transition data
        let transitions: Box<[TransitionData]> = self
            .guards
            .into_iter()
            .zip(self.actions)
            .zip(self.timings)
            .zip(inputs.into_iter().zip(outputs))
            .map(|(((guard, action), timing), (inp, out))| TransitionData {
                inputs: inp.into_boxed_slice(),
                outputs: out.into_boxed_slice(),
                guard,
                action,
                timing,
            })
            .collect();

        Ok(Net {
            n_places: self.n_places,
            transitions,
        })
    }
}

/// Arc specification for the builder.
#[derive(Debug, Clone, Copy)]
pub enum Arc {
    PlaceToTransition(Place, Transition),
    TransitionToPlace(Transition, Place),
}

impl From<(Place, Transition)> for Arc {
    fn from((p, t): (Place, Transition)) -> Self {
        Arc::PlaceToTransition(p, t)
    }
}

impl From<(Transition, Place)> for Arc {
    fn from((t, p): (Transition, Place)) -> Self {
        Arc::TransitionToPlace(t, p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_simple_net() {
        let mut builder = NetBuilder::new();
        let [p0, p1] = builder.add_places();
        let t0 = builder.add_simple_transition();
        builder.add_arc((p0, t0));
        builder.add_arc((t0, p1));

        let net = builder.build().unwrap();
        let mut marking = Marking::from([1, 0]);

        assert!(net.is_enabled(&marking, t0));
        assert_eq!(net.enabled_transitions(&marking).count(), 1);

        net.fire(&mut marking, t0).unwrap();
        assert_eq!(marking[p0], 0);
        assert_eq!(marking[p1], 1);
    }

    #[test]
    fn test_guard_blocks_transition() {
        let mut builder = NetBuilder::new();
        let [p0, p1] = builder.add_places();
        let t0 = builder.add_transition().guard(|| false).build();
        builder.add_arc((p0, t0));
        builder.add_arc((t0, p1));

        let net = builder.build().unwrap();
        let mut marking = Marking::from([1, 0]);

        // Has tokens but guard returns false
        assert!(!net.is_enabled(&marking, t0));
        assert_eq!(net.enabled_transitions(&marking).count(), 0);
        assert_eq!(net.fire(&mut marking, t0), Err(FireError::NotEnabled));
    }

    #[test]
    fn test_action_executes() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let mut builder = NetBuilder::new();
        let [p0, p1] = builder.add_places();
        let t0 = builder
            .add_transition()
            .action(|| {
                COUNTER.fetch_add(1, Ordering::SeqCst);
            })
            .build();
        builder.add_arc((p0, t0));
        builder.add_arc((t0, p1));

        let net = builder.build().unwrap();
        let mut marking = Marking::from([1, 0]);

        assert_eq!(COUNTER.load(Ordering::SeqCst), 0);
        net.fire(&mut marking, t0).unwrap();
        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_insufficient_tokens() {
        let mut builder = NetBuilder::new();
        let [p0, p1] = builder.add_places();
        let t0 = builder.add_transition().build();
        builder.add_arc((p0, t0));
        builder.add_arc((t0, p1));

        let net = builder.build().unwrap();
        let mut marking = Marking::from([0, 0]); // No tokens

        assert!(!net.is_enabled(&marking, t0));
        assert_eq!(net.fire(&mut marking, t0), Err(FireError::NotEnabled));
    }

    #[test]
    fn test_weighted_arcs() {
        let mut builder = NetBuilder::new();
        let [p0, p1] = builder.add_places();
        let t0 = builder.add_transition().build();
        builder.add_weighted_arc((p0, t0), 2); // Consumes 2 tokens
        builder.add_weighted_arc((t0, p1), 3); // Produces 3 tokens

        let net = builder.build().unwrap();

        // Not enough tokens
        let mut marking = Marking::from([1, 0]);
        assert!(!net.is_enabled(&marking, t0));

        // Enough tokens
        marking[p0] = 2;
        assert!(net.is_enabled(&marking, t0));
        net.fire(&mut marking, t0).unwrap();
        assert_eq!(marking[p0], 0);
        assert_eq!(marking[p1], 3);
    }

    #[test]
    fn test_multiple_enabled_transitions() {
        let mut builder = NetBuilder::new();
        let [p0, p1, p2] = builder.add_places();
        let t0 = builder.add_transition().build();
        let t1 = builder.add_transition().build();
        builder.add_arc((p0, t0));
        builder.add_arc((t0, p1));
        builder.add_arc((p0, t1));
        builder.add_arc((t1, p2));

        let net = builder.build().unwrap();
        let marking = Marking::from([1, 0, 0]);

        let enabled: Vec<_> = net.enabled_transitions(&marking).collect();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&t0));
        assert!(enabled.contains(&t1));
    }

    #[test]
    fn test_concurrent_paths() {
        // Models a fork-join pattern:
        //       -> p1 -> t1 ->
        // p0 -> t0              p3 -> t3 -> p4
        //       -> p2 -> t2 ->
        let mut builder = NetBuilder::new();
        let [p0, p1, p2, p3, p4] = builder.add_places();
        let t0 = builder.add_transition().build(); // fork
        let t1 = builder.add_transition().build();
        let t2 = builder.add_transition().build();
        let t3 = builder.add_transition().build(); // join

        builder.add_arc((p0, t0));
        builder.add_arc((t0, p1));
        builder.add_arc((t0, p2));
        builder.add_arc((p1, t1));
        builder.add_arc((t1, p3));
        builder.add_arc((p2, t2));
        builder.add_arc((t2, p3));
        builder.add_weighted_arc((p3, t3), 2); // join needs 2 tokens
        builder.add_arc((t3, p4));

        let net = builder.build().unwrap();
        let mut marking = Marking::from([1, 0, 0, 0, 0]);

        // Initial: only t0 enabled
        assert_eq!(
            net.enabled_transitions(&marking).collect::<Vec<_>>(),
            vec![t0]
        );

        // Fire fork
        net.fire(&mut marking, t0).unwrap();
        assert_eq!(marking, Marking::from([0, 1, 1, 0, 0]));

        // t1 and t2 now enabled
        let enabled: Vec<_> = net.enabled_transitions(&marking).collect();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&t1));
        assert!(enabled.contains(&t2));

        // Fire t1
        net.fire(&mut marking, t1).unwrap();
        assert_eq!(marking, Marking::from([0, 0, 1, 1, 0]));

        // t3 not yet enabled (needs 2 tokens in p3)
        assert!(!net.is_enabled(&marking, t3));

        // Fire t2
        net.fire(&mut marking, t2).unwrap();
        assert_eq!(marking, Marking::from([0, 0, 0, 2, 0]));

        // Now t3 (join) is enabled
        assert!(net.is_enabled(&marking, t3));
        net.fire(&mut marking, t3).unwrap();
        assert_eq!(marking, Marking::from([0, 0, 0, 0, 1]));
    }

    #[test]
    fn test_build_error_no_places() {
        let builder = NetBuilder::new();
        assert!(matches!(builder.build(), Err(BuildError::NoPlaces)));
    }

    #[test]
    fn test_build_error_no_transitions() {
        let mut builder = NetBuilder::new();
        builder.add_place();
        assert!(matches!(builder.build(), Err(BuildError::NoTransitions)));
    }

    #[test]
    fn test_build_error_disconnected_transition() {
        let mut builder = NetBuilder::new();
        let [p0, p1] = builder.add_places();
        let _t0 = builder.add_transition().build(); // No arcs
        let t1 = builder.add_transition().build();
        builder.add_arc((p0, t1));
        builder.add_arc((t1, p1));

        assert!(matches!(
            builder.build(),
            Err(BuildError::DisconnectedTransition(_))
        ));
    }
}
