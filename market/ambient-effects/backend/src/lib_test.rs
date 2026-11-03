use super::*;
use serde_json::json;

#[test]
fn presets_scale_with_density_and_low_power() {
    let out = preset(PresetArgs {
        key: "snow".into(),
        density: Some(50.0),
        low_power: false,
    })
    .unwrap();
    assert_eq!(out.count, 150);
    assert_eq!(out.shape, "flake");
    let out = preset(PresetArgs {
        key: "snow".into(),
        density: Some(100.0),
        low_power: false,
    })
    .unwrap();
    assert_eq!(out.count, 300);
    let out = preset(PresetArgs {
        key: "snow".into(),
        density: Some(100.0),
        low_power: true,
    })
    .unwrap();
    assert_eq!(out.count, 120);
    let out = preset(PresetArgs {
        key: "rain".into(),
        density: Some(50.0),
        low_power: false,
    })
    .unwrap();
    assert_eq!(out.shape, "streak");
}

#[test]
fn validates_preset_and_density() {
    let err = preset(PresetArgs {
        key: "stars".into(),
        density: None,
        low_power: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "ambient.unknown_preset");
    let err = preset(PresetArgs {
        key: "snow".into(),
        density: Some(120.0),
        low_power: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "ambient.bad_density");
}

#[test]
fn noise_params_per_kind() {
    let out = noise(NoiseArgs {
        kind: "rain".into(),
        volume: Some(80.0),
    })
    .unwrap();
    assert_eq!(out.method, "filtered_noise");
    assert!((out.gain - 0.32).abs() < 1e-9);
    assert!(out.crackle_ms.is_none());
    let out = noise(NoiseArgs {
        kind: "campfire".into(),
        volume: Some(50.0),
    })
    .unwrap();
    assert_eq!(out.method, "crackle");
    assert_eq!(out.crackle_ms, Some((80, 600)));
    let err = noise(NoiseArgs {
        kind: "ocean".into(),
        volume: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "ambient.unknown_noise");
    let err = noise(NoiseArgs {
        kind: "rain".into(),
        volume: Some(101.0),
    })
    .unwrap_err();
    assert_eq!(err.code, "ambient.bad_volume");
}

#[test]
fn dispatches_via_plugin() {
    let out = AmbientEffects::default()
        .call("preset", json!({"key": "bubbles", "density": 30}))
        .unwrap();
    assert!(out["count"].as_u64().unwrap() > 0);
    assert_eq!(
        AmbientEffects::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&AmbientEffects::default()).unwrap();
    assert_eq!(checked, 2);
}
