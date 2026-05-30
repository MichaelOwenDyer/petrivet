use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use petrivet::{
    prelude::*,
    state_space::ExplorationOrder,
};

/// Hub-and-spoke: one shared center place, `m` arms each of `k` transitions
/// (and `k-1` intermediate places). Initial marking: `m` tokens at center.
///
/// With `m` tokens circulating independently through `m` distinguishable arms,
/// the state space grows combinatorially with both `m` and `k`.
/// Use for: build, explore_reachability, explore_coverability, liveness.
fn hub_spoke(m: usize, k: usize) -> (Net, Place) {
    assert!(k >= 2, "arm length k must be at least 2");
    let mut b = NetBuilder::new();
    let center = b.add_place();

    for _ in 0..m {
        let t_enter = b.add_transition();
        let mut chain_end = b.add_place();
        b.add_arcs((center, t_enter, chain_end));
        for _ in 1..(k - 1) {
            let new_t = b.add_transition();
            let new_p = b.add_place();
            b.add_arcs((chain_end, new_t, new_p));
            chain_end = new_p;
        }

        let t_exit = b.add_transition();
        b.add_arcs((chain_end, t_exit, center));
    }

    (b.build().unwrap(), center)
}

fn hub_spoke_system(m: usize, k: usize) -> PetriNet<Net> {
    let (net, center) = hub_spoke(m, k);
    PetriNet::new(net, [(center, m as u32)])
}

/// Free-choice diamond chain: `n` stages in a cycle. Each stage has one entry
/// place, two independent choice paths (each path: choice transition → private
/// middle place → commit transition), both paths leading to the next stage's
/// entry place. One token circulates.
///
/// Satisfies the free-choice property. Use for: commoner_hack_criterion,
/// analyze_liveness.
fn diamond_chain(n: usize) -> (Net, Place) {
    assert!(n >= 1);
    let mut b = NetBuilder::new();
    let stages: Vec<Place> = (0..n).map(|_| b.add_place()).collect();

    for i in 0..n {
        let p_in = stages[i];
        let p_out = stages[(i + 1) % n];

        for _ in 0..2 {
            let t_choose = b.add_transition();
            let q = b.add_place();
            let t_commit = b.add_transition();
            b.add_arc((p_in, t_choose));
            b.add_arc((t_choose, q));
            b.add_arc((q, t_commit));
            b.add_arc((t_commit, p_out));
        }
    }

    (b.build().unwrap(), stages[0])
}

fn diamond_chain_system(n: usize) -> PetriNet<Net> {
    let (net, start) = diamond_chain(n);
    PetriNet::new(net, [(start, 1)])
}

// ── Benchmarks ───────────────────────────────────────────────────────────────

/// NetBuilder::build() — construction and validation cost at varying arm lengths.
fn bench_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("build");
    for k in [5, 10, 20, 50] {
        group.bench_with_input(BenchmarkId::new("hub_spoke/m4", k), &k, |b, &k| {
            b.iter(|| {
                let (_, center) = hub_spoke(4, k);
                std::hint::black_box(center);
            });
        });
        group.bench_with_input(BenchmarkId::new("diamond_chain", k * 4), &(k * 4), |b, &n| {
            b.iter(|| {
                let (_, start) = diamond_chain(n);
                std::hint::black_box(start);
            });
        });
    }
    group.finish();
}

/// explore_iter().take(N) — reachability exploration step throughput.
/// Benchmarks the BFS loop itself without omega-acceleration overhead.
fn bench_explore_reachability(c: &mut Criterion) {
    let mut group = c.benchmark_group("explore_reachability");
    // m=4 arms, k=10: large enough state space for 100K steps
    let sys = hub_spoke_system(4, 10);
    for n in [1_000, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("hub_spoke/m4k10", n), &n, |b, &n| {
            b.iter(|| {
                let mut explorer = sys.explore_reachability(ExplorationOrder::BreadthFirst);
                std::hint::black_box(explorer.explore_iter().take(n).count())
            });
        });
    }
    group.finish();
}

/// explore_iter().take(N) via coverability explorer — same bounded net as above,
/// so omega never triggers. The delta vs. bench_explore_reachability isolates
/// the per-step cost of the omega-domination check.
fn bench_explore_coverability_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("explore_coverability_overhead");
    let sys = hub_spoke_system(4, 10);
    for n in [1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("hub_spoke/m4k10", n), &n, |b, &n| {
            b.iter(|| {
                let mut explorer = sys.explore_coverability(ExplorationOrder::BreadthFirst);
                std::hint::black_box(explorer.explore_iter().take(n).count())
            });
        });
    }
    group.finish();
}

/// rg.liveness() — SCC-based liveness analysis on a pre-built reachability graph.
/// The hub-spoke RG has m SCCs (one per arm cycle).
fn bench_scc_liveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("liveness");
    for (m, k) in [(2, 4), (3, 6), (4, 8)] {
        let sys = hub_spoke_system(m, k);
        let rg = sys.build_reachability_graph();
        group.bench_with_input(
            BenchmarkId::new("hub_spoke", format!("m{m}k{k}")),
            &rg,
            |b, rg| {
                b.iter(|| std::hint::black_box(rg.transition_liveness()));
            },
        );
    }
    group.finish();
}

/// commoner_hack_criterion() — structural free-choice liveness shortcut.
/// Diamond chain is a valid free-choice net at various sizes.
fn bench_commoner_hack_criterion(c: &mut Criterion) {
    let mut group = c.benchmark_group("commoner_hack_criterion");
    for n in [10, 50, 100, 200] {
        let sys = diamond_chain_system(n);
        group.bench_with_input(BenchmarkId::new("diamond_chain", n), &sys, |b, sys| {
            b.iter(|| std::hint::black_box(sys.commoner_hack_criterion()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_build,
    bench_explore_reachability,
    bench_explore_coverability_overhead,
    bench_scc_liveness,
    bench_commoner_hack_criterion,
);
criterion_main!(benches);
