use crate::core::marking::IdxMarking;
use crate::core::net::{DenseNet, PlaceIdx, TransitionIdx};
use std::collections::HashSet;
use fixedbitset::FixedBitSet;

/// Shrinks `places` to the maximal subset closed under a "no starving witness"
/// property, the shared fixpoint behind [`maximal_siphon_in`] and
/// [`maximal_trap_in`].
///
/// Repeatedly removes any place `p` that has a witness transition
/// `t ∈ place_adj[p]` none of whose adjacent places `trans_adj[t]` remain in the
/// set. The two callers are duals and differ only in the adjacency passed:
/// - siphon: `place_adj = •p` (`preset_p`), `trans_adj = •t` (`preset_t`);
/// - trap:   `place_adj = p•` (`postset_p`), `trans_adj = t•` (`postset_t`).
///
/// Termination is guaranteed because the set strictly shrinks each pass; the
/// result is the unique maximal subset closed under the chosen property.
fn shrink_to_closed(
    mut places: FixedBitSet,
    place_adj: &[Box<[TransitionIdx]>],
    trans_adj: &[Box<[PlaceIdx]>],
) -> FixedBitSet {
    loop {
        let to_remove: Vec<PlaceIdx> = places
            .ones()
            .filter(|&p| {
                place_adj[p]
                    .iter()
                    .any(|&t| trans_adj[t].iter().all(|&q| !places.contains(q)))
            })
            .collect();
        if to_remove.is_empty() {
            break;
        }
        for p_idx in to_remove {
            places.remove(p_idx);
        }
    }
    places
}

/// Computes the maximal siphon contained in a given set of places.
///
/// A siphon is a set of places D such that •D ⊆ D•: every transition that
/// produces into D also consumes from D. Once empty, it stays empty forever.
///
/// Uses the shrinking algorithm from the [Petri Net Primer, Algorithm 6.19](crate::literature#algorithm-619--maximal-siphontrap-in-a-subset):
/// iteratively remove any place p where some transition t ∈ •p has no
/// input place in the current set. Runs in O(|S|² · |T|²).
#[must_use]
pub fn maximal_siphon_in(net: &DenseNet, places: FixedBitSet) -> FixedBitSet {
    shrink_to_closed(places, &net.preset_p, &net.preset_t)
}

/// Computes the maximal trap contained in a given set of places.
///
/// A trap Q satisfies Q• ⊆ •Q: every transition that consumes from Q also
/// produces into Q. Once marked, a trap stays marked forever.
///
/// Uses the dual of the shrinking algorithm: iteratively remove any place p
/// where some transition t ∈ p• has no output place in the current set.
#[must_use]
pub fn maximal_trap_in(net: &DenseNet, places: FixedBitSet) -> FixedBitSet {
    shrink_to_closed(places, &net.postset_p, &net.postset_t)
}

/// Upper bound on the number of maximal-siphon shrink operations
/// [`minimal_siphons`] performs before it stops and reports an incomplete
/// enumeration.
///
/// Minimal-siphon enumeration is worst-case exponential in the number of places
/// (a net whose every subset is a siphon has `2^|S|` of them, and the search
/// visits the whole subset lattice). This bound caps the work so a pathological
/// net degrades to an incomplete result — surfaced via [`MinimalSiphons::complete`]
/// and propagated as an inconclusive answer at the decision layer — instead of
/// hanging. Typical structural nets finish in far fewer steps; a net that reaches
/// the bound simply falls back to reachability-graph analysis, which stays sound.
///
/// This is a conservative safety valve, not a correctness parameter: raising it
/// never changes a definite answer, only how large a net can be before the search
/// gives up. The bound is on a deterministic step count (not wall-clock time) so
/// results remain reproducible.
const MAX_SHRINK_CALLS: usize = 100_000;

/// The minimal siphons of a net, together with whether their enumeration was
/// exhaustive.
///
/// When `complete` is `false` the search hit the [`MAX_SHRINK_CALLS`] work bound
/// and `siphons` holds only those discovered so far; callers must then treat the
/// absence of a particular siphon as unknown rather than proven.
#[derive(Debug, Clone)]
pub struct MinimalSiphons {
    /// The minimal siphons found. A prefix of the true set when `complete` is
    /// `false`.
    pub siphons: Box<[FixedBitSet]>,
    /// `true` if enumeration ran to completion; `false` if it stopped at the
    /// [`MAX_SHRINK_CALLS`] work bound, in which case `siphons` may be incomplete.
    pub complete: bool,
}

/// Finds the minimal siphons of a net as sets of [`PlaceIdx`].
///
/// Enumeration is worst-case exponential, so it is bounded by [`MAX_SHRINK_CALLS`].
/// If the bound is reached, [`MinimalSiphons::complete`] is `false` and the siphon
/// list may be incomplete; callers must treat an incomplete result as inconclusive
/// rather than assuming the listed siphons are all that exist.
#[must_use]
pub fn minimal_siphons(net: &DenseNet) -> MinimalSiphons {
    let mut results: Vec<FixedBitSet> = Vec::new();
    let mut stack: Vec<FixedBitSet> = vec![{
        let mut set = FixedBitSet::with_capacity(net.place_count());
        set.insert_range(..);
        set
    }];
    let mut visited: ahash::HashSet<FixedBitSet> = HashSet::default();
    let mut shrink_calls: usize = 0;
    let mut complete = true;

    'search: while let Some(candidate_set) = stack.pop() {
        if shrink_calls >= MAX_SHRINK_CALLS {
            complete = false;
            break;
        }
        shrink_calls += 1;
        let siphon = maximal_siphon_in(net, candidate_set);
        if siphon.is_clear() {
            continue;
        }

        if !visited.insert(siphon.clone()) {
            continue;
        }

        // Try excluding each place to find potentially smaller siphons.
        let mut is_minimal = true;
        for p_idx in siphon.ones() {
            let mut reduced = siphon.clone();
            reduced.remove(p_idx);
            if reduced.is_clear() {
                continue;
            }
            if shrink_calls >= MAX_SHRINK_CALLS {
                complete = false;
                break 'search;
            }
            shrink_calls += 1;
            let smaller_siphon = maximal_siphon_in(net, reduced);
            if !smaller_siphon.is_clear() {
                is_minimal = false;
                stack.push(smaller_siphon);
            }
        }

        if is_minimal {
            let dominated = results.iter().any(|existing| existing.is_subset(&siphon));
            if !dominated {
                results.retain(|existing| !siphon.is_subset(existing));
                results.push(siphon);
            }
        }
    }

    MinimalSiphons {
        siphons: results.into_boxed_slice(),
        complete,
    }
}

/// A siphon is a set of places D such that •D ⊆ D•.
///
/// In other words, every transition that produces to D also consumes from D.
/// This is significant because it means once a siphon is unmarked,
/// it can never be marked again (all transitions which could mark it are dead).
pub type Siphon = FixedBitSet;

/// A trap is a set of places Q such that Q• ⊆ •Q.
///
/// In other words, every transition that consumes from Q also produces to Q.
/// This is significant because it means once a trap is marked, it can never be unmarked again.
pub type Trap = FixedBitSet;

/// A minimal siphon and the maximal trap found within it,
/// and whether that trap is marked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiphonTrapPair {
    /// The minimal siphon (a set of places D with •D ⊆ D•).
    pub siphon: Siphon,
    /// The maximal trap contained in this siphon (a set of places Q with Q• ⊆ •Q).
    /// Empty if no trap was found.
    pub trap: Trap,
}

/// Result of a *completed* Commoner/Hack criterion check.
///
/// `Ok` carries, for every minimal siphon, the maximal trap found within it (all
/// marked); `Err` carries a single siphon whose maximal trap is unmarked — a
/// counterexample. See [`commoner_hack_criterion`], which wraps this in an
/// [`Option`] to also represent an inconclusive (bounded-out) enumeration.
///
/// For free-choice nets, this criterion is both necessary and sufficient for
/// liveness: a free-choice system (N, M₀) is live if and only if every proper
/// siphon of N contains a trap that is marked under M₀.
///
/// For general nets, the condition is sufficient for deadlock-freedom but
/// not necessary: if every siphon contains a marked trap, the net is
/// deadlock-free, but the converse does not hold.
///
/// References:
/// - [Murata 1989, Theorem 12](crate::literature#theorem-12--commonerhack-criterion)
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc)
pub type CommonerHackCriterionResult = Result<Box<[SiphonTrapPair]>, SiphonTrapPair>;

/// Checks the Commoner/Hack Criterion (CHC): every proper siphon contains
/// a trap that is marked under the given marking.
///
/// For free-choice nets, this is a necessary and sufficient condition for
/// liveness — [Murata, Theorem 12](crate::literature#theorem-12--commonerhack-criterion).
/// For asymmetric-choice nets it is sufficient but not necessary
/// — [Murata, Theorem 15](crate::literature#theorem-15--liveness-of-asymmetric-choice-nets).
///
/// This is the key structural shortcut for proving liveness in free-choice nets
/// without exploring the full state space. This is significant because it runs
/// in polynomial time rather than exponential (in the number of reachable states).
///
/// For general nets, it is a sufficient condition for deadlock-freedom.
///
/// Instead of pre-enumerating all traps and checking containment, this
/// computes the maximal trap *inside* each siphon directly using the
/// shrinking algorithm. If the maximal trap is non-empty and marked,
/// the siphon satisfies the condition; if it is empty or unmarked,
/// no trap inside the siphon can be marked.
///
/// # Bounded enumeration
///
/// Because siphon enumeration is worst-case exponential it is capped (see
/// [`minimal_siphons`]). The result is therefore an [`Option`]:
/// - `Some(Ok(pairs))` — every siphon contains a marked trap; enumeration was
///   exhaustive, so the criterion definitely holds.
/// - `Some(Err(counterexample))` — a siphon with no marked trap was found; this
///   is definitive even under a truncated enumeration, since the witness is a
///   genuine siphon.
/// - `None` — enumeration hit its work bound without finding a counterexample, so
///   the criterion is **inconclusive**. This must never be read as either outcome;
///   the decision layer degrades it to an inefficient (reachability-graph) answer.
///
/// References:
/// - [Murata 1989, Theorem 12](crate::literature#theorem-12--commonerhack-criterion):
///   "A free-choice net (N, M₀) is live iff every siphon in N contains a marked trap."
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc) (Commoner/Hack Criterion)
/// - [Primer, Algorithm 6.19](crate::literature#algorithm-619--maximal-siphontrap-in-a-subset) (maximal siphon/trap in a subset)
pub fn commoner_hack_criterion(
    net: &DenseNet,
    marking: &IdxMarking<u32>,
) -> Option<CommonerHackCriterionResult> {
    let MinimalSiphons { siphons, complete } = minimal_siphons(net);

    let checked = siphons.into_iter().try_fold(Vec::new(), |mut acc, siphon| {
        let trap = maximal_trap_in(net, siphon.clone());
        let trap_is_marked = !trap.is_clear() && trap.ones().any(|p_idx| marking[p_idx] > 0);
        if trap_is_marked {
            acc.push(SiphonTrapPair { siphon, trap });
            Ok(acc)
        } else {
            Err(SiphonTrapPair { siphon, trap })
        }
    });

    match checked {
        // A siphon with no marked trap is a definitive counterexample, even if the
        // enumeration was truncated: the witness is a genuine siphon.
        Err(counterexample) => Some(Err(counterexample)),
        // Every enumerated siphon contained a marked trap. Conclusive only if the
        // enumeration was exhaustive; otherwise an unseen siphon might violate it.
        Ok(pairs) if complete => Some(Ok(pairs.into_boxed_slice())),
        Ok(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::class::NetClass;

    fn boxed(rows: Vec<Vec<usize>>) -> Box<[Box<[usize]>]> {
        rows.into_iter().map(Vec::into_boxed_slice).collect()
    }

    /// `n` places, each with its own self-loop transition `t_i : p_i → p_i`.
    ///
    /// Every subset of places is a siphon here, so the backtracking search visits
    /// the entire subset lattice (`2^n` sets) — the pathological case the work
    /// bound exists to contain.
    fn self_loop_net(n: usize) -> DenseNet {
        let singletons = || boxed((0..n).map(|i| vec![i]).collect());
        DenseNet {
            class: NetClass::General,
            is_strongly_connected: false,
            preset_t: singletons(),
            postset_t: singletons(),
            preset_p: singletons(),
            postset_p: singletons(),
        }
    }

    /// `p0 → t0 → p1 → t1 → p0`. Its only siphon is the whole place set `{p0, p1}`,
    /// which is also a trap.
    fn two_place_cycle() -> DenseNet {
        DenseNet {
            class: NetClass::General,
            is_strongly_connected: true,
            preset_t: boxed(vec![vec![0], vec![1]]),  // t0 consumes p0, t1 consumes p1
            postset_t: boxed(vec![vec![1], vec![0]]), // t0 produces p1, t1 produces p0
            preset_p: boxed(vec![vec![1], vec![0]]),  // p0 produced by t1, p1 produced by t0
            postset_p: boxed(vec![vec![0], vec![1]]), // p0 consumed by t0, p1 consumed by t1
        }
    }

    #[test]
    fn minimal_siphons_complete_on_small_net() {
        let result = minimal_siphons(&two_place_cycle());
        assert!(result.complete, "small net should enumerate exhaustively");
        assert_eq!(result.siphons.len(), 1);
        let siphon = &result.siphons[0];
        assert_eq!(siphon.count_ones(..), 2);
        assert!(siphon.contains(0) && siphon.contains(1));
    }

    #[test]
    fn minimal_siphons_bounded_on_pathological_net() {
        // 2^20 siphons would take millions of shrink operations; the bound must stop
        // it and report an incomplete (but sound) result rather than hang.
        let result = minimal_siphons(&self_loop_net(20));
        assert!(!result.complete, "enumeration should hit the work bound");
    }

    #[test]
    fn commoner_hack_decides_completed_small_net() {
        let net = two_place_cycle();
        // Marked at p0: the sole siphon {p0,p1} is a marked trap → criterion holds.
        assert!(matches!(
            commoner_hack_criterion(&net, &IdxMarking(vec![1u32, 0])),
            Some(Ok(_))
        ));
        // Fully unmarked: that siphon's maximal trap is unmarked → definitive counterexample.
        assert!(matches!(
            commoner_hack_criterion(&net, &IdxMarking(vec![0u32, 0])),
            Some(Err(_))
        ));
    }

    #[test]
    fn commoner_hack_inconclusive_when_enumeration_bounded() {
        // Every place marked, so no enumerated siphon is a counterexample; but the
        // enumeration is truncated, so the honest answer is `None` (inconclusive),
        // never a spurious `Ok`/`Err`.
        let net = self_loop_net(20);
        let marking = IdxMarking(vec![1u32; 20]);
        assert!(commoner_hack_criterion(&net, &marking).is_none());
    }
}
