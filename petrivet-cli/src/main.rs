//! `petrivet`: analyze a place/transition Petri net given as PNML.
//!
//! Prints one verdict per invocation, either as text or as a single line of JSON (`--json`).
//! Exit status is 0 whenever a verdict (including `unknown`) was produced, and 1 on any error.

use clap::{Parser, Subcommand, ValueEnum};
use petrivet::net::Net;
use petrivet::system::PetriNet;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

#[derive(Parser)]
#[command(version, about = "Analyze a place/transition Petri net given as PNML")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decide one property of a net
    Analyze {
        /// PNML file; the first place/transition net in it is analyzed
        file: PathBuf,
        /// Property to decide
        #[arg(short, long, value_enum)]
        property: Property,
        /// Which procedures may be used
        #[arg(short, long, value_enum, default_value_t = Mode::Dispatch)]
        mode: Mode,
        /// Print a single line of JSON instead of text
        #[arg(long)]
        json: bool,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum Property {
    /// Every reachable marking enables some transition
    #[value(name = "df")]
    DeadlockFreedom,
    /// The set of reachable markings is finite
    Bounded,
    /// Every transition is live (L4)
    Live,
    /// Every transition is enabled in some reachable marking (L1)
    QuasiLive,
    /// Number of reachable markings and edges (bounded nets only; general mode only)
    #[value(name = "statespace")]
    StateSpace,
}

#[derive(Copy, Clone, ValueEnum)]
enum Mode {
    /// The cheapest applicable procedure, as chosen by the library
    Dispatch,
    /// Only polynomial-time procedures; `unknown` if none applies
    Poly,
    /// The general state-space procedure, no structural shortcuts
    General,
}

#[derive(Copy, Clone)]
enum Verdict {
    True,
    False,
    Unknown,
    NotSupported,
    Unbounded,
}

impl Verdict {
    const fn as_str(self) -> &'static str {
        match self {
            Verdict::True => "true",
            Verdict::False => "false",
            Verdict::Unknown => "unknown",
            Verdict::NotSupported => "not_supported",
            Verdict::Unbounded => "unbounded",
        }
    }
}

impl From<bool> for Verdict {
    fn from(b: bool) -> Self {
        if b { Verdict::True } else { Verdict::False }
    }
}

impl From<Option<bool>> for Verdict {
    fn from(b: Option<bool>) -> Self {
        b.map_or(Verdict::Unknown, Verdict::from)
    }
}

struct Outcome {
    verdict: Verdict,
    states: Option<usize>,
    edges: Option<usize>,
}

impl From<Verdict> for Outcome {
    fn from(verdict: Verdict) -> Self {
        Outcome { verdict, states: None, edges: None }
    }
}

/// The general procedure: build the full reachability graph and read the answer off it.
/// Unbounded systems have no finite reachability graph, so the answer is `unknown`.
fn via_reachability_graph(
    sys: &PetriNet<Net>,
    answer: impl FnOnce(&petrivet::state_space::ReachabilityGraph<'_>) -> Verdict,
) -> Outcome {
    match sys.try_build_reachability_graph() {
        Ok(rg) => Outcome {
            verdict: answer(&rg),
            states: Some(rg.marking_count()),
            edges: Some(rg.transition_count()),
        },
        Err(_) => Verdict::Unknown.into(),
    }
}

fn analyze(sys: &PetriNet<Net>, property: Property, mode: Mode) -> Outcome {
    match (property, mode) {
        (Property::DeadlockFreedom, Mode::Dispatch) => Verdict::from(sys.is_deadlock_free()).into(),
        (Property::DeadlockFreedom, Mode::Poly) => Verdict::from(sys.is_efficiently_deadlock_free()).into(),
        (Property::DeadlockFreedom, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.is_deadlock_free().into())
        }

        (Property::Bounded, Mode::Dispatch) => Verdict::from(sys.is_bounded()).into(),
        (Property::Bounded, Mode::Poly) => Verdict::from(sys.is_efficiently_bounded()).into(),
        (Property::Bounded, Mode::General) => {
            Verdict::from(sys.build_coverability_graph().is_bounded()).into()
        }

        (Property::Live, Mode::Dispatch) => Verdict::from(sys.liveness().map(|l| l.is_live())).into(),
        (Property::Live, Mode::Poly) => {
            Verdict::from(sys.efficient_liveness().map(|l| l.is_live())).into()
        }
        (Property::Live, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.transition_liveness().is_live().into())
        }

        (Property::QuasiLive, Mode::Dispatch) => {
            Verdict::from(sys.liveness().map(|l| l.is_quasi_live())).into()
        }
        (Property::QuasiLive, Mode::Poly) => {
            Verdict::from(sys.efficient_liveness().map(|l| l.is_quasi_live())).into()
        }
        (Property::QuasiLive, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.transition_liveness().is_quasi_live().into())
        }

        (Property::StateSpace, Mode::General) => match sys.try_build_reachability_graph() {
            Ok(rg) => Outcome {
                verdict: Verdict::True,
                states: Some(rg.marking_count()),
                edges: Some(rg.transition_count()),
            },
            Err(_) => Verdict::Unbounded.into(),
        },
        (Property::StateSpace, _) => Verdict::NotSupported.into(),
    }
}

fn name<T: ValueEnum>(value: T) -> String {
    value.to_possible_value().expect("no skipped variants").get_name().to_owned()
}

/// Minimal JSON string escaping; file names and error messages are the only free text.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).expect("write to String"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn run(file: &PathBuf, property: Property, mode: Mode, json: bool) -> Result<(), String> {
    let start = Instant::now();
    let pnml = std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let sys = PetriNet::from_pnml(&pnml).map_err(|e| e.to_string())?;
    let parse_ms = ms(start);

    let start = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| analyze(&sys, property, mode)))
        .map_err(|_| "analysis panicked".to_owned())?;
    let analyze_ms = ms(start);

    let class = format!("{:?}", sys.class());
    if json {
        let mut line = String::new();
        write!(
            line,
            "{{\"tool\":\"petrivet\",\"version\":{},\"file\":{},\"property\":{},\"mode\":{},\"verdict\":{},\
             \"class\":{},\"places\":{},\"transitions\":{},\"arcs\":{},\"parse_ms\":{parse_ms:.3},\"analyze_ms\":{analyze_ms:.3}",
            json_str(env!("CARGO_PKG_VERSION")),
            json_str(&file.display().to_string()),
            json_str(&name(property)),
            json_str(&name(mode)),
            json_str(outcome.verdict.as_str()),
            json_str(&class),
            sys.place_count(),
            sys.transition_count(),
            sys.arc_count(),
        )
        .expect("write to String");
        if let Some(n) = outcome.states {
            write!(line, ",\"states\":{n}").expect("write to String");
        }
        if let Some(n) = outcome.edges {
            write!(line, ",\"edges\":{n}").expect("write to String");
        }
        line.push('}');
        println!("{line}");
    } else {
        println!("{}", outcome.verdict.as_str());
        println!("class: {class}");
        if let (Some(s), Some(e)) = (outcome.states, outcome.edges) {
            println!("states: {s}, edges: {e}");
        }
        println!("parse: {parse_ms:.3} ms, analysis: {analyze_ms:.3} ms");
    }
    Ok(())
}

fn main() -> ExitCode {
    let Command::Analyze { file, property, mode, json } = Cli::parse().command;
    match run(&file, property, mode, json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            if json {
                println!(
                    "{{\"tool\":\"petrivet\",\"file\":{},\"property\":{},\"mode\":{},\"verdict\":\"error\",\"error\":{}}}",
                    json_str(&file.display().to_string()),
                    json_str(&name(property)),
                    json_str(&name(mode)),
                    json_str(&message),
                );
            } else {
                eprintln!("error: {message}");
            }
            ExitCode::FAILURE
        }
    }
}
