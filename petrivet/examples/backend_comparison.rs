//! Compares `HashMap`- vs `MarkingDecisionDiagram`-backed state space
//! exploration on larger versions of the net shapes used in
//! `benches/analysis.rs`, reporting both wall-clock time and real
//! (measured, not estimated) peak incremental memory via a counting global
//! allocator.
//!
//! Run with: `cargo run -p petrivet --release --example backend_comparison`
//! (release mode matters a lot here -- these nets are large enough that
//! debug-mode overhead would dominate the comparison).

use petrivet::prelude::*;
use petrivet::state_space::{ExplorationOrder, MarkingDecisionDiagram, StateGraphExplorer};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// --- Counting allocator: tracks currently-live bytes and the peak reached
// --- since the last `begin_phase()` call, so each measured phase's own
// --- incremental footprint can be isolated from whatever came before it
// --- (net construction, the other backend's already-freed exploration).

struct CountingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PHASE_PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let now = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PHASE_PEAK.fetch_max(now, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            let now = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PHASE_PEAK.fetch_max(now, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size >= layout.size() {
                let now = LIVE_BYTES.fetch_add(new_size - layout.size(), Ordering::Relaxed) + (new_size - layout.size());
                PHASE_PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Marks the start of a measured phase: baseline is "live bytes right now",
/// and the peak tracker resets to that baseline so growth *during* the
/// phase can be isolated from bytes already live beforehand.
fn begin_phase() -> usize {
    let baseline = LIVE_BYTES.load(Ordering::Relaxed);
    PHASE_PEAK.store(baseline, Ordering::Relaxed);
    baseline
}

/// Peak incremental bytes allocated since `begin_phase()`, i.e. how far live
/// memory rose above the baseline at its highest point during the phase.
fn phase_peak_growth(baseline: usize) -> usize {
    PHASE_PEAK.load(Ordering::Relaxed).saturating_sub(baseline)
}

type NetSize = usize;

struct NetFixture {
    label: &'static str,
    build: fn(NetSize) -> PetriNet<Net>,
}

const CIRCUIT: NetFixture = NetFixture { label: "circuit", build: circuit };
const STATE_MACHINE_SC: NetFixture = NetFixture { label: "state_machine/sc", build: state_machine_sc };
const GENERAL: NetFixture = NetFixture { label: "general", build: general };

/// n-place n-transition ring, 1 token in the first place. Exactly n
/// reachable markings, each place always 0 except one -- the best case for
/// Rice(k)'s "mostly zero" assumption.
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

/// n-arm hub-and-spoke state machine, m tokens circulating among m arms --
/// a genuinely combinatorial reachable state space, places take a range of
/// small values rather than just 0/1.
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

/// n-cluster cycle with symmetric confusion (see `benches/analysis.rs` for
/// the full structural description) -- 2 tokens conserved, moderate reachable
/// state space, exercises places holding values other than exactly the token
/// total.
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

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn main() {
    // Both configurable via env vars so different scales can be tried
    // without recompiling: `NET_SIZE=12 STATE_CAP=1000 cargo run --release
    // --example backend_comparison`.
    let net_size = env_usize("NET_SIZE", 10);
    let state_cap = env_usize("STATE_CAP", 500);
    let initial_capacity = env_usize("INITIAL_CAPACITY", 4096);

    let fixtures_and_sizes: &[(&NetFixture, NetSize)] =
        &[(&CIRCUIT, net_size), (&STATE_MACHINE_SC, net_size), (&GENERAL, net_size)];

    println!("NET_SIZE={net_size} STATE_CAP={state_cap}");
    println!(
        "{:<22} {:>10} {:>12} {:>10} {:>12} {:>10} {:>12}",
        "net", "markings", "hm_time_ms", "hm_peak_kb", "mtbdd_ms", "mtbdd_peak_kb", "mem_ratio"
    );


    for &(fixture, size) in fixtures_and_sizes {
        let sys = (fixture.build)(size);

        let baseline = begin_phase();
        let start = Instant::now();
        let mut hash_map_explorer = StateGraphExplorer::<u32>::new(&sys, ExplorationOrder::BreadthFirst);
        let hm_steps = hash_map_explorer.explore_iter().take(state_cap).count();
        let hm_time = start.elapsed();
        let hm_peak = phase_peak_growth(baseline);
        let marking_count = hash_map_explorer.marking_count();
        let hm_capped = hm_steps == state_cap && !hash_map_explorer.is_fully_explored();
        drop(hash_map_explorer);

        let baseline = begin_phase();
        let start = Instant::now();
        let mut mtbdd_explorer = StateGraphExplorer::<u32, MarkingDecisionDiagram<u32>>::with_marking_map(
            &sys,
            ExplorationOrder::BreadthFirst,
            MarkingDecisionDiagram::new(initial_capacity, None),
        );
        let mtbdd_steps = mtbdd_explorer.explore_iter().take(state_cap).count();
        let mtbdd_time = start.elapsed();
        let mtbdd_peak = phase_peak_growth(baseline);
        let mtbdd_marking_count = mtbdd_explorer.marking_count();
        let mtbdd_capped = mtbdd_steps == state_cap && !mtbdd_explorer.is_fully_explored();
        drop(mtbdd_explorer);

        // Only a meaningful cross-check if neither run was cut off by the cap
        // at a different point (both hit the same `STATE_CAP` step count, so
        // this only fails if the two backends actually disagree).
        if !hm_capped && !mtbdd_capped {
            assert_eq!(mtbdd_marking_count, marking_count, "backends disagree on marking count!");
        }

        let mem_ratio = hm_peak as f64 / mtbdd_peak.max(1) as f64;
        let capped_note = if hm_capped || mtbdd_capped { " (capped)" } else { "" };

        println!(
            "{:<22} {:>10} {:>12} {:>10} {:>12} {:>10} {:>12.2}{capped_note}",
            format!("{}(n={size})", fixture.label),
            marking_count,
            hm_time.as_millis(),
            hm_peak / 1024,
            mtbdd_time.as_millis(),
            mtbdd_peak / 1024,
            mem_ratio,
        );
    }
}
