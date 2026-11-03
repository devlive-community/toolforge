use super::*;
use serde_json::json;
use std::collections::HashMap;

fn given(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn solves_kinematics_all_directions() {
    let out = solve(SolveArgs {
        card: "kinematics_v".into(),
        target: "v".into(),
        given: given(&[("v0", 2.0), ("a", 3.0), ("t", 4.0)]),
    })
    .unwrap();
    assert_eq!(out.value, 14.0);
    assert_eq!(out.unit, "m/s");
    let out = solve(SolveArgs {
        card: "kinematics_v".into(),
        target: "a".into(),
        given: given(&[("v", 14.0), ("v0", 2.0), ("t", 4.0)]),
    })
    .unwrap();
    assert_eq!(out.value, 3.0);
}

#[test]
fn quadratic_kinematics_picks_positive_root() {
    let out = solve(SolveArgs {
        card: "kinematics_s".into(),
        target: "t".into(),
        given: given(&[("s", 100.0), ("v0", 0.0), ("a", 2.0)]),
    })
    .unwrap();
    assert!((out.value - 10.0).abs() < 1e-12);
}

#[test]
fn electricity_and_heat() {
    let out = solve(SolveArgs {
        card: "ohm".into(),
        target: "i".into(),
        given: given(&[("u", 220.0), ("r", 44.0)]),
    })
    .unwrap();
    assert_eq!(out.value, 5.0);
    let out = solve(SolveArgs {
        card: "parallel_r".into(),
        target: "r".into(),
        given: given(&[("r1", 6.0), ("r2", 3.0)]),
    })
    .unwrap();
    assert_eq!(out.value, 2.0);
    let out = solve(SolveArgs {
        card: "heat".into(),
        target: "q".into(),
        given: given(&[("m", 2.0), ("cc", 4186.0), ("dt", 10.0)]),
    })
    .unwrap();
    assert_eq!(out.value, 83_720.0);
}

#[test]
fn guards_missing_extra_and_zero_division() {
    let err = solve(SolveArgs {
        card: "newton".into(),
        target: "a".into(),
        given: given(&[("f", 10.0)]),
    })
    .unwrap_err();
    assert_eq!(err.code, "physics.missing_given");
    let err = solve(SolveArgs {
        card: "newton".into(),
        target: "a".into(),
        given: given(&[("f", 10.0), ("m", 2.0), ("a", 5.0)]),
    })
    .unwrap_err();
    assert_eq!(err.code, "physics.extra_given");
    let err = solve(SolveArgs {
        card: "newton".into(),
        target: "a".into(),
        given: given(&[("f", 10.0), ("m", 0.0)]),
    })
    .unwrap_err();
    assert_eq!(err.code, "physics.divide_by_zero");
    let err = solve(SolveArgs {
        card: "astro".into(),
        target: "x".into(),
        given: given(&[]),
    })
    .unwrap_err();
    assert_eq!(err.code, "physics.unknown_card");
}

#[test]
fn constants_table() {
    let list = constants();
    assert!(list.iter().any(|c| c.key == "g" && c.value == 9.8));
}

#[test]
fn dispatches_via_plugin() {
    let tool = PhysicsCalculator::default();
    let out = tool
        .call(
            "solve",
            json!({"card": "newton", "target": "f", "given": {"m": 2, "a": 3}}),
        )
        .unwrap();
    assert_eq!(out["value"], 6.0);
    let out = tool.call("constants", json!({})).unwrap();
    assert!(out.as_array().unwrap().len() >= 5);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PhysicsCalculator::default()).unwrap();
    assert_eq!(checked, 2);
}
