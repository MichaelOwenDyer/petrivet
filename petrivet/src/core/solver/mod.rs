//! The [`SmtSolver`] trait abstracts over incremental SMT solver backends.
//!
//! Two backends are provided:
//! - [`oxiz::OxiZ`], backed by the pure-Rust `oxiz` solver.
//! - [`z3::Z3`], backed by the mature `z3` solver via its Rust bindings.

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

/// An incremental SMT solver.
pub trait SmtSolver: Default {
    /// An integer-sorted SMT term.
    type Int: Clone;
    /// A boolean-sorted SMT term.
    type Bool: Clone;

    /// Declare an integer-sorted variable with the given name.
    /// Multiple calls with the same name return the same variable.
    fn mk_int_var(&mut self, name: &str) -> Self::Int;
    /// Create an integer constant.
    fn mk_int(&mut self, value: i64) -> Self::Int;
    /// Declare a boolean-sorted variable with the given name.
    /// Multiple calls with the same name return the same variable.
    fn mk_bool_var(&mut self, name: &str) -> Self::Bool;

    /// The sum of `terms`. Callers must not pass an empty collection.
    fn add(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int;
    /// The product of `terms`. Callers must not pass an empty collection.
    fn mul(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int;

    /// `a == b`
    fn eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    /// `a >= b`
    fn ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    /// `a > b`
    fn gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    /// `a <= b`
    fn le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;
    /// `a < b`
    fn lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// The conjunction of `terms`. An empty conjunction resolves to `true`.
    fn and(&mut self, terms: &[Self::Bool]) -> Self::Bool;
    /// The disjunction of `terms`. An empty disjunction resolves to `false`.
    fn or(&mut self, terms: &[Self::Bool]) -> Self::Bool;
    /// The implication `a => b`. Equivalent to `!a || b`.
    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool;
    /// The logical negation `!a`.
    fn not(&mut self, a: &Self::Bool) -> Self::Bool;

    /// Assert a constraint.
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

    /// Check satisfiability of the current assertions. Panics if the underlying solver answers "unknown".
    fn check(&mut self) -> Satisfiability;
    /// Read the concrete value assigned to `term` by the model of the last [`Satisfiability::Sat`]
    /// result. Only valid to call after `check` returned `Sat`.
    fn eval_int(&self, term: &Self::Int) -> Option<u32>;
    /// Read the concrete value assigned to `term` by the model of the last [`Satisfiability::Sat`]
    /// result. Only valid to call after `check` returned `Sat`.
    fn eval_bool(&self, term: &Self::Bool) -> Option<bool>;
    /// The [`IdxLemma`]s (tagged via [`SmtSolver::assert_tracked`]) that were used to derive
    /// unsatisfiability. Only valid to call after `check` returned [`Satisfiability::Unsat`].
    fn unsat_core(&mut self) -> Vec<IdxLemma>;
}
