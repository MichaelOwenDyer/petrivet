#![allow(dead_code)]

mod protocol;

use protocol::{Examination, ParticipationError, RunContext};

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
    match run(ctx) {
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

fn run(ctx: RunContext) -> Result<(), ParticipationError> {
    match &ctx.examination {
        Examination::StateSpace => run_state_space(&ctx),
        Examination::UpperBounds
        | Examination::ReachabilityDeadlock
        | Examination::QuasiLiveness
        | Examination::StableMarking
        | Examination::Liveness
        | Examination::OneSafe
        | Examination::ReachabilityFireability
        | Examination::ReachabilityCardinality
        | Examination::CTLFireability
        | Examination::CTLCardinality
        | Examination::LTLFireability
        | Examination::LTLCardinality => Err(ParticipationError::DoNotCompete),
    }
}

fn run_state_space(_ctx: &RunContext) -> Result<(), ParticipationError> {
    Err(ParticipationError::CannotCompute)
}

