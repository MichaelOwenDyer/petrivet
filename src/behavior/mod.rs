mod marking;

pub use crate::behavior::marking::{Marking, NormalMarking, Omega, OmegaMarking, Tokens};
pub use crate::behavior::model::CoverabilityGraph;
use crate::behavior::model::ReachabilityGraph;
use crate::behavior::state_machine::NoStrategy;
use crate::structure::{Net, Place, SNet, Transition};
use std::fmt::Debug;
use std::ops::{Add, Index};

pub mod model {
    use crate::behavior::marking::{NormalMarking, OmegaMarking};
    use crate::structure::Transition;
    use ahash::HashMapExt;
    use petgraph::graph::NodeIndex;
    use petgraph::Graph;
    use std::collections::VecDeque;
    use std::fmt::Debug;
    use std::hash::Hash;

    /// An exploration of the state space of a Petri net.
    #[derive(Debug, Clone)]
    pub struct StateSpace<M> {
        /// The graph structure (V, E). Node weights are Markings, edge weights are Transitions.
        pub graph: Graph<M, Transition>,

        /// A fast lookup set to check if an *exact* state has been seen before.
        pub seen_nodes: ahash::HashMap<M, NodeIndex>,

        /// The worklist from the algorithm. We store the `NodeIndex` to locate the markings
        /// in the graph where we need to explore successors.
        /// Use this as a queue (FIFO) for breadth-first search,
        /// or as a stack (LIFO) for depth-first search.
        pub frontier: VecDeque<NodeIndex>,
    }

    pub type ReachabilityGraph = StateSpace<NormalMarking>;
    pub type CoverabilityGraph = StateSpace<OmegaMarking>;


    impl<State> StateSpace<State> where State: Clone + Eq + Hash {
        #[must_use]
        pub fn new(m0: &NormalMarking) -> StateSpace<State>
            where
                State: From<NormalMarking>
        {
            let mut graph = Graph::new();
            let mut seen_nodes = ahash::HashMap::new();
            let mut frontier = VecDeque::new();
            let initial_state = State::from(m0.clone());
            let idx = graph.add_node(initial_state.clone());
            seen_nodes.insert(initial_state, idx);
            frontier.push_back(idx);
            StateSpace { graph, seen_nodes, frontier }
        }

        /// Registers a new marking in the state space.
        /// This is a lookup and insertion operation.
        /// If the marking has been seen before, it simply adds an edge from source_idx to that marking.
        /// If the marking is new, it adds it to the graph, the seen_nodes map, and the frontier,
        /// then adds the edge.
        /// Returns the `NodeIndex` of the registered marking.
        pub fn register_marking(
            &mut self,
            from: NodeIndex,
            over: Transition,
            to: State
        ) -> NodeIndex {
            let to = if let Some(idx) = self.seen_nodes.get(&to) {
                *idx
            } else {
                let idx = self.graph.add_node(to.clone());
                self.seen_nodes.insert(to, idx);
                self.frontier.push_back(idx);
                idx
            };
            self.graph.update_edge(from, to, over);
            to
        }
        #[must_use]
        pub fn path_exists(&self, source: NodeIndex, target: NodeIndex) -> bool {
            petgraph::algo::has_path_connecting(&self.graph, source, target, None)
        }
        #[must_use]
        pub fn steps(&self) -> usize {
            self.graph.edge_count()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liveness {
    L0,
    L1,
    L2,
    L3,
    L4,
}

/// Cumulative analysis results for a Petri net.
/// Meant to be computed once and then queried multiple times.
#[derive(Debug, Clone)]
pub struct Findings {
    pub reachability_graph: ReachabilityGraph,
    pub coverability_graph: CoverabilityGraph,
    pub liveness: Box<[Option<Liveness>]>,
    pub boundedness: Box<[Option<Boundedness>]>,
    pub deadlock_free: Option<bool>,
}

impl Findings {
    #[must_use]
    pub fn new(net: &Net, m0: &NormalMarking) -> Self {
        Self {
            reachability_graph: ReachabilityGraph::new(m0),
            coverability_graph: CoverabilityGraph::new(m0),
            liveness: vec![None; net.n_transitions().into()].into_boxed_slice(),
            boundedness: vec![None; net.n_places().into()].into_boxed_slice(),
            deadlock_free: None,
        }
    }
}

/// (N, M0)
/// A Petri net structure with an initial marking.
#[derive(Debug, Clone)]
pub struct PetriNet<'net> {
    net: &'net Net,
    m0: NormalMarking,
    findings: Findings,
}

impl<'net> From<(&'net Net, NormalMarking)> for PetriNet<'net> {
    fn from((net, m0): (&'net Net, NormalMarking)) -> Self {
        let findings = Findings::new(net, &m0);
        Self { net, m0, findings }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundedness {
    Bounded(Tokens),
    Unbounded,
}

impl Boundedness {
    #[must_use]
    pub fn is_unbounded(&self) -> bool {
        matches!(self, Boundedness::Unbounded)
    }
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        matches!(self, Boundedness::Bounded(_))
    }
    #[must_use]
    pub fn bound(&self) -> Option<Tokens> {
        match self {
            Boundedness::Bounded(t) => Some(*t),
            Boundedness::Unbounded => None,
        }
    }
}

impl Net {
    pub fn enabled_transitions<M>(&self, marking: &M) -> impl Iterator<Item = Transition>
    where
        M: Default + Clone + Index<Place, Output: PartialOrd<Tokens>>
    {
        self.transitions().filter(|&t| {
            self.preset_t(t).all(|&p| {
                marking[p] >= Tokens(1)
            })
        })
    }

    pub fn enabled_transitions_iter<M>(&self, marking: &M) -> impl Iterator<Item = (Transition, M)>
        where
            M: Clone,
            M: Index<Place, Output: PartialOrd<Tokens>>,
            M: for <'a> Add<&'a [i8], Output = Result<M, ()>>,
    {
        self.transitions().filter_map(move |transition| {
            // todo: optimize by performing the check and addition in one pass
            if self.preset_t(transition).all(|&p| marking[p] >= Tokens(1)) {
                let column = self.incidence_matrix.column(transition);
                let transition_effect = column.as_slice();
                match marking.clone() + transition_effect {
                    Ok(m) => Some((transition, m)),
                    Err(()) => None,
                }
            } else {
                None
            }
        })
    }
}

impl PetriNet<'_> {
    pub fn coverability_iter(&mut self) -> state_machine::StateSpaceIterator<'_, NoStrategy, OmegaMarking> {
        state_machine::StateSpaceIterator::new(
            self.net,
            &mut self.findings.coverability_graph,
        )
    }
    pub fn reachability_iter(&mut self) -> state_machine::StateSpaceIterator<'_, NoStrategy, NormalMarking> {
        state_machine::StateSpaceIterator::new(
            self.net,
            &mut self.findings.reachability_graph,
        )
    }
    /// Explores the coverability graph until the boundedness of all places has been discovered.
    pub fn boundedness(&mut self) -> &[Option<Boundedness>] {
        algorithm::boundedness(self.net, &self.m0, &mut self.findings.coverability_graph, &mut self.findings.boundedness);
        &self.findings.boundedness
    }
    pub fn place_boundedness(&mut self, place: Place) -> Boundedness {
        todo!()
    }
    /// Liveness is decidable because it can be reduced to reachability.
    pub fn check_live(&mut self) -> bool {
        self.net.transitions().all(|t| self.check_transition_live(t))
    }
    /// A transition is live if for any reachable marking M,
    /// there exists a firing sequence from M that enables t.
    /// This is decidable because it can be reduced to reachability.
    /// However, there are certain shortcuts we can take if we have already computed
    /// the coverability graph or the reachability graph.
    pub fn check_transition_live(&mut self, transition: Transition) -> bool {
        todo!("check if transition is live by exploring the reachability graph")
    }
    /// This method tries to find a firing sequence which reaches the provided marking from m0, if one exists.
    /// Leroux (2012) algorithm using Presburger arithmetic to find semilinear sets of reachable markings
    /// to disprove reachability, parallel to reachability graph exploration to prove reachability.
    /// This is decidable; one of these must terminate.
    /// This is an operation of non-elementary complexity (Ackermann-complete).
    pub fn reach(&mut self, target: &NormalMarking) -> Option<Box<[Transition]>> {
        // Use the provided reachability graph to search for the target marking.
        // If found, reconstruct the firing sequence from m0 to that marking.

        // Compute solutions in the rational numbers to the marking equation M = M0 + N * X
        // where N is the incidence matrix and X is a vector of transition counts.
        // If no such solution exists, return None.
        // If some finite set of solutions S exists, we need to check if any of them are realizable.
        // We can do this by exploring the reachability graph in parallel, and searching for
        // the target marking using the solutions in S as a heuristic to guide the search.
        // Specifically, we can use the solutions to eliminate paths in the reachability graph
        // that cannot lead to the target marking because they would require more firings of
        // certain transitions than allowed by any solution in S.
        // TODO: What to do if there are infinitely many solutions? Can we find a finite representation?
        //  There will frequently be infinitely many solutions because adding any T-invariant will
        //  yield another solution. Can we find a finite basis of solutions?
        // Meanwhile, we can also try to prove that the marking is not reachable by
        // finding a semilinear set such that
        // 1. it contains the initial marking M0
        // 2. it is closed under the firing rule
        // 3. it does not contain the target marking M
        // If we find a firing sequence to the target marking, we return it.
        // If we find a semilinear set that proves the target marking is not reachable,
        // we return None.
        // TODO: Return a Result<Vec<Transition>, enum {
        todo!()
    }
    /// This method tries to find a firing sequence which covers the provided marking from m0, if one exists.
    /// Karp-Miller tree, backward reachability graph algorithm, Rackoff's theorem
    /// This is EXPSPACE-complete.
    pub fn cover(&mut self, target: &NormalMarking) -> Option<Box<[Transition]>> {
        // algorithm::coverability_graph(self.net, &mut self.findings.coverability_graph);
        // Use the provided coverability graph to search for a node that covers the target marking.
        // If found, reconstruct the firing sequence from m0 to that node.
        // If not found, continue expanding the coverability graph until either
        // a covering node is found or the graph is fully explored.
        // If the graph is fully explored without finding a covering node, return None.
        todo!("find a firing sequence which covers the provided marking from m0, if one exists")
    }
}

impl SNet {
    #[must_use]
    pub fn is_live(&self, initial_marking: &NormalMarking) -> bool {
        self.0.is_strongly_connected() && !initial_marking.is_zero()
    }
}

pub mod state_machine {
    use crate::behavior::model::StateSpace;
    use crate::behavior::{NormalMarking, Omega, OmegaMarking};
    use crate::structure::{Net, Transition};
    use petgraph::graph::NodeIndex;
    use std::hash::Hash;

    pub struct StateSpaceIterator<'a, S, M> {
        net: &'a Net,
        state_space: &'a mut StateSpace<M>,
        firing_transitions: Vec<Transition>,
        current_explore_marking: Option<(NodeIndex, M)>,
        steps: usize,
        strategy: S,
    }

    pub type ReachabilityIterator<'a, S> = StateSpaceIterator<'a, S, NormalMarking>;
    pub type CoverabilityIterator<'a, S> = StateSpaceIterator<'a, S, OmegaMarking>;

    #[diagnostic::on_unimplemented(
        message = "The exploration strategy `{Self}` is not compatible with the marking type `{M}`.",
        label = "exploration strategy must implement `ExplorationStrategy` for the marking type"
    )]
    pub trait ExplorationStrategy<M> {
        fn find_next_unexplored_state<'a>(&self, state_space: &'a mut StateSpace<M>) -> Option<(NodeIndex, &'a M)>;
    }

    /// Retrieves the next state for which we have not yet explored successors,
    /// following breadth-first search order.
    /// This method removes the state from the frontier, and it is expected that
    /// the caller will explore its successors and add them to the frontier as needed.
    #[derive(Debug, Clone, Copy)]
    pub struct BreadthFirst;

    impl<M> ExplorationStrategy<M> for BreadthFirst {
        fn find_next_unexplored_state<'a>(&self, coverability_graph: &'a mut StateSpace<M>) -> Option<(NodeIndex, &'a M)> {
            coverability_graph.frontier.pop_front().map(|idx| {
                (idx, &coverability_graph.graph[idx])
            })
        }
    }

    /// Retrieves the next state for which we have not yet explored successors,
    /// following depth-first search order.
    /// This method removes the state from the frontier, and it is expected that
    /// the caller will explore its successors and add them to the frontier as needed.
    #[derive(Debug, Clone, Copy)]
    pub struct DepthFirst;

    impl<M> ExplorationStrategy<M> for DepthFirst {
        fn find_next_unexplored_state<'a>(&self, coverability_graph: &'a mut StateSpace<M>) -> Option<(NodeIndex, &'a M)> {
            coverability_graph.frontier.pop_back().map(|idx| {
                (idx, &coverability_graph.graph[idx])
            })
        }
    }

    pub struct NoStrategy;

    impl<'a, M> StateSpaceIterator<'a, NoStrategy, M> {
        pub fn new(net: &'a Net, state_space: &'a mut StateSpace<M>) -> Self {
            Self {
                net,
                state_space,
                firing_transitions: Vec::new(),
                current_explore_marking: None,
                steps: 0,
                strategy: NoStrategy,
            }
        }
    }

    impl<'a, S, M> StateSpaceIterator<'a, S, M> {
        #[must_use]
        pub fn bfs(self) -> StateSpaceIterator<'a, BreadthFirst, M> {
            StateSpaceIterator {
                net: self.net,
                state_space: self.state_space,
                firing_transitions: self.firing_transitions,
                current_explore_marking: self.current_explore_marking,
                steps: self.steps,
                strategy: BreadthFirst,
            }
        }
        #[must_use]
        pub fn dfs(self) -> StateSpaceIterator<'a, DepthFirst, M> {
            StateSpaceIterator {
                net: self.net,
                state_space: self.state_space,
                firing_transitions: self.firing_transitions,
                current_explore_marking: self.current_explore_marking,
                steps: self.steps,
                strategy: DepthFirst,
            }
        }
        #[must_use]
        pub fn with_strategy<S2>(self, strategy: S2) -> StateSpaceIterator<'a, S2, M> {
            StateSpaceIterator {
                net: self.net,
                state_space: self.state_space,
                firing_transitions: self.firing_transitions,
                current_explore_marking: self.current_explore_marking,
                steps: self.steps,
                strategy,
            }
        }
    }

    impl<S: ExplorationStrategy<OmegaMarking>> Iterator for CoverabilityIterator<'_, S> {
        type Item = (OmegaMarking, Transition, OmegaMarking);

        fn next(&mut self) -> Option<Self::Item> {
            if self.firing_transitions.is_empty() {
                let next = self.strategy.find_next_unexplored_state(self.state_space);
                if let Some((idx, marking_to_explore)) = next {
                    let enabled_transitions = self.net.enabled_transitions(marking_to_explore);
                    self.firing_transitions.extend(enabled_transitions);
                    self.current_explore_marking = Some((idx, marking_to_explore.clone()));
                }
            }
            self.firing_transitions.pop().map(|firing_transition| {
                let (source_idx, current_marking) = self.current_explore_marking.clone().expect("current marking should be set if there are firing transitions");
                let column = self.net.incidence_matrix.column(firing_transition);
                let transition_effect = column.as_slice();

                let mut result_marking = (current_marking.clone() + transition_effect).expect("transition was checked to be enabled");
                for (seen_marking, seen_idx) in &self.state_space.seen_nodes {
                    if seen_marking < &result_marking && self.state_space.path_exists(*seen_idx, source_idx) {
                        for (res, prev) in Iterator::zip(result_marking.iter_mut(), seen_marking.iter()) {
                            if &*res > prev {
                                *res = Omega::Omega;
                            }
                        }
                    }
                }
                let result_marking = result_marking;
                self.state_space.register_marking(source_idx, firing_transition, result_marking.clone());
                self.steps = self.steps.saturating_add(1);
                (current_marking, firing_transition, result_marking)
            })
        }
    }

    impl<S: ExplorationStrategy<NormalMarking>> Iterator for ReachabilityIterator<'_, S> {
        type Item = (NormalMarking, Transition, NormalMarking);

        fn next(&mut self) -> Option<Self::Item> {
            if self.firing_transitions.is_empty() {
                let next = self.strategy.find_next_unexplored_state(self.state_space);
                if let Some((idx, marking_to_explore)) = next {
                    let enabled_transitions = self.net.enabled_transitions(marking_to_explore);
                    self.firing_transitions.extend(enabled_transitions);
                    self.current_explore_marking = Some((idx, marking_to_explore.clone()));
                }
            }
            self.firing_transitions.pop().map(|firing_transition| {
                let (source_idx, current_marking) = self.current_explore_marking.clone().expect("current marking should be set if there are firing transitions");
                let column = self.net.incidence_matrix.column(firing_transition);
                let transition_effect = column.as_slice();

                let result_marking = (current_marking.clone() + transition_effect).expect("transition was checked to be enabled");
                self.state_space.register_marking(source_idx, firing_transition, result_marking.clone());
                self.steps = self.steps.saturating_add(1);
                (current_marking, firing_transition, result_marking)
            })
        }
    }
}

pub mod algorithm {
    use crate::behavior::marking::Omega;
    use crate::behavior::model::CoverabilityGraph;
    use crate::behavior::{Boundedness, NormalMarking, PetriNet, ReachabilityGraph, Tokens};
    use crate::structure::{Net, Place, Transition};

    pub(crate) fn transition_liveness(
        net: &PetriNet<'_>,
        reachability: &mut ReachabilityGraph,
        liveness: &mut Option<super::Liveness>,
        transition: Transition,
    ) {
        todo!("analyze the net to determine the liveness of the specific transition")
    }

    pub(crate) fn liveness(
        net: &PetriNet<'_>,
        reachability: &mut ReachabilityGraph,
        liveness: &mut [Option<super::Liveness>],
    ) {
        todo!("analyze the net to determine the liveness of all transitions")
    }

    pub(crate) fn deadlock_freedom(
        net: &PetriNet<'_>,
        reachability: &mut ReachabilityGraph,
        deadlock_free: &mut Option<bool>,
    ) {
        todo!("analyze the net to determine if it is deadlock-free")
    }

    pub(crate) fn place_boundedness(
        net: &PetriNet<'_>,
        coverability: &mut CoverabilityGraph,
        boundedness: &mut Option<Boundedness>,
        place: Place,
    ) {
        todo!("analyze the net to determine the boundedness of the specific place")
    }

    pub(crate) fn boundedness(
        net: &Net,
        m0: &NormalMarking,
        coverability: &mut CoverabilityGraph,
        boundedness: &mut [Option<Boundedness>],
    ) {
        // coverability_graph(net, coverability);
        
        // Analyze the coverability graph to determine boundedness
        for (place_idx, boundedness_result) in boundedness.iter_mut().enumerate() {
            let mut is_unbounded = false;
            let mut max_tokens = Tokens::default();
            
            for (omega_marking, idx) in &coverability.seen_nodes {
                if place_idx < omega_marking.len() {
                    let omega_token = &omega_marking[Place{index: place_idx as u8}];
                    match omega_token {
                        Omega::Finite(token_count) => {
                            // This place has a finite bound
                            if *token_count > max_tokens {
                                max_tokens = *token_count;
                            }
                        }
                        Omega::Omega => {
                            // This place has omega, so it's unbounded
                            is_unbounded = true;
                            break;
                        }
                    }
                }
            }
            
            if is_unbounded {
                *boundedness_result = Some(Boundedness::Unbounded);
            } else {
                *boundedness_result = Some(Boundedness::Bounded(max_tokens));
            }
        }
    }
}
