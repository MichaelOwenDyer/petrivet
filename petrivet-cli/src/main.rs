//! `petrivet`: analyze a place/transition Petri net given as PNML.
//!
//! Prints one verdict per invocation, either as text or as a single line of JSON (`--json`).
//! Exit status is 0 whenever a verdict (including `unknown`) was produced, and 1 on any error.

use clap::{Parser, Subcommand, ValueEnum};
use petrivet::net::{Net, Place};
use petrivet::state_space::Omega;
use petrivet::system::PetriNet;
use petrivet::system::marking::Marking;
use petrivet::system::boundedness::BoundednessEffort;
use petrivet::system::deadlock_freedom::DeadlockFreedomEffort;
use petrivet::system::reachability::{CoverabilityResult, ReachabilityResult};
use petrivet::system::liveness::{LivenessEffort, LivenessLevel};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

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
        /// Give up (verdict `unknown`) after this many milliseconds, where the library supports a
        /// deadline (dispatch and poly modes). General mode has no deadline; use an external limit.
        #[arg(long, value_name = "N")]
        timeout_ms: Option<u64>,
        /// JSON object mapping PNML place ids to token counts (unlisted places are 0); required
        /// for `reach` and `cover`
        #[arg(long, value_name = "FILE.json")]
        target: Option<PathBuf>,
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
    /// Every reachable marking has at most one token per place
    #[value(name = "onesafe")]
    OneSafe,
    /// Some place holds the same number of tokens in all reachable markings (general mode only)
    Stable,
    /// The target marking is reachable (needs --target)
    Reach,
    /// Some reachable marking is >= the target marking (needs --target)
    Cover,
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

impl Outcome {
    fn new(verdict: impl Into<Verdict>) -> Self {
        Outcome { verdict: verdict.into(), states: None, edges: None }
    }

    /// A verdict from an optional `(answer, method)` pair, `unknown` when absent.
    fn decided<T: Into<Verdict>>(answer: Option<T>) -> Self {
        answer.map_or_else(|| Self::new(Verdict::Unknown), Self::new)
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
        Err(_) => Outcome::new(Verdict::Unknown),
    }
}

/// Inputs shared by all procedures.
struct Params {
    /// Deadline passed to the library wherever it accepts one. `None` means no deadline.
    timeout: Option<Duration>,
    /// Target marking for `reach` and `cover`.
    target: Option<Marking<u32>>,
}

fn analyze(sys: &PetriNet<Net>, property: Property, mode: Mode, params: &Params) -> Outcome {
    let timeout = params.timeout;
    // `Duration::MAX` overflows `Instant::checked_add` inside the library, which means "no deadline".
    let long_timeout = timeout.unwrap_or(Duration::MAX);
    match (property, mode) {
        (Property::DeadlockFreedom, Mode::Dispatch) => {
            Outcome::decided(sys.is_deadlock_free().maybe_timeout(timeout).call())
        }
        (Property::DeadlockFreedom, Mode::Poly) => Outcome::decided(
            sys.is_deadlock_free().effort(DeadlockFreedomEffort::Efficient).maybe_timeout(timeout).call(),
        ),
        // The explicit-state procedures below have no deadline in the library; the external
        // runner's limit applies.
        (Property::DeadlockFreedom, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.is_deadlock_free().into())
        }

        (Property::Bounded, Mode::Dispatch) => {
            Outcome::decided(sys.is_bounded().maybe_timeout(timeout).call())
        }
        (Property::Bounded, Mode::Poly) => Outcome::decided(
            sys.is_bounded().effort(BoundednessEffort::Efficient).maybe_timeout(timeout).call()
        ),
        (Property::Bounded, Mode::General) => {
            Outcome::new(sys.build_coverability_graph().is_bounded())
        }

        (Property::OneSafe, Mode::Dispatch) => {
            Outcome::decided(sys.is_bounded().k(1).maybe_timeout(timeout).call())
        }
        (Property::OneSafe, Mode::Poly) => Outcome::decided(
            sys.is_bounded().k(1).effort(BoundednessEffort::Efficient).maybe_timeout(timeout).call()
        ),
        // An unbounded net is in particular not 1-safe.
        (Property::OneSafe, Mode::General) => match sys.try_build_reachability_graph() {
            Ok(rg) => Outcome {
                verdict: rg.is_one_safe().into(),
                states: Some(rg.marking_count()),
                edges: Some(rg.transition_count()),
            },
            Err(_) => Outcome::new(Verdict::False),
        },

        (Property::Live, Mode::Dispatch) => Outcome::decided(
            sys.liveness().maybe_timeout(timeout).call().map(|l| l.is_live())
        ),
        (Property::Live, Mode::Poly) => Outcome::decided(
            sys.liveness().effort(LivenessEffort::Efficient).maybe_timeout(timeout).call().map(|l| l.is_live()),
        ),
        (Property::Live, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.transition_liveness().is_live().into())
        }

        (Property::QuasiLive, Mode::Dispatch) => Outcome::decided(
            sys.liveness().maybe_timeout(timeout).call().map(|l| l >= LivenessLevel::L1)
        ),
        (Property::QuasiLive, Mode::Poly) => Outcome::decided(
            sys.liveness().effort(LivenessEffort::Efficient).maybe_timeout(timeout).call().map(|l| l >= LivenessLevel::L1)
        ),
        (Property::QuasiLive, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.transition_liveness().is_quasi_live().into())
        }

        (Property::StateSpace, Mode::General) => match sys.try_build_reachability_graph() {
            Ok(rg) => Outcome {
                verdict: Verdict::True,
                states: Some(rg.marking_count()),
                edges: Some(rg.transition_count()),
            },
            Err(_) => Outcome::new(Verdict::Unbounded),
        },
        (Property::StateSpace, _) => Outcome::new(Verdict::NotSupported),

        (Property::Stable, Mode::General) => {
            via_reachability_graph(sys, |rg| rg.has_stable_place().into())
        }
        (Property::Stable, _) => Outcome::new(Verdict::NotSupported),

        (Property::Reach, Mode::Dispatch) => {
            let target = params.target.clone().expect("target checked in run");
            Outcome::new(match sys.analyze_reachability_within(target, long_timeout, None) {
                ReachabilityResult::Reachable { .. } => Verdict::True,
                ReachabilityResult::Unreachable { .. } => Verdict::False,
                ReachabilityResult::Inconclusive => Verdict::Unknown,
            })
        }
        // Reachability in an unbounded net is not decided by the coverability graph: `unknown`.
        (Property::Reach, Mode::General) => {
            let target = params.target.clone().expect("target checked in run");
            via_reachability_graph(sys, |rg| rg.contains_marking(target).into())
        }
        (Property::Reach | Property::Cover, Mode::Poly) => Outcome::new(Verdict::NotSupported),

        (Property::Cover, Mode::Dispatch) => {
            let target = params.target.clone().expect("target checked in run");
            Outcome::new(match sys.analyze_coverability_within(target, long_timeout, None) {
                CoverabilityResult::Coverable { .. } => Verdict::True,
                CoverabilityResult::Uncoverable { .. } => Verdict::False,
                CoverabilityResult::Inconclusive => Verdict::Unknown,
            })
        }
        (Property::Cover, Mode::General) => {
            let target = params.target.clone().expect("target checked in run");
            match sys.try_build_reachability_graph() {
                Ok(rg) => Outcome {
                    verdict: rg.cover(target).is_some().into(),
                    states: Some(rg.marking_count()),
                    edges: Some(rg.transition_count()),
                },
                // Unbounded: the Karp-Miller coverability graph decides coverability exactly.
                Err(_) => {
                    let cg = sys.build_coverability_graph();
                    let omega_target: Marking<Omega> = target
                        .into_iter()
                        .map(|(p, n)| (p, Omega::from(n)))
                        .collect();
                    Outcome {
                        verdict: cg.cover(omega_target).is_some().into(),
                        states: Some(cg.marking_count()),
                        edges: Some(cg.transition_count()),
                    }
                }
            }
        }
    }
}

/// Parses a flat JSON object of string keys to non-negative integers, e.g. `{"p1": 2, "p7": 1}`.
fn parse_target_json(text: &str) -> Result<Vec<(String, u32)>, String> {
    let mut it = text.chars().peekable();
    fn ws(it: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        while it.next_if(|c| c.is_whitespace()).is_some() {}
    }
    fn expect(it: &mut std::iter::Peekable<std::str::Chars<'_>>, want: char) -> Result<(), String> {
        ws(it);
        match it.next() {
            Some(c) if c == want => Ok(()),
            other => Err(format!("expected '{want}', found {other:?}")),
        }
    }
    fn string(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, String> {
        expect(it, '"')?;
        let mut out = String::new();
        loop {
            match it.next().ok_or("unterminated string")? {
                '"' => return Ok(out),
                '\\' => match it.next().ok_or("unterminated escape")? {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'u' => {
                        let hex: String = it.by_ref().take(4).collect();
                        let code = u32::from_str_radix(&hex, 16).map_err(|_| "bad \\u escape")?;
                        out.push(char::from_u32(code).ok_or("unsupported \\u escape")?);
                    }
                    c => return Err(format!("unsupported escape \\{c}")),
                },
                c => out.push(c),
            }
        }
    }
    expect(&mut it, '{')?;
    let mut entries: Vec<(String, u32)> = Vec::new();
    ws(&mut it);
    if it.next_if_eq(&'}').is_none() {
        loop {
            let key = string(&mut it)?;
            expect(&mut it, ':')?;
            ws(&mut it);
            let mut digits = String::new();
            while let Some(c) = it.next_if(char::is_ascii_digit) {
                digits.push(c);
            }
            let n: u32 = digits.parse().map_err(|_| format!("token count for \"{key}\" is not a non-negative integer"))?;
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(format!("duplicate place id \"{key}\""));
            }
            entries.push((key, n));
            ws(&mut it);
            match it.next() {
                Some(',') => {}
                Some('}') => break,
                other => return Err(format!("expected ',' or '}}', found {other:?}")),
            }
        }
    }
    ws(&mut it);
    if it.next().is_some() {
        return Err("trailing characters after object".to_owned());
    }
    Ok(entries)
}

/// Reads the target file and resolves PNML place ids to places. Unknown ids are an error.
fn load_target(sys: &PetriNet<Net>, file: &PathBuf) -> Result<Marking<u32>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let entries = parse_target_json(&text).map_err(|e| format!("invalid target {}: {e}", file.display()))?;
    let labels = sys.labels.as_deref();
    let ids: HashMap<&str, Place> = sys
        .places()
        .filter_map(|p| labels.and_then(|l| l.place_id(p)).map(|id| (id, p)))
        .collect();
    entries
        .iter()
        .map(|(id, n)| {
            ids.get(id.as_str())
                .map(|&p| (p, *n))
                .ok_or_else(|| format!("unknown place id \"{id}\" in target"))
        })
        .collect()
}

fn name<T: ValueEnum>(value: &T) -> String {
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

fn run(
    file: &PathBuf,
    property: Property,
    mode: Mode,
    timeout_ms: Option<u64>,
    target_file: Option<&PathBuf>,
    json: bool,
) -> Result<(), String> {
    let start = Instant::now();
    let pnml = std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let sys = PetriNet::from_pnml(&pnml).map_err(|e| e.to_string())?;
    let target = match (property, target_file) {
        (Property::Reach | Property::Cover, None) => return Err("--target is required for reach and cover".to_owned()),
        (Property::Reach | Property::Cover, Some(f)) => Some(load_target(&sys, f)?),
        _ => None,
    };
    let parse_ms = ms(start);
    let params = Params { timeout: timeout_ms.map(Duration::from_millis), target };

    let start = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| analyze(&sys, property, mode, &params)))
        .map_err(|_| "analysis panicked".to_owned())?;
    let analyze_ms = ms(start);

    let class = format!("{:?}", sys.class());
    if json {
        let mut line = String::new();
        write!(
            line,
            "{{\"tool\":\"petrivet\",\"version\":{},\"file\":{},\"property\":{},\"mode\":{},\"verdict\":{},\
             \"class\":{},\"places\":{},\"transitions\":{},\"arcs\":{},\"parse_ms\":{parse_ms:.3},\"analyze_ms\":{analyze_ms:.3},\"timeout_ms\":{}",
            json_str(env!("CARGO_PKG_VERSION")),
            json_str(&file.display().to_string()),
            json_str(&name(&property)),
            json_str(&name(&mode)),
            json_str(outcome.verdict.as_str()),
            json_str(&class),
            sys.place_count(),
            sys.transition_count(),
            sys.arc_count(),
            timeout_ms.map_or_else(|| "null".to_owned(), |n| n.to_string()),
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
    let Command::Analyze { file, property, mode, timeout_ms, target, json } = Cli::parse().command;
    match run(&file, property, mode, timeout_ms, target.as_ref(), json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            if json {
                println!(
                    "{{\"tool\":\"petrivet\",\"file\":{},\"property\":{},\"mode\":{},\"verdict\":\"error\",\"error\":{}}}",
                    json_str(&file.display().to_string()),
                    json_str(&name(&property)),
                    json_str(&name(&mode)),
                    json_str(&message),
                );
            } else {
                eprintln!("error: {message}");
            }
            ExitCode::FAILURE
        }
    }
}
