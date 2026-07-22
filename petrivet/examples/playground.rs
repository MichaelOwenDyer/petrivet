use petrivet::pnml::labels::NetLabels;
use petrivet::prelude::{Marking, PetriNet};

fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/CopsAndRobbers-PT-Heawood-Random-L014X002.pnml"
    );
    let champagne = std::fs::read_to_string(path).unwrap();

    let system = PetriNet::from_pnml(&champagne).unwrap();
    println!("{:?}", system.marking());
    let enabled = system.enabled_transitions().collect::<Vec<_>>();
    println!("{enabled:?}");

    let rg = system.build_reachability_graph();
    println!("Marking count: {}", rg.marking_count());
    println!("Transition count: {}", rg.transition_count());
    for marking in rg.markings() {
        println!(
            "Marking: {}",
            Display(marking, system.labels.as_ref().unwrap())
        );
    }
    println!("Max tokens per place: {}", rg.max_token_in_any_place());
    println!("Max tokens per marking: {}", rg.max_token_per_marking());
}

struct Display<'a>(Marking<u32>, &'a NetLabels);

impl std::fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{")?;
        for place in self.0.support() {
            let id = self.1.place_id(place).unwrap_or("unnamed");
            let tokens = self.0.get(place);
            write!(f, "{id}: {tokens}, ")?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}
