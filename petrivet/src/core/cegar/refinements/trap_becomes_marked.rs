use crate::core::analysis::siphon_trap::maximal_trap_in;
use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::SmtSolver;
use crate::core::marking::IdxMarking;
use crate::core::parikh::IdxParikhVector;
use crate::core::siphon_trap::IdxTrap;
use fixedbitset::FixedBitSet;

/// This rule identifies spurious solutions of marking + Parikh vector where the candidate
/// marking contains an unmarked trap that firing the Parikh vector's support would necessarily
/// have marked.
///
/// A real firing sequence realizing a Parikh vector fires *exactly* the transitions in its
/// support `U` (every transition with a positive firing count, and no other). So if we can find
/// a trap `Q` which is completely empty in the candidate marking, but which some transition in `U`
/// would feed, that's a contradiction: firing `U` must put a token into `Q` at some point, and
/// once fed, `Q` can never be emptied again, so it cannot possibly be empty in the final marking.
pub struct TrapBecomesMarkedRule;

impl TrapBecomesMarkedRule {
    pub fn check(
        problem: &CegarProblem,
        candidate_marking: &IdxMarking<u32>,
        candidate_parikh_vector: &IdxParikhVector<u32>,
    ) -> Option<TrapBecomesMarkedRefinement> {
        let net = problem.net;

        // U = support of the candidate Parikh vector: the transitions a real firing sequence
        // realizing it would actually fire.
        let mut support = FixedBitSet::with_capacity(net.transition_count());
        for t_idx in net.transition_indices() {
            support.set(t_idx, candidate_parikh_vector[t_idx] > 0);
        }
        if support.is_clear() {
            return None;
        }

        // The places of the subnet induced by U (•U ∪ U•), restricted to those unmarked in the
        // candidate marking - a marked place can never be part of an *unmarked* trap.
        let mut places = FixedBitSet::with_capacity(net.place_count());
        for t_idx in support.ones() {
            for &p in net.preset_t[t_idx].iter().chain(net.postset_t[t_idx].iter()) {
                if candidate_marking[p] == 0 {
                    places.insert(p);
                }
            }
        }

        // The maximal trap of the whole net contained within that subnet.
        let trap = maximal_trap_in(net, places);
        if trap.is_clear() {
            return None;
        }

        // The transitions which produce into the trap (•trap, over the whole net). Once any of
        // them fires, the trap can never be unmarked again.
        let mut feeders = FixedBitSet::with_capacity(net.transition_count());
        for p in trap.ones() {
            for &t in &net.preset_p[p] {
                feeders.insert(t);
            }
        }

        // Only report this if firing U is actually what would feed the trap. Otherwise `trap`
        // is really just an unmarked trap of the whole net unrelated to this candidate - not
        // what this rule is meant to find (and if it happens to also be marked in `m0`,
        // `InitiallyMarkedTrapRule` already covers that case independently of any Parikh vector).
        if feeders.is_disjoint(&support) {
            return None;
        }

        Some(TrapBecomesMarkedRefinement {
            trap,
            feeders,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TrapBecomesMarkedRefinement {
    /// A trap of the whole net which is unmarked in the candidate marking.
    trap: IdxTrap,
    /// The transitions which produce into `trap` (i.e. •trap, over the whole net).
    feeders: FixedBitSet,
}

impl TrapBecomesMarkedRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        place_terms: &[S::Int],
        transition_terms: &[S::Int],
    ) {
        let trap_sum = solver.add(self.trap.ones().map(|p| place_terms[p].clone()));
        let zero = solver.mk_int(0);
        let trap_marked = solver.gt(&trap_sum, &zero);
        for t_idx in self.feeders.ones() {
            let fires = solver.gt(&transition_terms[t_idx], &zero);
            let implication = solver.implies(&fires, &trap_marked);
            solver.assert_tracked(
                &implication,
                IdxLemma::TrapBecomesMarked { feeder: t_idx, trap: self.trap.clone() },
            );
        }
    }
}
