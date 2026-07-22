use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use petrivet::prelude::*;
use petrivet::state_space::ExplorationOrder;

type NetSize = usize;

struct NetFixture {
    label: &'static str,
    build: fn(NetSize) -> PetriNet<Net>,
}

const CIRCUIT: NetFixture = NetFixture {
    label: "circuit",
    build: circuit,
};
const STATE_MACHINE_SC: NetFixture = NetFixture {
    label: "state_machine/sc",
    build: state_machine_sc,
};
const STATE_MACHINE_NON_SC: NetFixture = NetFixture {
    label: "state_machine/non_sc",
    build: state_machine_non_sc,
};
const MARKED_GRAPH_LIVE: NetFixture = NetFixture {
    label: "marked_graph/live",
    build: marked_graph_live,
};
const MARKED_GRAPH_DARK: NetFixture = NetFixture {
    label: "marked_graph/dark",
    build: marked_graph_dark,
};
const FREE_CHOICE: NetFixture = NetFixture {
    label: "free_choice",
    build: free_choice,
};
const ASYM_CHOICE: NetFixture = NetFixture {
    label: "asym_choice",
    build: asym_choice,
};
const GENERAL: NetFixture = NetFixture {
    label: "general",
    build: general,
};
const GENERATORS: &[NetFixture] = &[
    CIRCUIT,
    STATE_MACHINE_SC,
    STATE_MACHINE_NON_SC,
    MARKED_GRAPH_LIVE,
    MARKED_GRAPH_DARK,
    FREE_CHOICE,
    ASYM_CHOICE,
    GENERAL,
];

const SMALL: (&str, NetSize) = ("small", 5);
const MEDIUM: (&str, NetSize) = ("medium", 25);
const LARGE: (&str, NetSize) = ("large", 25);
const SIZES: &[(&str, NetSize)] = &[SMALL, MEDIUM, LARGE];

/// n-place n-transition ring, 1 token in the first place.
fn circuit(n: NetSize) -> PetriNet<Net> {
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
fn state_machine_sc(n: NetSize) -> PetriNet<Net> {
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
fn state_machine_non_sc(n: NetSize) -> PetriNet<Net> {
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

    // One-way switch: p_a[0] is in the preset of t_switch, so p_a[0] has two
    // output transitions: trans_a[0] and t_switch. Still S-net (one input per
    // transition, one output per transition).
    let t_switch = b.add_transition();
    b.add_arcs((places_a[0], t_switch, places_b[0]));

    let net = b.build().unwrap();
    PetriNet::new(net, [(places_a[0], 1)])
}

/// Fork/join marked graph with n parallel paths and 1 token in the shared
/// place. All n circuits are marked (M(p_shared) = 1) → all transitions L4.
/// Exercises the fast no-unmarked-circuit exit in liveness_via_marked_graph.
fn marked_graph_live(n: NetSize) -> PetriNet<Net> {
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

/// Same fork/join structure with zero tokens. All n circuits are unmarked →
/// the DFS propagation runs over every circuit. Exercises the full
/// liveness_via_marked_graph_unmarked_circuits code path.
fn marked_graph_dark(n: NetSize) -> PetriNet<Net> {
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
    PetriNet::new(net, Marking::default())
}

/// n-stage diamond chain. Each stage has one entry place and two parallel
/// choice paths (choice → private middle → commit) leading to the next stage.
/// Satisfies the free-choice property. One token circulates.
fn free_choice(n: NetSize) -> PetriNet<Net> {
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

/// n-segment asymmetric-choice cycle. Each segment i has a shared place
/// p_main_i and a private place p_low_i. Transition t_high_i consumes only
/// p_main_i; t_low_i consumes both p_main_i and p_low_i. This satisfies the
/// AC property (p_low_i• ⊆ p_main_i•) but not the free-choice property
/// (p_main_i• ≠ p_low_i•). Total token count is conserved → always bounded.
fn asym_choice(n: NetSize) -> PetriNet<Net> {
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

/// n-cluster cycle with symmetric confusion. Each cluster i has two places
/// p_a_i and p_b_i sharing a synchronisation transition t_shared_i, plus
/// private transitions t_priv_a_i (only p_a_i as input) and t_priv_b_i (only
/// p_b_i as input). Neither p_a_i• ⊆ p_b_i• nor p_b_i• ⊆ p_a_i•, so the
/// net is General. Total token count 2 is conserved → always bounded.
fn general(n: NetSize) -> PetriNet<Net> {
    let m = n.max(1);
    let mut b = NetBuilder::new();
    let p_a: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    let p_b: Vec<Place> = (0..m).map(|_| b.add_place()).collect();
    for i in 0..m {
        let next = (i + 1) % m;
        let t_shared = b.add_transition();
        let t_priv_a = b.add_transition();
        let t_priv_b = b.add_transition();
        // Shared synchronisation: consumes both, produces both next
        b.add_arc((p_a[i], t_shared));
        b.add_arc((p_b[i], t_shared));
        b.add_arc((t_shared, p_a[next]));
        b.add_arc((t_shared, p_b[next]));
        // Private moves: each place independently advances one token
        b.add_arc((p_a[i], t_priv_a));
        b.add_arc((t_priv_a, p_a[next]));
        b.add_arc((p_b[i], t_priv_b));
        b.add_arc((t_priv_b, p_b[next]));
    }
    let net = b.build().unwrap();
    PetriNet::new(net, [(p_a[0], 1), (p_b[0], 1)])
}

/// Total cost of constructing a PetriNet from a NetBuilder, including all arc
/// insertions and the build() call (RCM ordering + dense conversion + classification).
fn bench_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("build");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            b.iter(|| std::hint::black_box((fixture.build)(size)))
        });
    };
    for fixture in GENERATORS {
        for &size in SIZES {
            run(fixture, size);
        }
    }
    group.finish();
}

/// Cost of Net → NetBuilder round-trip via NetBuilder::from(net).
/// The net is re-created in the setup closure so each iteration gets a fresh value.
fn bench_unbuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbuild");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            b.iter_batched(
                || (fixture.build)(size).net,
                |net| std::hint::black_box(NetBuilder::from(net)),
                criterion::BatchSize::SmallInput,
            );
        });
    };
    for fixture in GENERATORS {
        for &size in SIZES {
            run(fixture, size);
        }
    }
    group.finish();
}

/// sys.liveness() — dispatches to the cheapest known algorithm for each class.
fn bench_liveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("liveness");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            let sys = (fixture.build)(size);
            b.iter(|| std::hint::black_box(sys.liveness()));
        });
    };
    for fixture in &[
        CIRCUIT,
        STATE_MACHINE_SC,
        STATE_MACHINE_NON_SC,
        MARKED_GRAPH_LIVE,
        MARKED_GRAPH_DARK,
        FREE_CHOICE,
    ] {
        for &size in SIZES {
            run(fixture, size);
        }
    }
    // large asymmetric choice and general nets are too slow
    for fixture in &[ASYM_CHOICE, GENERAL] {
        for &size in &[SMALL, MEDIUM] {
            run(fixture, size);
        }
    }
    group.finish();
}

/// sys.analyze_boundedness() — builds the coverability graph and derives per-place bounds.
fn bench_boundedness(c: &mut Criterion) {
    let mut group = c.benchmark_group("boundedness");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            let sys = (fixture.build)(size);
            b.iter(|| std::hint::black_box(sys.boundedness()));
        });
    };
    for fixture in GENERATORS {
        for &size in SIZES {
            run(fixture, size);
        }
    }
    group.finish();
}

/// sys.is_deadlock_free() — dispatches to efficient algorithm when available,
/// otherwise performs full state-space exploration.
fn bench_deadlock(c: &mut Criterion) {
    let mut group = c.benchmark_group("deadlock_freedom");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            let sys = (fixture.build)(size);
            b.iter(|| std::hint::black_box(sys.is_deadlock_free()));
        });
    };
    for fixture in GENERATORS {
        for &size in &[SMALL, MEDIUM] {
            run(fixture, size);
        }
    }
    group.finish();
}

/// sys.explore_reachability() — reachability graph exploration.
fn bench_reachability_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("reachability_graph");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            let sys = (fixture.build)(size);
            b.iter(|| {
                std::hint::black_box(
                    sys.explore_reachability(ExplorationOrder::BreadthFirst)
                        .explore_iter()
                        .take(100_000)
                        .count(),
                )
            });
        });
    };
    for fixture in GENERATORS {
        for &size in &[SMALL, MEDIUM] {
            run(fixture, size);
        }
    }
    group.finish();
}

/// sys.explore_coverability() — BFS exploration of the coverability graph.
fn bench_coverability_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("coverability_graph");
    let mut run = |fixture: &NetFixture, (size_label, size): (&str, NetSize)| {
        group.bench_function(BenchmarkId::new(fixture.label, size_label), |b| {
            let sys = (fixture.build)(size);
            b.iter(|| {
                std::hint::black_box(
                    sys.explore_coverability(ExplorationOrder::BreadthFirst)
                        .explore_iter()
                        .take(10_000)
                        .count(),
                )
            });
        });
    };
    for fixture in GENERATORS {
        for &size in &[SMALL, MEDIUM] {
            run(fixture, size);
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_build,
    bench_unbuild,
    bench_liveness,
    bench_boundedness,
    bench_deadlock,
    bench_reachability_graph,
    bench_coverability_graph,
);
criterion_main!(benches);
