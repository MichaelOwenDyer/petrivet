use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Examination {
    StateSpace,
    ReachabilityDeadlock,
    OneSafe,
    QuasiLiveness,
    StableMarking,
    Liveness,
    UpperBounds,
    ReachabilityFireability,
    ReachabilityCardinality,
    CTLFireability,
    CTLCardinality,
    LTLFireability,
    LTLCardinality,
}

impl FromStr for Examination {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let exam = match s {
            "StateSpace" => Self::StateSpace,
            "ReachabilityDeadlock" => Self::ReachabilityDeadlock,
            "OneSafe" => Self::OneSafe,
            "QuasiLiveness" => Self::QuasiLiveness,
            "StableMarking" => Self::StableMarking,
            "Liveness" => Self::Liveness,
            "UpperBounds" => Self::UpperBounds,
            "ReachabilityFireability" => Self::ReachabilityFireability,
            "ReachabilityCardinality" => Self::ReachabilityCardinality,
            "CTLFireability" => Self::CTLFireability,
            "CTLCardinality" => Self::CTLCardinality,
            "LTLFireability" => Self::LTLFireability,
            "LTLCardinality" => Self::LTLCardinality,
            _ => return Err(()),
        };
        Ok(exam)
    }
}

impl Display for Examination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateSpace => "StateSpace",
            Self::ReachabilityDeadlock => "ReachabilityDeadlock",
            Self::OneSafe => "OneSafe",
            Self::QuasiLiveness => "QuasiLiveness",
            Self::StableMarking => "StableMarking",
            Self::Liveness => "Liveness",
            Self::UpperBounds => "UpperBounds",
            Self::ReachabilityFireability => "ReachabilityFireability",
            Self::ReachabilityCardinality => "ReachabilityCardinality",
            Self::CTLFireability => "CTLFireability",
            Self::CTLCardinality => "CTLCardinality",
            Self::LTLFireability => "LTLFireability",
            Self::LTLCardinality => "LTLCardinality",
        }
        .fmt(f)
    }
}

/// One expected result line from an oracle file.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// A `STATE_SPACE <name> <value>` line. `value = None` means `?` (consensus not reached).
    StateSpace { name: String, value: Option<u64> },
    /// A `FORMULA <name> <value>` line. `value = None` means `?`.
    Formula { name: String, value: Option<bool> },
}

/// One parsed oracle file: a single (model, examination) pair with expected verdicts.
#[derive(Debug, Clone)]
pub struct OracleEntry {
    pub model_id: String,
    pub examination: Examination,
    pub verdicts: Box<[Verdict]>,
}

/// Parse a single oracle file's text content.
///
/// Expected format:
/// ```text
/// Anderson-PT-04 StateSpace
/// STATE_SPACE STATES 29641 TECHNIQUES TEDD2025
/// STATE_SPACE TRANSITIONS 97516 TECHNIQUES TEDD2025
/// ...
/// ```
///
/// Returns `None` if the first line cannot be parsed.
pub fn parse_oracle_file(content: &str) -> Option<OracleEntry> {
    let mut lines = content.lines();

    // First line: "<model_id> <EXAMINATION>"
    let first = lines.next()?;
    let mut fields = first.split_whitespace();
    let model_id = fields.next()?.to_string();
    let examination = Examination::from_str(fields.next()?).ok()?;
    let verdicts: Box<[Verdict]> = lines.filter_map(parse_verdict_line).collect();

    Some(OracleEntry {
        model_id,
        examination,
        verdicts,
    })
}

fn parse_verdict_line(line: &str) -> Option<Verdict> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "STATE_SPACE" => {
            let name = parts.next()?.to_owned();
            let value = parse_value_u64(parts.next()?);
            Some(Verdict::StateSpace { name, value })
        }
        "FORMULA" => {
            let name = parts.next()?.to_owned();
            let value = parse_value_bool(parts.next()?);
            Some(Verdict::Formula { name, value })
        }
        _ => None,
    }
}

fn parse_value_u64(s: &str) -> Option<u64> {
    match s {
        "?" => None,
        v => v.parse().ok(),
    }
}

fn parse_value_bool(s: &str) -> Option<bool> {
    match s {
        "TRUE" => Some(true),
        "FALSE" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_state_space_oracle_file() {
        let content = "\
Anderson-PT-04 StateSpace
STATE_SPACE STATES 29641 TECHNIQUES TEDD2025
STATE_SPACE TRANSITIONS 97516 TECHNIQUES TEDD2025
STATE_SPACE MAX_TOKEN_IN_PLACE 1 TECHNIQUES TEDD2025
STATE_SPACE MAX_TOKEN_PER_MARKING 6 TECHNIQUES TEDD2025
";
        let entry = parse_oracle_file(content).unwrap();
        assert_eq!(entry.model_id, "Anderson-PT-04");
        assert_eq!(entry.examination, Examination::StateSpace);
        assert_eq!(entry.verdicts.len(), 4);
        assert!(matches!(
            &entry.verdicts[0],
            Verdict::StateSpace { name, value: Some(29641) } if name == "STATES"
        ));
    }

    #[test]
    fn parse_formula_oracle_file() {
        let content = "\
SomeModel-PT-01 Liveness
FORMULA Liveness TRUE TECHNIQUES ORACLE2025
";
        let entry = parse_oracle_file(content).unwrap();
        assert_eq!(entry.examination, Examination::Liveness);
        assert_eq!(entry.verdicts.len(), 1);
        assert!(matches!(
            &entry.verdicts[0],
            Verdict::Formula { name, value: Some(true) } if name == "Liveness"
        ));
    }

    #[test]
    fn parse_unknown_verdict_is_none() {
        let content = "\
SomeModel-PT-01 OneSafe
FORMULA OneSafe ? TECHNIQUES ORACLE2025
";
        let entry = parse_oracle_file(content).unwrap();
        assert!(matches!(
            &entry.verdicts[0],
            Verdict::Formula { value: None, .. }
        ));
    }
}
