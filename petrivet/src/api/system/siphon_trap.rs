use crate::core::net::siphon_trap::{ProperSiphonWithNoMarkedTrapResult, find_proper_siphon_with_no_marked_trap, maximal_siphon_in, maximal_trap_in};
use crate::core::solver::DefaultSolver;
use crate::net::siphon_trap::{Siphon, Trap};
use crate::net::{Net, Place};
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
/// - [Primer, Theorem 5.17]
pub type CommonerHackCriterionResult = Result<(), Box<(Siphon, Trap)>>;

impl<N: AsRef<Net>> PetriNet<N> {
    /// Checks the Commoner/Hack criterion:
    /// whether every proper siphon contains a trap marked under the initial marking, with no timeout.
    ///
    /// This is a necessary and sufficient condition for liveness in [free-choice nets],
    /// a sufficient condition for liveness in [asymmetric-choice nets],
    /// and a sufficient condition for deadlock-freedom in general.
    ///
    /// Returns `None` if no conclusion could be reached (the problem overwhelmed the solver).
    ///
    /// # Errors
    ///
    /// Returns `Err` with a `(Siphon, Trap)` counterexample if the criterion is violated:
    /// a siphon whose maximal trap is unmarked under the initial marking.
    ///
    /// # References:
    /// - [Best & Devillers 2024, Theorem 5.17](crate::literature#best--devillers-2024)
    /// - [Murata 1989, Theorem 12](crate::literature#murata-1989)
    /// - [Oanea et al. 2010](crate::literature#oanea-et-al--2010)
    ///
    /// [free-choice nets]: crate::net::class::NetClass::FreeChoice
    /// [asymmetric-choice nets]: crate::net::class::NetClass::AsymmetricChoice
    pub fn commoner_hack_criterion(&self) -> Option<CommonerHackCriterionResult> {
        self.commoner_hack_criterion_until(None)
    }

    /// Checks the Commoner/Hack criterion:
    /// whether every proper siphon contains a trap marked under the initial marking, with a timeout.
    ///
    /// This is a necessary and sufficient condition for liveness in [free-choice nets],
    /// a sufficient condition for liveness in [asymmetric-choice nets],
    /// and a sufficient condition for deadlock-freedom in general.
    ///
    /// Returns `None` if no conclusion could be reached (the problem overwhelmed the solver or timed out).
    ///
    /// # Errors
    ///
    /// Returns `Err` with a `(Siphon, Trap)` counterexample if the criterion is violated:
    /// a siphon whose maximal trap is unmarked under the initial marking.
    ///
    /// # References:
    /// - [Best & Devillers 2024, Theorem 5.17](crate::literature#best--devillers-2024)
    /// - [Murata 1989, Theorem 12](crate::literature#murata-1989)
    /// - [Oanea et al. 2010](crate::literature#oanea-et-al--2010)
    ///
    /// [free-choice nets]: crate::net::class::NetClass::FreeChoice
    /// [asymmetric-choice nets]: crate::net::class::NetClass::AsymmetricChoice
    pub fn commoner_hack_criterion_within(&self, timeout: std::time::Duration) -> Option<CommonerHackCriterionResult> {
        let deadline = std::time::Instant::now().checked_add(timeout);
        self.commoner_hack_criterion_until(deadline)
    }

    /// Checks the Commoner/Hack criterion with an optional deadline.
    pub(crate) fn commoner_hack_criterion_until(&self, deadline: Option<std::time::Instant>) -> Option<CommonerHackCriterionResult> {
        let result = find_proper_siphon_with_no_marked_trap::<DefaultSolver>(&self.dense_net, &self.marking, deadline);
        match result {
            ProperSiphonWithNoMarkedTrapResult::Found(siphon, trap) => Some(Err(Box::new((
                self.mapping.place_set(&siphon),
                self.mapping.place_set(&trap)
            )))),
            ProperSiphonWithNoMarkedTrapResult::None => Some(Ok(())),
            ProperSiphonWithNoMarkedTrapResult::Unknown => None,
        }
    }

    /// Computes the maximal [`Siphon`] contained in a given set of places.
    pub fn maximal_siphon_in(&self, places: &[Place]) -> Siphon {
        let idx_places = self.mapping.place_idx_set(places);
        let idx_siphon = maximal_siphon_in(&self.dense_net, idx_places);
        self.mapping.place_set(&idx_siphon)
    }

    /// Computes the maximal [`Trap`] contained in a given set of places.
    pub fn maximal_trap_in(&self, places: &[Place]) -> Trap {
        let idx_places = self.mapping.place_idx_set(places);
        let idx_trap = maximal_trap_in(&self.dense_net, idx_places);
        self.mapping.place_set(&idx_trap)
    }
}
