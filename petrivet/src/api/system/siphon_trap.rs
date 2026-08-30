use crate::core::net::siphon_trap;
use crate::core::solver::DefaultSolver;
use crate::net::Net;
use crate::net::siphon_trap::{Siphon, Trap};
use crate::system::PetriNet;

/// Checks the Commoner/Hack criterion:
/// whether every proper siphon contains a trap marked under the initial marking.
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
/// - [Murata 1989, Theorem 12]
/// - [Primer, Theorem 5.17](crate::literature#theorem-517--commonerhack-criterion-chc)
pub type CommonerHackCriterionResult = Result<(), Box<(Siphon, Trap)>>;

impl<N: AsRef<Net>> PetriNet<N> {
    /// Checks the Commoner/Hack criterion:
    /// whether every proper siphon contains a trap marked under the initial marking.
    ///
    /// This is a necessary and sufficient condition for liveness in free-choice nets,
    /// and a sufficient condition for deadlock-freedom in general nets.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a `(Siphon, Trap)` counterexample if the criterion is violated:
    /// a siphon whose maximal trap is unmarked under the initial marking.
    pub fn commoner_hack_criterion(&self) -> CommonerHackCriterionResult {
        siphon_trap::commoner_hack_criterion::<DefaultSolver>(&self.dense_net, &self.marking)
            .map_err(|counterexample| Box::new((
                self.mapping.place_set(&counterexample.siphon),
                self.mapping.place_set(&counterexample.trap)
            )))
    }
}
