//! Binary protocol tests: spawns `petrivet-mcc` as a subprocess and verifies
//! that it reads BenchKit env vars correctly and produces well-formed output
//! whose values match the oracle.
//!
//! One test per supported examination. Each test finds the smallest (by filesize)
//! model that has model.pnml downloaded and a definitive oracle verdict, then
//! runs the binary against it.
//!
//! All tests skip silently if no suitable model is available (oracle not set up
//! or no models downloaded). Build `petrivet-mcc` before running:
//!   cargo build -p petrivet-mcc

use assert_cmd::Command;
use mcc_tests::oracle::{Examination, OracleEntry, Verdict, parse_oracle_file};
use std::path::{Path, PathBuf};

fn has_definitive_verdict(v: &Verdict) -> bool {
    matches!(
        v,
        Verdict::StateSpace { value: Some(_), .. } | Verdict::Formula { value: Some(_), .. }
    )
}

/// Find the smallest suitable downloaded model for the given examination.
/// "Smallest" = smallest model.pnml by file size, as a fast proxy for quick analysis.
/// Skips colored, infinite, and large-marking models, and models with no definitive verdicts.
fn find_model_for_exam(models_dir: &Path, exam_suffix: &str) -> Option<(PathBuf, OracleEntry)> {
    let mut candidates: Vec<(u64, PathBuf, OracleEntry)> = std::fs::read_dir(models_dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let model_dir = e.path();
            let pnml_path = model_dir.join("model.pnml");
            let pnml_size = std::fs::metadata(&pnml_path).ok()?.len();
            if let Ok(c) = std::fs::read_to_string(model_dir.join("iscolored"))
                && c.trim().eq_ignore_ascii_case("TRUE")
            {
                return None;
            }
            if model_dir.join("unfinite").exists() || model_dir.join("large_marking").exists() {
                return None;
            }
            let out_path = std::fs::read_dir(&model_dir)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .find(|p| p.to_str().is_some_and(|s| s.ends_with(exam_suffix)))?;
            let content = std::fs::read_to_string(&out_path).ok()?;
            let entry = parse_oracle_file(&content)?;
            entry
                .verdicts
                .iter()
                .any(has_definitive_verdict)
                .then_some((pnml_size, model_dir, entry))
        })
        .collect();

    candidates.sort_by_key(|(size, _, _)| *size);
    candidates
        .into_iter()
        .next()
        .map(|(_, dir, entry)| (dir, entry))
}

fn run_protocol_test(exam: Examination, exam_suffix: &str) {
    let models_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("oracle")
        .join("models");

    let Some((model_dir, entry)) = find_model_for_exam(&models_dir, exam_suffix) else {
        eprintln!(
            "Skipping {exam}: no suitable model available — run ./mcc-tests/scripts/setup.sh"
        );
        return;
    };

    let model_id = model_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let output = Command::cargo_bin("petrivet-mcc")
        .unwrap()
        .env("BK_EXAMINATION", exam.to_string())
        .env("BK_INPUT", model_id)
        .env("BK_TOOL", "petrivet")
        .current_dir(&model_dir)
        .output()
        .expect("failed to spawn petrivet-mcc — run `cargo build -p petrivet-mcc` first");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "petrivet-mcc exited non-zero for {exam} on {model_id}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    for verdict in &entry.verdicts {
        let expected = match verdict {
            Verdict::StateSpace {
                name,
                value: Some(v),
            } => format!("STATE_SPACE {name} {v}"),
            Verdict::Formula {
                name,
                value: Some(v),
            } => {
                format!("FORMULA {name} {}", if *v { "TRUE" } else { "FALSE" })
            }
            _ => continue,
        };
        assert!(
            stdout.contains(&expected),
            "{exam} on {model_id}: expected stdout to contain '{expected}'\nActual stdout:\n{stdout}"
        );
    }
}

#[test]
fn state_space() {
    run_protocol_test(Examination::StateSpace, "-SS.out");
}

#[test]
fn reachability_deadlock() {
    run_protocol_test(Examination::ReachabilityDeadlock, "-RD.out");
}

#[test]
fn one_safe() {
    run_protocol_test(Examination::OneSafe, "-OS.out");
}

#[test]
fn quasi_liveness() {
    run_protocol_test(Examination::QuasiLiveness, "-QL.out");
}

#[test]
fn stable_marking() {
    run_protocol_test(Examination::StableMarking, "-SM.out");
}

#[test]
fn liveness() {
    run_protocol_test(Examination::Liveness, "-L.out");
}
