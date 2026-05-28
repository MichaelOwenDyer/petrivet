use crate::oracle::{Examination, Verdict};
use petrivet::model::LivenessMethod;
use petrivet::PetriNet;

/// Outcome of running petrivet on one (model, examination) pair.
#[derive(Debug)]
pub enum RunResult {
    /// petrivet produced definitive verdicts.
    Verdicts(Box<[Verdict]>),
    /// petrivet opted out of this examination or model.
    DoNotCompete,
    /// petrivet tried but failed (parse error, overflow, etc.).
    CannotCompute,
}

/// A single correctness failure: expected X, got Y for a named verdict.
#[derive(Debug)]
pub struct Mismatch {
    pub model_id: String,
    pub examination: Examination,
    pub name: String,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}/{}: expected {} got {}",
            self.model_id, self.examination, self.name, self.expected, self.actual
        )
    }
}

/// Run petrivet analysis on an already-loaded system.
/// This is the measured function for benchmarks (PNML loading excluded).
///
/// To add support for a new examination, add a match arm here.
#[must_use]
pub fn run_analysis(sys: &PetriNet, examination: Examination) -> RunResult {
    match examination {
        Examination::StateSpace => {
            match sys.try_build_reachability_graph() {
                Ok(rg) => {
                    RunResult::Verdicts(Box::new([
                        Verdict::StateSpace {
                            name: "STATES".to_string(),
                            value: Some(rg.marking_count() as u64),
                        },
                        Verdict::StateSpace {
                            name: "TRANSITIONS".to_string(),
                            value: Some(rg.transition_count() as u64),
                        },
                        Verdict::StateSpace {
                            name: "MAX_TOKEN_PER_MARKING".to_string(),
                            value: Some(u64::from(rg.max_token_per_marking())),
                        },
                        Verdict::StateSpace {
                            name: "MAX_TOKEN_IN_PLACE".to_string(),
                            value: Some(u64::from(rg.max_token_in_any_place())),
                        },
                    ]))
                },
                Err(_cg) => RunResult::Verdicts(Box::new([])),
            }
        },

        Examination::ReachabilityDeadlock => {
            RunResult::Verdicts(Box::new([
                Verdict::Formula {
                    name: examination.to_string(),
                    value: Some(sys.has_reachable_deadlock_marking()),
                }
            ]))
        },

        Examination::OneSafe => {
            let result = if sys.is_structurally_one_safe() {
                true
            } else {
                !sys.has_reachable_unsafe_marking()
            };
            RunResult::Verdicts(Box::new([
                Verdict::Formula {
                    name: examination.to_string(),
                    value: Some(result),
                }
            ]))
        }

        Examination::QuasiLiveness => match sys.try_build_reachability_graph() {
            Ok(rg) => {
                RunResult::Verdicts(Box::new([
                    Verdict::Formula {
                        name: examination.to_string(),
                        value: Some(rg.is_quasi_live()),
                    }
                ]))
            },
            Err(_) => RunResult::DoNotCompete,
        },

        Examination::StableMarking => match sys.try_build_reachability_graph() {
            Ok(rg) => RunResult::Verdicts(Box::new([
                Verdict::Formula {
                    name: examination.to_string(),
                    value: Some(rg.has_stable_place()),
                }
            ])),
            Err(_) => RunResult::DoNotCompete,
        },

        Examination::Liveness => {
            let liveness = sys.analyze_liveness();
            if liveness.method == LivenessMethod::Inconclusive {
                return RunResult::DoNotCompete;
            }
            RunResult::Verdicts(Box::new([
                Verdict::Formula {
                    name: examination.to_string(),
                    value: Some(liveness.global_level().is_live()),
                }
            ]))
        }

        // Examinations not yet implemented by petrivet.
        // Add arms here as petrivet gains support for new examinations.
        _ => RunResult::DoNotCompete,
    }
}

/// Compare expected oracle verdicts against a `RunResult`.
/// Returns an empty `Vec` if all expected verdicts are satisfied or skipped.
#[must_use]
pub fn compare_verdicts(
    model_id: &str,
    examination: Examination,
    expected: &[Verdict],
    actual: &[Verdict],
) -> Vec<Mismatch> {
    expected
        .iter()
        .filter_map(|exp| {
            match exp {
                Verdict::StateSpace {
                    name,
                    value: Some(exp_val),
                } => {
                    let act_val = actual.iter().find_map(|v| match v {
                        Verdict::StateSpace {
                            name: n,
                            value: Some(v),
                        } if n == name => Some(*v),
                        _ => None,
                    });
                    match act_val {
                        Some(got) if got == *exp_val => None,
                        Some(got) => Some(Mismatch {
                            model_id: model_id.to_string(),
                            examination,
                            name: name.clone(),
                            expected: exp_val.to_string(),
                            actual: got.to_string(),
                        }),
                        None => Some(Mismatch {
                            model_id: model_id.to_string(),
                            examination,
                            name: name.clone(),
                            expected: exp_val.to_string(),
                            actual: "missing".to_string(),
                        })
                    }
                }
                Verdict::Formula {
                    name,
                    value: Some(exp_val),
                } => {
                    let act_val = actual.iter().find_map(|v| match v {
                        Verdict::Formula {
                            name: n,
                            value: Some(v),
                        } if n == name => Some(*v),
                        _ => None,
                    });
                    match act_val {
                        Some(got) if got == *exp_val => None,
                        Some(got) => Some(Mismatch {
                            model_id: model_id.to_string(),
                            examination,
                            name: name.clone(),
                            expected: if *exp_val { "TRUE" } else { "FALSE" }.to_string(),
                            actual: if got { "TRUE" } else { "FALSE" }.to_string(),
                        }),
                        None => None,
                    }
                }
                // Oracle says "?" — any answer is acceptable.
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::Verdict;

    #[test]
    fn unsupported_examination_gives_do_not_compete() {
        let sys = tiny_system();
        assert!(matches!(
            run_analysis(&sys, Examination::CTLFireability),
            RunResult::DoNotCompete
        ));
    }

    #[test]
    fn compare_matching_verdicts_gives_no_mismatches() {
        let expected = vec![Verdict::Formula {
            name: "ReachabilityDeadlock".to_string(),
            value: Some(false),
        }];
        let sys = tiny_system();
        let RunResult::Verdicts(actual) = run_analysis(&sys, Examination::ReachabilityDeadlock) else {
            panic!("expected Verdicts, got something else");
        };
        let mismatches = compare_verdicts(
            "M-PT-01",
            Examination::ReachabilityDeadlock,
            &expected,
            &actual
        );
        assert!(
            mismatches.is_empty(),
            "unexpected mismatches: {mismatches:?}"
        );
    }

    fn tiny_system() -> PetriNet {
        use petrivet::NetBuilder;
        let mut b = NetBuilder::new();
        let [p0, p1] = b.add_places();
        let [t0, t1] = b.add_transitions();
        b.add_arc((p0, t0));
        b.add_arc((t0, p1));
        b.add_arc((p1, t1));
        b.add_arc((t1, p0));
        let net = b.build().unwrap();
        PetriNet::new(net, [(p0, 1)])
    }
}
