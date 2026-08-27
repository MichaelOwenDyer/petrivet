use crate::core::analysis::incidence::IncidenceMatrix;
use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::solver::SmtSolver;
use crate::core::marking::IdxMarking;
use crate::core::net::{IdxNode, TransitionIdx};
use crate::core::parikh::IdxParikhVector;
use ahash::{HashMap, HashMapExt, HashSet};
use fixedbitset::FixedBitSet;
use petgraph::Direction;
use petgraph::Graph;
use petgraph::visit::IntoNodeReferences;
use std::hash::{Hash, Hasher};
use crate::core::cegar::lemma::IdxLemma;

/// A mutable budget of transition firings, which we attempt to completely consume in a
/// guided depth-first search of the state space. If we reach a dead end, we can report
/// the state and remaining budget to the SMT solver as a refinement.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TransitionFiringBudget {
    /// The number of times each transition can still be fired.
    by_transition: Vec<u32>,
    /// The sum of all remaining firings across all transitions.
    total: u32,
}

impl TransitionFiringBudget {
    /// Create a new `TransitionFiringBudget` from the given Parikh vector.
    fn new(parikh_vector: IdxParikhVector<u32>) -> Self {
        Self {
            total: parikh_vector.iter().sum(),
            by_transition: parikh_vector.into_inner(),
        }
    }

    /// Decrements the remaining budget for the given transition.
    /// The caller must ensure that the transition has a positive
    /// remaining budget before calling this.
    fn fire(&mut self, t: TransitionIdx) {
        self.by_transition[t] -= 1;
        self.total -= 1;
    }

    /// Increments the remaining budget for the given transition,
    /// used when backtracking in the DFS search.
    fn unfire(&mut self, t: TransitionIdx) {
        self.by_transition[t] += 1;
        self.total += 1;
    }

    /// Returns true if there is no budget remaining
    /// (this should actually indicate that we have reached a valid solution).
    const fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// An Increment Constraint (Wimmel & Wolf 2011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementRefinement {
    /// The trace of transitions fired before hitting the dead end.
    pub firing_sequence: Vec<TransitionIdx>,
    /// The dead end marking reached at the end of the firing sequence.
    pub marking: IdxMarking<u32>,
    /// The remaining budget of transition firings that could not be executed.
    pub remaining_budget: TransitionFiringBudget,
}

/// A guided state space explorer which attempts to execute a given Parikh vector
/// in the underlying Petri net. It uses a depth-first search with backtracking to explore
/// the state space, and it keeps track of the remaining budget of transition firings to
/// ensure that it does not exceed the given Parikh vector. If it reaches a dead end, it
/// records the bottleneck places and the remaining budget for diagnosis.
pub struct GuidedExplorer;

/// The maximum number of dead ends to keep track of for reporting to the SMT solver.
/// We only track the "best" dead ends (those with the smallest remaining budget) to avoid
/// excessive memory usage.
const MAX_DEAD_ENDS: usize = 5;

impl GuidedExplorer {
    /// Attempts to realize the given Parikh vector from the initial marking `m0`
    /// as a firing sequence in the Petri net.
    /// If successful, returns the firing sequence and the marking at the end of the sequence.
    /// If unsuccessful, returns a batch of closest dead ends with information about
    /// which places prevented further progress.
    pub fn realize_parikh_vector(
        problem: &CegarProblem,
        parikh_vector: IdxParikhVector<u32>
    ) -> Result<(Vec<TransitionIdx>, IdxMarking<u32>), Vec<IncrementRefinement>> {
        /// A frame in the DFS stack.
        struct DfsFrame {
            /// The ample set of transitions chosen to explore from this state.
            ample_set: Vec<TransitionIdx>,
            /// The index of the next transition in the ample set to explore.
            next_idx: usize,
            /// The transition that was fired to REACH this state (used for backtracking).
            /// None for the root state.
            fired_to_reach: Option<TransitionIdx>,
        }

        let mut budget = TransitionFiringBudget::new(parikh_vector);
        let mut marking = problem.m0.clone();

        let mut stack: Vec<DfsFrame> = Vec::with_capacity(budget.total as usize);
        let mut current_trace: Vec<TransitionIdx> = Vec::with_capacity(budget.total as usize);
        let mut visited: HashSet<u64> = HashSet::default();
        let mut closest_dead_ends: Vec<IncrementRefinement> = Vec::with_capacity(MAX_DEAD_ENDS + 1);

        let initial_ample_set = Self::compute_ample_set(problem, &marking, &budget);
        if initial_ample_set.is_empty() {
            // The vector is immediately unexecutable.
            Self::record_dead_end(&budget, &marking, &current_trace, &mut closest_dead_ends);
            return Err(closest_dead_ends);
        }

        stack.push(DfsFrame {
            ample_set: initial_ample_set,
            next_idx: 0,
            fired_to_reach: None,
        });

        visited.insert(Self::hash_state(&marking, &budget));

        while let Some(frame) = stack.last_mut() {
            if frame.next_idx < frame.ample_set.len() {
                // Explore the next transition in the ample set
                let t_idx = frame.ample_set[frame.next_idx];
                frame.next_idx += 1;

                // Update the DFS state
                problem.net.fire(t_idx, &mut marking);
                budget.fire(t_idx);
                current_trace.push(t_idx);

                if budget.is_empty() {
                    return Ok((current_trace, marking));
                }

                // Check if we have visited this state before
                let state_hash = Self::hash_state(&marking, &budget);
                if !visited.insert(state_hash) {
                    // We have seen this state. Backtrack immediately.
                    budget.unfire(t_idx);
                    problem.net.unfire(t_idx, &mut marking);
                    current_trace.pop();
                    continue;
                }

                let next_ample_set = Self::compute_ample_set(problem, &marking, &budget);
                if next_ample_set.is_empty() {
                    // We hit a dead end! Record it if it's one of the best we've seen so far.
                    Self::record_dead_end(&budget, &marking, &current_trace, &mut closest_dead_ends);
                    budget.unfire(t_idx);
                    problem.net.unfire(t_idx, &mut marking);
                    current_trace.pop();
                } else {
                    // Push a new frame to recurse deeper
                    stack.push(DfsFrame {
                        ample_set: next_ample_set,
                        next_idx: 0,
                        fired_to_reach: Some(t_idx),
                    });
                }
            } else {
                // We have exhausted all transitions in this frame's ample set.
                // Pop the frame and backtrack the transition that got us here.
                let popped = stack.pop().expect("stack should not be empty here");
                if let Some(t_fired) = popped.fired_to_reach {
                    budget.unfire(t_fired);
                    problem.net.unfire(t_fired, &mut marking);
                    current_trace.pop();
                }
            }
        }

        // If we exhaust the stack without ever consuming the entire budget, the vector is spurious.
        Err(closest_dead_ends)
    }

    /// Computes the Strong Stubborn Set (restricted to active budget) to use as the ample set.
    /// Returns an empty vector if no active transitions are enabled (a dead end).
    fn compute_ample_set(
        problem: &CegarProblem,
        marking: &IdxMarking<u32>,
        budget: &TransitionFiringBudget
    ) -> Vec<TransitionIdx> {
        let mut active_enabled = FixedBitSet::with_capacity(problem.net.transition_count());
        for t_idx in problem.net.transition_indices() {
            if budget.by_transition[t_idx] > 0 && problem.net.is_enabled_in(t_idx, marking) {
                active_enabled.insert(t_idx);
            }
        }

        if active_enabled.is_clear() {
            return Vec::new();
        }

        // Fast Path: Singleton Check
        // Look for an enabled transition that shares NO input places with any other active transition.
        // Such a transition can always be fired without affecting the executability of any other
        // transition in the budget, so we do not need to branch on any other transitions in the ample set.
        'singleton_search: for t_idx in active_enabled.ones() {
            for &p_idx in &problem.net.preset_t[t_idx] {
                for &t_rival in &problem.net.postset_p[p_idx] {
                    if t_rival != t_idx && budget.by_transition[t_rival] > 0 {
                        continue 'singleton_search;
                    }
                }
            }
            // If we reach here, t_idx is a singleton. Return it as the ample set.
            return vec![t_idx];
        }

        // Fallback: Strong Stubborn Set Algorithm
        let mut stubborn_set = FixedBitSet::with_capacity(problem.net.transition_count());
        let mut worklist = FixedBitSet::with_capacity(problem.net.transition_count());

        // Initialize with the first enabled active transition
        let t0: TransitionIdx = active_enabled.ones().next().expect("there should be at least one enabled transition");
        stubborn_set.insert(t0);
        worklist.insert(t0);

        while let Some(t_idx) = worklist.ones().next() {
            worklist.remove(t_idx);
            if problem.net.is_enabled_in(t_idx, marking) {
                // Rule for enabled transitions:
                // Add all transitions that share an input place with t_idx
                for &p_idx in &problem.net.preset_t[t_idx] {
                    for &t_rival in &problem.net.postset_p[p_idx] {
                        if budget.by_transition[t_rival] > 0 && !stubborn_set.contains(t_rival) {
                            stubborn_set.insert(t_rival);
                            worklist.insert(t_rival);
                        }
                    }
                }
            } else {
                // Rule for disabled transitions: find one insufficiently marked preset place and
                // add all of its budgeted feeders.
                for &p_idx in &problem.net.preset_t[t_idx] {
                    if marking[p_idx] < u32::from(problem.net.incidence_matrix.get_consume(t_idx, p_idx)) {
                        // p_idx is insufficiently marked for t_idx.
                        // Add all of its budgeted feeders to the stubborn set.
                        // If no feeder transition has remaining budget, then t_idx is permanently
                        // dead, and we don't need to consider it further.
                        for &t_feeder in &problem.net.preset_p[p_idx] {
                            if budget.by_transition[t_feeder] > 0 && !stubborn_set.contains(t_feeder) {
                                stubborn_set.insert(t_feeder);
                                worklist.insert(t_feeder);
                            }
                        }
                        // We only need to consider one insufficiently marked place.
                        break;
                    }
                }
            }
        }

        // The ample set is the intersection of the stubborn set and the enabled set
        active_enabled.intersection(&stubborn_set).collect()
    }

    /// Records a dead end if it is among the "best" (smallest remaining budget) seen so far.
    fn record_dead_end(
        budget: &TransitionFiringBudget,
        marking: &IdxMarking<u32>,
        trace: &[TransitionIdx],
        best_dead_ends: &mut Vec<IncrementRefinement>,
    ) {
        // If we haven't reached capacity
        if best_dead_ends.len() < MAX_DEAD_ENDS {
            // Do nothing, we can just add this dead end
        } else if budget.total < best_dead_ends.last().expect("len >= MAX_DEAD_ENDS").remaining_budget.total {
            // We have reached capacity, but this dead end is better than the worst we've seen so far.
            best_dead_ends.pop();
        } else {
            // This dead end is worse than the worst we've seen so far, so we don't record it.
            return;
        }
        let dead_end = IncrementRefinement {
            firing_sequence: trace.to_vec(),
            remaining_budget: budget.clone(),
            marking: marking.clone(),
        };
        best_dead_ends.push(dead_end);
        // Keep sorted by total remaining budget (ascending) so the worst is at the end
        best_dead_ends.sort_by_key(|d| d.remaining_budget.total);
    }

    /// Computes a hash of the current state (marking and remaining budget) for cycle detection.
    fn hash_state(marking: &IdxMarking<u32>, budget: &TransitionFiringBudget) -> u64 {
        let mut hasher = ahash::AHasher::default();
        marking.hash(&mut hasher);
        budget.hash(&mut hasher);
        hasher.finish()
    }
}

/// Wimmel & Wolf 2011, Section 4, Part 1: build the bipartite graph `G = (S0 ∪ T0, E)` and
/// return the (places, transitions) of each *source* strongly-connected-component (one with no
/// incoming edges from outside itself) - the independent bottleneck clusters that can only
/// receive further tokens from *outside* the remainder `r`, not from firing more of `r` itself.
///
/// `T0` is the remainder transitions (`r(t) > 0`); `S0` is the places insufficiently marked for
/// some transition in `T0`. An edge `s → t` means `t` (in `T0`) needs more of `s` than `marking`
/// currently has; an edge `t → s` means `t` (in `T0`) is a net producer of `s` (in `S0`).
fn find_bottleneck_components(
    problem: &CegarProblem,
    marking: &IdxMarking<u32>,
    remaining_budget: &TransitionFiringBudget,
) -> Vec<(FixedBitSet, FixedBitSet)> {
    let mut graph: Graph<IdxNode, ()> = Graph::new();
    let mut node_index: HashMap<IdxNode, petgraph::graph::NodeIndex> = HashMap::new();

    let t0: Vec<TransitionIdx> = problem.net
        .transition_indices()
        .filter(|&t| remaining_budget.by_transition[t] > 0)
        .collect();

    // s -> t: t needs more of s than `marking` has. This is also exactly S0's membership test.
    for &t in &t0 {
        for &p in &problem.net.preset_t[t] {
            if u32::from(problem.net.incidence_matrix.get_consume(t, p)) > marking[p] {
                let p_node = *node_index.entry(IdxNode::Place(p))
                    .or_insert_with(|| graph.add_node(IdxNode::Place(p)));
                let t_node = *node_index.entry(IdxNode::Transition(t))
                    .or_insert_with(|| graph.add_node(IdxNode::Transition(t)));
                graph.add_edge(p_node, t_node, ());
            }
        }
    }
    // t -> s: t is a net producer of s (only meaningful for s already known to be in S0).
    for &t in &t0 {
        for &p in &problem.net.postset_t[t] {
            if let Some(&p_node) = node_index.get(&IdxNode::Place(p))
                && problem.net.incidence_matrix.get_effect(t, p) > 0
            {
                let t_node = *node_index.entry(IdxNode::Transition(t))
                    .or_insert_with(|| graph.add_node(IdxNode::Transition(t)));
                graph.add_edge(t_node, p_node, ());
            }
        }
    }

    if graph.node_count() == 0 {
        return Vec::new();
    }

    let condensation_graph = petgraph::algo::condensation(graph, true);
    condensation_graph.node_references()
        .filter(|(idx, _)| {
            // filter for source SCCs
            condensation_graph.neighbors_directed(*idx, Direction::Incoming).next().is_none()
        })
        .map(|(_, members)| {
            let mut places = FixedBitSet::with_capacity(problem.net.place_count());
            let mut transitions = FixedBitSet::with_capacity(problem.net.transition_count());
            for &n in members {
                match n {
                    IdxNode::Place(p_idx) => {
                        places.insert(p_idx);
                    }
                    IdxNode::Transition(t_idx) => {
                        transitions.insert(t_idx);
                    }
                }
            }
            (places, transitions)
        })
        .collect()
}

/// Remainder transitions outside the component that still need tokens from
/// one of the component's places.
fn external_consumers(
    problem: &CegarProblem,
    marking: &IdxMarking<u32>,
    remaining_budget: &TransitionFiringBudget,
    component_places: &FixedBitSet,
    component_transitions: &FixedBitSet,
) -> FixedBitSet {
    let mut xi = FixedBitSet::with_capacity(problem.net.transition_count());
    for t_idx in component_transitions.zeroes() {
        if remaining_budget.by_transition[t_idx] > 0 && component_places.ones().any(|p_idx| {
            u32::from(problem.net.incidence_matrix.get_consume(t_idx, p_idx)) > marking[p_idx]
        }) {
            xi.insert(t_idx);
        }
    }
    xi
}

/// Wimmel & Wolf 2011, Section 4, Part 2: estimate how many additional tokens a bottleneck
/// component needs from outside itself. Guaranteed (Corollary 3) to return a value in
/// `[1, actual_number_needed]`.
fn estimate_needed_tokens(
    incidence_matrix: &IncidenceMatrix,
    marking: &IdxMarking<u32>,
    component_places: &FixedBitSet,
    component_transitions: &FixedBitSet,
    external_consumers: &FixedBitSet,
) -> u32 {
    if component_transitions.is_clear() {
        // The component is a single place `s` with no internal producer: every transition that
        // could feed it comes from outside (`external_consumers`). Group those consumers by how
        // many tokens they give back to `s` when they fire (some are net-neutral loops, not pure
        // sinks), and process the highest-value groups first, so a group's leftover can offset
        // the token need of the next (lower-value) group instead of being double-counted.
        let s = component_places.ones().next().expect("non-empty component");
        let mut groups: HashMap<u8, (u32, u32)> = HashMap::new(); // production-back j -> (count, total consumption)
        for t in external_consumers.ones() {
            let produce_back = incidence_matrix.get_produce(t, s);
            let consume = u32::from(incidence_matrix.get_consume(t, s));
            let (count, consumption) = groups.entry(produce_back).or_insert((0, 0));
            *count += 1;
            *consumption += consume;
        }
        let mut js: Vec<u8> = groups.keys().copied().collect();
        js.sort_unstable_by(|a, b| b.cmp(a));

        let mut n: i64 = 0;
        let mut c: i64 = 0;
        for j in js {
            let (count, total_consume) = groups[&j];
            c = c - i64::from(j) * (i64::from(count) - 1) + i64::from(total_consume);
            if c > 0 {
                n += c;
            }
            c = -i64::from(j);
        }
        // Corollary 3 guarantees n > 0 here; fall back to the loosest still-sound bound (1)
        // rather than emit a vacuous or unsound constraint if that guarantee is ever violated.
        u32::try_from(n).unwrap_or(1).max(1)
    } else {
        // The component has internal remainder transitions: once the *cheapest* one to satisfy
        // becomes enabled, it can produce tokens for the rest of the component in turn.
        // n := min_{t∈Ti} Σ_{s∈Si} (F(s,t) - m̂(s))
        component_transitions
            .ones()
            .map(|t| {
                component_places
                    .ones()
                    .map(|p| {
                        let consume = u32::from(incidence_matrix.get_consume(t, p));
                        consume.saturating_sub(marking[p])
                    })
                    .sum::<u32>()
            })
            .min()
            .unwrap_or(1)
            .max(1)
    }
}

impl IncrementRefinement {
    /// Encodes an increment constraint (Wimmel & Wolf 2011, Section 4) for every independent
    /// bottleneck component found in this dead end - see `find_bottleneck_components`.
    pub fn encode_into<S: SmtSolver>(
        self,
        problem: &CegarProblem,
        solver: &mut S,
        transition_terms: &[S::Int],
        callback: Option<&dyn Fn(IdxLemma)>,
    ) {
        for (component_places, component_transitions) in
            find_bottleneck_components(problem, &self.marking, &self.remaining_budget)
        {
            let xi = external_consumers(
                problem,
                &self.marking,
                &self.remaining_budget,
                &component_places,
                &component_transitions,
            );
            let needed_tokens = estimate_needed_tokens(
                &problem.net.incidence_matrix,
                &self.marking,
                &component_places,
                &component_transitions,
                &xi,
            );

            // Wimmel & Wolf 2011, Corollary 4: encode the increment constraint for one bottleneck
            // component. `Ti` here is Corollary 4's `Ti` (transitions outside the remainder that are net
            // producers of the component), distinct from - and unrelated to - Part 1's `Ti` (the
            // component's own internal remainder transitions).
            let mut lhs_terms = Vec::new();
            let mut tokens_already_produced: u32 = 0;

            for t in problem.net.transition_indices() {
                if self.remaining_budget.by_transition[t] == 0 {
                    let net_production: i32 = component_places
                        .ones()
                        .map(|p| i32::from(problem.net.incidence_matrix.get_effect(t, p)))
                        .sum();

                    if net_production > 0 {
                        let weight_term = solver.mk_int(i64::from(net_production));
                        let t_term = transition_terms[t].clone();
                        lhs_terms.push(solver.mul([weight_term, t_term]));

                        let fired_count = self.firing_sequence.iter().filter(|&&fired_t| fired_t == t).count() as u32;
                        tokens_already_produced += (net_production as u32) * fired_count;
                    }
                }
            }

            if lhs_terms.is_empty() {
                return;
            }

            let lhs = solver.add(lhs_terms);
            let rhs_val = needed_tokens + tokens_already_produced;
            let rhs = solver.mk_int(i64::from(rhs_val));
            let constraint = solver.ge(&lhs, &rhs);
            let lemma = IdxLemma::Increment {
                component_places,
                component_transitions,
                firing_sequence: self.firing_sequence.clone(),
            };
            if let Some(callback) = callback {
                callback(lemma.clone());
            }
            solver.assert_tracked(&constraint, lemma);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::NetBuilder;

    /// A 2-cycle p <-> q, both places empty. If both t_pq and t_qp still need to fire (the
    /// `Ti != ∅` case of Wimmel & Wolf's Part 2), the cheapest one to unblock needs exactly 1
    /// extra token (each is short by exactly 1 in its own preset place, and produces enough for
    /// the other once it fires).
    #[test]
    fn estimate_needed_tokens_internal_component() {
        let mut b = NetBuilder::new();
        let [p, q] = b.add_places();
        let [t_pq, t_qp] = b.add_transitions();
        b.add_arcs((p, t_pq, q));
        b.add_arcs((q, t_qp, p));
        let net = b.build().unwrap().dense_net;

        let p_idx = 0;
        let q_idx = 1;
        let marking = IdxMarking(vec![0, 0]);

        let mut component_places = FixedBitSet::with_capacity(2);
        component_places.insert(p_idx);
        component_places.insert(q_idx);
        let mut component_transitions = FixedBitSet::with_capacity(2);
        component_transitions.insert(0);
        component_transitions.insert(1);
        let external_consumers = FixedBitSet::with_capacity(2); // unused when Ti != ∅

        let n = estimate_needed_tokens(
            &net.incidence_matrix,
            &marking,
            &component_places,
            &component_transitions,
            &external_consumers,
        );
        assert_eq!(n, 1);
    }

    /// A single starved place `s` fed by no internal producer (`Ti = ∅`), consumed by three
    /// external transitions: two self-loops on `s` (consume 1, produce 1 back - net-neutral) and
    /// one pure sink (consume 1, produce nothing back). Since the self-loops don't cost any net
    /// tokens once correctly grouped, the true minimum extra tokens needed is exactly 1 (for the
    /// sink alone) - not 3, which is what naively summing every transition's consumption would
    /// give. This is the case Wimmel & Wolf's group-by-production-value accounting exists for.
    #[test]
    fn estimate_needed_tokens_external_component_with_self_loops() {
        let mut b = NetBuilder::new();
        let s = b.add_place();
        let [t1, t2, t3] = b.add_transitions();
        b.add_arcs((s, t1, s)); // self-loop: consumes 1, produces 1 back
        b.add_arcs((s, t2, s)); // self-loop: consumes 1, produces 1 back
        b.add_arc((s, t3)); // pure sink: consumes 1, produces nothing back
        let net = b.build().unwrap().dense_net;

        let s_idx = 0;
        let marking = IdxMarking(vec![0]);

        let mut component_places = FixedBitSet::with_capacity(1);
        component_places.insert(s_idx);
        let component_transitions = FixedBitSet::with_capacity(3); // Ti = ∅
        let mut external_consumers = FixedBitSet::with_capacity(3);
        external_consumers.insert(0);
        external_consumers.insert(1);
        external_consumers.insert(2);

        let n = estimate_needed_tokens(
            &net.incidence_matrix,
            &marking,
            &component_places,
            &component_transitions,
            &external_consumers,
        );
        assert_eq!(n, 1, "self-loops should not be double-counted against the sink's real need");
    }
}
