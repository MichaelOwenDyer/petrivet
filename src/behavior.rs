use crate::behavior::model::{CoverabilityGraph, Omega};
use crate::structure::{Net, NetIndex, Place, SNet, Transition};
use ahash::HashSetExt;
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::VecDeque;
use std::hash::Hash;
use std::ops::{Add, Sub};

/// Any type which can be used as a token count in a petri net.
pub trait TokenTrait: Add + Sub + Sized + Copy + Default + PartialEq + Eq + Ord + Hash {}

impl<T> TokenTrait for T where T: Add + Sub + Sized + Copy + Default + PartialEq + Eq + Ord + Hash {}

/// A simple token count type for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tokens(pub u32);

impl Add for Tokens {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Tokens(self.0 + other.0)
    }
}

impl Sub for Tokens {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Tokens(self.0.saturating_sub(other.0))
    }
}

/// A marking is a vector of token counts, indexed by place ID.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Marking<Token: TokenTrait>(Box<[Token]>);

pub enum TokenDelta {
    Add = 1,
    None = 0,
    Sub = -1,
}

impl<Token: TokenTrait> Add<&[TokenDelta]> for Marking<Token> {
    type Output = Result<Self, &'static str>;

    fn add(self, rhs: &[TokenDelta]) -> Self::Output {
        if self.len() != rhs.len() {
            return Err("Length mismatch between marking and delta");
        }
        
        let result: Result<Vec<Token>, _> = Iterator::zip(self.iter(), rhs.iter())
            .map(|(a, &b)| {
                match b {
                    TokenDelta::Add => Ok(*a + Token::default() + Token::default()), // Add 1
                    TokenDelta::None => Ok(*a),
                    TokenDelta::Sub => {
                        if *a > Token::default() {
                            Ok(*a - (Token::default() + Token::default())) // Subtract 1
                        } else {
                            Err("Cannot subtract from zero tokens")
                        }
                    }
                }
            })
            .collect();
            
        result.map(|tokens| Marking::new(tokens))
    }
}

impl<Token: TokenTrait> Marking<Token> {
    /// Creates a new empty marking with the given capacity (number of places).
    #[must_use]
    pub fn with_places(capacity: usize) -> Self {
        Self(vec![Token::default(); capacity].into_boxed_slice())
    }
    
    /// Creates a new marking from a vector of tokens.
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self(tokens.into_boxed_slice())
    }

    /// Returns the number of places in the marking.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Sets the number of tokens in a place.
    /// Panics if the place ID is out of bounds.
    pub fn set<Index: NetIndex>(&mut self, place: Place<Index>, tokens: Token) {
        let index = place.index.into();
        self.0[index] = tokens;
    }

    pub fn get<Index: NetIndex>(&self, place: Place<Index>) -> Token {
        let index = place.index.into();
        self.0.get(index).copied().unwrap_or_default()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Token> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Token> {
        self.0.iter_mut()
    }

    /// Returns true if the marking is zero (all places have zero tokens).
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|t| *t == Token::default())
    }
}

// Add the same methods for OmegaMarking
impl<Token: TokenTrait> Marking<Omega<Token>> {
    pub fn get<Index: NetIndex>(&self, place: Place<Index>) -> Omega<Token> {
        let index = place.index.into();
        self.0.get(index).copied().unwrap_or_default()
    }
}

impl<T: TokenTrait> PartialOrd for Marking<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Enables comparison of markings based on the covering relation.
/// ```
/// use petrivet::structure::Place;
/// use petrivet::behavior::{Marking, Tokens};
/// let mut m1 = Marking::<Tokens>::with_places(2);
/// m1.set(Place{index:0}, Tokens(1));
/// m1.set(Place{index:1}, Tokens(2));
/// let mut m2 = Marking::<Tokens>::with_places(2);
/// m2.set(Place{index:0}, Tokens(1));
/// m2.set(Place{index:1}, Tokens(3));
/// assert!(m2 > m1);
/// assert!(m1 < m2);
/// assert!(m1 != m2);
/// ```
impl<T: TokenTrait> Ord for Marking<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        for (a, b) in Iterator::zip(self.iter(), other.iter()) {
            match a.cmp(b) {
                std::cmp::Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }
        std::cmp::Ordering::Equal
    }
}

mod model {
    use crate::behavior::{Marking, OmegaMarking, TokenTrait};
    use crate::structure::{NetIndex, Transition};
    use ahash::HashSetExt;
    use petgraph::graph::NodeIndex;
    use petgraph::Graph;
    use std::collections::VecDeque;

    /// Omega represents either a specific number of tokens (Finite)
    /// or an unbounded number of tokens (Omega).
    /// This is used in the coverability graph to represent places that can grow without bound.
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub enum Omega<Token: TokenTrait> {
        Finite(Token),
        Omega,
    }

    /// Allow easy conversion from a Token count to an Omega.
    impl<T: TokenTrait> From<T> for Omega<T> {
        fn from(tc: T) -> Self {
            Omega::Finite(tc)
        }
    }

    impl<Token: TokenTrait> Omega<Token> {
        pub fn is_omega(&self) -> bool {
            matches!(self, Omega::Omega)
        }
        pub fn count(&self) -> Option<Token> {
            match self {
                Omega::Finite(t) => Some(*t),
                Omega::Omega => None,
            }
        }
    }

    /// Default Omega is Finite(0).
    impl<T: TokenTrait> Default for Omega<T> {
        fn default() -> Self {
            Omega::Finite(T::default())
        }
    }

    /// Enables comparison of Omega values.
    /// Omega is greater than any Finite value.
    /// Finite values are compared normally.
    /// ```rust
    /// use petrivet::behavior::model::Omega;
    /// use petrivet::structure::builder::NetBuilder;
    /// use petrivet::structure::Place;
    /// use petrivet::behavior::Marking;
    /// use petrivet::behavior::Tokens;
    /// let o1 = Omega::Finite(Tokens(5));
    /// let o2 = Omega::Finite(Tokens(10));
    /// let o3 = Omega::Omega;
    /// assert!(o1 < o2);
    /// assert!(o2 < o3);
    /// assert!(o1 < o3);
    /// ```
    impl<Token: TokenTrait> PartialOrd<Self> for Omega<Token> {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    /// Allows comparison of Omega with Token.
    impl<Token: TokenTrait> PartialEq<Token> for Omega<Token> {
        fn eq(&self, other: &Token) -> bool {
            match self {
                Omega::Finite(t) => t == other,
                Omega::Omega => false,
            }
        }
    }

    /// Allows comparison of Omega with Token.
    /// Omega is greater than any Finite value.
    impl<Token: TokenTrait> PartialOrd<Token> for Omega<Token> {
        fn partial_cmp(&self, other: &Token) -> Option<std::cmp::Ordering> {
            match self {
                Omega::Finite(t) => Some(t.cmp(other)),
                Omega::Omega => Some(std::cmp::Ordering::Greater),
            }
        }
    }

    /// Implements total ordering for Omega.
    /// Omega is greater than any Finite value.
    impl<Token: TokenTrait> Ord for Omega<Token> {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
                (Omega::Omega, Omega::Omega) => std::cmp::Ordering::Equal,
                (Omega::Omega, _) => std::cmp::Ordering::Greater,
                (_, Omega::Omega) => std::cmp::Ordering::Less,
                (Omega::Finite(a), Omega::Finite(b)) => a.cmp(b),
            }
        }
    }

    /// The main struct holding the state of the coverability graph construction.
    #[derive(Debug, Clone)]
    pub struct CoverabilityGraph<Index: NetIndex, Token: TokenTrait> {
        /// The graph structure (V, E). Node weights are Markings, edge weights are Transitions.
        pub graph: Graph<OmegaMarking<Token>, Transition<Index>>,

        /// A fast lookup set to check if an *exact* marking has been seen before.
        pub seen_nodes: ahash::HashSet<OmegaMarking<Token>>,

        /// The worklist from the algorithm. We store the `NodeIndex` to locate the markings
        /// in the graph where we need to explore successors.
        /// Use this as a queue (FIFO) for breadth-first search,
        /// or as a stack (LIFO) for depth-first search.
        pub frontier: VecDeque<NodeIndex>,
    }

    impl<Index: NetIndex, Token: TokenTrait> CoverabilityGraph<Index, Token> {
        pub fn new(m0: &Marking<Token>) -> CoverabilityGraph<Index, Token> {
            let mut graph = Graph::new();
            let mut seen_nodes = ahash::HashSet::new();
            let mut frontier = VecDeque::new();
            let omega_marking = OmegaMarking::from(m0.clone());
            let idx = graph.add_node(omega_marking.clone());
            seen_nodes.insert(omega_marking);
            frontier.push_back(idx);
            CoverabilityGraph { graph, seen_nodes, frontier }
        }
    }
}

type OmegaMarking<T> = Marking<Omega<T>>;

impl<T: TokenTrait> From<Marking<T>> for Marking<Omega<T>> {
    fn from(marking: Marking<T>) -> Self {
        Marking::new(
            marking
                .0
                .into_iter()
                .map(Omega::from)
                .collect(),
        )
    }
}

impl<T: TokenTrait> From<Vec<T>> for Marking<T> {
    fn from(tokens: Vec<T>) -> Self {
        Marking::new(tokens)
    }
}

#[derive(Debug, Clone)]
pub struct ReachabilityGraph<Index: NetIndex, Token: TokenTrait> {
    pub graph: Graph<Marking<Token>, Transition<Index>>,
    pub seen_nodes: ahash::HashSet<Marking<Token>>,
    pub worklist: VecDeque<NodeIndex>,
}

impl<Index: NetIndex, Token: TokenTrait> ReachabilityGraph<Index, Token> {
    pub fn new(m0: &Marking<Token>) -> ReachabilityGraph<Index, Token> {
        let mut graph = Graph::new();
        let mut seen_nodes = ahash::HashSet::new();
        let mut worklist = VecDeque::new();
        let marking = m0.clone();
        let id = graph.add_node(marking.clone());
        seen_nodes.insert(marking);
        worklist.push_back(id);
        ReachabilityGraph { graph, seen_nodes, worklist }
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
pub struct Findings<Index: NetIndex, Token: TokenTrait> {
    pub reachability: ReachabilityGraph<Index, Token>,
    pub coverability: CoverabilityGraph<Index, Token>,
    pub liveness: Box<[Option<Liveness>]>,
    pub boundedness: Box<[Option<Boundedness<Token>>]>,
    pub deadlock_free: Option<bool>,
}

impl<Index: NetIndex, Token: TokenTrait> Findings<Index, Token> {
    pub fn new(net: &Net<Index>, m0: &Marking<Token>) -> Self {
        Self {
            reachability: ReachabilityGraph::new(m0),
            coverability: CoverabilityGraph::new(m0),
            liveness: vec![None; net.n_transitions().into()].into_boxed_slice(),
            boundedness: vec![None; net.n_places().into()].into_boxed_slice(),
            deadlock_free: None,
        }
    }
}

/// (N, M0)
/// A Petri net structure with an initial marking.
#[derive(Debug, Clone)]
pub struct PetriNet<'net, Index: NetIndex, Token: TokenTrait> {
    net: &'net Net<Index>,
    m0: Marking<Token>,
    findings: Findings<Index, Token>,
}

impl<'net, Index: NetIndex, Token: TokenTrait> From<(&'net Net<Index>, Marking<Token>)> for PetriNet<'net, Index, Token> {
    fn from((net, m0): (&'net Net<Index>, Marking<Token>)) -> Self {
        let findings = Findings::new(net, &m0);
        Self { net, m0, findings }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundedness<Token: TokenTrait> {
    Bounded(Token),
    Unbounded,
}

impl<Token: TokenTrait> Boundedness<Token> {
    pub fn is_unbounded(&self) -> bool {
        matches!(self, Boundedness::Unbounded)
    }
    pub fn is_bounded(&self) -> bool {
        matches!(self, Boundedness::Bounded(_))
    }
    pub fn bound(&self) -> Option<Token> {
        match self {
            Boundedness::Bounded(t) => Some(*t),
            Boundedness::Unbounded => None,
        }
    }
}

impl<Index: NetIndex, Token: TokenTrait> PetriNet<'_, Index, Token> {
    /// Explores the coverability graph until the boundedness of all places has been discovered.
    pub fn boundedness(&mut self) -> &[Option<Boundedness<Token>>] {
        algorithm::boundedness(self.net, &self.m0, &mut self.findings.coverability, &mut self.findings.boundedness);
        &self.findings.boundedness
    }
    pub fn place_boundedness(&mut self, place: Place<Index>) -> Boundedness<Token> {
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
    pub fn check_transition_live(&mut self, transition: Transition<Index>) -> bool {
        todo!("check if transition is live by exploring the reachability graph")
    }
    /// This method tries to find a firing sequence which reaches the provided marking from m0, if one exists.
    /// Leroux (2012) algorithm using Presburger arithmetic to find semilinear sets of reachable markings
    /// to disprove reachability, parallel to reachability graph exploration to prove reachability.
    /// This is decidable; one of these must terminate.
    /// This is an operation of non-elementary complexity (Ackermann-complete).
    pub fn reach(&mut self, target: &Marking<Token>) -> Option<Box<[Transition<Index>]>> {
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
    pub fn cover(&mut self, target: &Marking<Token>) -> Option<Box<[Transition<Index>]>> {
        // Use the provided coverability graph to search for a node that covers the target marking.
        // If found, reconstruct the firing sequence from m0 to that node.
        // If not found, continue expanding the coverability graph until either
        // a covering node is found or the graph is fully explored.
        // If the graph is fully explored without finding a covering node, return None.
        todo!("find a firing sequence which covers the provided marking from m0, if one exists")
    }
}

impl<Index: NetIndex> SNet<Index> {
    pub fn is_live(&self, initial_marking: Marking<impl TokenTrait>) -> bool {
        self.0.is_strongly_connected() && !initial_marking.is_zero()
    }
}

mod algorithm {
    use std::ops::Index;
    use crate::behavior::model::CoverabilityGraph;
    use crate::behavior::{Boundedness, Marking, PetriNet, ReachabilityGraph, TokenTrait};
    use crate::structure::{Net, NetIndex, Place, Transition};

    pub(crate) fn coverability<Index: NetIndex, Token: TokenTrait>(
        net: &Net<Index>,
        coverability: &mut CoverabilityGraph<Index, Token>,
    ) {
        for &next_work in coverability.frontier.iter() {
            let marking = &coverability.graph[next_work];
            let enabled_transitions: Vec<_> = net
                .transitions()
                .filter(|&t| {
                    net.preset_t(t).all(|&p| {
                        let omega_token = marking.get(p);
                        match omega_token {
                            Omega::Finite(count) => count > Token::default(),
                            Omega::Omega => true, // Omega is always "greater than" any finite value
                        }
                    })
                })
                .collect();
            for firing_transition in enabled_transitions {
                // Convert the incidence matrix column to TokenDelta slice
                let delta: Vec<TokenDelta> = net.incidence_matrix.index(firing_transition)
                    .iter()
                    .map(|&val| match val {
                        1 => TokenDelta::Add,
                        0 => TokenDelta::None,
                        -1 => TokenDelta::Sub,
                        _ => TokenDelta::None, // For now, ignore non-standard weights
                    })
                    .collect();
                
                if let Ok(result_marking) = marking.clone() + &delta {
                    // Add the new marking to the graph if not already seen
                    if !coverability.seen_nodes.contains(&result_marking) {
                        let new_idx = coverability.graph.add_node(result_marking.clone());
                        coverability.seen_nodes.insert(result_marking);
                        coverability.frontier.push_back(new_idx);
                        coverability.graph.add_edge(next_work, new_idx, firing_transition);
                    }
                }
            }
        }
    }

    pub(crate) fn reachability<Index: NetIndex, Token: TokenTrait>(
        net: &PetriNet<'_, Index, Token>,
        reachability: &mut ReachabilityGraph<Index, Token>,
    ) {
        todo!("implement the reachability graph construction algorithm")
    }

    pub(crate) fn transition_liveness<Index: NetIndex, Token: TokenTrait>(
        net: &PetriNet<'_, Index, Token>,
        reachability: &mut ReachabilityGraph<Index, Token>,
        liveness: &mut Option<super::Liveness>,
        transition: Transition<Index>,
    ) {
        todo!("analyze the net to determine the liveness of the specific transition")
    }

    pub(crate) fn liveness<Index: NetIndex, Token: TokenTrait>(
        net: &PetriNet<'_, Index, Token>,
        reachability: &mut ReachabilityGraph<Index, Token>,
        liveness: &mut [Option<super::Liveness>],
    ) {
        todo!("analyze the net to determine the liveness of all transitions")
    }

    pub(crate) fn deadlock_freedom<Index: NetIndex, Token: TokenTrait>(
        net: &PetriNet<'_, Index, Token>,
        reachability: &mut ReachabilityGraph<Index, Token>,
        deadlock_free: &mut Option<bool>,
    ) {
        todo!("analyze the net to determine if it is deadlock-free")
    }

    pub(crate) fn place_boundedness<Index: NetIndex, Token: TokenTrait>(
        net: &PetriNet<'_, Index, Token>,
        coverability: &mut CoverabilityGraph<Index, Token>,
        boundedness: &mut Option<Boundedness<Token>>,
        place: Place<Index>,
    ) {
        todo!("analyze the net to determine the boundedness of the specific place")
    }

    pub(crate) fn boundedness<Index: NetIndex, Token: TokenTrait>(
        net: &Net<Index>,
        m0: &Marking<Token>,
        coverability: &mut CoverabilityGraph<Index, Token>,
        boundedness: &mut [Option<Boundedness<Token>>],
    ) {
        self::coverability(net, coverability);
        
        // Analyze the coverability graph to determine boundedness
        for (place_idx, boundedness_result) in boundedness.iter_mut().enumerate() {
            let mut is_unbounded = false;
            let mut max_tokens = Token::default();
            
            for omega_marking in &coverability.seen_nodes {
                if place_idx < omega_marking.len() {
                    let omega_token = &omega_marking.0[place_idx];
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
