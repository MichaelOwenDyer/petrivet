//! Sidecar for `dipn_io`: owns the physical system state and communicates
//! with the Petri net simulation over stdin/stdout pipes.
//!
//! Protocol (one JSON object per line in each direction):
//!   sidecar  -> dipn_io : `{"lit101": f64, "mv101": bool, "p101": bool}`
//!   dipn_io  -> sidecar : `{"enabled":[..], "fired":.., "duration_secs":.., "marking":[..]}`
//!
//! Each round the sidecar:
//! 1. Sends the current system state.
//! 2. Reads which transition fired and for how long.
//! 3. Updates actuator state based on the fired transition.
//! 4. Advances the physics simulation by the transition's duration.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

const INLET_RATE: f64 = 150.0;  // mm/s added when valve open
const PUMP_RATE: f64 = 80.0;    // mm/s removed when pump running
const LOSS_RATE: f64 = 30.0;    // mm/s natural evaporation/leakage
const TANK_MAX: f64 = 1000.0;
const DEFAULT_DT: f64 = 1.0;    // seconds to advance when no transition fires

#[derive(Debug, Clone, Serialize)]
struct SystemState {
    lit101: f64,
    mv101: bool,
    p101: bool,
}

#[derive(Debug, Deserialize)]
struct StepOutput {
    enabled: Vec<String>,
    fired: Option<String>,
    duration_secs: Option<f64>,
    marking: Vec<i32>,
}

fn simulate_physics(state: &mut SystemState, dt: f64) {
    if state.mv101 {
        state.lit101 += INLET_RATE * dt;
    }
    if state.p101 {
        state.lit101 -= PUMP_RATE * dt;
    }
    state.lit101 -= LOSS_RATE * dt;
    state.lit101 = state.lit101.clamp(0.0, TANK_MAX);
}

fn apply_transition(state: &mut SystemState, name: &str) {
    match name {
        "open_mv101" => state.mv101 = true,
        "close_mv101" => state.mv101 = false,
        "start_p101" => state.p101 = true,
        "stop_p101" => state.p101 = false,
        other => eprintln!("Unknown transition: {other}"),
    }
}

fn main() {
    let mut child = Command::new("cargo")
        .args(["run", "--example", "dipn_io"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn dipn_io");

    let mut child_in = child.stdin.take().expect("no stdin on child");
    let mut child_out = BufReader::new(child.stdout.take().expect("no stdout on child"));

    let mut state = SystemState {
        lit101: 600.0,
        mv101: false,
        p101: false,
    };

    let mut sim_time = 0.0_f64;

    println!("=== Water Treatment Physics Sidecar ===");
    println!("Initial state: {state:?}\n");

    for step in 1..=250 {
        println!("--- Step {step}  (t = {sim_time:.2}s) ---");
        println!("  state  -> dipn:  lit101={:.1}mm  mv101={}  p101={}",
            state.lit101,
            if state.mv101 { "open" } else { "closed" },
            if state.p101 { "on" } else { "off" },
        );

        serde_json::to_writer(&mut child_in, &state).unwrap();
        writeln!(child_in).unwrap();
        child_in.flush().unwrap();

        let mut line = String::new();
        if child_out.read_line(&mut line).unwrap() == 0 {
            eprintln!("dipn_io closed its stdout unexpectedly");
            break;
        }

        let resp: StepOutput = serde_json::from_str(line.trim()).unwrap_or_else(|e| {
            panic!("bad response from dipn_io: {e}\nline: {line}");
        });

        println!("  dipn   -> sidecar:  enabled={:?}", resp.enabled);

        let dt = if let Some(ref name) = resp.fired {
            let dur = resp.duration_secs.unwrap_or(0.0);
            println!("  fired  {name}  (duration {dur:.3}s)");
            apply_transition(&mut state, name);
            if dur > 0.0 { dur } else { DEFAULT_DT }
        } else {
            println!("  (no transition fired)");
            DEFAULT_DT
        };

        simulate_physics(&mut state, dt);
        sim_time += dt;

        println!("  physics  dt={dt:.3}s  ->  lit101={:.1}mm", state.lit101);
        println!("  marking  {:?}\n", resp.marking);
    }

    drop(child_in);
    let status = child.wait().expect("failed to wait on dipn_io");
    println!("=== Simulation complete (dipn_io exited with {status}) ===");
}
