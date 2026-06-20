mod protocol;

use petrivet::state_space::ReachabilityGraph;
use petrivet::prelude::{Net, PetriNet};
use protocol::{
    BooleanFormulaReport, Examination, ParticipationError, RunContext, StateSpaceReport, Technique,
};
use std::path::{Path, PathBuf};

const HELP: &str =
    "Usage: petrivet-mcc [--help|-h]
    Reads MCC execution context from environment variables and prints the contest
    response keywords expected by BenchKit.

    Environment:
      BK_TOOL         tool name selected by BenchKit
      BK_EXAMINATION  contest examination category
      BK_INPUT        contest input directory name
      BIN_DIR         location of tool binaries inside the guest
    ";

const DEFAULT_TECHNIQUES: &[Technique] = &[
    Technique::SequentialProcessing,
    Technique::Explicit,
    Technique::Topological,
];

// F0 (backlog): `Technique::StructuralReduction` must be emitted ONLY when a
// certified structural *reduction* has actually fired (Epic F). The Commoner-Hack
// criterion on free-choice nets is a structural *decision*, not a reduction, and
// no reduction code exists yet, so tagging it `STRUCTURAL_REDUCTION` was a
// mislabel. The CHC liveness shortcut therefore reports `DEFAULT_TECHNIQUES`
// until a real reduction exists to claim the tag. Rationale: foundational-design
// §F0 / BACKLOG F0.

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    if matches!(args.next().as_deref(), Some("--help" | "-h")) {
        println!("{HELP}");
        return Ok(());
    }

    let ctx = RunContext::from_env()?;
    match std::panic::catch_unwind(|| run(&ctx)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            println!("{err}");
            Ok(())
        },
        Err(_panic_cause) => {
            eprintln!("panic!");
            println!("{}", ParticipationError::CannotCompute);
            Ok(())
        }
    }
}

fn run(ctx: &RunContext) -> Result<(), ParticipationError> {
    let input_dir = PathBuf::from(".");

    if is_colored_model(&input_dir, &ctx.input_name) {
        return Err(ParticipationError::DoNotCompete);
    }
    // Per the Submission Manual page 5: `unfinite` and `large_marking` are
    // empty marker files dropped by BenchKit when it knows in advance that
    // the marking graph is infinite, or that some marking has more than 2³²
    // tokens, respectively. petrivet stores token counts as u32, so a
    // `large_marking` instance would overflow before we got anywhere; bail
    // out before trying to parse the model.
    if has_marker(&input_dir, "unfinite") || has_marker(&input_dir, "large_marking") {
        return Err(ParticipationError::DoNotCompete);
    }

    match &ctx.examination {
        Examination::StateSpace => run_state_space(&input_dir),
        Examination::ReachabilityDeadlock => run_reachability_deadlock(&input_dir),
        Examination::OneSafe => run_one_safe(&input_dir),
        Examination::QuasiLiveness => {
            run_global_property_via_rg(
                &input_dir,
                ctx.examination.as_str(),
                |rg| {
                    rg.transition_liveness().is_quasi_live()
                }
            )
        }
        Examination::StableMarking => {
            run_global_property_via_rg(
                &input_dir,
                ctx.examination.as_str(),
                |rg| {
                    rg.has_stable_place()
                }
            )
        }
        Examination::Liveness => run_liveness(&input_dir),
        Examination::UpperBounds
        | Examination::ReachabilityFireability
        | Examination::ReachabilityCardinality
        | Examination::CTLFireability
        | Examination::CTLCardinality
        | Examination::LTLFireability
        | Examination::LTLCardinality => Err(ParticipationError::DoNotCompete),
    }
}

fn run_state_space(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let rg = system
        .try_build_reachability_graph()
        .map_err(|_| ParticipationError::DoNotCompete)?;
    let report = StateSpaceReport {
        states: rg.marking_count(),
        transitions: rg.transition_count(),
        max_tokens_per_marking: rg.max_token_per_marking(),
        max_tokens_in_place: rg.max_token_in_any_place(),
        techniques: DEFAULT_TECHNIQUES,
    };
    println!("{report}");
    Ok(())
}

/// `ReachabilityDeadlock`: return true if there is any reachable deadlock marking.
fn run_reachability_deadlock(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    print_boolean_result(
        Examination::ReachabilityDeadlock.as_str(),
        system.deadlocks().next().is_some(),
        DEFAULT_TECHNIQUES,
    );
    Ok(())
}

fn run_one_safe(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let name = Examination::OneSafe.as_str();
    let result = system.is_safe();
    print_boolean_result(name, result, DEFAULT_TECHNIQUES);
    Ok(())
}

/// The technique set reported for a liveness verdict.
///
/// F0 (backlog): both the Commoner-Hack structural *decision* and the
/// reachability-graph fallback report `DEFAULT_TECHNIQUES`. Neither path runs a
/// certified structural *reduction*, so neither may claim
/// `Technique::StructuralReduction` — that tag is reserved until Epic F lands a
/// real reduction. Factored into a pure function so the F0 invariant is
/// unit-testable without filesystem I/O.
///
/// The two liveness paths report the *same* technique set today, so this takes
/// no parameter (lean — no speculative per-path argument until a real distinction
/// exists). The regression test pins the constant directly: it must never carry
/// `StructuralReduction`.
const fn liveness_techniques() -> &'static [Technique] {
    DEFAULT_TECHNIQUES
}

/// Liveness has a cheap structural shortcut on free-choice nets via the
/// Commoner-Hack criterion. Otherwise we fall back to the SCC-based
/// decision on the explicit RG.
fn run_liveness(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let name = Examination::Liveness.as_str();

    if system.class().is_free_choice() && system.commoner_hack_criterion().is_ok() {
        print_boolean_result(name, true, liveness_techniques());
        return Ok(());
    }

    let rg = system
        .try_build_reachability_graph()
        .map_err(|_unbounded_graph| ParticipationError::DoNotCompete)?;
    print_boolean_result(name, rg.transition_liveness().is_live(), liveness_techniques());
    Ok(())
}

/// For examinations where we don't have a structural shortcut: build the
/// reachability graph (short-circuiting on ω) and answer from it.
fn run_global_property_via_rg(
    input_dir: &Path,
    formula_name: &str,
    answer: impl FnOnce(&ReachabilityGraph<'_>) -> bool,
) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let rg = system
        .try_build_reachability_graph()
        .map_err(|_unbounded_graph| ParticipationError::DoNotCompete)?;
    print_boolean_result(formula_name, answer(&rg), DEFAULT_TECHNIQUES);
    Ok(())
}

fn print_boolean_result(formula: &str, value: bool, techniques: &[Technique]) {
    println!("{}", BooleanFormulaReport { formula, value: Some(value), techniques });
}

fn load_system(input_dir: &Path) -> Result<PetriNet<Net>, ParticipationError> {
    let pnml = std::fs::read_to_string(input_dir.join("model.pnml"))
        .map_err(|e| {
            eprintln!("failed to read model.pnml: {e}");
            ParticipationError::CannotCompute
        })?;
    PetriNet::from_pnml(&pnml).map_err(|e| {
        eprintln!("failed to parse system: {e}");
        ParticipationError::CannotCompute
    })
}

fn is_colored_model(input_dir: &Path, input_name: &str) -> bool {
    if let Ok(content) = std::fs::read_to_string(input_dir.join("iscolored")) {
        return content.trim().eq_ignore_ascii_case("TRUE");
    }
    input_name.contains("-COL-")
}

fn has_marker(input_dir: &Path, name: &str) -> bool {
    input_dir.join(name).exists()
}

#[cfg(test)]
mod tests {
    use super::{liveness_techniques, Technique};

    /// F0 regression: the liveness verdict (the Commoner-Hack structural
    /// *decision* on free-choice nets and the reachability-graph fallback alike)
    /// is not a certified structural *reduction*, so its technique set must NOT
    /// emit `STRUCTURAL_REDUCTION`. This would have caught the original mislabel,
    /// and guards against a future change re-introducing a reduction-bearing
    /// constant on this path before Epic F lands a real reduction.
    #[test]
    fn liveness_path_is_not_tagged_structural_reduction() {
        let techniques = liveness_techniques();
        assert!(
            !techniques.contains(&Technique::StructuralReduction),
            "liveness technique set must not claim STRUCTURAL_REDUCTION (F0): {techniques:?}"
        );
    }
}
