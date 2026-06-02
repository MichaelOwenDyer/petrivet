use crate::core::analysis::siphon_trap;
use crate::core::mapping::DenseMapping;
use crate::net::siphon_trap::{Siphon, Trap};
use crate::net::Net;
use crate::prelude::PetriNet;

/// Represents fulfillment of the Commoner/Hack criterion,
/// which states that every siphon in the net contains a trap marked at the initial marking.
/// Every minimal siphon and its maximal contained trap are provided as evidence for this result.
pub type CommonerHackCriterion = Box<[SiphonTrapPair]>;

/// Represents a counterexample to the Commoner/Hack criterion:
/// a siphon which contains no trap marked at the initial marking.
pub type UnmarkedSiphonTrapPair = SiphonTrapPair;

/// Result of the Commoner/Hack criterion check.
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
pub type CommonerHackCriterionResult = Result<CommonerHackCriterion, UnmarkedSiphonTrapPair>;

/// A minimal siphon and the maximal trap found within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiphonTrapPair {
    /// A minimal siphon in the net.
    pub siphon: Siphon,
    /// The maximal trap contained in this siphon (a set of places Q with Q• ⊆ •Q).
    /// Empty if no trap was found.
    pub trap: Trap,
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// Checks the Commoner/Hack criterion, which is fulfilled when all siphons in the system
    /// contain a trap marked at the initial marking.
    /// This is a necessary and sufficient condition for liveness in free-choice nets,
    /// and a sufficient condition for deadlock-freedom in general nets.
    pub fn commoner_hack_criterion(&self) -> CommonerHackCriterionResult {
        fn to_api(mapping: &DenseMapping, pair: siphon_trap::SiphonTrapPair) -> SiphonTrapPair {
            SiphonTrapPair {
                siphon: pair.siphon.into_iter().map(|p_idx| mapping.place(p_idx)).collect(),
                trap: pair.trap.into_iter().map(|p_idx| mapping.place(p_idx)).collect(),
            }
        }

        siphon_trap::commoner_hack_criterion(&self.dense_net, &self.marking)
            .map(|siphon_trap_pairs| {
                siphon_trap_pairs.into_iter().map(|pair| to_api(&self.mapping, pair)).collect()
            })
            .map_err(|counterexample| {
                to_api(&self.mapping, counterexample)
            })
    }
}