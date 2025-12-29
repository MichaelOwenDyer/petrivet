//! Demonstration of the specialized analysis architecture.
//!
//! This example shows how the trait-based analysis system automatically
//! selects the most efficient algorithm based on the structural class
//! of the Petri net.

use petrivet::analysis::boundedness::BoundednessAnalysis;
use petrivet::analysis::liveness::LivenessAnalysis;
use petrivet::analysis::reachability::ReachabilityAnalysis;
use petrivet::analysis::{ClassifiedSystem, System};
use petrivet::structure::builder::NetBuilder;
use petrivet::structure::class::{SNet, StructureClass};

fn main() {
    println!("=== Petri Net Analysis Architecture Demo ===\n");

    // Build a simple S-net (state machine)
    // This is a producer-consumer pattern with a single token
    //
    //    ┌───┐     ┌───┐     ┌───┐
    //    │p0 │────▶│t0 │────▶│p1 │
    //    └───┘     └───┘     └───┘
    //      ▲                   │
    //      │       ┌───┐       │
    //      └───────│t1 │◀──────┘
    //              └───┘
    //
    let mut builder = NetBuilder::new();
    let [p0, p1] = builder.add_places();
    let [t0, t1] = builder.add_transitions();
    builder.add_arc((p0, t0));
    builder.add_arc((t0, p1));
    builder.add_arc((p1, t1));
    builder.add_arc((t1, p0));

    let structure_class = builder.build().expect("valid net");
    println!("Built net: {:?}", std::mem::discriminant(&structure_class));

    // Initial marking: one token in p0
    let initial_marking = (1, 0);

    // ==========================================================================
    // Approach 1: Statically-typed specialized analysis
    // ==========================================================================
    println!("\n--- Approach 1: Static Typing ---");

    // If we know the structure class at compile time, we get the specialized
    // implementation automatically through trait dispatch.
    if let StructureClass::Circuit(circuit) = structure_class.clone() {
        let system = System::new(circuit, initial_marking);

        // This calls Circuit's O(|S|) liveness check
        println!("Circuit liveness: {:?}", system.is_live());
        println!("Circuit boundedness: {:?}", system.boundedness());

        // Target marking: token in p1
        let target = (0, 1).into();
        println!("Reachability of (0,1): {:?}", system.is_reachable(&target));
    }

    // ==========================================================================
    // Approach 2: Runtime-dispatched analysis via ClassifiedSystem
    // ==========================================================================
    println!("\n--- Approach 2: Runtime Dispatch ---");

    // If we don't know the structure class until runtime, we can use
    // ClassifiedSystem which dispatches to the correct implementation.
    let classified = ClassifiedSystem::new(structure_class.clone(), initial_marking);

    println!("Net class: {}", classified.class_name());
    println!("Liveness: {:?}", classified.is_live());
    println!("Boundedness: {:?}", classified.boundedness());

    // ==========================================================================
    // Approach 3: Explicit type conversion for known subclasses
    // ==========================================================================
    println!("\n--- Approach 3: TryFrom Conversion ---");

    // You can also convert a general Net into a specialized type
    let net = structure_class.into_inner();

    // This checks if the net satisfies S-net properties
    match SNet::try_from(net.clone()) {
        Ok(s_net) => {
            let system = System::new(s_net, initial_marking.clone());
            // Now we get S-net's specialized O(|S|+|T|+|F|) algorithm
            println!("S-net liveness: {:?}", system.is_live());
            println!("S-net boundedness: {:?}", system.boundedness());
        }
        Err(_net) => {
            println!("Net is not an S-net");
        }
    }

    // ==========================================================================
    // Demonstrate the analysis hierarchy
    // ==========================================================================
    println!("\n--- Analysis Complexity Hierarchy ---");
    println!("
╔═══════════════════╦═══════════════════════════════════════════════════════╗
║ Structural Class  ║ Analysis Complexity                                   ║
╠═══════════════════╬═══════════════════════════════════════════════════════╣
║ Circuit           ║ Liveness: O(|S|)     Boundedness: O(|S|)              ║
║ S-net             ║ Liveness: O(V+E)     Boundedness: O(|S|)              ║
║ T-net             ║ Liveness: O(poly)    Boundedness: O(V+E)              ║
║ Free-choice       ║ Liveness: O(poly)    Boundedness: O(poly)             ║
║                   ║ Reachability: O(poly) ← This is remarkable!           ║
║ Unrestricted      ║ Liveness: EXPSPACE   Boundedness: EXPSPACE            ║
║                   ║ Reachability: Ackermann-complete                      ║
╚═══════════════════╩═══════════════════════════════════════════════════════╝
");

    // ==========================================================================
    // Build an unrestricted net for comparison
    // ==========================================================================
    println!("--- Unrestricted Net Example ---");

    // Non-free-choice: p0 feeds both t0 and t1, but they have different presets
    //
    //    ┌───┐
    //    │p0 │──┬──▶t0──▶p2
    //    └───┘  │
    //           └──▶t1
    //    ┌───┐      │
    //    │p1 │──────┘──▶p3
    //    └───┘
    //
    let mut builder = NetBuilder::new();
    let [p0, p1, p2, p3] = builder.add_places();
    let [t0, t1] = builder.add_transitions();
    builder.add_arc((p0, t0));
    builder.add_arc((t0, p2));
    builder.add_arc((p0, t1));
    builder.add_arc((p1, t1));
    builder.add_arc((t1, p3));

    let unrestricted_class = builder.build().expect("valid net");
    println!("Built unrestricted net");

    let classified = ClassifiedSystem::new(
        unrestricted_class,
        (1, 1, 0, 0),
    );

    println!("Net class: {}", classified.class_name());
    // For unrestricted nets, specialized algorithms aren't available
    println!("Liveness: {:?}", classified.is_live());
    println!("Boundedness: {:?}", classified.boundedness());

    println!("\n=== Demo Complete ===");
}
