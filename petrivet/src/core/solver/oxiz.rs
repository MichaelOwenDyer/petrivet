//! [`SmtSolver`] backed by the pure-Rust `oxiz` SMT solver.

use crate::core::cegar::lemma::IdxLemma;
use crate::core::solver::{Satisfiability, SmtSolver};
use ahash::{HashMap, HashMapExt};

/// An incremental SMT solver backed by the pure-Rust `oxiz` SMT solver.
pub struct OxiZ {
    /// The `oxiz` SMT solver.
    solver: Box<oxiz::Solver>,
    /// The `oxiz` term manager, which owns all terms and sorts.
    terms: Box<oxiz::TermManager>,
    /// Maps a tracking literal to the refinement it was tagged with, so that it can be
    /// recovered when the literal is later found in an unsat core.
    tracked: HashMap<oxiz::TermId, IdxLemma>,
    /// Counter used to give every tracking literal a unique name. `oxiz::TermManager::mk_var`
    /// hash-conses on `(name, sort)`, so two calls with the same name return the *same* term -
    /// a fixed literal name here would silently alias every tracked constraint onto one literal.
    next_tracking_id: u32,
}

impl OxiZ {
    pub fn new() -> Self {
        Self {
            solver: Box::new(oxiz::Solver::new()),
            terms: Box::new(oxiz::TermManager::new()),
            tracked: HashMap::new(),
            next_tracking_id: 0,
        }
    }
}

impl Default for OxiZ {
    fn default() -> Self {
        Self::new()
    }
}

impl SmtSolver for OxiZ {
    type Int = oxiz::TermId;
    type Bool = oxiz::TermId;

    fn mk_int_var(&mut self, name: &str) -> Self::Int {
        self.terms.mk_var(name, self.terms.sorts.int_sort)
    }

    fn mk_int(&mut self, value: i64) -> Self::Int {
        self.terms.mk_int(value)
    }

    fn mk_bool_var(&mut self, name: &str) -> Self::Bool {
        self.terms.mk_var(name, self.terms.sorts.bool_sort)
    }

    fn mk_bool(&mut self, value: bool) -> Self::Bool {
        if value {
            self.terms.mk_true()
        } else {
            self.terms.mk_false()
        }
    }

    fn add(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int {
        self.terms.mk_add(terms)
    }

    fn mul(&mut self, terms: impl IntoIterator<Item = Self::Int>) -> Self::Int {
        self.terms.mk_mul(terms)
    }

    fn eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        self.terms.mk_eq(*a, *b)
    }

    fn ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        self.terms.mk_ge(*a, *b)
    }

    fn gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        self.terms.mk_gt(*a, *b)
    }

    fn le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        self.terms.mk_le(*a, *b)
    }

    fn lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        self.terms.mk_lt(*a, *b)
    }

    fn and(&mut self, terms: &[Self::Bool]) -> Self::Bool {
        if terms.is_empty() {
            self.terms.mk_true()
        } else {
            self.terms.mk_and(terms)
        }
    }

    fn or(&mut self, terms: &[Self::Bool]) -> Self::Bool {
        if terms.is_empty() {
            self.terms.mk_false()
        } else {
            self.terms.mk_or(terms)
        }
    }

    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        self.terms.mk_implies(*a, *b)
    }
    fn not(&mut self, a: &Self::Bool) -> Self::Bool {
        self.terms.mk_not(*a)
    }
    fn iff(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        self.terms.mk_iff(*a, *b)
    }

    fn assert(&mut self, constraint: &Self::Bool) {
        self.solver.assert(*constraint, &mut self.terms);
    }

    fn assert_tracked(&mut self, constraint: &Self::Bool, lemma: IdxLemma) {
        let name = format!("t{}", self.next_tracking_id);
        self.next_tracking_id += 1;
        let lit = self.terms.mk_var(&name, self.terms.sorts.bool_sort);
        let tracked_constraint = self.terms.mk_implies(lit, *constraint);
        self.solver.assert(tracked_constraint, &mut self.terms);
        self.solver.assert(lit, &mut self.terms);
        self.tracked.insert(lit, lemma);
    }

    fn push(&mut self) {
        self.solver.push();
    }
    fn pop(&mut self) {
        self.solver.pop();
    }

    fn check(&mut self) -> Satisfiability {
        match self.solver.check(&mut self.terms) {
            oxiz::SolverResult::Sat => Satisfiability::Sat,
            oxiz::SolverResult::Unsat => Satisfiability::Unsat,
            oxiz::SolverResult::Unknown => panic!(
                "oxiz solver returned Unknown result! Debug info: {:?}",
                self.solver
            ),
        }
    }

    fn eval_int(&self, term: &Self::Int) -> Option<u32> {
        let model = self.solver.model()?;
        let value = model.get(*term)?;
        match &self.terms.get(value)?.kind {
            oxiz::core::TermKind::IntConst(n) => u32::try_from(n).ok(),
            oxiz::core::TermKind::RealConst(r) if *r.denom() == 1 => {
                u32::try_from(*r.numer()).ok()
            }
            _ => None,
        }
    }

    fn eval_bool(&self, term: &Self::Bool) -> Option<bool> {
        let model = self.solver.model()?;
        let value = model.get(*term)?;
        match &self.terms.get(value)?.kind {
            oxiz::core::TermKind::True => Some(true),
            oxiz::core::TermKind::False => Some(false),
            _ => None,
        }
    }

    fn unsat_core(&mut self) -> Vec<IdxLemma> {
        self.solver
            .get_unsat_core()
            .into_iter()
            .flat_map(|core| core.indices.iter().copied())
            .map(|assertion_index| self.solver.assertions[assertion_index as usize])
            .filter_map(|term_id| self.tracked.remove(&term_id))
            .collect()
    }
}
