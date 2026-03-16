use petrivet::net::Net;
use petrivet::system::System;
use petrivet::ExplorationOrder::BreadthFirst;

fn main() {
    let mut b = Net::builder();
    let [p1, p2, p3, p4, p5] = b.add_places();
    let [t1, t2, t3, t4] = b.add_transitions();
    b.add_arcs((t1, p1, t2, p3, t4));
    b.add_arcs((t1, p2, t3, p4, t4));
    b.add_arcs((t4, p5, t1));
    let net = b.build().unwrap();
    let system = System::new(net, [0, 0, 0, 0, 1]);

    for s in system.explore_reachability(BreadthFirst).iter() {
        println!("{s:#?}");
    }
}