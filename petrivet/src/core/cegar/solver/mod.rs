//! The [`SmtSolver`] trait abstracts over the incremental SMT solver that drives the CEGAR
//! loop in [`super::cegar`], so that the algorithm and its refinement rules (`refinements/*.rs`)
//! can be written once, generic over `S: CegarSolver`, and reused with any conforming backend.
//!
//! Two backends are provided:
//! - [`oxiz::OxiZ`], backed by the pure-Rust `oxiz` solver.
//! - [`z3::Z3`], backed by the mature `z3` solver via its Rust bindings.
//!
//! The trait is deliberately small and shaped by exactly what the CEGAR encoding needs: fresh
//! integer variables, linear arithmetic and boolean combinations over them, asserting
//! constraints (optionally tagged with the domain-level [`IdxLemma`] that produced them, for
//! unsat-core reporting), and reading integer values back out of a satisfying model. Where the
//! two backends' native APIs differ incidentally (e.g. how a model is represented, or how an
//! unsat core is reported), that difference is absorbed inside the backend implementation
//! rather than exposed through the trait.

#[cfg(feature = "oxiz")]
pub mod oxiz;
#[cfg(feature = "z3")]
pub mod z3;

use crate::core::cegar::lemma::IdxLemma;

/// The result of an SMT satisfiability check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Satisfiability {
    Sat,
    Unsat,
}

#[cfg(feature = "oxiz")]
pub type DefaultSolver = oxiz::OxiZ;
#[cfg(feature = "z3")]
pub type DefaultSolver = z3::Z3;
#[cfg(all(feature = "z3", feature = "oxiz"))]
compile_error!("features `z3` and `oxiz` are mutually exclusive; pick one backend");

/// An incremental SMT solver capable of driving the CEGAR loop.
///
/// Implementors own both "the solver" and "the term manager/context" (however a given backend
/// splits those concerns internally) behind this single interface.
pub trait SmtSolver: Default {
    /// An integer-sorted SMT term.
    type Int: Clone;
    /// A boolean-sorted SMT term.
    type Bool: Clone;

    /// Declare a fresh integer-sorted variable. `name` is used for diagnostics only backends
    /// are not required to enforce uniqueness.
    fn mk_int_var(&mut self, name: &str) -> Self::Int;
    /// Create an integer constant.
    fn mk_int(&mut self, value: i64) -> Self::Int;

    /// The sum of `terms`. Callers must not pass an empty collection.
    fn add(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int;
    /// The product of `terms`. Callers must not pass an empty collection.
    fn mul(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int;

    fn eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    fn ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    fn gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    fn lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// The conjunction of `terms`. Callers must not pass an empty collection.
    fn and(&mut self, terms: impl IntoIterator<Item = Self::Bool>) -> Self::Bool;
    /// The disjunction of `terms`. Callers must not pass an empty collection.
    fn or(&mut self, terms: impl IntoIterator<Item = Self::Bool>) -> Self::Bool;
    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool;

    /// Assert a constraint unconditionally, with no attribution in the unsat core. Reserved for
    /// constraints that aren't independently-verifiable domain facts in their own right (e.g.
    /// non-negativity of place/transition variables) - anything that states a checkable claim
    /// about the net should go through [`SmtSolver::assert_tracked`] instead, tagged with an
    /// [`IdxLemma`] precise enough for a reader to verify it independently.
    fn assert(&mut self, constraint: &Self::Bool);
    /// Assert a constraint, tagging it with the [`IdxLemma`] it corresponds to. If this
    /// constraint is used to derive unsatisfiability, `lemma` will be present in the
    /// result of a subsequent [`SmtSolver::unsat_core`] call.
    fn assert_tracked(&mut self, constraint: &Self::Bool, lemma: IdxLemma);

    /// Push a new assertion scope.
    /// Subsequent assertions are local to this scope and can be discarded by a later `pop`.
    fn push(&mut self);
    /// Pop the most recent assertion scope, discarding any assertions made since the matching `push`.
    /// Does nothing if there is no matching `push` (i.e. the solver is at the root scope).
    fn pop(&mut self);

    /// Check satisfiability of the current assertions. Panics if the underlying solver answers
    /// "unknown" the CEGAR encoding is quantifier-free linear arithmetic and should always be
    /// decidable.
    fn check(&mut self) -> Satisfiability;
    /// Read the concrete value assigned to `term` by the model of the last [`Satisfiability::Sat`]
    /// result. Only valid to call after `check` returned `Sat`. Returns `None` if the model does
    /// not assign `term` a non-negative integer value, which should not happen for well-formed
    /// CEGAR problems.
    fn eval_int(&self, term: &Self::Int) -> Option<u32>;
    /// The [`IdxLemma`]s (tagged via [`SmtSolver::assert_tracked`]) that were used to derive
    /// unsatisfiability. Only valid to call after `check` returned [`Satisfiability::Unsat`].
    fn unsat_core(&mut self) -> Vec<IdxLemma>;
}
