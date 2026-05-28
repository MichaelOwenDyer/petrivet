use datatest_stable::Result;
use petrivet::system::PetriNet;
use std::path::Path;
use std::str::FromStr;
use mcc_tests::oracle::{parse_oracle_file, Examination, Verdict};
use mcc_tests::runner::RunResult;

fn has_definitive_verdict(v: &Verdict) -> bool {
    matches!(
        v,
        Verdict::StateSpace { value: Some(_), .. } | Verdict::Formula { value: Some(_), .. }
    )
}

fn run_exam_test(verdict_path: &Path, examination: Examination) -> Result<()> {
    let model_dir = verdict_path
        .parent()
        .ok_or("verdict file has no parent directory")?;

    let content = std::fs::read_to_string(verdict_path)?;
    let entry = parse_oracle_file(&content)
        .ok_or_else(|| format!("failed to parse {}", verdict_path.display()))?;

    let pnml_path = model_dir.join("model.pnml");
    if !pnml_path.exists() {
        return Err(format!("missing model file: {}", pnml_path.display()).into());
    }

    let Ok(pnml) = std::fs::read_to_string(&pnml_path) else {
        return Err("failed to read model.pnml".into());
    };

    if model_dir.join("iscolored").exists()
        && let Ok(c) = std::fs::read_to_string(model_dir.join("iscolored"))
        && c.trim().eq_ignore_ascii_case("TRUE") {
        return Err("colored model".into());
    }

    let Ok(sys) = PetriNet::from_pnml(&pnml) else {
        return Err("failed to parse PNML".into());
    };

    if let Ok(max_places) = std::env::var("MCC_MAX_PLACES")
        && let Ok(max_places) = u32::from_str(&max_places) {
        let place_count = mcc_tests::models::place_count(&pnml);
        if place_count > max_places {
            return Ok(());
        }
    }

    let result = mcc_tests::runner::run_analysis(&sys, examination);

    match &result {
        RunResult::DoNotCompete => return Err("DoNotCompete — petrivet failed on a model that should be computable".into()),
        RunResult::CannotCompute => {
            if entry.verdicts.iter().any(has_definitive_verdict) {
                return Err("CannotCompute — petrivet failed on a model with definitive oracle verdicts".into());
            }
            return Ok(()); // if oracle has no definitive verdicts, we can't call this a failure
        }
        RunResult::Verdicts(actual) => {
            let mismatches = mcc_tests::runner::compare_verdicts(&entry.model_id, examination, &entry.verdicts, actual);

            if !mismatches.is_empty() {
                let detail = mismatches
                    .iter()
                    .map(|m| format!("  {}/{}: expected '{}', got '{}'", m.model_id, m.name, m.expected, m.actual))
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(format!("{} correctness failure(s):\n{detail}", mismatches.len()).into());
            }
        }
    }

    Ok(())
}

fn state_space(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::StateSpace)
}

fn reachability_deadlock(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::ReachabilityDeadlock)
}

fn one_safe(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::OneSafe)
}

fn quasi_liveness(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::QuasiLiveness)
}

fn stable_marking(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::StableMarking)
}

fn liveness(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::Liveness)
}

#[expect(unused)]
fn upper_bounds(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::UpperBounds)
}

#[expect(unused)]
fn reachability_fireability(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::ReachabilityFireability)
}

#[expect(unused)]
fn reachability_cardinality(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::ReachabilityCardinality)
}

#[expect(unused)]
fn ctl_fireability(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::CTLFireability)
}

#[expect(unused)]
fn ctl_cardinality(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::CTLCardinality)
}

#[expect(unused)]
fn ltl_fireability(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::LTLFireability)
}

#[expect(unused)]
fn ltl_cardinality(path: &Path) -> Result<()> {
    run_exam_test(path, Examination::LTLCardinality)
}

datatest_stable::harness! {
    { test = state_space,              root = "oracle/models", pattern = r".*-SS\.out$"   },
    { test = reachability_deadlock,    root = "oracle/models", pattern = r".*-RD\.out$"   },
    { test = one_safe,                 root = "oracle/models", pattern = r".*-OS\.out$"   },
    { test = quasi_liveness,           root = "oracle/models", pattern = r".*-QL\.out$"   },
    { test = stable_marking,           root = "oracle/models", pattern = r".*-SM\.out$"   },
    { test = liveness,                 root = "oracle/models", pattern = r".*-L\.out$"    },
    // petrivet does not yet support the following examinations:
    // { test = upper_bounds,             root = "oracle/models", pattern = r".*-UB\.out$"   },
    // { test = reachability_fireability, root = "oracle/models", pattern = r".*-RF\.out$"   },
    // { test = reachability_cardinality, root = "oracle/models", pattern = r".*-RC\.out$"   },
    // { test = ctl_fireability,          root = "oracle/models", pattern = r".*-CTLF\.out$" },
    // { test = ctl_cardinality,          root = "oracle/models", pattern = r".*-CTLC\.out$" },
    // { test = ltl_fireability,          root = "oracle/models", pattern = r".*-LTLF\.out$" },
    // { test = ltl_cardinality,          root = "oracle/models", pattern = r".*-LTLC\.out$" },
}
