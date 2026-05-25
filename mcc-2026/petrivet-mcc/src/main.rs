mod protocol;

use petrivet::state_space::ReachabilityGraph;
use petrivet::{Net, PetriNet};
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

const STRUCTURAL_TECHNIQUES: &[Technique] = &[
    Technique::SequentialProcessing,
    Technique::Topological,
    Technique::StructuralReduction,
];

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
                    rg.is_quasi_live()
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
        .build_reachability_or_coverability()
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
        system.has_reachable_deadlock_marking(),
        DEFAULT_TECHNIQUES,
    );
    Ok(())
}

fn run_one_safe(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let name = Examination::OneSafe.as_str();

    if system.is_structurally_one_safe() {
        print_boolean_result(name, true, STRUCTURAL_TECHNIQUES);
        return Ok(());
    }

    print_boolean_result(name, !system.has_reachable_unsafe_marking(), DEFAULT_TECHNIQUES);
    Ok(())
}

/// Liveness has a cheap structural shortcut on free-choice nets via the
/// Commoner-Hack criterion. Otherwise we fall back to the SCC-based
/// decision on the explicit RG.
fn run_liveness(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let name = Examination::Liveness.as_str();

    if system.class().is_free_choice() && system.commoner_hack_criterion().is_satisfied() {
        print_boolean_result(name, true, STRUCTURAL_TECHNIQUES);
        return Ok(());
    }

    let rg = system
        .build_reachability_or_coverability()
        .map_err(|_unbounded_graph| ParticipationError::DoNotCompete)?;
    print_boolean_result(name, rg.is_live(), DEFAULT_TECHNIQUES);
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
        .build_reachability_or_coverability()
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
