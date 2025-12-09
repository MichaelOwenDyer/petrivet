mod marking;

pub use crate::behavior::marking::{Marking, NormalMarking, Omega, OmegaMarking, Tokens};
pub use crate::behavior::model::CoverabilityGraph;
use crate::behavior::model::{StateSpaceEntry, ReachabilityGraph, StateSpace};
use crate::behavior::state_machine::NoStrategy;
use crate::structure::{Net, Place, Transition};
use std::fmt::Debug;
use std::ops::{Add, Index};

pub mod model {
    use crate::behavior::marking::{NormalMarking, OmegaMarking};
    use crate::structure::{Net, Transition};
    use ahash::{HashMap, HashMapExt, HashSet};
    use petgraph::graph::NodeIndex;
    use petgraph::Graph;
    use std::collections::VecDeque;
    use std::fmt::Debug;
    use std::hash::Hash;
    use num_traits::Zero;
    use crate::behavior::Marking;

    /// An entry in the state space graph,
    /// representing a marking and its depth in the exploration tree,
    /// e.g. the number of transitions fired from the initial marking to reach it.
    #[derive(Debug, Clone)]
    pub struct StateSpaceEntry<M> {
        pub marking: M,
        pub depth: u32,
    }

    /// An exploration of the state space of a Petri net.
    #[derive(Debug, Clone)]
    pub struct StateSpace<'net, M> {
        /// The net being analyzed.
        pub net: &'net Net,

        /// The graph representing the state space.
        /// Nodes are markings with their depth in the exploration tree.
        /// Edges are transitions fired to reach new markings.
        pub graph: Graph<StateSpaceEntry<M>, Transition>,

        // todo: these two attributes only need to exist during exploration.
        //  once the state space is fully explored, they can be discarded to save memory.
        /// A set of already seen markings, mapping each marking to its node index in the graph.
        /// This allows for quick lookup to avoid adding duplicate nodes.
        pub seen_nodes: HashMap<M, NodeIndex>,

        /// The frontier of unexplored states.
        /// Each entry is a (`NodeIndex`, `Transition`) pair,
        /// where the `NodeIndex` is index of the source marking in the graph,
        /// and the `Transition` is the transition to be fired from that marking.
        /// Once this is empty, the state space has been fully explored.
        pub frontier: VecDeque<(NodeIndex, Transition)>,
    }

    pub type ReachabilityGraph<'net> = StateSpace<'net, NormalMarking>;
    pub type CoverabilityGraph<'net> = StateSpace<'net, OmegaMarking>;


    impl<'net, T> StateSpace<'net, Marking<T>> where T: Clone + Eq + Hash + Zero + PartialOrd {
        #[must_use]
        pub fn new<R>(net: &'net Net, m0: R) -> Self
            where
                R: Into<Marking<T>>,
        {
            let mut graph = Graph::new();
            let mut seen_nodes = HashMap::new();
            let mut frontier = VecDeque::new();
            let initial_state: Marking<T> = m0.into();
            let idx = graph.add_node(StateSpaceEntry {
                marking: initial_state.clone(),
                depth: 0
            });
            initial_state
                .support()
                .flat_map(|p| net.postset_p(p))
                .collect::<HashSet<_>>()
                .into_iter()
                .for_each(|t| {
                    frontier.push_back((idx, t));
                });
            seen_nodes.insert(initial_state, idx);
            StateSpace { net, graph, seen_nodes, frontier }
        }

        /// Registers a new marking in the state space.
        /// This is a lookup and insertion operation.
        /// If the marking has been seen before, it simply adds an edge from `source_idx` to that marking.
        /// If the marking is new, it adds it to the graph, the `seen_nodes` map, and the frontier,
        /// then adds the edge.
        /// Returns the `NodeIndex` of the registered marking.
        pub fn register_marking(
            &mut self,
            from: NodeIndex,
            over: Transition,
            to: Marking<T>,
        ) -> NodeIndex {
            let to = if let Some(&idx) = self.seen_nodes.get(&to) {
                self.graph[idx].depth = self.graph[idx].depth.min(self.graph[from].depth);
                idx
            } else {
                let idx = self.graph.add_node(StateSpaceEntry {
                    marking: to.clone(),
                    depth: self.graph[from].depth + 1,
                });
                self.seen_nodes.insert(to, idx);
                self.net
                    .postset_t(over)
                    .flat_map(|p| self.net.postset_p(p))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .for_each(|t| {
                        self.frontier.push_back((idx, t));
                    });
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

        #[must_use]
        pub fn is_fully_explored(&self) -> bool {
            self.frontier.is_empty()
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

#[derive(Debug, Clone)]
pub enum StateSpaces<'net> {
    /// The net is (as far as we know) bounded.
    /// The reachability graph and coverability graph are equivalent.
    Bounded {
        graph: StateSpace<'net, NormalMarking>,
    },
    /// The net is unbounded.
    /// The coverability graph contains omega markings.
    /// The reachability graph is finite but incomplete.
    Unbounded {
        coverability_graph: StateSpace<'net, OmegaMarking>,
        reachability_graph: StateSpace<'net, NormalMarking>,
    }
}

impl<'net> StateSpaces<'net> {
    #[must_use]
    pub fn new(net: &'net Net, initial_marking: Marking<Tokens>) -> Self {
        StateSpaces::Bounded {
            graph: ReachabilityGraph::new(net, initial_marking),
        }
    }
}

/// Cumulative analysis results for a Petri net.
/// Meant to be computed once and then queried multiple times.
#[derive(Debug, Clone)]
pub struct Findings<'net> {
    pub graphs: StateSpaces<'net>,
    pub reachability_graph: ReachabilityGraph<'net>,
    pub coverability_graph: CoverabilityGraph<'net>,
    /// Highest known liveness classification for each transition.
    /// The classes L0, L1, (L2?), and L3 can be confirmed during coverability graph exploration.
    /// L4, or strict liveness, is reducible to reachability, which is much harder.
    /// L0 is assumed until proven otherwise.
    /// Only once the coverability graph is fully explored can L0 transitions be confirmed.
    pub liveness: Box<[Liveness]>,
    /// Boundedness of each place, to our current knowledge according to the coverability graph.
    /// The boundedness of the initial marking is assumed until proven otherwise.
    pub boundedness: Box<[Omega<Tokens>]>,
    pub deadlock_free: Option<bool>,
}

impl<'net> Findings<'net> {
    #[must_use]
    pub fn new(net: &'net Net, m0: NormalMarking) -> Self {
        Self {
            graphs: StateSpaces::new(net, m0.clone()),
            reachability_graph: ReachabilityGraph::new(net, m0.clone()),
            coverability_graph: CoverabilityGraph::new(net, m0.clone()),
            liveness: vec![Liveness::L0; net.n_transitions()].into_boxed_slice(),
            boundedness: m0.into_iter().map(Omega::Finite).collect(),
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
    findings: Findings<'net>,
}

impl<'net> From<(&'net Net, NormalMarking)> for PetriNet<'net> {
    fn from((net, m0): (&'net Net, NormalMarking)) -> Self {
        let findings = Findings::new(net, m0.clone());
        Self { net, m0, findings }
    }
}

impl<'net> PetriNet<'net> {
    pub fn coverability_iter(&'net mut self) -> state_machine::StateSpaceIterator<'net, NoStrategy, OmegaMarking> {
        state_machine::StateSpaceIterator::new(
            self.net,
            &mut self.findings.coverability_graph
        )
    }
    pub fn reachability_iter(&'net mut self) -> state_machine::StateSpaceIterator<'net, NoStrategy, NormalMarking> {
        state_machine::StateSpaceIterator::new(
            self.net,
            &mut self.findings.reachability_graph,
        )
    }
    /// Returns the boundedness of the net as per the current state of the coverability graph.
    /// If the coverability graph is not fully explored, this is an under-approximation;
    /// the real boundedness of some places may be higher than reported by this method,
    /// but never lower.
    pub fn boundedness(&mut self) -> OmegaMarking {
        self.findings.coverability_graph.graph.node_weights().fold(
            OmegaMarking::zeroes(self.net.n_places()),
            |mut acc, StateSpaceEntry { marking: next, depth: _depth }| {
                acc.ceil(next);
                acc
            }
        )
    }
    /// Returns the boundedness of a specific place as per the current state of the coverability graph.
    /// If the coverability graph is not fully explored, this is an under-approximation;
    /// the real boundedness of the place may be higher than reported by this method,
    /// but never lower.
    pub fn place_boundedness(&mut self, place: Place) -> Option<Omega<Tokens>> {
        self.findings.coverability_graph.graph
            .node_weights()
            .map(|StateSpaceEntry { marking, depth: _depth }| marking[place])
            .max()
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
    #[must_use]
    pub fn is_l0(&self, transition: Transition) -> bool {
        !self.findings.coverability_graph.graph.edge_weights().any(|&t| t == transition)
    }
    #[must_use]
    pub fn is_strictly_l0(&self, transition: Transition) -> bool {
        self.is_l0(transition) && !self.is_l1(transition)
    }
    #[must_use]
    pub fn is_l1(&self, transition: Transition) -> bool {
        self.findings.coverability_graph.graph.edge_weights().any(|&t| t == transition)
    }
    #[must_use]
    pub fn is_strictly_l1(&self, transition: Transition) -> bool {
        self.is_l1(transition) && !self.is_l2(transition)
    }
    #[must_use]
    pub fn is_l2(&self, transition: Transition) -> bool {
        todo!("determine if the transition is L2-live")
        // explore the reachability graph to see if from any reachable marking
        // we can reach another marking that enables the transition
    }
    #[must_use]
    pub fn is_strictly_l2(&self, transition: Transition) -> bool {
        self.is_l2(transition) && !self.is_l3(transition)
    }
    #[must_use]
    pub fn is_l3(&self, transition: Transition) -> bool {
        todo!("determine if the transition is L3-live")
        // check: is there any SCC in the coverability graph that contains the transition
        // that would imply that there is a firing sequence which enables the transition infinitely often
    }
    #[must_use]
    pub fn is_strictly_l3(&self, transition: Transition) -> bool {
        self.is_l3(transition) && !self.is_l4(transition)
    }
    #[must_use]
    pub fn is_l4(&self, transition: Transition) -> bool {
        todo!("determine if the transition is L4-live")
        // check: does every bottom SCC of the coverability graph contain the transition
        // that would imply that every infinite firing sequence visits the transition infinitely often
    }
}

pub mod state_machine {
    use crate::behavior::model::StateSpace;
    use crate::behavior::{NormalMarking, Omega, OmegaMarking, StateSpaces};
    use crate::structure::{Net, Transition};
    use petgraph::graph::NodeIndex;
    use std::hash::Hash;
    use petgraph::Graph;

    pub struct StateSpaceIterator<'net, S, M> {
        net: &'net Net,
        state_space: &'net mut StateSpace<'net, M>,
        steps: usize,
        strategy: S,
    }

    pub type ReachabilityIterator<'net, Strategy> = StateSpaceIterator<'net, Strategy, NormalMarking>;
    pub type CoverabilityIterator<'net, Strategy> = StateSpaceIterator<'net, Strategy, OmegaMarking>;

    #[diagnostic::on_unimplemented(
        message = "The exploration strategy `{Self}` is not compatible with the marking type `{M}`.",
        label = "exploration strategy must implement `ExplorationStrategy` for the marking type"
    )]
    pub trait ExplorationStrategy<M> {
        fn find_next_unexplored_state(&self, state_space: &'_ mut StateSpace<M>) -> Option<(NodeIndex, Transition)>;
    }

    /// Retrieves the next state for which we have not yet explored successors,
    /// following breadth-first search order.
    /// This method removes the state from the frontier, and it is expected that
    /// the caller will explore its successors and add them to the frontier as needed.
    #[derive(Debug, Clone, Copy)]
    pub struct BreadthFirst;

    impl<M> ExplorationStrategy<M> for BreadthFirst {
        fn find_next_unexplored_state(&self, state_space: &'_ mut StateSpace<M>) -> Option<(NodeIndex, Transition)> {
            state_space.frontier.pop_front()
        }
    }

    /// Retrieves the next state for which we have not yet explored successors,
    /// following depth-first search order.
    /// This method removes the state from the frontier, and it is expected that
    /// the caller will explore its successors and add them to the frontier as needed.
    #[derive(Debug, Clone, Copy)]
    pub struct DepthFirst;

    impl<M> ExplorationStrategy<M> for DepthFirst {
        fn find_next_unexplored_state(&self, state_space: &'_ mut StateSpace<M>) -> Option<(NodeIndex, Transition)> {
            state_space.frontier.pop_back()
        }
    }

    pub struct NoStrategy;

    impl<'net, M> StateSpaceIterator<'net, NoStrategy, M> {
        pub fn new(net: &'net Net, state_space: &'net mut StateSpace<'net, M>) -> Self {
            Self {
                net,
                state_space,
                steps: 0,
                strategy: NoStrategy,
            }
        }
    }

    impl<'net, S, M> StateSpaceIterator<'net, S, M> {
        #[must_use]
        pub fn bfs(self) -> StateSpaceIterator<'net, BreadthFirst, M> {
            StateSpaceIterator {
                net: self.net,
                state_space: self.state_space,
                steps: self.steps,
                strategy: BreadthFirst,
            }
        }
        #[must_use]
        pub fn dfs(self) -> StateSpaceIterator<'net, DepthFirst, M> {
            StateSpaceIterator {
                net: self.net,
                state_space: self.state_space,
                steps: self.steps,
                strategy: DepthFirst,
            }
        }
        #[must_use]
        pub fn with_strategy<Strategy>(self, strategy: Strategy) -> StateSpaceIterator<'net, Strategy, M> {
            StateSpaceIterator {
                net: self.net,
                state_space: self.state_space,
                steps: self.steps,
                strategy,
            }
        }
    }

    impl<S> Iterator for CoverabilityIterator<'_, S>
    where
        S: ExplorationStrategy<OmegaMarking>
    {
        type Item = (OmegaMarking, Transition, OmegaMarking);

        fn next(&mut self) -> Option<Self::Item> {
            while let Some((source_marking_idx, transition)) = self.strategy.find_next_unexplored_state(self.state_space) {
                let source_marking = self.state_space.graph[source_marking_idx].clone().marking;
                if let Some(result) = source_marking.clone()
                    .try_add(self.net.incidence_marking(transition))
                    .ok()
                    .map(|mut result_marking| {
                        // todo: refactor this into an add_omegas function
                        for (seen_marking, seen_idx) in &self.state_space.seen_nodes {
                            if seen_marking < &result_marking && self.state_space.path_exists(*seen_idx, source_marking_idx) {
                                for (res, prev) in Iterator::zip(result_marking.iter_mut(), seen_marking.iter()) {
                                    if &*res > prev {
                                        *res = Omega::Omega;
                                    }
                                }
                            }
                        }
                        self.state_space.register_marking(source_marking_idx, transition, result_marking.clone());
                        self.steps = self.steps.saturating_add(1);
                        (source_marking, transition, result_marking)
                    }) {
                    return Some(result);
                }
            }
            None
        }
    }

    impl<S> Iterator for ReachabilityIterator<'_, S>
    where
        S: ExplorationStrategy<NormalMarking>
    {
        type Item = (NormalMarking, Transition, NormalMarking);

        fn next(&mut self) -> Option<Self::Item> {
            while let Some((source_marking_idx, transition)) = self.strategy.find_next_unexplored_state(self.state_space) {
                let source_marking = self.state_space.graph[source_marking_idx].clone().marking;
                if source_marking >= *self.net.input_marking(transition)
                    && let Ok(result_marking) = source_marking
                        .clone()
                        .try_add(self.net.incidence_marking(transition))
                {
                    self.state_space.register_marking(source_marking_idx, transition, result_marking.clone());
                    self.steps = self.steps.saturating_add(1);
                    return Some((source_marking, transition, result_marking));
                }
            }
            None
        }
    }
}
