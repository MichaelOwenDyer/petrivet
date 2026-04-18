use std::fmt;
use std::path::PathBuf;

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

    pub fn supports_formulas(&self) -> bool {
        !matches!(self, Self::StateSpace)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormulaEncoding {
    Fireability,
    Cardinality,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalProperty {
    Deadlock,
    QuasiLiveness,
    StableMarking,
    Liveness,
    OneSafe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParticipationError {
    DoNotCompete,
    CannotCompute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunContext {
    pub tool_name: String,
    pub examination: Examination,
    pub input_name: String,
    pub bin_dir: PathBuf,
}

impl RunContext {
    pub fn from_env() -> Result<Self, String> {
        let tool_name = std::env::var("BK_TOOL").unwrap_or_else(|_| "petrivet-mcc".to_string());
        let examination = std::env::var("BK_EXAMINATION")
            .ok()
            .and_then(|value| Examination::parse(&value))
            .ok_or_else(|| "missing or invalid BK_EXAMINATION".to_string())?;
        let input_name = std::env::var("BK_INPUT").unwrap_or_else(|_| ".".to_string());
        let bin_dir = std::env::var("BIN_DIR").map_or_else(
            |_| PathBuf::from("/home/mcc/BenchKit/bin"),
            PathBuf::from
        );

        Ok(Self {
            tool_name,
            examination,
            input_name,
            bin_dir,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Technique {
    SequentialProcessing,
}

impl fmt::Display for Technique {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Technique::SequentialProcessing => write!(f, "SEQUENTIAL PROCESSING"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSpaceReport {
    pub states: Option<i64>,
    pub transitions: Option<i64>,
    pub max_tokens_per_marking: Option<i64>,
    pub max_tokens_in_place: Option<i64>,
    pub techniques: Vec<Technique>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpperBoundReport {
    pub formula: String,
    pub value: Option<i64>,
    pub techniques: Vec<Technique>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BooleanFormulaReport {
    pub formula: String,
    pub value: Option<bool>,
    pub techniques: Vec<Technique>,
}

