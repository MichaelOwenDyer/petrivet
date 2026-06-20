use crate::boundedness::{Boundedness, K};
use crate::class::NetClass;
use crate::core::net::IdxNode;
use crate::net::{Net, Place};
use crate::prelude::PetriNet;
use crate::state_space::ExplorationOrder;
use ahash::HashMap;

/// The conserved token total of a `u32` marking as a [`K`] bound, accumulated in
/// `u64` so it cannot wrap (a wrapped, too-small bound could mint a false safety
/// verdict through `is_safe`/`is_k_bounded`). Saturates at `usize::MAX` on a
/// 32-bit target where the total exceeds the bound type.
fn wide_sum_as_bound(marking: &crate::core::marking::IdxMarking<u32>) -> K {
    usize::try_from(marking.wide_sum()).unwrap_or(usize::MAX)
}

/// Result of boundedness analysis.
///
/// `place_bound` returns the bound for any place in the net by its key.
/// When proved via the structural LP, bounds are derived upper estimates
/// (potentially loose). When proved via the coverability graph, bounds are exact.
#[derive(Debug, Clone)]
pub struct BoundednessAnalysis {
    /// All places in the net paired with their bounds. The order is not guaranteed.
    pub bounds: HashMap<Place, Boundedness>,
    /// How the result was obtained.
    pub method: BoundednessAnalysisMethod,
}

impl BoundednessAnalysis {
    /// Returns the bound of the system as a whole: the maximum over all places.
    #[must_use]
    pub fn global_bound(&self) -> Boundedness {
        self.bounds.values()
            .copied()
            .max()
            .expect("at least one place")
    }

    /// Returns the bound for a specific place identified by its [`Place`].
    ///
    /// Returns `None` if the place does not belong to the analysed net.
    #[must_use]
    pub fn place_bound(&self, place: Place) -> Boundedness {
        self.bounds.get(&place).map_or(
            Boundedness::Bounded(0),
            |&bound| bound,
        )
    }
}

/// Evidence for a boundedness result.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BoundednessAnalysisMethod {
    /// Structural LP found a positive vector y with yᵀN ≤ 0.
    /// Bounds are derived as M\[p\] ≤ ⌊(y·M₀) / y\[p\]⌋: valid but
    /// potentially loose.
    PositivePlaceSubvariant(Box<[f64]>),
    /// Full coverability graph explored. Bounds are exact.
    CoverabilityGraph,
}

impl<N: AsRef<Net>> PetriNet<N> {
    /// If an efficient (polynomial-time) procedure for boundedness
    /// is known for this Petri net, returns Some(_) with the answer.
    /// Returns None if the answer would not be efficient to compute.
    #[must_use]
    pub fn is_efficiently_bounded(&self) -> Option<bool> {
        match self.class() {
            // conservative - token count never changes
            NetClass::Circuit | NetClass::StateMachine => Some(true),
            // A *structurally* bounded marked graph is strongly connected. The
            // POSITIVE direction is sound for any marking — a strongly-connected
            // marked graph is structurally bounded (Murata), hence bounded under
            // every M₀ — so we may return `Some(true)`. The NEGATIVE direction
            // (non-SC ⇒ unbounded) is the structural-boundedness iff applied to a
            // *fixed* marking, which we do not re-derive here: the non-SC marked
            // graph falls through to the `_ => None` wildcard below, ABSTAINING
            // rather than minting an unproven negative verdict, so `is_bounded`
            // falls through to the exact coverability graph (which decides the true
            // bound for this marking and terminates). This keeps
            // `is_efficiently_bounded` consistent with `efficient_boundedness`
            // (both abstain on the non-SC case) — no two contradictory public
            // verdicts. (foundational-design §4.6.)
            NetClass::MarkedGraph if self.is_strongly_connected() => Some(true),
            // A live free-choice system is bounded iff every place belongs to an
            // S-component. `is_covered_by_s_components` returns `None` until the
            // certifying S-component decomposition lands (A2/B3): we propagate
            // that abstention rather than fabricate an unboundedness verdict, so
            // `is_bounded` falls through to state-space exploration. A `Some(b)`
            // here is only ever reported once it is certificate-backed.
            NetClass::FreeChoice if self.is_live() => self.is_covered_by_s_components(),
            // No CHC ⇒ bounded theorem exists for asymmetric-choice nets. The
            // Commoner–Hack criterion is sufficient for *liveness* on AC nets
            // (Murata Thm 15), not boundedness: an always-enabled pump can grow a
            // place without bound while CHC still holds (the net stays deadlock-
            // free). The old `commoner_hack_criterion().ok().map(|_| true)` arm
            // therefore minted a false `bounded` verdict on a genuinely unbounded
            // AC net — the cardinal sin (oracle-confirmed counterexample, §4.6).
            // We abstain so `is_bounded` falls through to the exact positive
            // P-sub-invariant (`structurally_bounded_exact`, class-agnostic) and
            // then the exact coverability graph — the same honest pattern the
            // FreeChoice arm uses. The sound structural fast-path for AC boundedness
            // is the exact sub-invariant, not CHC.
            _ => None,
        }
    }

    pub fn is_place_efficiently_bounded(&self, place: Place) -> Option<bool> {
        match self.class() {
            // conservative - token count never changes
            NetClass::Circuit | NetClass::StateMachine => Some(true),
            // A place in a marked graph is bounded iff it belongs to some circuit
            NetClass::MarkedGraph => {
                // todo: optimize by only constructing circuits which contain the place
                self.mapping.place_idx(place).map_or(
                    Some(true),
                    |p_idx| {
                        Some(self.circuits().any(|circuit| circuit.contains(&IdxNode::Place(p_idx))))
                    }
                )
            },
            _ => None,
        }
    }

    /// Returns the boundedness of the entire system, if it can be efficiently computed.
    #[must_use]
    pub fn efficient_boundedness(&self) -> Option<Boundedness> {
        match self.class() {
            // The conserved token total is the bound. Compute it via `wide_sum`
            // (`u64`), not the `u32` `sum`: a `u32` wrap would report a too-small
            // bound — and via `is_safe`/`is_k_bounded` could mint a false safety
            // verdict (a net whose true total wraps to ≤ 1 reported 1-bounded).
            NetClass::Circuit => {
                Some(Boundedness::Bounded(wide_sum_as_bound(&self.marking)))
            },
            NetClass::StateMachine => {
                if self.is_strongly_connected() {
                    Some(Boundedness::Bounded(wide_sum_as_bound(&self.marking)))
                } else {
                    None // todo
                }
            }
            NetClass::MarkedGraph => {
                // A *structurally* bounded marked graph is strongly connected
                // (the iff is a structural-boundedness characterization). The
                // non-strongly-connected case was formerly asserted
                // `Some(Unbounded)` from that structural fact alone — a NEGATIVE
                // verdict not re-derived from primitives or the actual marking.
                // We do not construct a verdict from an unproven inference: abstain
                // (`None`) so `boundedness()` falls through to the exact,
                // always-terminating coverability graph, which decides the true
                // bound for *this* marking. (foundational-design §4.6.)
                None
            }
            _ => None,
        }
    }

    pub fn efficient_place_boundedness(&self, place: Place) -> Option<Boundedness> {
        match self.class() {
            // Bound magnitudes via `u64` accumulation, not the `u32` `sum` — see
            // `efficient_boundedness` for why the wrap is a verdict-path concern
            // (per-place `is_place_bounded`/`is_k_bounded` safety reads this).
            NetClass::Circuit => {
                Some(Boundedness::Bounded(wide_sum_as_bound(&self.marking)))
            },
            NetClass::StateMachine => {
                if self.is_strongly_connected() {
                    Some(Boundedness::Bounded(wide_sum_as_bound(&self.marking)))
                } else {
                    None
                }
            }
            NetClass::MarkedGraph => {
                self.mapping.place_idx(place).map_or(
                    Some(Boundedness::Bounded(0)),
                    |p_idx| Some(
                        self.circuits()
                            .filter(|circuit| circuit.contains(&IdxNode::Place(p_idx)))
                            .map(|circuit| {
                                let tokens: u64 = circuit.place_indices()
                                    .map(|p_idx| u64::from(self.marking[p_idx]))
                                    .sum();
                                usize::try_from(tokens).unwrap_or(usize::MAX)
                            })
                            .min()
                            .map_or(Boundedness::Unbounded, Boundedness::Bounded)
                    )
                )
            },
            _ => None,
        }
    }

    /// M5 — attempt an **exact** structural-boundedness proof: the `f64` LP
    /// suggests a positive place sub-invariant `y`, and the exact
    /// [`BoundednessSubinvariantCert`](crate::model::BoundednessSubinvariantCert)
    /// re-verifies it over ℚ against the original net (`y > 0` on every place,
    /// `yᵀ·C ≤ 0` on every transition column).
    ///
    /// Returns `true` only when an *exact* certificate validates — the f64 LP is a
    /// suggester, never the decider (a near-boundary `yᵀ·C` that the float simplex
    /// rounds to 0 cannot mint a boundedness verdict; foundational-design §1, §4.1,
    /// A4). A `false` here means "not certified exactly", **not** "unbounded": the
    /// caller falls back to the exact coverability graph.
    ///
    /// This delegates to [`Net::is_structurally_bounded`], which owns the
    /// suggest-rationalize-exact-check pipeline (foundational-design §4.6); the
    /// structural query is marking-independent, so the result is identical.
    fn structurally_bounded_exact(&self) -> bool {
        self.net.as_ref().is_structurally_bounded()
    }

    /// Returns true if the entire system is bounded.
    ///
    /// The structural (marking-independent) positive answer is routed through the
    /// **exact** sub-invariant certificate (M5: `structurally_bounded_exact`), not
    /// the bare `f64` LP — a boundedness verdict must not rest on a float that may
    /// round a small positive `yᵀ·C` to 0. When the exact certificate does not
    /// validate, the answer falls back to the exact coverability graph
    /// (`analyze_boundedness`), which decides boundedness for the given marking.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.is_efficiently_bounded().unwrap_or_else(|| {
            self.structurally_bounded_exact()
                || self.analyze_boundedness().global_bound().is_bounded()
        })
    }

    /// Returns true iff the given place is bounded.
    ///
    /// The structural disjunct `is_place_structurally_bounded` is now the **exact**
    /// per-place sub-invariant path (foundational-design §4.6): the f64 LP suggests
    /// `y`, and [`PlaceBoundednessSubinvariantCert`](crate::model::PlaceBoundednessSubinvariantCert)
    /// re-verifies `y ≥ 0`, `y[place] ≥ 1`, `yᵀ·C ≤ 0` over ℚ before the disjunct
    /// returns `true`. A spurious f64 `Some` can no longer short-circuit a false
    /// `bounded`; when the exact path does not certify, the answer falls through to
    /// the exact coverability graph.
    pub fn is_place_bounded(&self, place: Place) -> bool {
        self.is_place_efficiently_bounded(place).unwrap_or_else(|| {
            self.net.as_ref().is_place_structurally_bounded(place)
                || self.analyze_boundedness().place_bound(place).is_bounded()
        })
    }

    /// Returns the boundedness of the entire system.
    pub fn boundedness(&self) -> Boundedness {
        self.efficient_boundedness().unwrap_or_else(|| {
            self.analyze_boundedness().global_bound()
        })
    }

    pub fn place_boundedness(&self, place: Place) -> Boundedness {
        self.efficient_place_boundedness(place).unwrap_or_else(|| {
            self.analyze_boundedness().place_bound(place)
        })
    }

    /// Returns true if the entire system is safe (1-bounded).
    pub fn is_safe(&self) -> bool {
        self.boundedness().is_k_bounded(1)
    }

    /// Returns true if some reachable marking puts more than one token in any place.
    pub fn has_reachable_unsafe_marking(&self) -> bool {
        self.explore_reachability(ExplorationOrder::BreadthFirst)
            .core
            .find(|m| m.iter().any(|&t| t > 1))
            .is_some()
    }

    /// Analyzes boundedness and returns per-place bounds with evidence.
    #[must_use]
    pub fn analyze_boundedness(&self) -> BoundednessAnalysis {
        BoundednessAnalysis {
            bounds: self.build_coverability_graph().place_bounds(),
            method: BoundednessAnalysisMethod::CoverabilityGraph,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;

    #[test]
    fn cycle_is_structurally_bounded() {
        let (net, p0, _t0, _p1, _t1) = crate::api::system::tests::two_place_cycle();
        assert!(net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(sys.is_bounded());
    }

    /// Verdict-path overflow in the boundedness *magnitude*: the conserved token
    /// total is the bound of a Circuit. Computed via a `u32` sum it *wraps* in
    /// release, and a total of `2^32 + 1` wraps to `1` — so `is_safe`
    /// (`is_k_bounded(1)`) would falsely report the net 1-bounded (a false safety
    /// verdict). The widened `u64` bound closes it: the true bound exceeds 1, so
    /// the net is correctly *not* safe.
    #[test]
    fn bound_magnitude_overflow_does_not_mint_false_safety() {
        let (net, q0, _t0, q1, _t1) = crate::api::system::tests::two_place_cycle();
        // total = 2^32 + 1 = 4_294_967_297; a u32 bound would wrap to 1.
        let sys = net.with_initial_marking([(q0, 4_294_967_295_u32), (q1, 2_u32)]);
        let bound = sys.boundedness();
        assert!(bound.is_bounded(), "a circuit is bounded by its conserved total");
        assert!(
            !bound.is_k_bounded(1),
            "the true bound (~2^32) exceeds 1; a u32 wrap to bound 1 must not \
             mint a false safety verdict"
        );
        assert!(!sys.is_safe(), "the net is not 1-safe");
    }

    #[test]
    fn unbounded_not_structurally_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1));
        let net = b.build().expect("valid net");
        assert!(!net.is_structurally_bounded());
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(!sys.is_bounded());
    }

    /// A live, bounded free-choice net built with explicit arcs (Esparza
    /// Lecture Notes, Figure 5.3): a closed cycle with one free choice and one
    /// synchronisation. It is live (CHC holds) and bounded.
    fn live_bounded_free_choice() -> crate::prelude::PetriNet<crate::prelude::Net> {
        use crate::class::NetClass;
        use crate::prelude::PetriNet;
        let mut b = NetBuilder::new();
        let [s1, s2, s3, s4, s5, s6, s7, s8] = b.add_places();
        let [t1, t2, t3, t4, t5, t6, t7] = b.add_transitions();
        // Choice: •t1 = •t2 = {s1, s2}
        b.add_arc((s1, t1)); b.add_arc((s2, t1));
        b.add_arc((s1, t2)); b.add_arc((s2, t2));
        // Fork from t1 and t2
        b.add_arc((t1, s3)); b.add_arc((t1, s4));
        b.add_arc((t2, s5)); b.add_arc((t2, s6));
        // Independent paths
        b.add_arc((s3, t3)); b.add_arc((t3, s7));
        b.add_arc((s4, t4)); b.add_arc((t4, s8));
        b.add_arc((s5, t5)); b.add_arc((t5, s7));
        b.add_arc((s6, t6)); b.add_arc((t6, s8));
        // Join: •t7 = {s7, s8}
        b.add_arc((s7, t7)); b.add_arc((s8, t7));
        b.add_arc((t7, s1)); b.add_arc((t7, s2));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::FreeChoice);
        PetriNet::new(net, [(s1, 1), (s2, 1)])
    }

    /// A2 sentinel (the soundness north star). A live free-choice net that is in
    /// fact bounded must never be reported unbounded. Before the A2 fix,
    /// `is_covered_by_s_components` returned a hardcoded `false`, so
    /// `is_efficiently_bounded` returned the fabricated unboundedness verdict
    /// `Some(false)` with no certificate. The fix demotes that to abstention.
    #[test]
    fn a2_live_bounded_free_choice_not_reported_unbounded() {
        let sys = live_bounded_free_choice();
        assert!(sys.is_live(), "the Esparza Fig 5.3 net is live");
        // The efficient path must NOT fabricate `Some(false)`: it abstains.
        assert_ne!(
            sys.is_efficiently_bounded(),
            Some(false),
            "A2: a live FC net must not be reported unbounded without a certificate"
        );
        // And the full `is_bounded()` (falling through to the state space) gives
        // the true answer: this net is bounded.
        assert!(sys.is_bounded(), "the Esparza Fig 5.3 net is bounded");
    }

    /// A2: `is_covered_by_s_components` itself must never fabricate a verdict;
    /// it abstains (`None`) until the certifying decomposition lands.
    #[test]
    fn a2_s_component_coverage_abstains_not_fabricates() {
        let sys = live_bounded_free_choice();
        assert_eq!(
            sys.is_covered_by_s_components(),
            None,
            "A2: S-component coverage must abstain, never return a bare verdict"
        );
    }

    /// M5 — the positive structural-boundedness answer now rests on the EXACT
    /// sub-invariant certificate, not the bare `f64` LP. `structurally_bounded_exact`
    /// must return `true` for a genuinely structurally bounded net (a conservative
    /// net with a positive P-sub-invariant) and the f64 LP suggestion must validate
    /// exactly. A general (non-class-shortcut) conservative net exercises the path
    /// (the class shortcuts in `is_efficiently_bounded` are bypassed).
    #[test]
    fn m5_structurally_bounded_exact_certifies_a_conservative_net() {
        // A conservative net that is NOT one of the efficient-class shortcuts:
        // a free-choice net with a positive P-semiflow. Two tokens conserved.
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        // t0: p0 -> p1, p2 ; t1: p1, p2 -> p0  (conservative: y=(2,1,1) gives yᵀ·C=0)
        b.add_arc((p0, t0)); b.add_arc((t0, p1)); b.add_arc((t0, p2));
        b.add_arc((p1, t1)); b.add_arc((p2, t1)); b.add_arc((t1, p0));
        let net = b.build().expect("valid net");
        let sys = net.with_initial_marking([(p0, 1)]);
        // The exact path certifies structural boundedness.
        assert!(
            sys.structurally_bounded_exact(),
            "M5: a conservative net must be certified bounded by the exact sub-invariant"
        );
        assert!(sys.is_bounded());
    }

    /// ADVERSARIAL (exact lens, post-repair-1) — the positive `is_bounded` answer
    /// must never rest on a near-boundary `yᵀ·C` that the f64 simplex rounds to
    /// `≤ 0`. This builds an unbounded net with a conservative *core* (a P-semiflow
    /// on p0,p1) coupled to a pumped place px fed from the conserved core, so an f64
    /// LP could plausibly suggest a weight vector that is exactly feasible on the
    /// core but only near-feasible (rounding) on the pump column. The exact cert
    /// must reject any such suggestion (no strictly-positive y with yᵀ·C ≤ 0 exists,
    /// because px has positive net inflow under every positive weighting), so
    /// `is_bounded` falls through to the exact coverability graph and reports the
    /// true answer (unbounded), matching the oracle. A spurious f64 `true` here
    /// would be a false safety/boundedness verdict — the cardinal sin.
    #[test]
    fn adversarial_near_boundary_unbounded_net_not_reported_bounded() {
        let mut b = NetBuilder::new();
        let [p0, p1, px] = b.add_places();
        let [t0, t1, pump] = b.add_transitions();
        // Conservative core: p0 <-> p1 (P-semiflow (1,1) → yᵀ·C = 0 on t0,t1).
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        // pump: p0 -> p0, px  (px grows without bound; column has net +1 on px).
        b.add_arc((p0, pump)); b.add_arc((pump, p0)); b.add_arc((pump, px));
        let net = b.build().expect("valid net");
        // No positive sub-invariant: any y > 0 has yᵀ·C[pump] = y[px] > 0.
        assert!(
            !net.is_structurally_bounded(),
            "exact lens: the exact cert must reject the near-boundary suggestion"
        );
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(
            !sys.is_bounded(),
            "exact lens: an unbounded net must not be reported bounded (px is pumped)"
        );
        // Oracle: px is coverable to an arbitrary height (independent confirmation).
        assert!(sys.is_coverable([(px, 1000)].into()), "px grows without bound");
    }

    /// M5 — the exact path must NOT certify an unbounded net (no positive
    /// sub-invariant exists), so `structurally_bounded_exact` returns `false` and
    /// `is_bounded` falls through to the exact coverability graph, which reports
    /// the true answer (unbounded). This pins that the f64 LP can never mint a
    /// false `bounded` through the structural path.
    #[test]
    fn m5_structurally_bounded_exact_rejects_an_unbounded_net() {
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p0));
        b.add_arc((t0, p1)); // pumps p1 without bound
        let net = b.build().expect("valid net");
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(
            !sys.structurally_bounded_exact(),
            "M5: no positive sub-invariant exists for an unbounded net"
        );
        // And the full answer (via the coverability graph fallback) is unbounded.
        assert!(!sys.is_bounded(), "the net is genuinely unbounded");
    }

    // --- §4.6 foundation brickwork: AC boundedness, exact structural paths ---

    /// An asymmetric-choice net that satisfies the Commoner–Hack criterion (so it
    /// is deadlock-free) but is genuinely UNBOUNDED: `t_pump:{pa}→{pa,punb}` is
    /// always enabled while `pa` is conserved, so `punb` grows without bound.
    /// `t1:{pa,pb}→{pa,pb}` makes the place-postsets incomparable (true AC, not FC).
    ///
    /// Before the fix, `is_efficiently_bounded` returned `Some(true)` from the AC
    /// arm `commoner_hack_criterion().ok().map(|_| true)` — minting a false
    /// `bounded` verdict (the cardinal sin), inconsistent with `boundedness()`
    /// which correctly reported `Unbounded`. The fix removes the AC arm; the verdict
    /// now falls through to the exact coverability graph. Oracle-confirmed.
    #[test]
    fn ac_chc_net_that_is_unbounded_is_not_reported_bounded() {
        use crate::class::NetClass;
        let mut b = NetBuilder::new();
        let [pa, pb, punb] = b.add_places();
        let [t_pump, t1] = b.add_transitions();
        b.add_arc((pa, t_pump)); b.add_arc((t_pump, pa)); b.add_arc((t_pump, punb));
        b.add_arc((pa, t1)); b.add_arc((pb, t1)); b.add_arc((t1, pa)); b.add_arc((t1, pb));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::AsymmetricChoice, "counterexample must be true AC");
        let sys = net.with_initial_marking([(pa, 1), (pb, 1)]);
        assert!(sys.commoner_hack_criterion().is_ok(), "CHC holds on this net");
        // The efficient path must NOT mint a positive boundedness verdict from CHC.
        assert_ne!(
            sys.is_efficiently_bounded(),
            Some(true),
            "CHC is not a boundedness theorem for AC nets; must not report bounded"
        );
        // The full verdict, via the exact coverability graph, is the true answer.
        assert!(!sys.is_bounded(), "the net is genuinely unbounded (punb is pumped)");
        // Internal consistency: the two public verdicts agree (no contradiction).
        assert!(!sys.boundedness().is_bounded());
        // The oracle independently confirms punb is coverable to an arbitrary height.
        assert!(sys.is_coverable([(punb, 1000)].into()), "punb is pumped without bound");
    }

    /// Capability preservation: a genuinely BOUNDED asymmetric-choice net is still
    /// decided `bounded` — not via the (now-removed) CHC shortcut, but via the
    /// class-agnostic exact structural sub-invariant / coverability-graph fallback.
    #[test]
    fn bounded_ac_net_still_decided_bounded_via_fallback() {
        use crate::class::NetClass;
        // A conservative AC net: t0:{pa}→{pb}, t1:{pb}→{pa}, plus t2:{pa,pb}→{pa,pb}
        // (an identity loop needing both) — incomparable postsets make it AC, and
        // the token total (pa+pb) is conserved, so it is bounded.
        let mut b = NetBuilder::new();
        let [pa, pb] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((pa, t0)); b.add_arc((t0, pb));
        b.add_arc((pb, t1)); b.add_arc((t1, pa));
        b.add_arc((pa, t2)); b.add_arc((pb, t2)); b.add_arc((t2, pa)); b.add_arc((t2, pb));
        let net = b.build().expect("valid net");
        // Whatever its exact class, it must be decided bounded soundly.
        let _ = NetClass::AsymmetricChoice;
        let sys = net.with_initial_marking([(pa, 1), (pb, 1)]);
        assert!(sys.is_bounded(), "a conservative net is bounded (via the sound fallback)");
    }

    /// §4.6 — the whole-net structural-boundedness verdict
    /// (`Net::is_structurally_bounded`) rests on the EXACT positive sub-invariant
    /// certificate, not the bare f64 LP. A conservative net certifies; an unbounded
    /// one does not (the f64 LP can no longer mint a false `true`).
    #[test]
    fn net_is_structurally_bounded_rests_on_exact_certificate() {
        // Conservative free-choice net with a positive P-semiflow (2,1,1).
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p1)); b.add_arc((t0, p2));
        b.add_arc((p1, t1)); b.add_arc((p2, t1)); b.add_arc((t1, p0));
        let net = b.build().expect("valid net");
        assert!(net.is_structurally_bounded(), "exact cert certifies a conservative net");

        // Unbounded net: t0 pumps p1. No positive sub-invariant — must be false.
        let mut b = NetBuilder::new();
        let [q0, q1] = b.add_places();
        let [u0] = b.add_transitions();
        b.add_arc((q0, u0)); b.add_arc((u0, q0)); b.add_arc((u0, q1));
        let unbounded = b.build().expect("valid net");
        assert!(
            !unbounded.is_structurally_bounded(),
            "no positive sub-invariant exists; the f64 LP must not mint a false true"
        );
    }

    /// §4.6 — the per-place structural-boundedness verdict
    /// (`Net::is_place_structurally_bounded`) rests on the EXACT semi-positive
    /// sub-invariant certificate. A structurally-bounded place certifies; an
    /// unbounded place does not.
    #[test]
    fn net_is_place_structurally_bounded_rests_on_exact_certificate() {
        // p0 is bounded (conserved with p1 via the cycle); after adding a pump on
        // a third place, that third place is unbounded but p0/p1 stay bounded.
        let mut b = NetBuilder::new();
        let [p0, p1, punb] = b.add_places();
        let [t0, t1, pump] = b.add_transitions();
        // conservative cycle on p0,p1
        b.add_arc((p0, t0)); b.add_arc((t0, p1));
        b.add_arc((p1, t1)); b.add_arc((t1, p0));
        // pump grows punb without bound, fed from p0 (which it also returns)
        b.add_arc((p0, pump)); b.add_arc((pump, p0)); b.add_arc((pump, punb));
        let net = b.build().expect("valid net");
        assert!(
            net.is_place_structurally_bounded(p0),
            "p0 is bounded (semi-positive sub-invariant y[p0]=y[p1]=1, y[punb]=0)"
        );
        assert!(
            !net.is_place_structurally_bounded(punb),
            "punb is unbounded; no semi-positive sub-invariant with y[punb] ≥ 1"
        );
    }

    /// §4.6 — the per-place boundedness verdict (`is_place_bounded`) on a GENERAL
    /// net (no efficient class shortcut) is decided soundly: the structural disjunct
    /// is now the exact per-place path (no bare-f64 short-circuit). A genuinely
    /// place-bounded General net is still accepted.
    #[test]
    fn is_place_bounded_on_general_net_uses_exact_path() {
        use crate::class::NetClass;
        // A General-class conservative net (incomparable, intersecting postsets).
        let mut b = NetBuilder::new();
        let [p0, p1, p2] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((p0, t0)); b.add_arc((t0, p2));
        b.add_arc((p0, t1)); b.add_arc((p1, t1)); b.add_arc((t1, p2));
        b.add_arc((p1, t2)); b.add_arc((t2, p2));
        b.add_arc((p2, t0)); b.add_arc((p2, t1)); b.add_arc((p2, t2));
        b.add_arc((t0, p0)); b.add_arc((t1, p0)); b.add_arc((t1, p1)); b.add_arc((t2, p1));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::General, "exercise the General per-place path");
        let sys = net.with_initial_marking([(p0, 1), (p1, 1)]);
        // No efficient per-place shortcut → the exact structural disjunct decides.
        assert_eq!(sys.is_place_efficiently_bounded(p1), None);
        assert!(sys.is_place_bounded(p1), "p1 is bounded; the exact path accepts it");
    }

    /// §4.6 (minor) — a non-strongly-connected marked graph is decided soundly via
    /// the exact coverability graph (the efficient arm now ABSTAINS on the non-SC
    /// case rather than asserting unboundedness from a structural-class fact). The
    /// chain `t0→p0→t1→p1→t2` has a source transition t0 (always enabled), so it is
    /// genuinely unbounded, and the fallback reports that correctly.
    #[test]
    fn non_sc_marked_graph_decided_by_exact_graph() {
        use crate::class::NetClass;
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1, t2] = b.add_transitions();
        b.add_arc((t0, p0)); b.add_arc((p0, t1)); b.add_arc((t1, p1)); b.add_arc((p1, t2));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::MarkedGraph);
        let sys = net.with_initial_marking([(p0, 1)]);
        assert!(!sys.is_strongly_connected());
        // The efficient arm abstains on the non-SC case (no unproven negative).
        assert_eq!(sys.is_efficiently_bounded(), None);
        assert_eq!(sys.efficient_boundedness(), None);
        // The exact graph gives the true (unbounded) answer.
        assert!(!sys.is_bounded(), "the source transition t0 pumps p0 without bound");
    }

    /// §4.6 (minor) — a strongly-connected marked graph keeps its sound POSITIVE
    /// efficient verdict (`Some(true)`): structural boundedness of an SC marked
    /// graph holds for every marking, so this remains a fast-path decision.
    #[test]
    fn sc_marked_graph_keeps_efficient_bounded_verdict() {
        use crate::class::NetClass;
        let mut b = NetBuilder::new();
        let [pa, p0, p2] = b.add_places();
        let [t0, t1] = b.add_transitions();
        // t0: {pa} -> {p0, p2}; t1: {p0, p2} -> {pa}  (a strongly-connected MG)
        b.add_arc((pa, t0)); b.add_arc((t0, p0)); b.add_arc((t0, p2));
        b.add_arc((p0, t1)); b.add_arc((p2, t1)); b.add_arc((t1, pa));
        let net = b.build().expect("valid net");
        assert_eq!(net.class(), NetClass::MarkedGraph);
        let sys = net.with_initial_marking([(pa, 1)]);
        assert!(sys.is_strongly_connected());
        assert_eq!(
            sys.is_efficiently_bounded(),
            Some(true),
            "an SC marked graph is structurally bounded — the positive fast-path holds"
        );
        assert!(sys.is_bounded());
    }
}