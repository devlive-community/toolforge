use super::*;
use crate::model::{Model, normalize};
use crate::tests::Context;
use serde_json::json;
use std::collections::BTreeSet;

pub fn binary() -> Compiled {
    normalize(serde_json::from_value::<Model>(json!({"factors":[{"name":"A","values":[0,1]},{"name":"B","values":[0,1]},{"name":"C","values":[0,1]}]})).unwrap()).unwrap()
}

#[test]
fn every_feasible_pair_has_independent_coverage_evidence() {
    let report = solve(
        binary(),
        "review-1".into(),
        Limits::default(),
        None,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(report.status, "complete");
    assert!(report.cases.len() <= 8);
    // Reference oracle: enumerate tiny rows with independent nested loops.
    let mut expected = BTreeSet::new();
    for a in 0..2 {
        for b in 0..2 {
            for c in 0..2 {
                expected.extend([(0, a, 1, b), (0, a, 2, c), (1, b, 2, c)]);
            }
        }
    }
    let mut covered = BTreeSet::new();
    for row in &report.cases {
        assert_eq!(row.len(), 3);
        assert!(row.iter().all(|v| *v < 2));
        covered.extend([
            (0, row[0], 1, row[1]),
            (0, row[0], 2, row[2]),
            (1, row[1], 2, row[2]),
        ]);
    }
    assert_eq!(covered, expected);
    for pair in &report.pairs {
        assert_eq!(pair.state, "covered");
        for &id in &pair.cases {
            assert_eq!(report.cases[id - 1][pair.left], pair.left_value);
            assert_eq!(report.cases[id - 1][pair.right], pair.right_value);
        }
    }
}

#[test]
fn proves_global_infeasibility_beyond_direct_forbidden_pairs() {
    let mut model = binary().model;
    model.forbidden = serde_json::from_value(json!([[{"factor":"A","value":0},{"factor":"C","value":0}],[{"factor":"B","value":0},{"factor":"C","value":1}]])).unwrap();
    let report = solve(
        normalize(model).unwrap(),
        "1".into(),
        Limits::default(),
        None,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(report.status, "complete");
    let pair = report
        .pairs
        .iter()
        .find(|p| p.left == 0 && p.right == 1 && p.left_value == 0 && p.right_value == 0)
        .unwrap();
    assert_eq!(pair.state, "infeasible");
    assert!(report.cases.iter().all(|row| report.compiled.allowed(row)));
    assert_eq!(report.summary("1".into()).coverage_percent, Some(100.0));
}

#[test]
fn completed_results_are_deterministic_for_normalized_model_seed_and_version() {
    let mut variants = BTreeSet::new();
    for seed in ["one", "two", "three", "four"] {
        let a = solve(
            binary(),
            seed.into(),
            Limits::default(),
            None,
            &Context::default(),
        )
        .unwrap();
        let mut model = binary().model;
        model.factors.reverse();
        for f in &mut model.factors {
            f.values.reverse();
        }
        let b = solve(
            normalize(model).unwrap(),
            format!(" {seed} "),
            Limits::default(),
            None,
            &Context::default(),
        )
        .unwrap();
        assert_eq!(a.case_maps(), b.case_maps());
        variants.insert(serde_json::to_string(&a.case_maps()).unwrap());
    }
    assert!(variants.len() > 1);
}

#[test]
fn distinguishes_no_solution_from_unknown_feasibility() {
    let mut model = binary().model;
    model.forbidden =
        serde_json::from_value(json!([[{"factor":"A","value":0}],[{"factor":"A","value":1}]]))
            .unwrap();
    let report = solve(
        normalize(model).unwrap(),
        "1".into(),
        Limits::default(),
        None,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(report.status, "unsatisfiable");
    assert!(report.pairs.iter().all(|p| p.state == "infeasible"));
    assert_eq!(report.summary("1".into()).coverage_percent, None);
    let report = solve(
        binary(),
        "1".into(),
        Limits {
            max_combinations: 1,
            ..Limits::default()
        },
        None,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(report.status, "incomplete");
    assert_eq!(report.reason, Some("combinationLimit"));
    assert!(report.pairs.iter().any(|p| p.state == "unknown"));
    assert!(!report.pairs.iter().any(|p| p.state == "infeasible"));
    assert_eq!(report.summary("1".into()).coverage_percent, None);
}

#[test]
fn case_budget_reports_uncovered_pairs_without_claiming_completion() {
    let report = solve(
        binary(),
        "1".into(),
        Limits {
            max_cases: 1,
            ..Limits::default()
        },
        None,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(report.status, "incomplete");
    assert_eq!(report.reason, Some("caseLimit"));
    assert_eq!(report.cases.len(), 1);
    assert!(report.pairs.iter().any(|p| p.state == "uncovered"));
    assert!(!report.pairs.iter().any(|p| p.state == "unknown"));
}

#[test]
fn cancellation_and_timeouts_return_honest_partial_evidence() {
    let ctx = Context::default();
    ctx.cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let report = solve(binary(), "1".into(), Limits::default(), None, &ctx).unwrap();
    assert_eq!(report.reason, Some("cancelled"));
    assert_eq!(report.enumerated, 0);
    assert_eq!(report.summary("1".into()).unknown, 12);
    let ctx = Context {
        delay: true,
        ..Context::default()
    };
    let report = solve(
        binary(),
        "1".into(),
        Limits {
            budget_ms: 1,
            ..Limits::default()
        },
        None,
        &ctx,
    )
    .unwrap();
    assert_eq!(report.reason, Some("timeLimit"));
    assert_eq!(report.status, "incomplete");
}

#[test]
fn imported_cases_are_verified_and_never_silently_replaced() {
    let report = solve(
        binary(),
        "1".into(),
        Limits::default(),
        Some(vec![]),
        &Context::default(),
    )
    .unwrap();
    assert!(report.cases.is_empty());
    assert_eq!(report.summary("1".into()).uncovered, 12);
    assert_eq!(report.status, "incomplete");
    assert!(
        solve(
            binary(),
            "1".into(),
            Limits::default(),
            Some(vec![vec![0, 0]]),
            &Context::default()
        )
        .is_err()
    );
    assert!(
        solve(
            binary(),
            "1".into(),
            Limits::default(),
            Some(vec![vec![0, 0, 2]]),
            &Context::default()
        )
        .is_err()
    );
    let row = BTreeMap::from_iter([
        ("A".into(), json!(0)),
        ("B".into(), json!(0)),
        ("C".into(), json!("0")),
    ]);
    assert!(imported_cases(&binary(), vec![row]).is_err());
}

#[test]
fn rejects_invalid_solver_budgets_and_supports_single_value_factors() {
    for limits in [
        Limits {
            max_cases: 0,
            ..Limits::default()
        },
        Limits {
            budget_ms: 5001,
            ..Limits::default()
        },
        Limits {
            max_combinations: 0,
            ..Limits::default()
        },
    ] {
        assert!(solve(binary(), "1".into(), limits, None, &Context::default()).is_err());
    }
    let compiled = normalize(
        serde_json::from_value(
            json!({"factors":[{"name":"A","values":[null]},{"name":"B","values":[true]}]}),
        )
        .unwrap(),
    )
    .unwrap();
    let report = solve(
        compiled,
        "1".into(),
        Limits::default(),
        None,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(report.status, "complete");
    assert_eq!(report.cases.len(), 1);
}

#[test]
fn checks_sixty_four_constraint_models_against_a_separate_exhaustive_oracle() {
    let clauses = [
        vec![(0, 0)],
        vec![(1, 1)],
        vec![(0, 1), (1, 0)],
        vec![(1, 0), (2, 1)],
        vec![(0, 0), (1, 1), (2, 0)],
        vec![(0, 1), (1, 1), (2, 1)],
    ];
    for mask in 0..64 {
        let selected = clauses
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, c)| c.clone())
            .collect::<Vec<_>>();
        let mut model = binary().model;
        model.forbidden = selected
            .iter()
            .map(|c| {
                c.iter()
                    .map(|&(f, v)| crate::model::Term {
                        factor: ["A", "B", "C"][f].into(),
                        value: json!(v),
                    })
                    .collect()
            })
            .collect();
        let mut valid = Vec::new();
        for a in 0..2 {
            for b in 0..2 {
                for c in 0..2 {
                    let row = vec![a, b, c];
                    if !selected
                        .iter()
                        .any(|terms| terms.iter().all(|&(f, v)| row[f] == v))
                    {
                        valid.push(row);
                    }
                }
            }
        }
        let report = solve(
            normalize(model).unwrap(),
            "oracle".into(),
            Limits::default(),
            None,
            &Context::default(),
        )
        .unwrap();
        for case in &report.cases {
            assert!(valid.contains(case));
        }
        assert_eq!(report.valid_combinations, valid.len());
        for pair in &report.pairs {
            let feasible = valid
                .iter()
                .any(|r| r[pair.left] == pair.left_value && r[pair.right] == pair.right_value);
            assert_eq!(
                pair.state,
                if feasible { "covered" } else { "infeasible" },
                "model {mask}"
            );
        }
        assert_eq!(
            report.status,
            if valid.is_empty() {
                "unsatisfiable"
            } else {
                "complete"
            }
        );
    }
}

#[test]
fn cancellation_after_one_case_retains_the_case_and_verified_witnesses() {
    let ctx = Context {
        cancel_after: Some(2),
        ..Context::default()
    };
    let report = solve(binary(), "1".into(), Limits::default(), None, &ctx).unwrap();
    assert_eq!(report.cases.len(), 1);
    assert_eq!(report.status, "incomplete");
    assert_eq!(report.reason, Some("cancelled"));
    assert_eq!(report.summary("1".into()).covered, 3);
    assert_eq!(report.summary("1".into()).uncovered, 9);
    assert!(
        report
            .pairs
            .iter()
            .filter(|p| p.state == "covered")
            .all(|p| p.cases == vec![1])
    );
}
