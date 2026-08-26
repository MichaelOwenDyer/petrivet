use crate::core::cegar::CegarProperty;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::observe::{CegarObserverFn, IdxCegarEvent};
use crate::core::cegar::refinements::explore::GuidedExplorer;
use crate::core::cegar::refinements::initially_marked_trap::InitiallyMarkedTrapRule;
use crate::core::cegar::refinements::marking_equation::MarkingEquationRefinement;
use crate::core::cegar::refinements::p_invariant::PInvariantRule;
use crate::core::cegar::refinements::transition_ordering::TransitionOrderingRule;
use crate::core::cegar::refinements::trap_becomes_marked::TrapBecomesMarkedRule;
use crate::core::cegar::solver::{Satisfiability, SmtSolver};
use crate::core::marking::IdxMarking;
use crate::core::net::{DenseNet, TransitionIdx};
use crate::core::parikh::IdxParikhVector;

/// CEGAR ("Counterexample-Guided Abstraction Refinement"), was first described by Clarke et al. (2000).
/// It is a general framework for solving verification problems by iteratively refining an
/// abstract model of the system until a solution is found or the problem is proven unsatisfiable.
/// CEGAR can significantly outperform explicit or symbolic state exploration in many cases,
/// especially when the property being verified is *not* satisfied by the system under test.
///
/// In the context of Petri nets, CEGAR can be used to solve reachability and coverability problems.
/// The basic idea is to encode the Petri net and the property being verified as a set of constraints
/// in an SMT solver. At first, the constraints are very imprecise and vastly overapproximate the
/// behavior of the Petri net, so the SMT solver may return a spurious solution which would not be
/// valid in the real system. We then analyze the spurious solution and refine the constraints
/// to eliminate it, and repeat this process until the solver hopefully returns UNSAT.
/// Since the SMT state is at all times a strict overapproximation of the Petri net, an UNSAT
/// result proves that the property is unsatisfiable in the actual Petri net, and we can terminate
/// early.
///
/// The CEGAR algorithm is divided into two phases:
/// 1. The structural phase, where we only consider the place variables and refine the SMT problem
///    using structural properties of the net (P-invariants and traps). We hope to eliminate many
///    spurious solutions or prove unsatisfiability in this phase, since it is very lightweight.
/// 2. The behavioral phase, where we introduce the transition variables into the SMT problem,
///    and the SMT solver begins to reason about their firing counts and the state equation.
///    We continue to check for structural refinement opportunities from the first phase,
///    but we also refine the SMT problem based on logical flaws in its proposed Parikh vectors.
///    If we cannot find any further refinements, we attempt to witness the Parikh vector
///    as a real firing sequence in the Petri net via a guided exploration of the state space.
///    If we successfully reach the candidate marking, we terminate with a positive result;
///    otherwise, we analyze the failed exploration to identify the states where we got the closest
///    to the target, and inform the SMT solver which places were lacking the necessary tokens.
///
/// `Cegar` is generic over the [`SmtSolver`] backend `S` so that the algorithm below is written
/// once and can be driven by any conforming SMT solver (see `crate::core::cegar::solver`).
pub struct Cegar<'a, T, S: SmtSolver> {
    /// The Petri net, initial marking, and target marking for the CEGAR analysis.
    pub problem: CegarProblem<'a>,
    /// The SMT solver used for the analysis.
    pub solver: S,
    /// The current phase of the CEGAR analysis.
    pub context: T,
    /// A sink for CEGAR events, if an observer is provided.
    pub observer: CegarObserver,
}

pub struct CegarProblem<'a> {
    /// The net being analyzed.
    pub net: &'a DenseNet,
    /// The initial marking.
    pub m0: &'a IdxMarking<u32>,
    /// The target marking.
    pub target: &'a IdxMarking<u32>,
}

/// A wrapper around an optional [`CegarObserverFn`], adding the (marking, Parikh vector) context
/// that's common to every event fired during one CEGAR step.
pub struct CegarObserver {
    sink: Option<CegarObserverFn>,
}

impl CegarObserver {
    /// Bind (marking, Parikh vector) context for the current step, returning a callback that
    /// refinement rules can invoke once per [`IdxLemma`] they actually assert - or `None` if no
    /// observer is registered, so callers can skip building lemmas they'd otherwise discard.
    ///
    /// Returns a boxed trait object rather than `impl Fn(IdxLemma)` for the same reason
    /// [`CegarObserverFn`] itself is boxed: refinement rules' `encode_into` methods are already
    /// generic over the `SmtSolver` backend, and accepting this by a second generic parameter
    /// would multiply that per distinct call site instead of erasing it once, here, where it's
    /// cheap (one allocation per CEGAR step, not per lemma).
    pub fn with_context(
        &self,
        marking: Option<IdxMarking<u32>>,
        parikh_vector: Option<IdxParikhVector<u32>>,
    ) -> Option<Box<dyn Fn(IdxLemma) + '_>> {
        self.sink.as_deref().map(|sink| {
            Box::new(move |lemma| {
                sink(IdxCegarEvent {
                    spurious_marking: marking.clone(),
                    spurious_parikh_vector: parikh_vector.clone(),
                    lemma,
                });
            }) as Box<dyn Fn(IdxLemma) + '_>
        })
    }
}

/// Read the model value of every term in `place_terms` into an [`IdxMarking`].
/// Only valid to call after a `Satisfiability::Sat` result.
fn extract_marking<S: SmtSolver>(
    solver: &S,
    place_terms: &[S::Int],
) -> Option<IdxMarking<u32>> {
    place_terms.iter().map(|term| solver.eval_int(term)).collect()
}

/// Read the model value of every term in `transition_terms` into an [`IdxParikhVector`].
/// Only valid to call after a `Satisfiability::Sat` result.
pub fn extract_parikh_vector<S: SmtSolver>(
    solver: &S,
    transition_terms: &[S::Int],
) -> Option<IdxParikhVector<u32>> {
    transition_terms.iter().map(|term| solver.eval_int(term)).collect()
}

/// Encode the target marking constraints into the SMT solver, and return a vector of SMT terms
/// in place index order.
fn encode_place_terms<S: SmtSolver>(
    solver: &mut S,
    problem: &CegarProblem,
    property: CegarProperty,
) -> Vec<S::Int> {
    let place_terms: Vec<S::Int> = problem.net.place_indices()
        .map(|p_idx| solver.mk_int_var(&format!("p{p_idx}")))
        .collect();

    for (place, &target_tokens) in place_terms.iter().zip(problem.target.iter()) {
        let target_term = solver.mk_int(i64::from(target_tokens));
        let domain = match property {
            CegarProperty::Reachability => solver.eq(place, &target_term),
            CegarProperty::Coverability => solver.ge(place, &target_term),
        };
        solver.assert(&domain);
    }

    place_terms
}

impl<'a, S: SmtSolver + Default> Cegar<'a, Structural<S>, S> {
    /// Create a new CEGAR context with the given Petri net, initial marking, target marking,
    /// and operation type.
    pub fn new(
        problem: CegarProblem<'a>,
        property: CegarProperty,
        observer: Option<CegarObserverFn>,
    ) -> Self {
        let mut solver = S::default();
        let places = encode_place_terms(&mut solver, &problem, property);
        let p_invariant_rule = PInvariantRule::new(&problem.net.incidence_matrix);
        let context = Structural { places, p_invariant_rule };
        let observer = CegarObserver { sink: observer };

        Self {
            problem,
            solver,
            context,
            observer,
        }
    }
}

/// The first phase of CEGAR refinement, where the SMT works to find a valid
/// marking assignment for the place variables which satisfies the target marking constraints.
/// We refine the SMT problem using structural properties of the net.
/// During this phase, the SMT problem stays within the space of place variables only,
/// which is very lightweight, and allows us to quickly eliminate many spurious solutions
/// before we move on to the more expensive transition-based phase.
pub struct Structural<S: SmtSolver> {
    /// The SMT variables corresponding to the place markings.
    /// We extract the values of these variables to form a candidate
    /// marking when the SMT solver returns a SAT result.
    pub places: Vec<S::Int>,
    /// The P-invariant rule, instantiated with its own persistent solver state.
    pub p_invariant_rule: PInvariantRule<S>,
}

/// The result of a single CEGAR step during the structural,
/// places-only phase of the algorithm.
pub enum StructuralStep<'a, S: SmtSolver> {
    /// The SMT solver returned a SAT result, and we encoded a
    /// reason why it cannot be a valid solution into the SMT problem.
    /// We can continue with the next step.
    Refined(Cegar<'a, Structural<S>, S>),
    /// The SMT solver returned a SAT result, and we exhausted our
    /// checklist of structural refinements. We now transition to
    /// the next phase of the algorithm: introducing transition
    /// variables into the problem and encoding the state equation.
    Advanced(Cegar<'a, Behavioral<S>, S>),
    /// Success! The SMT solver returned an UNSAT result, which proves that the
    /// target marking is unreachable/uncoverable in the actual Petri net.
    /// Contains a domain-native representation of the UNSAT core from the solver,
    /// describing a set of structural insights about the net which contradict the
    /// target marking constraints.
    Unsat(Vec<IdxLemma>),
}

impl<'a, S: SmtSolver> Cegar<'a, Structural<S>, S> {
    /// Call the SMT solver to check if the current problem is satisfiable.
    /// We hope to receive an UNSAT result, in which case we can terminate
    /// early. In the much more likely case of a SAT result, we go down a
    /// checklist of structural properties looking for
    pub fn step(mut self) -> StructuralStep<'a, S> {
        let candidate_marking = match self.solver.check() {
            Satisfiability::Sat => {
                extract_marking(&self.solver, &self.context.places)
                    .expect("Failed to extract marking from model")
            }
            Satisfiability::Unsat => return StructuralStep::Unsat(self.solver.unsat_core()),
        };
        if let Some(p_invariant_refinement) = self.context.p_invariant_rule.check(&self.problem, &candidate_marking) {
            p_invariant_refinement.encode_into(
                &mut self.solver,
                &self.context.places,
                self.observer.with_context(Some(candidate_marking), None).as_deref()
            );
            return StructuralStep::Refined(self);
        }
        if let Some(trap_refinement) = InitiallyMarkedTrapRule::check(&self.problem, &candidate_marking) {
            trap_refinement.encode_into(
                &mut self.solver,
                &self.context.places,
                self.observer.with_context(Some(candidate_marking), None).as_deref()
            );
            return StructuralStep::Refined(self);
        }
        StructuralStep::Advanced(Cegar::<Behavioral<S>, S>::from(self))
    }
}

impl<'a, S: SmtSolver> Cegar<'a, Behavioral<S>, S> {
    pub fn from(
        Cegar {
            problem,
            mut solver,
            context,
            observer,
        }: Cegar<'a, Structural<S>, S>,
    ) -> Self {
        // Encode the transition variables into the SMT solver, and assert that they are all
        // non-negative. We will extract the values of these variables to form a candidate
        // Parikh vector when the SMT solver returns a SAT result.
        let transitions: Vec<S::Int> = {
            let zero = solver.mk_int(0);
            problem.net.transition_indices()
                .map(|t_idx| {
                    let firing_count = solver.mk_int_var(&format!("t{t_idx}"));
                    let ge_zero = solver.ge(&firing_count, &zero);
                    solver.assert(&ge_zero);
                    firing_count
                })
                .collect()
        };

        MarkingEquationRefinement::encode_into(
            &mut solver,
            &problem,
            &context.places,
            &transitions,
        );

        if let Some(transition_ordering_refinement) = TransitionOrderingRule::check(
            problem.net,
            problem.m0,
        ) {
            transition_ordering_refinement.encode_into(&mut solver, &problem, &transitions);
        }

        let context = Behavioral {
            places: context.places,
            p_invariant_rule: context.p_invariant_rule,
            transitions,
        };

        Self {
            problem,
            solver,
            context,
            observer,
        }
    }
}

/// The second phase of CEGAR refinement, where the SMT works to find a valid
/// assignment for the transition variables within the Petri net's state equation.
pub struct Behavioral<S: SmtSolver> {
    /// The SMT variables corresponding to the place markings.
    /// We extract the values of these variables to form a candidate
    /// marking when the SMT solver returns a SAT result.
    pub places: Vec<S::Int>,
    /// The P-invariant rule, instantiated with its own persistent solver state.
    pub p_invariant_rule: PInvariantRule<S>,
    /// The SMT variables corresponding to the transition firing counts.
    /// We extract the values of these variables to form a candidate
    /// Parikh vector when the SMT solver returns a SAT result.
    pub transitions: Vec<S::Int>,
}

/// The result of a single CEGAR step during the second phase
/// of the algorithm, where we have introduced transition variables
/// and the state equation into the SMT problem.
pub enum BehavioralStep<'a, S: SmtSolver> {
    /// The SMT solver returned a SAT result, and we encoded a
    /// reason why it cannot be a valid solution into the SMT problem.
    /// We can continue with the next step.
    Refined(Cegar<'a, Behavioral<S>, S>),
    /// Finished! The SMT solver returned a SAT result, and it turned out to be a valid
    /// solution in the actual Petri net.
    Witnessed(IdxMarking<u32>, Vec<TransitionIdx>),
    /// Finished! The SMT solver returned an UNSAT result, which proves that the
    /// target marking is unreachable/uncoverable in the actual Petri net.
    /// Contains a domain-native representation of the UNSAT core from the solver,
    /// describing a set of structural and behavioral insights about the net which
    /// contradict the target marking constraints.
    Unsat(Vec<IdxLemma>),
}

impl<'a, S: SmtSolver> Cegar<'a, Behavioral<S>, S> {
    /// Step the CEGAR algorithm forward by one iteration.
    pub fn step(mut self) -> BehavioralStep<'a, S> {
        let (candidate_marking, candidate_parikh_vector) = match self.solver.check() {
            Satisfiability::Sat => {
                let marking = extract_marking(&self.solver, &self.context.places)
                    .expect("Failed to extract marking from model");
                let parikh_vector = extract_parikh_vector(&self.solver, &self.context.transitions)
                    .expect("Failed to extract parikh vector from model");
                (marking, parikh_vector)
            }
            Satisfiability::Unsat => return BehavioralStep::Unsat(self.solver.unsat_core()),
        };
        if let Some(p_invariant_refinement) =
            self.context.p_invariant_rule.check(&self.problem, &candidate_marking)
        {
            p_invariant_refinement.encode_into(
                &mut self.solver,
                &self.context.places,
                self.observer.with_context(Some(candidate_marking), Some(candidate_parikh_vector)).as_deref()
            );
            return BehavioralStep::Refined(self);
        }
        if let Some(trap_refinement) =
            InitiallyMarkedTrapRule::check(&self.problem, &candidate_marking)
        {
            trap_refinement.encode_into(
                &mut self.solver,
                &self.context.places,
                self.observer.with_context(Some(candidate_marking), Some(candidate_parikh_vector)).as_deref(),
            );
            return BehavioralStep::Refined(self);
        }
        if let Some(trap_refinement) = TrapBecomesMarkedRule::check(
            &self.problem,
            &candidate_marking,
            &candidate_parikh_vector,
        ) {
            trap_refinement.encode_into(
                &mut self.solver,
                &self.context.places,
                &self.context.transitions,
                self.observer.with_context(Some(candidate_marking), Some(candidate_parikh_vector)).as_deref()
            );
            return BehavioralStep::Refined(self);
        }
        match GuidedExplorer::realize_parikh_vector(&self.problem, candidate_parikh_vector.clone())
        {
            Ok((firing_sequence, marking)) => {
                assert_eq!(
                    marking, candidate_marking,
                    "Guided exploration reached a marking which does not match the candidate marking from the SMT solver"
                );
                BehavioralStep::Witnessed(marking, firing_sequence)
            },
            Err(increment_refinements) => {
                {
                    let callback = self.observer
                        .with_context(Some(candidate_marking), Some(candidate_parikh_vector));
                    for increment_refinement in increment_refinements {
                        increment_refinement.encode_into(
                            &self.problem,
                            &mut self.solver,
                            &self.context.transitions,
                            callback.as_deref()
                        );
                    }
                }
                BehavioralStep::Refined(self)
            }
        }
    }
}
