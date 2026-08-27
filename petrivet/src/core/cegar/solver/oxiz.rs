//! [`SmtSolver`] backed by the pure-Rust `oxiz` SMT solver.

use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::{SmtSolver, Satisfiability};
use ahash::{HashMap, HashMapExt};

/// Owns an `oxiz` solver and term manager, plus the bookkeeping needed to translate an unsat
/// core (a list of assertion indices, in `oxiz`'s API) back into the [`IdxLemma`]s that were
/// asserted via [`SmtSolver::assert_tracked`].
pub struct OxiZ {
    /// The `oxiz` SMT solver.
    smt: Box<oxiz::Solver>,
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
            smt: Box::new(oxiz::Solver::new()),
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

    fn and(&mut self, terms: impl IntoIterator<Item = Self::Bool>) -> Self::Bool {
        self.terms.mk_and(terms)
    }

    fn or(&mut self, terms: impl IntoIterator<Item = Self::Bool>) -> Self::Bool {
        self.terms.mk_or(terms)
    }

    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        self.terms.mk_implies(*a, *b)
    }

    fn assert(&mut self, constraint: &Self::Bool) {
        self.smt.assert(*constraint, &mut self.terms);
    }

    fn assert_tracked(&mut self, constraint: &Self::Bool, lemma: IdxLemma) {
        // oxiz has no direct "assert and track" primitive, so we build the standard
        // activation-literal encoding by hand: assert `lit => constraint` unconditionally, and
        // `lit` itself as a separate, named assertion. If the latter ends up in the unsat core,
        // `constraint` was necessary for unsatisfiability.
        let name = format!("t{}", self.next_tracking_id);
        self.next_tracking_id += 1;
        let lit = self.terms.mk_var(&name, self.terms.sorts.bool_sort);
        let tracked_constraint = self.terms.mk_implies(lit, *constraint);
        self.smt.assert(tracked_constraint, &mut self.terms);
        self.smt.assert(lit, &mut self.terms);
        self.tracked.insert(lit, lemma);
    }

    fn push(&mut self) {
        self.smt.push();
    }
    fn pop(&mut self) {
        self.smt.pop();
    }

    fn check(&mut self) -> Satisfiability {
        match self.smt.check(&mut self.terms) {
            oxiz::SolverResult::Sat => Satisfiability::Sat,
            oxiz::SolverResult::Unsat => Satisfiability::Unsat,
            oxiz::SolverResult::Unknown => panic!(
                "oxiz solver returned Unknown result! Debug info: {:?}",
                self.smt
            ),
        }
    }

    fn eval_int(&self, term: &Self::Int) -> Option<u32> {
        let model = self.smt.model()?;
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
        let model = self.smt.model()?;
        let value = model.get(*term)?;
        match &self.terms.get(value)?.kind {
            oxiz::core::TermKind::True => Some(true),
            oxiz::core::TermKind::False => Some(false),
            _ => None,
        }
    }

    fn unsat_core(&mut self) -> Vec<IdxLemma> {
        self.smt
            .get_unsat_core()
            .into_iter()
            .flat_map(|core| core.indices.iter().copied())
            .map(|assertion_index| self.smt.assertions[assertion_index as usize])
            .filter_map(|term_id| self.tracked.remove(&term_id))
            .collect()
    }
}
