use petrivet::behavior::PetriNet;
use petrivet::structure::builder::NetBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = NetBuilder::new();
    let [s1, s2, s3, s4] = builder.add_places();
    let [t1, t2, t3] = builder.add_transitions();
    builder.add_arc((s2, t1));
    builder.add_arc((t1, s3));
    builder.add_arc((s3, t2));
    builder.add_arc((t2, s2));
    builder.add_arc((s3, t3));
    builder.add_arc((t3, s2));
    builder.add_arc((t3, s1));
    builder.add_arc((s1, t2));
    builder.add_arc((t2, s4));
    builder.add_arc((s4, t3));
    let net = builder.build()?.into_inner();

    let initial_marking = (1, 1, 0, 0).into();

    println!("Initial marking: {initial_marking}");

    let mut petri_net = PetriNet::from((&net, initial_marking));

    for (from, over, to) in petri_net.reachability_iter().dfs() {
        println!("From: {from} --[{over}]--> To: {to}");
    }
    Ok(())
}