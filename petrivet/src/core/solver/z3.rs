//! [`SmtSolver`] backed by the `z3` SMT solver via its Rust bindings.

use crate::core::cegar::lemma::IdxLemma;
use crate::core::solver::{Satisfiability, SmtSolver};
use ahash::{HashMap, HashMapExt};
use z3::SatResult;
use z3::ast::{Bool, Int};

/// An incremental SMT solver backed by the `z3` SMT solver via its Rust bindings.
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

    fn mk_bool_var(&mut self, name: &str) -> Self::Bool {
        Bool::new_const(name)
    }
    fn mk_bool(&mut self, value: bool) -> Self::Bool {
        Bool::from_bool(value)
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

    fn le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.le(b.clone())
    }

    fn lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.lt(b.clone())
    }

    fn and(&mut self, terms: &[Self::Bool]) -> Self::Bool {
        if terms.is_empty() {
            Bool::from_bool(true)
        } else {
            Bool::and(terms)
        }
    }

    fn or(&mut self, terms: &[Self::Bool]) -> Self::Bool {
        if terms.is_empty() {
            Bool::from_bool(false)
        } else {
            Bool::or(terms)
        }
    }

    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.implies(b)
    }
    fn not(&mut self, a: &Self::Bool) -> Self::Bool {
        a.not()
    }
    fn iff(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.iff(b)
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
        let model = self.solver.get_model().expect("check() must have returned SAT before calling eval_int");
        let value = model.eval(term, true)?;
        u32::try_from(value.as_u64()?).ok()
    }

    fn eval_bool(&self, term: &Self::Bool) -> Option<bool> {
        let model = self.solver.get_model().expect("check() must have returned SAT before calling eval_bool");
        model.eval(term, true)?.as_bool()
    }

    fn unsat_core(&mut self) -> Vec<IdxLemma> {
        self.solver
            .get_unsat_core()
            .into_iter()
            .filter_map(|lit| self.tracked.remove(&lit))
            .collect()
    }
}
