use super::*;
use serde_json::json;

#[test]
fn bmi_uses_chinese_standard_with_who_reference() {
    let out = bmi(BmiArgs {
        height: 170.0,
        weight: 70.0,
        waist: Some(85.0),
        hip: Some(95.0),
    })
    .unwrap();
    assert!((out.bmi - 24.221_453).abs() < 1e-6);
    assert_eq!(
        out.cn_category, "overweight",
        "24.2 is overweight in CN standard"
    );
    assert_eq!(out.who_category, "normal");
    assert!((out.healthy_min - 18.5 * 1.7 * 1.7).abs() < 1e-9);
    assert!((out.healthy_max - 23.9 * 1.7 * 1.7).abs() < 1e-9);
    assert!((out.whr.unwrap() - 85.0 / 95.0).abs() < 1e-12);
    assert!(!out.whr_risk.unwrap());
}

#[test]
fn bmi_rejects_nonpositive() {
    let err = bmi(BmiArgs {
        height: 0.0,
        weight: 70.0,
        waist: None,
        hip: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "health.positive_required");
}

#[test]
fn energy_mifflin_st_jeor() {
    let out = energy(EnergyArgs {
        height: 175.0,
        weight: 75.0,
        age: 30.0,
        sex: "male".into(),
        activity: Some(1.55),
    })
    .unwrap();
    let bmr = 10.0 * 75.0 + 6.25 * 175.0 - 5.0 * 30.0 + 5.0;
    assert!((out.bmr - bmr).abs() < 1e-9);
    assert!((out.tdee - bmr * 1.55).abs() < 1e-9);
    assert!((out.cut - (bmr * 1.55 - 500.0)).abs() < 1e-9);
    let err = energy(EnergyArgs {
        height: 175.0,
        weight: 75.0,
        age: 30.0,
        sex: "other".into(),
        activity: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "health.bad_sex");
}

#[test]
fn heart_rate_five_zones_with_karvonen() {
    let out = heart_rate(HeartRateArgs {
        age: 40.0,
        resting: Some(60.0),
    })
    .unwrap();
    assert_eq!(out.max, 180.0);
    let fat = &out.zones[1];
    assert!((fat.low - (60.0 + 120.0 * 0.60)).abs() < 1e-9);
    assert!((fat.high - (60.0 + 120.0 * 0.70)).abs() < 1e-9);
    assert_eq!(out.zones.len(), 5);
    let err = heart_rate(HeartRateArgs {
        age: 200.0,
        resting: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "health.bad_age");
}

#[test]
fn body_uses_du_bois_and_devine() {
    let out = body(BodyArgs {
        height: 175.0,
        weight: 72.0,
        sex: "male".into(),
        waist: Some(80.0),
        hip: Some(95.0),
    })
    .unwrap();
    let m = 1.75f64;
    let expected_bsa = 0.007_184f64 * m.powf(0.725) * 72.0f64.powf(0.425);
    assert!((out.bsa - expected_bsa).abs() < 1e-9);
    let inches = (175.0 - 152.4) / 2.54;
    let expected_ideal = 50.0 + inches * 2.3;
    assert!((out.ideal_weight - expected_ideal).abs() < 1e-9);
    assert!((out.water_liters - 72.0 * 0.033).abs() < 1e-9);
    assert!((out.whr.unwrap() - 80.0 / 95.0).abs() < 1e-12);
    assert!(!out.whr_risk.unwrap());
}

#[test]
fn dispatches_via_plugin() {
    let tool = HealthCalculator::default();
    let out = tool
        .call("bmi", json!({"height": 170, "weight": 70}))
        .unwrap();
    assert!(out["bmi"].as_f64().unwrap() > 24.0);
    let out = tool.call("heart_rate", json!({"age": 30})).unwrap();
    assert_eq!(out["zones"].as_array().unwrap().len(), 5);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&HealthCalculator::default()).unwrap();
    assert_eq!(checked, 4);
}
