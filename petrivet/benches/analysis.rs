//! A broad, quick view of petrivet's performance.
//!
//! Every public analysis is measured on a spread of nets: the structure-class generators
//! (cheap, structural procedures), scalable models with large state spaces (exploration),
//! and real models from the Model Checking Contest (PNML import and analyses). Instances are
//! kept small enough that one run of everything takes a few minutes; depth comes from
//! breadth, not from long measurements.
//!
//! Run everything, or a part by name (a regular expression over `group/instance`):
//!
//! ```text
//! cargo bench --bench analysis
//! cargo bench --bench analysis -- liveness
//! cargo bench --bench analysis -- 'state_space/.*philosophers'
//! cargo bench --bench analysis -- --save-baseline before    # then: --baseline before
//! ```
//!
//! **Fair measurements.** Analyses cache their results on the system, so every iteration
//! gets a fresh system (built outside the measured time, borrowing the same net).
//!
//! **No hangs.** Before an analysis is measured on an instance, it runs once with a time
//! limit ([`GUARD`]). If it gives no answer within the limit, the instance is skipped for
//! that analysis and reported on standard error, so that one slow case cannot stall the
//! whole run. The answer of that first run is printed too, as a quick plausibility check.

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use petrivet::cost_control::Effort;
use petrivet::net::Net;
use petrivet::net::builder::NetBuilder;
use petrivet::net::{Place, Transition};
use petrivet::pnml::PnmlDocument;
use petrivet::system::PetriNet;
use petrivet::system::marking::Marking;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// How long an analysis may take on its first, unmeasured run before the instance is
/// skipped for it.
const GUARD: Duration = Duration::from_secs(1);

/// A net with an initial marking, and optionally a marking to ask reachability and
/// coverability questions about.
struct Instance {
    name: String,
    net: Net,
    initial: Marking<u32>,
    target: Option<Marking<u32>>,
}

impl Instance {
    fn new(name: impl Into<String>, system: PetriNet, target: Option<Marking<u32>>) -> Self {
        let initial = system.marking();
        Self {
            name: name.into(),
            net: system.net,
            initial,
            target,
        }
    }

    /// A fresh system on this instance's net: no cached analysis results.
    fn system(&self) -> PetriNet<&Net> {
        PetriNet::new(&self.net, self.initial.clone())
    }
}

/// Measures `analysis` on every instance in `group`, after a guarded first run.
fn bench_analysis<A: std::fmt::Debug>(
    c: &mut Criterion,
    group: &str,
    instances: &[Instance],
    analysis: impl Fn(&PetriNet<&Net>, Option<Instant>) -> Option<A>,
) {
    let mut g = c.benchmark_group(group);
    for instance in instances {
        let id = format!("{group}/{}", instance.name);
        if !c_filter_matches(&id) {
            continue;
        }
        let started = Instant::now();
        let answer = analysis(&instance.system(), Some(started + GUARD));
        if answer.is_none() && started.elapsed() >= GUARD {
            eprintln!("skipped {id}: no answer within {GUARD:?}");
            continue;
        }
        eprintln!("{id}: {answer:?}");
        g.bench_function(BenchmarkId::from_parameter(&instance.name), |b| {
            b.iter_batched(
                || instance.system(),
                |system| black_box(analysis(&system, None)),
                BatchSize::SmallInput,
            );
        });
    }
    g.finish();
}

/// Whether the command line selects `id`, so that the guard runs are skipped for analyses
/// that are filtered out anyway. Criterion applies its own filter to the measurements.
fn c_filter_matches(id: &str) -> bool {
    // Criterion options that take a value; that value is not a filter.
    const WITH_VALUE: &[&str] = &[
        "--save-baseline",
        "--baseline",
        "--baseline-lenient",
        "--load-baseline",
        "--measurement-time",
        "--warm-up-time",
        "--sample-size",
        "--nresamples",
        "--noise-threshold",
        "--confidence-level",
        "--significance-level",
        "--output-format",
        "--plotting-backend",
        "--profile-time",
        "--color",
        "-c",
    ];
    let mut filters = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if WITH_VALUE.contains(&arg.as_str()) {
            args.next();
        } else if !arg.starts_with('-') {
            filters.push(arg);
        }
    }
    filters.is_empty() || filters.iter().any(|f| regex_lite_match(f, id))
}

/// A small subset of regular expressions, enough for filters: `.*` and `.` wildcards, `^`
/// anchors, alternatives with `|`; anything else matches literally.
fn regex_lite_match(pattern: &str, text: &str) -> bool {
    fn here(p: &[u8], t: &[u8]) -> bool {
        match p {
            [] => true,
            [b'$'] => t.is_empty(),
            [b'.', b'*', rest @ ..] => (0..=t.len()).any(|i| here(rest, &t[i..])),
            [c, rest @ ..] => !t.is_empty() && (*c == b'.' || *c == t[0]) && here(rest, &t[1..]),
        }
    }
    pattern.split('|').any(|alternative| {
        let (anchored, p) = match alternative.strip_prefix('^') {
            Some(p) => (true, p.as_bytes()),
            None => (false, alternative.as_bytes()),
        };
        let t = text.as_bytes();
        if anchored {
            here(p, t)
        } else {
            (0..=t.len()).any(|i| here(p, &t[i..]))
        }
    })
}

/// Builds a net of a given size.
type Generator = fn(usize) -> PetriNet;

/// The structure-class generators: small nets whose properties the class-specific
/// procedures decide cheaply, at two sizes.
fn class_instances() -> Vec<Instance> {
    let generators: [(&str, Generator); 8] = [
        ("circuit", circuit),
        ("state_machine_sc", state_machine_sc),
        ("state_machine_non_sc", state_machine_non_sc),
        ("marked_graph_live", marked_graph_live),
        ("marked_graph_dark", marked_graph_dark),
        ("free_choice", free_choice),
        ("asym_choice", asym_choice),
        ("general", general),
    ];
    let mut instances = Vec::new();
    for (name, build) in generators {
        for size in [8, 64] {
            instances.push(Instance::new(format!("{name}/{size}"), build(size), None));
        }
    }
    instances
}

/// Models with large state spaces, at sizes that keep one exploration well below a second.
fn state_space_instances() -> Vec<Instance> {
    let mut instances = Vec::new();
    for n in [4, 6, 8] {
        let (system, deadlock) = philosophers(n);
        instances.push(Instance::new(
            format!("philosophers/{n}"),
            system,
            Some(deadlock),
        ));
    }
    for n in [8, 12] {
        instances.push(Instance::new(
            format!("independent/{n}"),
            independent(n),
            None,
        ));
    }
    for n in [2, 4] {
        let (system, target) = producer_consumer(n);
        instances.push(Instance::new(
            format!("producer_consumer/{n}"),
            system,
            Some(target),
        ));
    }
    instances
}

/// Models from the Model Checking Contest and classic examples, as PNML files, and whether
/// the behavioural analyses finish on them within the guard. The others (large models, and
/// `token-ring`, which has no initial marking) are only read, written and checked
/// structurally.
const PNML_FIXTURES: &[(&str, &str, bool)] = &[
    ("philo", include_str!("../tests/fixtures/philo.pnml"), true),
    (
        "swimming-pool",
        include_str!("../tests/fixtures/swimming-pool.pnml"),
        true,
    ),
    (
        "traffic",
        include_str!("../tests/fixtures/traffic.pnml"),
        true,
    ),
    (
        "train_tracks",
        include_str!("../tests/fixtures/train_tracks.pnml"),
        true,
    ),
    (
        "token-ring",
        include_str!("../tests/fixtures/token-ring.pnml"),
        false,
    ),
    (
        "champagne_H04_T0R",
        include_str!("../tests/fixtures/champagne_H04_T0R.pnml"),
        false,
    ),
    (
        "mainframe0-04-04",
        include_str!("../tests/fixtures/mainframe0-04-04.pnml"),
        false,
    ),
    (
        "medleyB01",
        include_str!("../tests/fixtures/medleyB01.pnml"),
        false,
    ),
    (
        "aslink_01_a",
        include_str!("../tests/fixtures/aslink_01_a.pnml"),
        false,
    ),
];

/// The PNML models; with `behavioural`, only those the behavioural analyses finish on.
fn pnml_instances(behavioural: bool) -> Vec<Instance> {
    PNML_FIXTURES
        .iter()
        .filter(|&&(_, _, analysed)| analysed || !behavioural)
        .filter_map(|&(name, xml, _)| {
            let document = PnmlDocument::from_xml(xml).ok()?;
            let system = document.nets.first()?.to_pt_system().ok()?;
            Some(Instance::new(name, system, None))
        })
        .collect()
}

/// Building nets, converting them back to builders, and reading and writing PNML.
fn bench_construction(c: &mut Criterion) {
    let mut g = c.benchmark_group("construction");
    for (name, build) in [
        ("general", general as Generator),
        ("free_choice", free_choice),
    ] {
        for size in [8, 64, 512] {
            g.bench_function(BenchmarkId::new(format!("build/{name}"), size), |b| {
                b.iter(|| black_box(build(size)));
            });
            g.bench_function(BenchmarkId::new(format!("unbuild/{name}"), size), |b| {
                b.iter_batched(
                    || build(size).net,
                    |net| black_box(NetBuilder::from(net)),
                    BatchSize::SmallInput,
                );
            });
        }
    }
    for &(name, xml, _) in PNML_FIXTURES {
        g.throughput(Throughput::Bytes(xml.len() as u64));
        g.bench_function(BenchmarkId::new("pnml_import", name), |b| {
            b.iter(|| {
                black_box(
                    PnmlDocument::from_xml(xml)
                        .ok()
                        .and_then(|d| d.nets[0].to_pt_system().ok()),
                )
            });
        });
    }
    g.throughput(Throughput::Elements(1));
    for instance in pnml_instances(false) {
        let system = instance.system();
        g.bench_function(BenchmarkId::new("pnml_export", &instance.name), |b| {
            b.iter(|| black_box(system.to_pnml()));
        });
    }
    g.finish();
}

/// Structural procedures: no state space involved.
fn bench_structural(c: &mut Criterion) {
    let mut instances = class_instances();
    instances.extend(pnml_instances(false));
    bench_analysis(c, "structural/structurally_bounded", &instances, |s, _| {
        Some(s.net.is_structurally_bounded())
    });
    bench_analysis(c, "structural/commoner", &instances, |s, deadline| {
        let remaining = deadline.map_or(Duration::MAX, |d| {
            d.saturating_duration_since(Instant::now())
        });
        s.commoner_hack_criterion_within(remaining)
            .map(|r| r.is_ok())
    });
    bench_analysis(c, "structural/unmarked_siphon", &instances, |s, _| {
        Some(s.contains_unmarked_proper_siphon())
    });
}

/// The property questions, through their dispatch (cheapest procedure first).
fn bench_properties(c: &mut Criterion) {
    let mut instances = class_instances();
    instances.extend(state_space_instances());
    instances.extend(pnml_instances(true));
    bench_analysis(c, "liveness", &instances, |s, deadline| {
        s.liveness().maybe_timeout(deadline).call()
    });
    bench_analysis(c, "liveness_efficient", &instances, |s, _| {
        Some(s.liveness().effort(Effort::Efficient).call())
    });
    bench_analysis(c, "boundedness", &instances, |s, deadline| {
        s.boundedness().maybe_timeout(deadline).call()
    });
    bench_analysis(c, "deadlock_freedom", &instances, |s, deadline| {
        s.is_deadlock_free().maybe_timeout(deadline).call()
    });
    let with_target: Vec<Instance> = instances
        .into_iter()
        .filter(|i| i.target.is_some())
        .collect();
    bench_analysis(c, "reachability", &with_target, |s, deadline| {
        let target = target_of(&with_target, s);
        s.is_reachable()
            .target(target)
            .maybe_timeout(deadline)
            .call()
    });
    bench_analysis(c, "coverability", &with_target, |s, deadline| {
        let target = target_of(&with_target, s);
        s.is_coverable()
            .target(target)
            .maybe_timeout(deadline)
            .call()
    });
}

/// The target marking of the instance whose net `system` borrows.
fn target_of(instances: &[Instance], system: &PetriNet<&Net>) -> Marking<u32> {
    instances
        .iter()
        .find(|i| std::ptr::eq(&i.net, system.net))
        .and_then(|i| i.target.clone())
        .expect("a system of an instance with a target")
}

/// Exploring whole state spaces: the reachability graph of bounded nets, the
/// coverability graph of all. Throughput is in markings per second.
fn bench_state_space(c: &mut Criterion) {
    let mut instances = state_space_instances();
    instances.extend(class_instances());
    instances.extend(pnml_instances(true));
    let mut g = c.benchmark_group("state_space");
    for instance in &instances {
        for graph in ["reachability_graph", "coverability_graph"] {
            let id = format!("state_space/{graph}/{}", instance.name);
            if !c_filter_matches(&id) {
                continue;
            }
            let system = instance.system();
            let explore = |timeout: Option<Duration>| -> Option<usize> {
                match (graph, timeout) {
                    ("reachability_graph", Some(t)) => system
                        .build_reachability_graph_timeout(t)
                        .ok()
                        .map(|rg| rg.marking_count()),
                    ("reachability_graph", None) => {
                        Some(system.build_reachability_graph().marking_count())
                    }
                    (_, Some(t)) => system
                        .build_coverability_graph_timeout(t)
                        .ok()
                        .map(|cg| cg.marking_count()),
                    (_, None) => Some(system.build_coverability_graph().marking_count()),
                }
            };
            let Some(markings) = explore(Some(GUARD)) else {
                eprintln!("skipped {id}: not finished within {GUARD:?}");
                continue;
            };
            eprintln!("{id}: {markings} markings");
            g.throughput(Throughput::Elements(markings as u64));
            g.bench_function(BenchmarkId::new(graph, &instance.name), |b| {
                b.iter(|| black_box(explore(None)));
            });
        }
    }
    g.finish();
}

/// n-place n-transition ring, 1 token in the first place.
fn circuit(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let places: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    let transitions: Vec<Transition> = (0..m).map(|_| b.add_transition()).collect();
    for i in 0..m {
        b.add_arcs((places[i], transitions[i], places[(i + 1) % m]));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(places[0], 1u32)])
}

/// n-arm hub-and-spoke state machine. Center place + n arms (enter + exit
/// transitions + one intermediate place). Token-conservative with n tokens.
fn state_machine_sc(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let center = b.add_place();
    for _ in 0..m {
        let t_enter = b.add_transition();
        let arm = b.add_place();
        let t_exit = b.add_transition();
        b.add_arcs((center, t_enter, arm, t_exit, center));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(center, m as u32)])
}

/// Two S-net cycles of n/2 places each, linked by a one-way switch transition.
/// Not strongly connected as a whole; token count is invariant, so always bounded.
fn state_machine_non_sc(n: usize) -> PetriNet {
    let half = (n / 2).max(2);
    let mut b = NetBuilder::new();
    let places_a: Vec<Place> = (0..half).map(|_| b.add_place()).collect();
    let trans_a: Vec<Transition> = (0..half).map(|_| b.add_transition()).collect();
    for i in 0..half {
        b.add_arcs((places_a[i], trans_a[i], places_a[(i + 1) % half]));
    }
    let places_b: Vec<Place> = (0..half).map(|_| b.add_place()).collect();
    let trans_b: Vec<Transition> = (0..half).map(|_| b.add_transition()).collect();
    for i in 0..half {
        b.add_arcs((places_b[i], trans_b[i], places_b[(i + 1) % half]));
    }
    // One-way switch: still an S-net (one input and one output per transition).
    let t_switch = b.add_transition();
    b.add_arcs((places_a[0], t_switch, places_b[0]));
    let net = b.build().unwrap();
    PetriNet::new(net, [(places_a[0], 1)])
}

/// Fork/join marked graph with n parallel paths and 1 token in the shared
/// place. All n circuits are marked, so all transitions are L4-live.
fn marked_graph_live(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let p_shared = b.add_place();
    let t_fork = b.add_transition();
    let t_join = b.add_transition();
    b.add_arcs((p_shared, t_fork, p_shared));
    for _ in 0..m {
        let p = b.add_place();
        b.add_arcs((t_fork, p, t_join));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(p_shared, 1)])
}

/// Same fork/join structure with zero tokens: every circuit is unmarked.
fn marked_graph_dark(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let p_shared = b.add_place();
    let t_fork = b.add_transition();
    let t_join = b.add_transition();
    b.add_arcs((p_shared, t_fork, p_shared));
    for _ in 0..m {
        let p = b.add_place();
        b.add_arcs((t_fork, p, t_join));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [])
}

/// n-stage diamond chain. Each stage has one entry place and two parallel
/// choice paths (choice → private middle → commit) leading to the next stage.
/// Free choice. One token circulates.
fn free_choice(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let stages: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    for i in 0..m {
        let p_in = stages[i];
        let p_out = stages[(i + 1) % m];
        for _ in 0..2 {
            let [t_choose, t_commit] = b.add_transitions();
            let q = b.add_place();
            b.add_arcs((p_in, t_choose, q, t_commit, p_out));
        }
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(stages[0], 1)])
}

/// n-segment asymmetric-choice cycle. Segment i has a shared place p_main_i and a
/// private place p_low_i; t_high_i consumes only p_main_i, t_low_i consumes both.
/// Asymmetric choice, not free choice. Token count is conserved, so bounded.
fn asym_choice(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let p_mains: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    let p_lows: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    let t_highs: Vec<Transition> = (0..m).map(|_| b.add_transition()).collect();
    let t_lows: Vec<Transition> = (0..m).map(|_| b.add_transition()).collect();
    for i in 0..m {
        let next = (i + 1) % m;
        b.add_arc((p_mains[i], t_highs[i]));
        b.add_arc((t_highs[i], p_mains[next]));
        b.add_arc((p_mains[i], t_lows[i]));
        b.add_arc((p_lows[i], t_lows[i]));
        b.add_arc((t_lows[i], p_mains[next]));
        b.add_arc((t_lows[i], p_lows[next]));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(p_mains[0], 1), (p_lows[0], 1)])
}

/// n-cluster cycle with symmetric confusion: two places share a synchronising
/// transition, and each also has a private one. General class. Token count 2 is
/// conserved, so bounded.
fn general(n: usize) -> PetriNet {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let p_a: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    let p_b: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    for i in 0..m {
        let next = (i + 1) % m;
        let t_shared = b.add_transition();
        let t_priv_a = b.add_transition();
        let t_priv_b = b.add_transition();
        b.add_arc((p_a[i], t_shared));
        b.add_arc((p_b[i], t_shared));
        b.add_arc((t_shared, p_a[next]));
        b.add_arc((t_shared, p_b[next]));
        b.add_arc((p_a[i], t_priv_a));
        b.add_arc((t_priv_a, p_a[next]));
        b.add_arc((p_b[i], t_priv_b));
        b.add_arc((t_priv_b, p_b[next]));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(p_a[0], 1), (p_b[0], 1)])
}

/// n dining philosophers, each taking the left fork, then the right, then putting both
/// down. Deadlocks when everyone holds a left fork; that marking is the target.
fn philosophers(n: usize) -> (PetriNet, Marking<u32>) {
    let mut b = NetBuilder::new();
    let forks: Vec<Place> = (0..n).map(|_| b.add_place()).collect();
    let thinking: Vec<Place> = (0..n).map(|_| b.add_place()).collect();
    let holding: Vec<Place> = (0..n).map(|_| b.add_place()).collect();
    let eating: Vec<Place> = (0..n).map(|_| b.add_place()).collect();
    for i in 0..n {
        let right = forks[(i + 1) % n];
        let [take_left, take_right, put_down] = b.add_transitions();
        b.add_arcs((
            thinking[i],
            take_left,
            holding[i],
            take_right,
            eating[i],
            put_down,
            thinking[i],
        ));
        b.add_arc((forks[i], take_left));
        b.add_arc((right, take_right));
        b.add_arc((put_down, forks[i]));
        b.add_arc((put_down, right));
    }
    let net = b.build().unwrap();
    let initial: Vec<(Place, u32)> = thinking.iter().chain(&forks).map(|&p| (p, 1)).collect();
    let deadlock: Marking<u32> = holding.iter().map(|&p| (p, 1)).collect();
    (
        PetriNet::new(net, initial.into_iter().collect::<Marking<u32>>()),
        deadlock,
    )
}

/// n independent two-state processes, joined only through a never-enabled transition
/// (petrivet requires connected nets): 2^n reachable markings, live and bounded.
fn independent(n: usize) -> PetriNet {
    let mut b = NetBuilder::new();
    let hub = b.add_place();
    let never = b.add_transition();
    b.add_arc((hub, never));
    let mut initial = Vec::new();
    for _ in 0..n {
        let [idle, busy] = b.add_places();
        let [start, stop] = b.add_transitions();
        b.add_arcs((idle, start, busy, stop, idle));
        b.add_arc((never, idle));
        initial.push((idle, 1));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, initial.into_iter().collect::<Marking<u32>>())
}

/// n producers that can always produce into one buffer, and one consumer: the buffer is
/// unbounded. The target asks for three items in the buffer.
fn producer_consumer(n: usize) -> (PetriNet, Marking<u32>) {
    let mut b = NetBuilder::new();
    let buffer = b.add_place();
    let consumer = b.add_place();
    let consume = b.add_transition();
    b.add_arcs((buffer, consume, consumer));
    b.add_arc((consumer, consume));
    let mut initial = vec![(consumer, 1)];
    for _ in 0..n {
        let ready = b.add_place();
        let produce = b.add_transition();
        b.add_arcs((ready, produce, ready));
        b.add_arc((produce, buffer));
        initial.push((ready, 1));
    }
    let net = b.build().unwrap();
    let target: Marking<u32> = [(buffer, 3)].into();
    (
        PetriNet::new(net, initial.into_iter().collect::<Marking<u32>>()),
        target,
    )
}

/// Short measurements: a broad view in a few minutes rather than a precise one per case.
/// For a closer look at one case, raise these with criterion's command-line options.
fn quick() -> Criterion {
    Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(400))
        .without_plots()
        .configure_from_args()
}

criterion_group! {
    name = benches;
    config = quick();
    targets = bench_construction, bench_structural, bench_properties, bench_state_space
}
criterion_main!(benches);
