use petrivet::structure::builder::NetBuilder;
use petrivet::behavior::{Marking, Tokens};

fn main() {
    let mut builder = NetBuilder::<u8>::new();
    let [p0, p1] = builder.add_places();
    let [t0, t1] = builder.add_transitions();
    builder.add_arc((p0, t0));
    builder.add_arc((t0, p1));
    builder.add_arc((p1, t1));
    builder.add_arc((t1, p0));
    
    let net = builder.build().unwrap().into_inner();
    println!("Net: {:#?}", net);
    
    // Create an initial marking
    let mut marking = Marking::with_places(2);
    marking.set(p0, Tokens(1));
    marking.set(p1, Tokens(0));
    
    println!("Initial marking: {:?}", marking);
    
    // Test the coverability graph
    let mut coverability = petrivet::behavior::model::CoverabilityGraph::new(&marking);
    println!("Coverability graph created with {} nodes", coverability.seen_nodes.len());
}