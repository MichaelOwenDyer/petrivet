use crate::core::net::PlaceIdx;
use crate::core::net::idx_set::PlaceIdxSet;
use crate::core::system::marking::IdxMarking;
use crate::system::boundedness::{Boundedness, BoundednessAnalysisMethod, K};

/// Current knowledge about the boundedness of a place or Petri net.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    /// Minimum boundedness that we can guarantee.
    pub lower: Boundedness,
    /// Evidence for the lower bound.
    pub lower_provenance: BoundednessAnalysisMethod,
    /// Maximum boundedness that we can guarantee.
    pub upper: Boundedness,
    /// Evidence for the upper bound.
    pub upper_provenance: BoundednessAnalysisMethod,
}

impl Bounds {
    pub fn new(initial_marking: K) -> Self {
        let initial_bound = Boundedness::Bounded(initial_marking);
        Self {
            lower: initial_bound,
            lower_provenance: BoundednessAnalysisMethod::InitialMarking,
            upper: Boundedness::Unbounded,
            upper_provenance: BoundednessAnalysisMethod::InitialMarking,
        }
    }

    /// Returns the componentwise maximum of a slice of `Bounds`.
    /// If the argument slice is the bounds of all places in a Petri net,
    /// this returns the bounds of the entire net.
    pub fn aggregate(place_bounds: &[Bounds]) -> Self {
        let mut max_lower = Boundedness::Bounded(0);
        let mut max_lower_provenance = BoundednessAnalysisMethod::InitialMarking;
        let mut max_upper = Boundedness::Bounded(0);
        let mut max_upper_provenance = BoundednessAnalysisMethod::InitialMarking;
        for bounds in place_bounds {
            if bounds.lower > max_lower {
                max_lower = bounds.lower;
                max_lower_provenance = bounds.lower_provenance;
            }
            if bounds.upper > max_upper {
                max_upper = bounds.upper;
                max_upper_provenance = bounds.upper_provenance;
            }
        }
        Self {
            lower: max_lower,
            lower_provenance: max_lower_provenance,
            upper: max_upper,
            upper_provenance: max_upper_provenance,
        }
    }

    /// Sets the upper bound to the given value, with the given provenance.
    pub fn update_upper(
        &mut self,
        boundedness: Boundedness,
        provenance: BoundednessAnalysisMethod
    ) {
        if boundedness < self.upper {
            debug_assert!(boundedness >= self.lower, "Upper bound cannot be less than lower bound");
            self.upper = boundedness;
            self.upper_provenance = provenance;
        }
    }

    /// Sets the lower bound to the given value, with the given provenance.
    pub fn update_lower(
        &mut self,
        boundedness: Boundedness,
        provenance: BoundednessAnalysisMethod
    ) {
        if boundedness > self.lower {
            debug_assert!(boundedness <= self.upper, "Lower bound cannot exceed upper bound");
            self.lower = boundedness;
            self.lower_provenance = provenance;
        }
    }

    /// Sets both the lower and upper bounds to the same value, with the same provenance.
    pub fn update_exact(
        &mut self,
        boundedness: Boundedness,
        provenance: BoundednessAnalysisMethod
    ) {
        self.update_upper(boundedness, provenance);
        self.update_lower(boundedness, provenance);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// The user is asking about the entire Petri net.
    PetriNet,
    /// The user is asking about a specific place in the net.
    Place(PlaceIdx),
}

impl Scope {
    /// Returns the range of place indices that this scope covers.
    pub fn places(self, place_count: usize) -> PlaceIdxSet {
        match self {
            Scope::PetriNet => PlaceIdxSet::all_of(place_count),
            Scope::Place(p_idx) => {
                let mut set = PlaceIdxSet::none_of(place_count);
                set.add(p_idx);
                set
            }
        }
    }

    /// Returns the bounds of the places covered by this scope, using the provided bounds cache.
    pub fn bounds(self, bounds: &[Bounds]) -> Bounds {
        match self {
            Scope::PetriNet => Bounds::aggregate(bounds),
            Scope::Place(p_idx) => bounds[p_idx],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Goal {
    /// The user wants to know if the place or net is bounded, but does not care about the exact bound.
    /// If the upper bound is known to be finite, we can return a positive result.
    /// If the lower bound is known to be infinite, we can return a negative result.
    Bounded,
    /// The user wants to know if the place or net is k-bounded for a specific k.
    /// If the upper bound is known to be less than or equal to k, we can return a positive result.
    /// If the lower bound is known to be greater than k, we can return a negative result.
    KBounded(K),
    /// The user wants to know the exact bound, if it can be computed.
    /// If the upper and lower bounds are equal, we can return the exact bound.
    Exact,
}

#[derive(Debug, Clone, Copy)]
pub enum Effort {
    /// Only use efficient (polynomial-time) procedures.
    Efficient,
    /// Use any available procedure.
    Any,
}

#[derive(Debug, Clone)]
pub struct BoundsCache {
    /// Cache of computed bounds for places, used to avoid recomputing them in repeated queries.
    /// Invalidated whenever the marking is mutated, since bounds depend on the current marking.
    pub by_place: Box<[Bounds]>,
    /// Set of places which we have not yet refined the bounds for by subclass analysis.
    pub should_refine_by_subclass: PlaceIdxSet,
    /// Set of places which we have not yet refined the bounds for by structural LP.
    pub should_refine_by_structural_lp: PlaceIdxSet,
}

impl BoundsCache {
    pub fn new(marking: &IdxMarking<u32>) -> Self {
        let by_place = marking.iter().map(|&tokens| Bounds::new(tokens as K)).collect();
        Self {
            by_place,
            should_refine_by_subclass: PlaceIdxSet::all_of(marking.place_count()),
            should_refine_by_structural_lp: PlaceIdxSet::all_of(marking.place_count()),
        }
    }

    /// Returns true if we can answer the boundedness question for the given scope and goal
    /// with the current bounds cache.
    pub fn is_answered(&self, scope: Scope, goal: Goal) -> bool {
        match goal {
            Goal::Bounded => self.read_bounded(scope).is_some(),
            Goal::KBounded(k) => self.read_k_bounded(scope, k).is_some(),
            Goal::Exact => self.read_exact(scope).is_some(),
        }
    }

    /// Returns Some(true) if the Petri net is bounded,
    /// Some(false) if it is unbounded,
    /// and None if the answer has not yet been computed.
    pub fn read_bounded(&self, scope: Scope) -> Option<bool> {
        let bounds = scope.bounds(&self.by_place);
        if bounds.upper.is_bounded() {
            Some(true)
        } else if bounds.lower.is_unbounded() {
            Some(false)
        } else {
            None
        }
    }

    /// Returns Some(true) if the Petri net is k-bounded for the given k,
    /// Some(false) if it is unbounded or bounded with a bound greater than k,
    /// and None if the answer has not yet been computed.
    pub fn read_k_bounded(&self, scope: Scope, k: K) -> Option<bool> {
        let bounds = scope.bounds(&self.by_place);
        if bounds.upper.is_k_bounded(k) {
            Some(true)
        } else if !bounds.lower.is_k_bounded(k) {
            Some(false)
        } else {
            None
        }
    }

    /// Returns the exact bound for the given scope, if it can be determined.
    pub fn read_exact(&self, scope: Scope) -> Option<Boundedness> {
        let b = scope.bounds(&self.by_place);
        (b.lower == b.upper).then_some(b.upper)
    }
}