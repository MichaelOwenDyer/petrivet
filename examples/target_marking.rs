use petrivet::behavior::{OmegaMarking, PetriNet};
use petrivet::structure::builder::NetBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = NetBuilder::new();
    let [p0, p1, p2] = builder.add_places();
    let [t0, t1] = builder.add_transitions();
    builder.add_arc((p0, t0));
    builder.add_arc((t0, p1));
    builder.add_arc((t0, p2));
    builder.add_arc((p1, t1));
    builder.add_arc((t1, p0));
    let net = builder.build()?.into_inner();

    let initial_marking = (1, 0, 0).into();

    println!("Initial marking: {initial_marking}");

    let mut petri_net: PetriNet = (&net, initial_marking).into();

    let target: OmegaMarking = (0, 1, 100).into();

    // TODO: Reduce lifetime of mutable borrow to avoid clone
    for (from, over, to) in petri_net.clone().coverability_iter().dfs() {
        println!("From: {from} --[{over}]--> To: {to}");
        if target <= to {
            println!("Covered target marking: {to} >= {target}");
            break;
        }
    }
    
    for (from, over, to) in petri_net.reachability_iter().dfs() {
        println!("From: {from} --[{over}]--> To: {to}");
        if target == to {
            println!("Reached target marking: {to} == {target}");
            break;
        }
    }
    Ok(())
}