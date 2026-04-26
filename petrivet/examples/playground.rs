use petrivet::System;

fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/Champagne/PT/champagne_H04_T1U.pnml"
    );
    let champagne = std::fs::read_to_string(path).unwrap();
    // println!("PNML content:\n{}", champagne);

    let system = System::from_pnml(&champagne).unwrap();
    println!("{:?}", system.initial_marking());
    let enabled = system.enabled_transitions().collect::<Box<_>>();
    println!("{enabled:?}");

    let cg = system.analyze_boundedness();
    println!("{cg:?}");
}