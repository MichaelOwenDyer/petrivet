//! [`SmtSolver`] backed by the pure-Rust `oxiz` SMT solver.

use crate::core::cegar::lemma::IdxLemma;
use crate::core::solver::{Satisfiability, SmtSolver};
use ahash::{HashMap, HashMapExt};
use std::time::Instant;

/// An incremental SMT solver backed by the pure-Rust `oxiz` SMT solver.
pub struct OxiZ {
    /// The `oxiz` SMT solver.
    solver: Box<oxiz::Solver>,
    /// The `oxiz` term manager, which owns all terms and sorts.
    terms: Box<oxiz::TermManager>,
    /// Maps the name of a tracked assertion to the refinement it was tagged with, so that it can be
    /// recovered when the assertion is later found in an unsat core.
    tracked: HashMap<String, IdxLemma>,
    /// Counter used to give every tracked assertion a unique name.
    next_tracking_id: u32,
    /// If set, `check` gives up once this instant has passed.
    deadline: Option<Instant>,
}

impl OxiZ {
    pub fn new() -> Self {
        Self {
            solver: {
                let mut solver = Box::new(oxiz::Solver::new());
                solver.set_produce_unsat_cores(true);
                solver
            },
            terms: Box::new(oxiz::TermManager::new()),
            tracked: HashMap::new(),
            next_tracking_id: 0,
            deadline: None,
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

    fn and(&mut self, terms: &[Self::Bool]) -> Self::Bool {
        if terms.is_empty() {
            self.terms.mk_true()
        } else {
            self.terms.mk_and(terms.iter().copied())
        }
    }

    fn or(&mut self, terms: &[Self::Bool]) -> Self::Bool {
        if terms.is_empty() {
            self.terms.mk_false()
        } else {
            self.terms.mk_or(terms.iter().copied())
        }
    }

    fn implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        self.terms.mk_implies(*a, *b)
    }
    fn not(&mut self, a: &Self::Bool) -> Self::Bool {
        self.terms.mk_not(*a)
    }

    fn assert(&mut self, constraint: &Self::Bool) {
        self.solver.assert(*constraint, &mut self.terms);
    }

    fn assert_tracked(&mut self, constraint: &Self::Bool, lemma: IdxLemma) {
        let name = format!("t{}", self.next_tracking_id);
        self.next_tracking_id += 1;
        self.solver.assert_named(*constraint, &name, &mut self.terms);
        self.tracked.insert(name, lemma);
    }

    fn push(&mut self) {
        self.solver.push();
    }
    fn pop(&mut self) {
        self.solver.pop();
    }

    fn set_deadline(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
    }

    fn check(&mut self) -> Satisfiability {
        if let Some(deadline) = self.deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Satisfiability::Unknown;
            }
            self.solver.set_timeout(remaining);
        }
        match self.solver.check(&mut self.terms) {
            oxiz::SolverResult::Sat => Satisfiability::Sat,
            oxiz::SolverResult::Unsat => Satisfiability::Unsat,
            oxiz::SolverResult::Unknown => Satisfiability::Unknown,
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
        let core = self
            .solver
            .minimize_unsat_core(&mut self.terms)
            .or_else(|| self.solver.get_unsat_core().cloned());
        core.into_iter()
            .flat_map(|core| core.names)
            .filter_map(|name| self.tracked.remove(&name))
            .collect()
    }
}
