//! G4a — the structural-coverage **floor** (`f_struct`), measured over the
//! in-tree PNML corpus.
//!
//! This is a deliberately minimal "D3-lite" pass: it measures the coverage the
//! *current* structural tier achieves, with no Epic-B generators landed yet. It
//! is the **starting line**, not the thesis result — the baseline against which
//! later structural deciders (Epic B / milestones M6–M9) are measured. The
//! headline thesis number is the *raised* `f_struct` of milestone M12, never
//! this floor.
//!
//! # What is measured
//!
//! For each corpus net and each query-free structural property
//! (boundedness, liveness, deadlock-freedom), the tier that would decide it,
//! read from the net's **class** (a cheap, cached, polynomial computation):
//!
//! - [`Tier::Structural`] — the class admits a *cheap* (polynomial, no
//!   siphon-enumeration) structural decider: Circuit, StateMachine, or
//!   MarkedGraph. This is the certificate-bearing tier the thesis is about.
//! - [`Tier::ChoiceClass`] — a free-choice or asymmetric-choice net: a
//!   structural procedure *exists* (the Commoner-Hack criterion) but its
//!   minimal-siphon enumeration is worst-case **exponential** (the B7 scaling
//!   risk), so it is not yet *efficiently* structural. The floor records these
//!   distinctly and does **not** invoke CHC (which can take seconds even on a
//!   20-place net). Making this tier genuinely efficient is Epic B (M6–M8).
//! - [`Tier::NotStructural`] — a general net: no structural procedure applies;
//!   the dispatcher would fall to state-space search (whose success/cost is the
//!   full rig's measurement, Epic G / M12).
//! - [`Tier::ConversionRejected`] — the PNML does not convert to a runnable net
//!   (e.g. non-unit-weight arcs, A7/E7); out of scope.
//!
//! # Two denominators (queries-decided)
//!
//! The floor is reported with **both** denominators required by the protocol:
//!
//! - **in-scope**: queries on nets that convert (everything but
//!   conversion-rejected).
//! - **all-corpus**: every (net, property) query, counting conversion-rejected
//!   queries in the denominator (the honest "all-MCC" view where out-of-scope
//!   counts against coverage).
//!
//! # Why class-only (no CHC, no state-space)
//!
//! Both the choice-class structural decider (Commoner-Hack) and the search tier
//! can blow up: CHC's minimal-siphon enumeration is exponential, and a state
//! space can be astronomically large or (when unbounded) non-terminating.
//! Reading the cheap, cached `NetClass` keeps the floor **fast, total, and
//! exactly reproducible** — the property a committed baseline needs. The cost
//! and decisiveness of the CHC and search tiers are the expensive measurements
//! of the full rig (M12), not of this floor.
//!
//! Soundness (S3): this module lives in `mcc-tests`, downstream of `petrivet`;
//! the core never imports it. It reads only the public analysis surface.

use petrivet::class::NetClass;
use petrivet::system::PetriNet;
use std::fmt;

/// The structural properties measured by the floor. Query-free global
/// properties with a class-driven structural decider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Property {
    Boundedness,
    Liveness,
    DeadlockFreedom,
}

impl Property {
    /// All measured properties, in a stable order.
    pub const ALL: [Property; 3] =
        [Property::Boundedness, Property::Liveness, Property::DeadlockFreedom];
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boundedness => "Boundedness",
            Self::Liveness => "Liveness",
            Self::DeadlockFreedom => "DeadlockFreedom",
        }
        .fmt(f)
    }
}

/// Which tier would decide (or fails to apply to) a single (net, property)
/// query, read from the net class alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// A cheap (polynomial, no siphon-enumeration) structural decider applies:
    /// Circuit / StateMachine / MarkedGraph.
    Structural,
    /// A choice-class net (free-choice / asymmetric-choice): a structural
    /// decider exists (Commoner-Hack) but is exponential in the worst case (B7).
    ChoiceClass,
    /// A general net: no structural shortcut; the dispatcher would fall to
    /// state-space search (cost/decisiveness is the M12 rig's measurement).
    NotStructural,
    /// The PNML does not convert to a runnable net (out of scope).
    ConversionRejected,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structural => "structural",
            Self::ChoiceClass => "choice-class (CHC; exponential, B7)",
            Self::NotStructural => "not-structural (search)",
            Self::ConversionRejected => "conversion-rejected",
        }
        .fmt(f)
    }
}

/// Classify a net (independent of the property — the class determines tier for
/// all three query-free properties) from its cheap, cached class.
#[must_use]
fn tier_of(sys: &PetriNet) -> Tier {
    match sys.class() {
        NetClass::Circuit | NetClass::StateMachine | NetClass::MarkedGraph => Tier::Structural,
        NetClass::FreeChoice | NetClass::AsymmetricChoice => Tier::ChoiceClass,
        _ => Tier::NotStructural,
    }
}

/// One row of the floor: a single (net, property) measurement.
#[derive(Debug, Clone)]
pub struct Row {
    pub model: String,
    pub property: Property,
    pub tier: Tier,
}

/// A tally of tiers, plus the two-denominator coverage fractions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FloorReport {
    pub structural: usize,
    pub choice_class: usize,
    pub not_structural: usize,
    pub conversion_rejected: usize,
}

impl FloorReport {
    /// Fold a set of rows into a tally.
    #[must_use]
    pub fn from_rows(rows: &[Row]) -> Self {
        let mut report = Self::default();
        for row in rows {
            match row.tier {
                Tier::Structural => report.structural += 1,
                Tier::ChoiceClass => report.choice_class += 1,
                Tier::NotStructural => report.not_structural += 1,
                Tier::ConversionRejected => report.conversion_rejected += 1,
            }
        }
        report
    }

    /// Every (net, property) query counted, including out-of-scope ones.
    #[must_use]
    pub const fn all_corpus_total(&self) -> usize {
        self.structural + self.choice_class + self.not_structural + self.conversion_rejected
    }

    /// In-scope queries: nets that convert (all but conversion-rejected).
    #[must_use]
    pub const fn in_scope_total(&self) -> usize {
        self.structural + self.choice_class + self.not_structural
    }

    /// `f_struct` against the in-scope denominator, as a fraction in `[0, 1]`.
    /// Counts only the cheap-structural tier as "decided structurally";
    /// choice-class (CHC) is excluded from the numerator because it is not yet
    /// *efficiently* structural. Returns 0.0 when there are no in-scope queries.
    #[must_use]
    pub fn f_struct_in_scope(&self) -> f64 {
        let denom = self.in_scope_total();
        if denom == 0 {
            0.0
        } else {
            self.structural as f64 / denom as f64
        }
    }

    /// `f_struct` against the all-corpus denominator (out-of-scope counts
    /// against coverage), as a fraction in `[0, 1]`.
    #[must_use]
    pub fn f_struct_all_corpus(&self) -> f64 {
        let denom = self.all_corpus_total();
        if denom == 0 {
            0.0
        } else {
            self.structural as f64 / denom as f64
        }
    }
}

impl fmt::Display for FloorReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "structural-coverage floor (f_struct) — the STARTING LINE, not the thesis result")?;
        writeln!(f, "  structural-decided : {}", self.structural)?;
        writeln!(f, "  choice-class (CHC) : {} (exponential B7; Epic B makes this efficient)", self.choice_class)?;
        writeln!(f, "  not-structural     : {} (search tier; cost/decisiveness is the M12 rig)", self.not_structural)?;
        writeln!(f, "  conversion-rejected: {}", self.conversion_rejected)?;
        writeln!(
            f,
            "  f_struct (in-scope denominator   = {}): {:.3}",
            self.in_scope_total(),
            self.f_struct_in_scope()
        )?;
        write!(
            f,
            "  f_struct (all-corpus denominator = {}): {:.3}",
            self.all_corpus_total(),
            self.f_struct_all_corpus()
        )
    }
}

/// Measure the structural-coverage floor over a corpus of `(model_id, pnml)`
/// pairs. Reads only the cheap net class; runs no CHC and no state-space
/// exploration, so it is fast and total.
#[must_use]
pub fn measure_corpus(fixtures: &[(String, String)]) -> Vec<Row> {
    let mut rows = Vec::with_capacity(fixtures.len() * Property::ALL.len());
    for (model, pnml) in fixtures {
        let tier = match PetriNet::from_pnml(pnml) {
            Ok(sys) => tier_of(&sys),
            // The net does not convert (e.g. weighted arcs, A7/E7): out of scope.
            Err(_) => Tier::ConversionRejected,
        };
        for property in Property::ALL {
            rows.push(Row { model: model.clone(), property, tier });
        }
    }
    rows
}
