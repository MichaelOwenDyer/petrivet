//! M2 · C6 — an interoperable, machine-checkable certificate **format**.
//!
//! Petri-net model checking lacks the DRAT/LRAT/GRAT/VeriPB analogue the SAT/ILP
//! world has. Petrivet's proof objects are already owned and borrow-free, so the
//! format is one `serde` derive and one net-anchoring convention away. This
//! module defines the canonical envelope
//! `Cert = (net_id, query, polarity, witness, theorem_id)` and proves it
//! round-trips through `serde`.
//!
//! # Why the witness is *name-anchored*, not handle-anchored
//!
//! A live witness ([`super::checkers`]) holds [`Transition`](crate::net::Transition)
//! / [`Place`](crate::net::Place) handles, which are `NonZeroU32` newtypes with
//! crate-private fields and *no* serde derive — by design, because a raw handle
//! is meaningful only relative to the net snapshot that minted it. To make a
//! certificate **tool-agnostic** (the C6/C7 goal: a certificate from a different
//! procedure, or in principle another tool, checks identically), the format
//! anchors the witness to PNML place/transition **names** (here: `String` ids),
//! not internal indices. The envelope is therefore generic over a name-anchored
//! witness type `W` that is itself serializable.
//!
//! # Scope held at M2 (claim-honest)
//!
//! M2 delivers the **envelope and its round-trip** (the C6 `[UNIT]` gate), plus
//! the name-anchored witness shapes for the M2 checkers. The remaining C6
//! breadth — a `to_named` bridge that walks a live `Certificate`'s handles
//! through the net's [`Mapping`](crate::core::mapping) to PNML names, and a
//! `from_named` that re-resolves names to handles re-establishing the
//! canonical-marking invariants on the way back in — is the deserialize-then-
//! re-check path that funnels a parsed certificate back through `accept` (named
//! in the M1 record as C6/M2 follow-on). It depends on exposing a stable name
//! anchor on the public net surface, which is itself a small core-API addition;
//! recording the dependency keeps it re-derivable (doctrine #8). The envelope
//! below is `serde`-gated and stands ready for that bridge.

#![cfg(feature = "serde")]

use super::frontier::Polarity;

/// A name-anchored query: the property question, with target markings expressed
/// as `(place_name, count)` pairs so the certificate is independent of any net's
/// internal indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NamedQuery {
    /// A query-free global property (liveness, boundedness, deadlock-freedom).
    Trivial,
    /// Is the named target marking reachable from `M₀`?
    Reachable(Vec<(String, u32)>),
    /// Is the named target marking coverable from `M₀`?
    Coverable(Vec<(String, u32)>),
}

/// The canonical certificate envelope: `(net_id, query, polarity, witness,
/// theorem_id)`.
///
/// Generic over a name-anchored, serializable witness `W`. The C1 checkers
/// consume the *handle* form; this is the offline-checkable, tool-agnostic
/// *wire* form. A hand-authored envelope for a verdict produced by a different
/// procedure deserializes and (once the `from_named` bridge lands) re-checks
/// identically against the original net.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cert<W> {
    /// A stable identifier for the net the certificate is about (e.g. the PNML
    /// net id), so a checker can confirm it is validating against the right net.
    pub net_id: String,
    /// The property question this certificate answers, name-anchored.
    pub query: NamedQuery,
    /// Which pole the witness establishes.
    pub polarity: Polarity,
    /// The name-anchored witness payload.
    pub witness: W,
    /// The theorem the verdict rests on (e.g. "MarkedGraph-marking-equation
    /// (Thm 5.2.7)"), so a reader knows *why* the witness suffices.
    pub theorem_id: String,
}

/// A name-anchored firing word: the transition *names* fired, in order. The wire
/// form of [`FiringSequenceCert`](super::checkers::FiringSequenceCert).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedFiringWord {
    /// The transition names, in firing order.
    pub transitions: Vec<String>,
}

/// A name-anchored Parikh vector: `(transition_name, firing_count)` pairs. The
/// wire form of [`ParikhVectorCert`](super::checkers::ParikhVectorCert).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedParikhVector {
    /// The firing counts, keyed by transition name (sorted for canonical form).
    pub counts: Vec<(String, u32)>,
}

/// A name-anchored siphon/trap cover: a list of `(siphon, trap)` pairs of place
/// names. The wire form of
/// [`SiphonTrapCoverCert`](super::checkers::SiphonTrapCoverCert).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedSiphonTrapCover {
    /// Each `(siphon_place_names, trap_place_names)` pair, names sorted for a
    /// canonical form.
    pub pairs: Vec<(Vec<String>, Vec<String>)>,
}

impl Polarity {
    /// A short wire label for the polarity, for human-readable serializations.
    #[must_use]
    pub const fn wire_label(self) -> &'static str {
        match self {
            Self::ProveYes => "prove-yes",
            Self::ProveNo => "prove-no",
            Self::Exact => "exact",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Cert, NamedFiringWord, NamedParikhVector, NamedQuery, NamedSiphonTrapCover,
    };
    use crate::model::frontier::Polarity;

    /// `[UNIT]` — a firing-word certificate envelope round-trips through serde.
    #[test]
    fn firing_word_cert_round_trips() {
        let cert = Cert {
            net_id: "philosophers-3".to_string(),
            query: NamedQuery::Reachable(vec![("eating_1".to_string(), 1)]),
            polarity: Polarity::ProveYes,
            witness: NamedFiringWord {
                transitions: vec!["pickup_left_1".to_string(), "pickup_right_1".to_string()],
            },
            theorem_id: "firing-word-replay".to_string(),
        };
        let json = serde_json::to_string(&cert).expect("serialize");
        let restored: Cert<NamedFiringWord> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cert, restored);
    }

    /// A Parikh-vector envelope round-trips.
    #[test]
    fn parikh_cert_round_trips() {
        let cert = Cert {
            net_id: "marked-graph".to_string(),
            query: NamedQuery::Reachable(vec![
                ("p0".to_string(), 1),
                ("p2".to_string(), 1),
            ]),
            polarity: Polarity::ProveYes,
            witness: NamedParikhVector {
                counts: vec![("t0".to_string(), 1)],
            },
            theorem_id: "MarkedGraph-marking-equation (Thm 5.2.7)".to_string(),
        };
        let json = serde_json::to_string(&cert).expect("serialize");
        let restored: Cert<NamedParikhVector> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cert, restored);
    }

    /// A siphon/trap-cover envelope (query-free, Trivial) round-trips.
    #[test]
    fn siphon_trap_cover_cert_round_trips() {
        let cert = Cert {
            net_id: "mutex".to_string(),
            query: NamedQuery::Trivial,
            polarity: Polarity::ProveYes,
            witness: NamedSiphonTrapCover {
                pairs: vec![(
                    vec!["p0".to_string(), "p1".to_string()],
                    vec!["p0".to_string(), "p1".to_string()],
                )],
            },
            theorem_id: "Commoner-Hack (Thm 5.17)".to_string(),
        };
        let json = serde_json::to_string(&cert).expect("serialize");
        let restored: Cert<NamedSiphonTrapCover> =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cert, restored);
    }

    /// A hand-authored certificate (a string a *different tool* could have
    /// emitted) deserializes into the envelope — the tool-agnostic property in
    /// its wire form. (Re-checking it against the original net needs the
    /// `from_named` handle-resolution bridge, deferred to the C6 follow-on; this
    /// proves the *format* admits a foreign certificate.)
    #[test]
    fn a_hand_authored_certificate_deserializes() {
        let hand_authored = r#"{
            "net_id": "some-other-tool-net",
            "query": { "Reachable": [["place_a", 2]] },
            "polarity": "ProveYes",
            "witness": { "transitions": ["fire_x", "fire_y", "fire_x"] },
            "theorem_id": "firing-word-replay"
        }"#;
        let cert: Cert<NamedFiringWord> =
            serde_json::from_str(hand_authored).expect("a foreign certificate deserializes");
        assert_eq!(cert.net_id, "some-other-tool-net");
        assert_eq!(cert.witness.transitions.len(), 3);
        assert_eq!(cert.polarity, Polarity::ProveYes);
    }
}
