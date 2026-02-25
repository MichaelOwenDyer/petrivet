use petrivet::explorer::ExplorationOrder;
use petrivet::net::Net;
use petrivet::reachability::ReachabilityExplorer;
use petrivet::system::System;

fn main() {
    let mut net = Net::builder();
    let [p1, p2, p3] = net.add_places();
    let [t1, t2] = net.add_transitions();
    net.add_arc((p1, t1));
    net.add_arc((t1, p1));
    net.add_arc((t1, p2));
    net.add_arc((p1, t2));
    net.add_arc((t2, p2));
    net.add_arc((p2, t2));
    net.add_arc((t2, p3));
    let net = net.build().expect("valid net");
    let sys = System::new(net, [2, 0, 0]);
    let mut explorer = ReachabilityExplorer::new(&sys, ExplorationOrder::BreadthFirst);

    for s in explorer.iter().take(1000) {
        println!("{s:#?}");
    }
}