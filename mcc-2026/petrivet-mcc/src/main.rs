mod protocol;

use std::path::{Path, PathBuf};

use petrivet::{Net, ReachabilityGraph, System};
use protocol::{
    BooleanFormulaReport, Examination, ParticipationError, RunContext, StateSpaceReport, Technique,
};

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

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    if matches!(args.next().as_deref(), Some("--help" | "-h")) {
        println!("{HELP}");
        return Ok(());
    }

    let ctx = RunContext::from_env()?;
    match run(&ctx) {
        Ok(()) => Ok(()),
        Err(ParticipationError::DoNotCompete) => {
            println!("DO NOT COMPETE");
            Ok(())
        }
        Err(ParticipationError::CannotCompute) => {
            println!("CANNOT COMPUTE");
            Ok(())
        }
    }
}

fn run(ctx: &RunContext) -> Result<(), ParticipationError> {
    let input_dir = PathBuf::from(".");

    if is_colored_model(&input_dir, &ctx.input_name) {
        return Err(ParticipationError::CannotCompute);
    }
    // Per the Submission Manual page 5: `unfinite` and `large_marking` are
    // empty marker files dropped by BenchKit when it knows in advance that
    // the marking graph is infinite, or that some marking has more than 2³²
    // tokens, respectively. petrivet stores token counts as u32, so a
    // `large_marking` instance would overflow before we got anywhere; bail
    // out before trying to parse the model.
    if has_marker(&input_dir, "unfinite") || has_marker(&input_dir, "large_marking") {
        return Err(ParticipationError::CannotCompute);
    }

    match &ctx.examination {
        Examination::StateSpace => run_state_space(&input_dir),
        Examination::ReachabilityDeadlock => run_reachability_deadlock(&input_dir),
        Examination::OneSafe => run_one_safe(&input_dir),
        Examination::QuasiLiveness => {
            run_global_property_via_rg(&input_dir, ctx.examination.as_str(), |rg| {
                rg.is_quasi_live()
            })
        }
        Examination::StableMarking => {
            run_global_property_via_rg(&input_dir, ctx.examination.as_str(), |rg| {
                rg.has_stable_place()
            })
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
        .map_err(|_| ParticipationError::CannotCompute)?;
    let report = StateSpaceReport {
        states: rg.state_count(),
        transitions: rg.transition_count(),
        max_tokens_per_marking: rg.max_token_per_marking(),
        max_tokens_in_place: rg.max_token_in_any_place(),
        techniques: default_techniques(),
    };
    println!("{report}");
    Ok(())
}

/// `ReachabilityDeadlock`: build the reachability graph (short-circuiting
/// on ω) and ask `has_reachable_deadlock`.
///
/// Deliberately not pre-running the Commoner-Hack siphon/trap check:
/// minimal-siphon enumeration is exponential and on nets that *do*
/// have deadlocks — which is the only time we care, since CHC failing
/// is inconclusive — that work is fully wasted because we still have
/// to build the RG. Where CHC pays off is in *proving* liveness, see
/// `run_liveness`.
///
/// For unbounded nets we report `CANNOT COMPUTE`. Deadlock-freedom is
/// in fact decidable for general P/T nets via backward reachability
/// (BACK2 in the lecture notes); petrivet does not yet implement that.
fn run_reachability_deadlock(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let rg = system
        .build_reachability_or_coverability()
        .map_err(|_| ParticipationError::CannotCompute)?;
    emit_global(
        Examination::ReachabilityDeadlock.as_str(),
        rg.has_reachable_deadlock(),
        default_techniques(),
    );
    Ok(())
}

/// `OneSafe` answer ladder:
///
/// 1. **Structural place bounds.** If `find_positive_place_subvariant`
///    pins every place at ≤ 1 token, we are done. Polynomial-time,
///    no exploration.
/// 2. **Short-circuiting RG walk** on a structurally bounded net:
///    `has_reachable_unsafe_marking` stops at the first marking that
///    holds ≥ 2 tokens in any place (FALSE witness).
/// 3. **Full RG fallback** when boundedness is unknown structurally:
///    we still attempt the explicit RG via the coverability path; if
///    exploration aborts on ω we emit `CANNOT COMPUTE`.
fn run_one_safe(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let name = Examination::OneSafe.as_str();

    if system.is_structurally_one_safe() {
        emit_global(name, true, structural_techniques());
        return Ok(());
    }

    if system.net().is_structurally_bounded() {
        emit_global(name, !system.has_reachable_unsafe_marking(), default_techniques());
        return Ok(());
    }

    let rg = system
        .build_reachability_or_coverability()
        .map_err(|_| ParticipationError::CannotCompute)?;
    emit_global(name, rg.is_one_safe(), default_techniques());
    Ok(())
}

/// Liveness has a cheap structural shortcut on free-choice nets via the
/// Commoner-Hack criterion. Otherwise we fall back to the SCC-based
/// decision on the explicit RG.
fn run_liveness(input_dir: &Path) -> Result<(), ParticipationError> {
    let system = load_system(input_dir)?;
    let name = Examination::Liveness.as_str();

    if system.net().is_free_choice_net() && system.commoner_hack_criterion().is_satisfied() {
        emit_global(name, true, structural_techniques());
        return Ok(());
    }

    let rg = system
        .build_reachability_or_coverability()
        .map_err(|_| ParticipationError::CannotCompute)?;
    emit_global(name, rg.is_live(), default_techniques());
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
        .map_err(|_| ParticipationError::CannotCompute)?;
    emit_global(formula_name, answer(&rg), default_techniques());
    Ok(())
}

fn emit_global(formula: &str, value: bool, techniques: Vec<Technique>) {
    let report = BooleanFormulaReport {
        formula: formula.to_string(),
        value: Some(value),
        techniques,
    };
    println!("{report}");
}

fn default_techniques() -> Vec<Technique> {
    vec![
        Technique::SequentialProcessing,
        Technique::Explicit,
        Technique::Topological,
    ]
}

fn structural_techniques() -> Vec<Technique> {
    vec![
        Technique::SequentialProcessing,
        Technique::Topological,
        Technique::StructuralReduction,
    ]
}

fn load_system(input_dir: &Path) -> Result<System<Net>, ParticipationError> {
    let pnml = std::fs::read_to_string(input_dir.join("model.pnml"))
        .map_err(|_| ParticipationError::CannotCompute)?;
    System::from_pnml(&pnml)
        .inspect_err(|err| eprintln!("{err:?}"))
        .map_err(|_| ParticipationError::CannotCompute)
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
