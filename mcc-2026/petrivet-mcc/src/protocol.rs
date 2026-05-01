use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Examination {
    StateSpace,
    UpperBounds,
    ReachabilityDeadlock,
    QuasiLiveness,
    StableMarking,
    Liveness,
    OneSafe,
    ReachabilityFireability,
    ReachabilityCardinality,
    CTLFireability,
    CTLCardinality,
    LTLFireability,
    LTLCardinality,
}

impl Examination {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "StateSpace" => Self::StateSpace,
            "UpperBounds" => Self::UpperBounds,
            "ReachabilityDeadlock" => Self::ReachabilityDeadlock,
            "QuasiLiveness" => Self::QuasiLiveness,
            "StableMarking" => Self::StableMarking,
            "Liveness" => Self::Liveness,
            "OneSafe" => Self::OneSafe,
            "ReachabilityFireability" => Self::ReachabilityFireability,
            "ReachabilityCardinality" => Self::ReachabilityCardinality,
            "CTLFireability" => Self::CTLFireability,
            "CTLCardinality" => Self::CTLCardinality,
            "LTLFireability" => Self::LTLFireability,
            "LTLCardinality" => Self::LTLCardinality,
            _ => return None,
        })
    }

    /// The exact spelling expected as the `<formula-id>` for `FORMULA …` lines
    /// in `GlobalProperties` examinations (per the MCC Submission Manual:
    /// "you may either use the value of the BK_EXAMINATION environment variable
    /// or the XML formula that is provided in the directory").
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StateSpace => "StateSpace",
            Self::UpperBounds => "UpperBounds",
            Self::ReachabilityDeadlock => "ReachabilityDeadlock",
            Self::QuasiLiveness => "QuasiLiveness",
            Self::StableMarking => "StableMarking",
            Self::Liveness => "Liveness",
            Self::OneSafe => "OneSafe",
            Self::ReachabilityFireability => "ReachabilityFireability",
            Self::ReachabilityCardinality => "ReachabilityCardinality",
            Self::CTLFireability => "CTLFireability",
            Self::CTLCardinality => "CTLCardinality",
            Self::LTLFireability => "LTLFireability",
            Self::LTLCardinality => "LTLCardinality",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParticipationError {
    DoNotCompete,
    CannotCompute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunContext {
    pub examination: Examination,
    pub input_name: String,
}

impl RunContext {
    pub fn from_env() -> Result<Self, String> {
        let examination = std::env::var("BK_EXAMINATION")
            .ok()
            .and_then(|value| Examination::parse(&value))
            .ok_or_else(|| "missing or invalid BK_EXAMINATION".to_string())?;
        let input_name = std::env::var("BK_INPUT")
            .map_err(|_| "missing BK_INPUT".to_string())?;

        Ok(Self {
            examination,
            input_name,
        })
    }
}

/// Subset of MCC Submission Manual table 2 (page 8) that we actually use.
/// Add more variants here as new techniques get wired into the dispatcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Technique {
    SequentialProcessing,
    Explicit,
    Topological,
    StructuralReduction,
}

impl fmt::Display for Technique {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Technique::SequentialProcessing => write!(f, "SEQUENTIAL_PROCESSING"),
            Technique::Explicit => write!(f, "EXPLICIT"),
            Technique::Topological => write!(f, "TOPOLOGICAL"),
            Technique::StructuralReduction => write!(f, "STRUCTURAL_REDUCTION"),
        }
    }
}

/// One `STATE_SPACE …` block, in the format BenchKit expects.
///
/// The four counts come straight from the library: `states` and `transitions`
/// are `usize` (the underlying `petgraph::Graph` node/edge counts), and the
/// token counts are `u32` (the integer marking representation). The
/// MCC Submission Manual marks `STATES` mandatory and allows `-1` for the
/// other three fields when a tool cannot provide them; we always provide
/// real values, since by the time we have a `ReachabilityGraph` the counts
/// are exact and free.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSpaceReport {
    pub states: usize,
    pub transitions: usize,
    pub max_tokens_per_marking: u32,
    pub max_tokens_in_place: u32,
    pub techniques: Vec<Technique>,
}

impl fmt::Display for StateSpaceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let techs = format_techniques(&self.techniques);
        writeln!(f, "STATE_SPACE STATES {} TECHNIQUES {techs}", self.states)?;
        writeln!(f, "STATE_SPACE TRANSITIONS {} TECHNIQUES {techs}", self.transitions)?;
        writeln!(f, "STATE_SPACE MAX_TOKEN_PER_MARKING {} TECHNIQUES {techs}", self.max_tokens_per_marking)?;
        write!(f, "STATE_SPACE MAX_TOKEN_IN_PLACE {} TECHNIQUES {techs}", self.max_tokens_in_place)
    }
}

fn format_techniques(techniques: &[Technique]) -> String {
    if techniques.is_empty() {
        return Technique::SequentialProcessing.to_string();
    }
    techniques
        .iter()
        .map(Technique::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A single `FORMULA …` line for `GlobalProperties` and the formula-driven
/// examinations. `value = None` is the protocol's `CANNOT COMPUTE`; the
/// Submission Manual forbids `DO NOT COMPETE` at this granularity (that
/// keyword is reserved for whole-subcategory opt-outs).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BooleanFormulaReport {
    pub formula: String,
    pub value: Option<bool>,
    pub techniques: Vec<Technique>,
}

impl fmt::Display for BooleanFormulaReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Some(value) => {
                let techs = format_techniques(&self.techniques);
                let bool_str = if value { "TRUE" } else { "FALSE" };
                write!(f, "FORMULA {} {bool_str} TECHNIQUES {techs}", self.formula)
            }
            None => write!(f, "FORMULA {} CANNOT COMPUTE", self.formula),
        }
    }
}

