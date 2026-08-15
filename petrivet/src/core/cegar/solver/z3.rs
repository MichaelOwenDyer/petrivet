//! [`SmtSolver`] backed by the `z3` SMT solver via its Rust bindings.

use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::{Satisfiability, SmtSolver};
use ahash::{HashMap, HashMapExt};
use z3::SatResult;
use z3::ast::{Bool, Int};

/// Owns a `z3` solver. Unlike `oxiz`, `z3`'s Rust bindings manage the `Context` implicitly via
/// a thread-local default (see `z3::Context::thread_local`), so there is no separate context
/// handle to store here, and `Int`/`Bool` terms carry no lifetime parameter.
pub struct Z3 {
    solver: z3::Solver,
    /// Maps a tracking literal (passed to `Solver::assert_and_track`) to the refinement it was
    /// tagged with, so it can be recovered when the literal is later found in an unsat core.
    tracked: HashMap<Bool, IdxLemma>,
}

impl Z3 {
    pub fn new() -> Self {
        Self {
            solver: z3::Solver::new(),
            tracked: HashMap::new(),
        }
    }
}

impl Default for Z3 {
    fn default() -> Self {
        Self::new()
    }
}

impl SmtSolver for Z3 {
    type Int = Int;
    type Bool = Bool;

    fn mk_int_var(&mut self, name: &str) -> Self::Int {
        Int::new_const(name)
    }

    fn mk_int(&mut self, value: i64) -> Self::Int {
        Int::from_i64(value)
    }

    fn add(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int {
        Int::add(&terms.into_iter().collect::<Vec<_>>())
    }

    fn mul(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int {
        Int::mul(&terms.into_iter().collect::<Vec<_>>())
    }

    fn eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.eq(b.clone())
    }

    fn ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.ge(b.clone())
    }

    fn gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.gt(b.clone())
    }

    fn lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.lt(b.clone())
    }

    fn and(&mut self, terms: impl IntoIterator<Item = Self::Bool>) -> Self::Bool {
        Bool::and(&terms.into_iter().collect::<Vec<_>>())
    }

    fn or(&mut self, terms: impl IntoIterator<Item = Self::Bool>) -> Self::Bool {
        Bool::or(&terms.into_iter().collect::<Vec<_>>())
    }

    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.implies(b.clone())
    }

    fn assert(&mut self, constraint: &Self::Bool) {
        self.solver.assert(constraint);
    }

    fn assert_tracked(&mut self, constraint: &Self::Bool, lemma: IdxLemma) {
        let lit = Bool::fresh_const("t");
        self.solver.assert_and_track(constraint.clone(), &lit);
        self.tracked.insert(lit, lemma);
    }

    fn push(&mut self) {
        self.solver.push();
    }

    fn pop(&mut self) {
        self.solver.pop(1);
    }

    fn check(&mut self) -> Satisfiability {
        match self.solver.check() {
            SatResult::Sat => Satisfiability::Sat,
            SatResult::Unsat => Satisfiability::Unsat,
            SatResult::Unknown => panic!(
                "z3 solver returned Unknown result! Reason: {:?}",
                self.solver.get_reason_unknown()
            ),
        }
    }

    fn eval_int(&self, term: &Self::Int) -> Option<u32> {
        let model = self
            .solver
            .get_model()
            .expect("eval_int called without a preceding Sat check result");
        let value = model.eval(term, true)?;
        u32::try_from(value.as_u64()?).ok()
    }

    fn unsat_core(&mut self) -> Vec<IdxLemma> {
        self.solver
            .get_unsat_core()
            .into_iter()
            .filter_map(|lit| self.tracked.remove(&lit))
            .collect()
    }
}
