//! Example: DIPN Water Treatment with stdin/stdout JSON I/O
//!
//! Same Petri net structure as `dipn_water_treatment.rs`, but instead of
//! simulating physics internally, the system state is read from stdin as
//! one JSON object per line. Enabled transitions are evaluated, the first
//! is fired, and the result (including sampled transition duration) is
//! written to stdout as JSON. The loop continues until stdin is closed.

use petrivet::dipn::{Marking, NetBuilder, Transition, TransitionTiming};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::io::{self, BufRead, Write};
use std::rc::Rc;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[expect(dead_code)]
struct SystemState {
    lit101: f64,
    mv101: bool,
    p101: bool,
}

#[derive(Debug, Serialize)]
struct StepOutput {
    enabled: Vec<String>,
    fired: Option<String>,
    duration_secs: Option<f64>,
    marking: Vec<i32>,
}

fn main() {
    let state = Rc::new(RefCell::new(SystemState {
        lit101: 0.0,
        mv101: false,
        p101: false,
    }));

    let mut builder = NetBuilder::new();

    let [mv101_closed, mv101_open] = builder.add_places();
    let [p101_off, p101_on] = builder.add_places();

    let t_open_mv101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 < 500.0
        })
        .timing(TransitionTiming::Range {
            min: Duration::from_secs(1),
            max: Duration::from_secs(3),
        })
        .build();

    let t_close_mv101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 > 800.0
        })
        .timing(TransitionTiming::Range {
            min: Duration::from_secs(1),
            max: Duration::from_secs(3),
        })
        .build();

    let t_start_p101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 > 300.0 && s.borrow().mv101
        })
        .timing(TransitionTiming::Fixed(Duration::from_secs(2)))
        .build();

    let t_stop_p101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 < 200.0
        })
        .build();

    builder.add_arc((mv101_closed, t_open_mv101));
    builder.add_arc((t_open_mv101, mv101_open));
    builder.add_arc((mv101_open, t_close_mv101));
    builder.add_arc((t_close_mv101, mv101_closed));

    builder.add_arc((p101_off, t_start_p101));
    builder.add_arc((t_start_p101, p101_on));
    builder.add_arc((p101_on, t_stop_p101));
    builder.add_arc((t_stop_p101, p101_off));

    let net = builder.build().expect("Failed to build net");

    let mut marking = Marking::from([1, 0, 1, 0]);

    let transitions: &[(Transition, &str)] = &[
        (t_open_mv101, "open_mv101"),
        (t_close_mv101, "close_mv101"),
        (t_start_p101, "start_p101"),
        (t_stop_p101, "stop_p101"),
    ];

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };

        let new_state: SystemState = match serde_json::from_str(&line) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse input: {e}");
                continue;
            }
        };

        *state.borrow_mut() = new_state;

        let enabled: Vec<String> = transitions
            .iter()
            .filter(|(t, _)| net.is_enabled(&marking, *t))
            .map(|(_, name)| (*name).to_owned())
            .collect();

        let (fired, duration_secs) = match transitions
            .iter()
            .find(|(t, _)| net.is_enabled(&marking, *t))
        {
            Some(&(t, name)) => {
                let duration = net.fire(&mut marking, t).unwrap();
                (Some(name.to_owned()), Some(duration.as_secs_f64()))
            }
            None => (None, None),
        };

        let marking = marking.iter().copied().collect();

        let output = StepOutput {
            enabled,
            fired,
            duration_secs,
            marking,
        };

        serde_json::to_writer(&mut stdout, &output).unwrap();
        writeln!(stdout).unwrap();
        stdout.flush().unwrap();
    }
}
