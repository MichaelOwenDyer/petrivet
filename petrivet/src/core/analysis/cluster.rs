//! The free-choice cluster partition and the Rank-Theorem relation
//! `rank(C) == c − 1`.
//!
//! # Clusters (the operational definition)
//!
//! A **cluster** is a connected component of the bipartite graph on
//! `places ∪ transitions` with an edge `p — t` iff `p ∈ •t` — the *consumption*
//! relation, i.e. `p ∈ preset_t[t]` (equivalently `t ∈ postset_p[p]`).
//! Production arcs (`t•`) are deliberately **not** edges of this graph: the
//! Desel–Esparza cluster uses consumption arcs only, and `rank(C) = c − 1` fails
//! under a weak-connectivity (all-arcs) partition. The count `c` is the number of
//! distinct components over the *whole* node set, singletons included.
//!
//! The library only admits connected nets (`classify` returns `None` otherwise,
//! `class.rs`), so every [`DenseNet`] this module sees is connected and the count
//! is well-posed. (Connectivity there is over the *full* underlying graph
//! preset ∪ postset; the cluster graph is the consumption-arc subgraph — a
//! different graph, not to be conflated.)
//!
//! # The Rank-Theorem relation — necessary, not sufficient
//!
//! The Simultaneous Liveness and Boundedness Theorem (`class.rs`) characterises a
//! *well-formed* (live ∧ bounded) free-choice system by **four** conditions: a
//! positive S-invariant, a positive T-invariant, `rank(C) = c − 1`, and every
//! proper siphon marked. `rank(C) == c − 1` is therefore a **necessary** condition
//! of well-formedness, **not** an equivalence: there are connected free-choice
//! nets that satisfy `rank == c − 1` yet are not well-formed (e.g. an unbounded
//! net with no *positive* S-invariant). This module supplies `c` and reports the
//! relation; it does **not** claim the relation decides well-formedness, and it
//! registers **no** decision procedure. The S/T-component and siphon analyses
//! that would close the gap to the full theorem are future work that consumes
//! this partition.
//!
//! The tested gates are accordingly: the forward (necessary) direction —
//! every oracle-confirmed live ∧ bounded net satisfies `rank == c − 1` — plus a
//! discriminating net where `rank ≠ c − 1` to pin that the relation is non-trivial.

// The cluster partition and the Rank-Theorem relation are built ahead of their
// consumers (the planned S/T-component and siphon analyses) and are exercised by
// this module's tests now. Remove this allow as each gains a non-test consumer.
#![allow(dead_code)]

use crate::core::analysis::exact_matrix::incidence_over_rationals;
use crate::core::net::{DenseNet, PlaceIdx, TransitionIdx};

/// A flat disjoint-set (union-find) over a node universe `0..n`, with path
/// halving on `find` and union by rank. Near-linear over the whole net.
struct UnionFind {
    parent: Vec<usize>,
    /// Union-by-rank heuristic rank — the tree-height bound, **not** matrix rank.
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), rank: vec![0; n] }
    }

    /// The canonical root of `x`, compressing the path by halving as it climbs.
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

/// The free-choice cluster partition of a net: the connected components of the
/// consumption-arc bipartite graph (see the module docs).
///
/// Node identifiers are flattened: place `p` is node `p`; transition `t` is node
/// `place_count + t` (the two index spaces overlap at `0`, so the offset is
/// mandatory). Two nodes share a cluster iff their [`representatives`] are equal.
///
/// [`representatives`]: ClusterPartition::representatives
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterPartition {
    /// Canonical cluster representative per node, length `place_count + transition_count`.
    representative: Vec<usize>,
    place_count: usize,
    count: usize,
}

impl ClusterPartition {
    /// The number of clusters `c`.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// The flattened cluster representative of a place.
    #[must_use]
    pub fn place_cluster(&self, place: PlaceIdx) -> usize {
        self.representative[place]
    }

    /// The flattened cluster representative of a transition.
    #[must_use]
    pub fn transition_cluster(&self, transition: TransitionIdx) -> usize {
        self.representative[self.place_count + transition]
    }

    /// The canonical representative of every node (places `0..place_count`, then
    /// transitions). Two nodes are co-clustered iff these are equal.
    #[must_use]
    pub fn representatives(&self) -> &[usize] {
        &self.representative
    }
}

/// Compute the free-choice cluster partition of a net (union-find over the
/// consumption arcs `p ∈ •t`). Near-linear in the number of arcs.
#[must_use]
pub fn clusters(net: &DenseNet) -> ClusterPartition {
    let place_count = net.place_count();
    let node_count = place_count + net.transition_count();

    let mut uf = UnionFind::new(node_count);
    for t in net.transition_indices() {
        for &p in &net.preset_t[t] {
            uf.union(p, place_count + t);
        }
    }

    let representative: Vec<usize> = (0..node_count).map(|node| uf.find(node)).collect();
    let count = representative
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<usize>>()
        .len();

    ClusterPartition { representative, place_count, count }
}

/// The Rank-Theorem relation for a net: the exact incidence rank against the
/// cluster count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankClusterRelation {
    /// Exact rank of the incidence matrix `C` (`|P| × |T|`, over ℚ).
    pub rank: usize,
    /// The cluster count `c`.
    pub clusters: usize,
}

impl RankClusterRelation {
    /// Whether `rank(C) == c − 1`.
    ///
    /// For a free-choice system this is a **necessary** condition of
    /// well-formedness (live ∧ bounded) — one of the four Rank-Theorem conditions
    /// — and **not** a sufficient one (see the module docs). Tested in the
    /// forward direction only.
    #[must_use]
    pub const fn holds(&self) -> bool {
        // Overflow-safe form of `rank == clusters - 1`: `clusters ≥ 1` for a
        // connected non-empty net, but `clusters - 1` on `usize` would underflow
        // at `clusters == 0`; `rank + 1 == clusters` avoids it.
        self.rank + 1 == self.clusters
    }
}

/// Compute `(rank(C), c)` for a net. `None` only if the exact `rank` computation
/// overflows `i128` (impossible on small nets); never on the in-tree fixtures.
#[must_use]
pub fn rank_cluster_relation(net: &DenseNet) -> Option<RankClusterRelation> {
    let cluster_count = clusters(net).count();
    let rank = incidence_over_rationals(net).rank()?;
    Some(RankClusterRelation { rank, clusters: cluster_count })
}

/// Tests tagged `[PROP]` state a property and discharge it over a family of
/// shapes; tests tagged `[ORACLE]` check the implementation against an
/// independently computed answer (a hand-derived table, or a second algorithm
/// sharing no code with the first).
#[cfg(test)]
mod tests {
    use super::{clusters, rank_cluster_relation};
    use crate::core::net::DenseNet;
    use crate::prelude::NetBuilder;

    /// An independent oracle for the cluster partition: flood-fill (BFS) the
    /// consumption-arc bipartite graph, NOT union-find. Returns, per node, the
    /// minimum node id of its component (a canonical min-label). Because we start
    /// floods from nodes in increasing order, the start node is its component's
    /// minimum, so labels are directly comparable to a min-canonicalised
    /// union-find partition.
    fn flood_components(net: &DenseNet) -> Vec<usize> {
        let place_count = net.place_count();
        let node_count = place_count + net.transition_count();
        let mut adjacency = vec![Vec::new(); node_count];
        for t in net.transition_indices() {
            for &p in &net.preset_t[t] {
                adjacency[p].push(place_count + t);
                adjacency[place_count + t].push(p);
            }
        }
        let mut component = vec![usize::MAX; node_count];
        for start in 0..node_count {
            if component[start] != usize::MAX {
                continue;
            }
            component[start] = start;
            let mut stack = vec![start];
            while let Some(x) = stack.pop() {
                for &y in &adjacency[x] {
                    if component[y] == usize::MAX {
                        component[y] = start;
                        stack.push(y);
                    }
                }
            }
        }
        component
    }

    /// Min-canonicalise a partition (representative-per-node) so it is comparable
    /// to [`flood_components`]: map each block to the smallest node id in it.
    fn min_canonical(representatives: &[usize]) -> Vec<usize> {
        let mut min_of = std::collections::HashMap::new();
        for (node, &root) in representatives.iter().enumerate() {
            let entry = min_of.entry(root).or_insert(node);
            *entry = (*entry).min(node);
        }
        representatives.iter().map(|root| min_of[root]).collect()
    }

    // ---- net builders (small, hand-checked) ----

    /// `p0 → t0 → p1 → t1 → p0`. A circuit (state machine ∩ marked graph). With
    /// one token on the returned place: live and 1-bounded. Hand: c = 2
    /// ({p0,t0},{p1,t1}), rank = 1.
    fn two_place_cycle() -> (crate::prelude::Net, crate::prelude::Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        (b.build().expect("valid two-place cycle"), p0)
    }

    /// The `class.rs` `Circuit` doc-example: a single loop through 3 places and 3
    /// transitions. With one token: live and 1-bounded. Hand: c = 3, rank = 2.
    fn circuit3() -> (crate::prelude::Net, crate::prelude::Place) {
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arcs((p0, t0, p1, t1, p2, t2, p0));
        (b.build().expect("valid 3-circuit"), p0)
    }

    /// A strongly-connected state machine with a real choice: `p0` has two output
    /// transitions (`t0`, `t2`) both producing `p1`, and `t1: p1 → p0`. Free-choice
    /// (the choice places agree). With one token: live and 1-bounded (safe).
    /// Hand: clusters {p0,t0,t2},{p1,t1} ⇒ c = 2; rank = 1.
    fn state_machine_with_choice() -> (crate::prelude::Net, crate::prelude::Place) {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        b.add_arc((p0, t2));
        b.add_arc((t2, p1));
        (b.build().expect("valid SM with choice"), p0)
    }

    /// A connected net where `rank ≠ c − 1` — the discriminator proving the
    /// relation is non-trivial. `t0: p0 → p1`; `t1` is a self-loop on both places
    /// (`•t1 = t1• = {p0,p1}` ⇒ its incidence column is zero). Hand: one cluster
    /// (every node consumption-connected through `t1`) ⇒ c = 1; rank = 1 (only
    /// `t0`'s column is non-zero). So `rank = 1 ≠ 0 = c − 1`.
    fn rank_defect_net() -> crate::prelude::Net {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p0, t1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        b.add_arc((t1, p1));
        b.build().expect("valid rank-defect net")
    }

    /// [PROP] cluster == flow-components: the union-find partition equals the
    /// connected components of the consumption-arc graph, cross-checked against an
    /// independent BFS flood over the same graph, for every shape.
    #[test]
    fn clusters_match_independent_flood_components() {
        for net in [
            two_place_cycle().0,
            circuit3().0,
            state_machine_with_choice().0,
            rank_defect_net(),
        ] {
            let partition = clusters(&net.dense_net);
            let union_find = min_canonical(partition.representatives());
            let flood = flood_components(&net.dense_net);
            assert_eq!(
                union_find, flood,
                "union-find clusters must equal the independent flood components",
            );
        }
    }

    /// [ORACLE] the falsifiable hand-computed table: exact `(c, rank)` per net.
    /// Authoring this is the validator for the consumption-arc cluster definition
    /// — if any row is wrong, the definition or the offset is wrong.
    #[test]
    fn cluster_count_and_rank_match_hand_computed_table() {
        // (builder, expected c, expected rank)
        let cases: [(crate::prelude::Net, usize, usize); 4] = [
            (two_place_cycle().0, 2, 1),
            (circuit3().0, 3, 2),
            (state_machine_with_choice().0, 2, 1),
            (rank_defect_net(), 1, 1),
        ];
        for (net, expected_c, expected_rank) in cases {
            let relation = rank_cluster_relation(&net.dense_net).expect("no overflow on small nets");
            assert_eq!(relation.clusters, expected_c, "cluster count c");
            assert_eq!(relation.rank, expected_rank, "incidence rank");
        }
    }

    /// [ORACLE] the Rank-Theorem forward direction: every net the state-space
    /// oracle confirms **live ∧ bounded** (well-formed) satisfies `rank == c − 1`.
    /// This is the sound, theorem-backed agreement — the *necessary* direction.
    #[test]
    fn well_formed_nets_satisfy_rank_equals_clusters_minus_one() {
        // (net, the place to mark with one token)
        let witnesses = [two_place_cycle(), circuit3(), state_machine_with_choice()];
        for (net, place) in witnesses {
            let relation =
                rank_cluster_relation(&net.dense_net).expect("no overflow on small nets");
            // Ground truth via the public state-space-backed API.
            let sys = net.with_initial_marking([(place, 1)]);
            assert!(sys.is_live(), "witness must be live (well-formed)");
            assert!(sys.is_bounded(), "witness must be bounded (well-formed)");
            assert!(
                relation.holds(),
                "well-formed ⇒ rank == c − 1 (rank {}, c {})",
                relation.rank,
                relation.clusters,
            );
        }
    }

    /// The relation is non-trivial: it does **not** hold universally. The
    /// rank-defect net is connected but `rank = 1 ≠ 0 = c − 1`, so `holds()` is
    /// false — guarding against a regression that makes the predicate vacuous.
    #[test]
    fn rank_cluster_relation_is_discriminating() {
        let net = rank_defect_net();
        let relation = rank_cluster_relation(&net.dense_net).expect("no overflow");
        assert_eq!(relation.clusters, 1);
        assert_eq!(relation.rank, 1);
        assert!(!relation.holds(), "rank == c − 1 must not hold universally");
    }
}
