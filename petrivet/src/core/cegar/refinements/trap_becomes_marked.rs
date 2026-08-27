use crate::core::analysis::siphon_trap::maximal_trap_in;
use crate::core::cegar::cegar::CegarProblem;
use crate::core::cegar::lemma::IdxLemma;
use crate::core::cegar::solver::SmtSolver;
use crate::core::marking::IdxMarking;
use crate::core::parikh::IdxParikhVector;
use crate::core::siphon_trap::IdxTrap;
use fixedbitset::FixedBitSet;

/// This rule identifies spurious marking + Parikh vector pairs where the candidate
/// marking contains an unmarked trap which, after firing the Parikh vector's support,
/// would necessarily have to have become marked.
///
/// A firing sequence realizing a Parikh vector fires the transitions in the vector's support `U`
/// (every transition with a positive firing count) and no others.
/// If we can find a trap `Q` which is completely empty in the candidate marking, but which some
/// transition in `U` would feed, that is a contradiction: firing `U` must put a token into `Q`,
/// marking `Q` forever.
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

        // The unmarked places in the subnet induced by U (•U ∪ U•)
        let mut unmarked_places_in_subnet = FixedBitSet::with_capacity(net.place_count());
        for t_idx in support.ones() {
            for &p in net.preset_t[t_idx].iter().chain(net.postset_t[t_idx].iter()) {
                if candidate_marking[p] == 0 {
                    unmarked_places_in_subnet.insert(p);
                }
            }
        }

        // The maximal trap of the whole net contained within that subnet.
        let trap = maximal_trap_in(net, unmarked_places_in_subnet);
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

        // If any of the traps' feeders is in the support of the candidate Parikh vector,
        // then the candidate marking is spurious: firing the Parikh vector would necessarily
        // put a token into the trap, marking it forever.
        (!feeders.is_disjoint(&support)).then_some(TrapBecomesMarkedRefinement {
            trap,
            feeders,
        })
    }
}

/// A refinement that encodes that a trap must be marked if any of its feeder transitions fire.
#[derive(Debug, Clone)]
pub struct TrapBecomesMarkedRefinement {
    /// A trap which is unmarked in the current candidate marking.
    trap: IdxTrap,
    /// The transitions which produce into `trap`.
    /// If any of these fire, the trap may not remain unmarked.
    feeders: FixedBitSet,
}

impl TrapBecomesMarkedRefinement {
    pub fn encode_into<S: SmtSolver>(
        self,
        solver: &mut S,
        place_terms: &[S::Int],
        transition_terms: &[S::Int],
        callback: Option<&dyn Fn(IdxLemma)>,
    ) {
        let trap_sum = solver.add(self.trap.ones().map(|p| place_terms[p].clone()));
        let zero = solver.mk_int(0);
        let trap_marked = solver.gt(&trap_sum, &zero);
        for t_idx in self.feeders.ones() {
            let fires = solver.gt(&transition_terms[t_idx], &zero);
            let implication = solver.implies(&fires, &trap_marked);
            let lemma = IdxLemma::TrapBecomesMarked { feeder: t_idx, trap: self.trap.clone() };
            if let Some(callback) = callback {
                callback(lemma.clone());
            }
            solver.assert_tracked(&implication, lemma);
        }
    }
}
