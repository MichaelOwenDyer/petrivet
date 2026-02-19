//! Example: Simplified Water Treatment System using DIPN
//!
//! This demonstrates a Data-Interpreted Petri Net modeling a simplified
//! water treatment system similar to Hussain et al. (2023).
//!
//! The system has:
//! - A raw water tank with level sensor
//! - An inlet valve (MV101) controlled by water level
//! - A pump (P101) that transfers water
//!
//! Transitions have guards that check continuous sensor values
//! and actions (junction functions) that update actuator states.

use petrivet::dipn::{Marking, NetBuilder};
use std::cell::RefCell;
use std::rc::Rc;

/// Simulated sensor/actuator state shared between guards and actions.
#[derive(Debug, Clone)]
struct SystemState {
    /// Water level in raw water tank (mm)
    lit101: f64,
    /// Inlet valve state: true = open
    mv101: bool,
    /// Pump state: true = running
    p101: bool,
}

impl Default for SystemState {
    fn default() -> Self {
        Self {
            lit101: 600.0, // Start at middle level
            mv101: false,
            p101: false,
        }
    }
}

fn main() {
    // Shared state that guards and actions can access
    let state = Rc::new(RefCell::new(SystemState::default()));

    // Build the DIPN
    let mut builder = NetBuilder::new();

    // Places represent discrete states of the system
    // For MV101 (inlet valve):
    let [mv101_closed, mv101_open] = builder.add_places();
    // For P101 (pump):
    let [p101_off, p101_on] = builder.add_places();

    // Transitions with guards (checking sensor values) and actions (setting actuator states)

    // t1: Open MV101 when water level is low (< 500mm)
    let t_open_mv101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 < 500.0
        })
        .action({
            let s = Rc::clone(&state);
            move || {
                s.borrow_mut().mv101 = true;
                println!("ACTION: MV101 opened (inlet valve on)");
            }
        })
        .build();

    // t2: Close MV101 when water level is high (> 800mm)
    let t_close_mv101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 > 800.0
        })
        .action({
            let s = Rc::clone(&state);
            move || {
                s.borrow_mut().mv101 = false;
                println!("ACTION: MV101 closed (inlet valve off)");
            }
        })
        .build();

    // t3: Start P101 when water level is sufficient (> 300mm) and MV101 is open
    let t_start_p101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 > 300.0 && s.borrow().mv101
        })
        .action({
            let s = Rc::clone(&state);
            move || {
                s.borrow_mut().p101 = true;
                println!("ACTION: P101 started (pump on)");
            }
        })
        .build();

    // t4: Stop P101 when water level is too low (< 200mm)
    let t_stop_p101 = builder
        .add_transition()
        .guard({
            let s = Rc::clone(&state);
            move || s.borrow().lit101 < 200.0
        })
        .action({
            let s = Rc::clone(&state);
            move || {
                s.borrow_mut().p101 = false;
                println!("ACTION: P101 stopped (pump off)");
            }
        })
        .build();

    // Arcs define the Petri net structure
    // MV101 state machine
    builder.add_arc((mv101_closed, t_open_mv101));
    builder.add_arc((t_open_mv101, mv101_open));
    builder.add_arc((mv101_open, t_close_mv101));
    builder.add_arc((t_close_mv101, mv101_closed));

    // P101 state machine
    builder.add_arc((p101_off, t_start_p101));
    builder.add_arc((t_start_p101, p101_on));
    builder.add_arc((p101_on, t_stop_p101));
    builder.add_arc((t_stop_p101, p101_off));

    let net = builder.build().expect("Failed to build net");

    // Initial marking: MV101 closed, P101 off
    let mut marking = Marking::from([1, 0, 1, 0]);

    println!("=== DIPN Water Treatment Simulation ===\n");
    println!("Places: mv101_closed, mv101_open, p101_off, p101_on");
    println!("Initial marking: {marking}");
    println!("Initial state: {:?}\n", state.borrow());

    // Simulation loop
    let transitions = [t_open_mv101, t_close_mv101, t_start_p101, t_stop_p101];
    let transition_names = ["open_mv101", "close_mv101", "start_p101", "stop_p101"];

    for step in 1..=25 {
        println!("--- Step {step} ---");
        println!("Water level: {:.1}mm", state.borrow().lit101);
        println!("Marking: {marking}");

        // Find enabled transitions
        let enabled: Vec<_> = net.enabled_transitions(&marking).collect();
        print!("Enabled transitions: ");
        if enabled.is_empty() {
            println!("none");
        } else {
            for t in &enabled {
                let idx = transitions.iter().position(|x| x == t).unwrap();
                print!("{} ", transition_names[idx]);
            }
            println!();
        }

        // Fire the first enabled transition (if any)
        if let Some(&transition) = enabled.first() {
            let idx = transitions.iter().position(|x| x == &transition).unwrap();
            println!("Firing: {}", transition_names[idx]);
            net.fire(&mut marking, transition).unwrap();
        }

        // Simulate continuous dynamics (water level changes)
        simulate_physics(&mut state.borrow_mut());

        println!("State after: {:?}\n", state.borrow());
    }

    println!("=== Simulation Complete ===");
}

/// Simple physics simulation
fn simulate_physics(state: &mut SystemState) {
    // Inlet adds water when valve is open
    if state.mv101 {
        state.lit101 += 150.0;
    }
    // Pump removes water when running
    if state.p101 {
        state.lit101 -= 80.0;
    }
    // Natural evaporation/leakage
    state.lit101 -= 30.0;
    // Clamp to valid range
    state.lit101 = state.lit101.clamp(0.0, 1000.0);
}
