use super::*;
use serde_json::json;

fn stages(inputs: &[(f64, &str)]) -> Vec<StageInput> {
    inputs
        .iter()
        .map(|(fraction, label)| StageInput {
            fraction: Some(*fraction),
            ml: None,
            label: Some(label.to_string()),
            seconds: None,
        })
        .collect()
}

#[test]
fn plan_totals_and_cumulative() {
    let out = plan(PlanArgs {
        coffee_g: 15.0,
        ratio: 15.0,
        stages: stages(&[(0.133, "bloom"), (0.4, "pour2"), (0.467, "pour3")]),
    })
    .unwrap();
    assert_eq!(out.total_ml, 225.0);
    assert_eq!(out.stages.len(), 3);
    assert_eq!(out.stages[0].target_ml, 29.93, "225 × 0.133");
    let cumulative: f64 = out.stages.last().unwrap().cumulative_ml;
    assert!((cumulative - 225.0).abs() < 0.1);
    assert_eq!(out.covered, 1.0);
}

#[test]
fn partial_coverage_is_reported() {
    let out = plan(PlanArgs {
        coffee_g: 20.0,
        ratio: 16.0,
        stages: stages(&[(0.2, "bloom")]),
    })
    .unwrap();
    assert_eq!(out.total_ml, 320.0);
    assert_eq!(out.covered, 0.2);
}

#[test]
fn rejects_bad_inputs() {
    let err = plan(PlanArgs {
        coffee_g: 0.0,
        ratio: 15.0,
        stages: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "coffee.positive_required");
    let err = plan(PlanArgs {
        coffee_g: 15.0,
        ratio: 15.0,
        stages: stages(&[(1.5, "x")]),
    })
    .unwrap_err();
    assert_eq!(err.code, "coffee.bad_fraction");
    // 分段超出总量
    let err = plan(PlanArgs {
        coffee_g: 15.0,
        ratio: 15.0,
        stages: vec![
            StageInput {
                fraction: None,
                ml: Some(200.0),
                label: None,
                seconds: None,
            },
            StageInput {
                fraction: None,
                ml: Some(100.0),
                label: None,
                seconds: None,
            },
        ],
    })
    .unwrap_err();
    assert_eq!(err.code, "coffee.stage_overflow");
    let err = plan(PlanArgs {
        coffee_g: 15.0,
        ratio: 15.0,
        stages: vec![StageInput {
            fraction: Some(0.5),
            ml: Some(100.0),
            label: None,
            seconds: None,
        }],
    })
    .unwrap_err();
    assert_eq!(err.code, "coffee.both_fraction_ml");
}

#[test]
fn scale_preserves_proportions() {
    let out = scale(ScaleArgs {
        original_coffee_g: 15.0,
        new_coffee_g: 22.5,
        stage_ml: vec![45.0, 90.0, 90.0],
        original_ratio: Some(15.0),
    })
    .unwrap();
    assert_eq!(out.factor, 1.5);
    assert_eq!(out.stage_ml, vec![67.5, 135.0, 135.0]);
    assert_eq!(out.new_total_ml, 337.5);
}

#[test]
fn scale_rejects_overflowing_stages() {
    let err = scale(ScaleArgs {
        original_coffee_g: 15.0,
        new_coffee_g: 15.0,
        stage_ml: vec![300.0],
        original_ratio: Some(15.0),
    })
    .unwrap_err();
    assert_eq!(err.code, "coffee.stage_overflow");
}

#[test]
fn dispatches_via_plugin() {
    let out = PourOverCoffee::default()
        .call(
            "plan",
            json!({"coffeeG": 15, "ratio": 15, "stages": [{"fraction": 0.2, "label": "bloom"}]}),
        )
        .unwrap();
    assert_eq!(out["totalMl"], 225.0);
    assert_eq!(
        PourOverCoffee::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PourOverCoffee::default()).unwrap();
    assert_eq!(checked, 2);
}
