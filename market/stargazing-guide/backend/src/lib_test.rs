use super::*;
use serde_json::json;

/// 北极星高度 ≈ 北纬（经典校验）
#[test]
fn polaris_altitude_matches_latitude() {
    let out = sky(Args {
        latitude: 40.0,
        longitude: 116.4,
        timestamp_ms: 1_760_000_000_000,
        min_altitude: Some(-90.0),
    })
    .unwrap();
    let polaris = out.stars.iter().find(|s| s.key == "polaris").unwrap();
    assert!(
        (polaris.altitude - 40.0).abs() < 1.5,
        "Polaris altitude {:.2} ≈ lat 40",
        polaris.altitude
    );
    // 北京 40°N：北极星在北（方位接近 0）
    assert!(polaris.azimuth < 5.0 || polaris.azimuth > 355.0);
}

#[test]
fn southern_hemisphere_hides_polaris() {
    // 南纬 40°：北极星在地平线下，默认不出现在结果里
    let out = sky(Args {
        latitude: -40.0,
        longitude: 0.0,
        timestamp_ms: 1_760_000_000_000,
        min_altitude: None,
    })
    .unwrap();
    assert!(out.stars.iter().all(|s| s.key != "polaris"));
    // 南天亮星能看到（如天狼星随时间可能可见；这里只断言结构）
    assert!(!out.stars.is_empty());
}

#[test]
fn filters_below_horizon_and_lines() {
    let out = sky(Args {
        latitude: 40.0,
        longitude: 116.4,
        timestamp_ms: 1_760_000_000_000,
        min_altitude: None,
    })
    .unwrap();
    assert!(out.stars.iter().all(|s| s.altitude >= -0.5));
    // 连线两端都在地平线上方
    for (a, b) in &out.lines {
        assert!(out.stars.iter().any(|s| s.key == a.as_str()));
        assert!(out.stars.iter().any(|s| s.key == b.as_str()));
    }
    // 明确的 min_altitude 过滤
    let out = sky(Args {
        latitude: 40.0,
        longitude: 116.4,
        timestamp_ms: 1_760_000_000_000,
        min_altitude: Some(30.0),
    })
    .unwrap();
    assert!(out.stars.iter().all(|s| s.altitude >= 30.0));
}

#[test]
fn time_moves_stars_across_the_sky() {
    let morning = sky(Args {
        latitude: 0.0,
        longitude: 0.0,
        timestamp_ms: 1_760_000_000_000,
        min_altitude: Some(30.0),
    })
    .unwrap();
    let evening = sky(Args {
        latitude: 0.0,
        longitude: 0.0,
        timestamp_ms: 1_760_000_000_000 + 6 * 3_600_000,
        min_altitude: Some(30.0),
    })
    .unwrap();
    // 6 小时后 LST 前进约 90°
    assert!(
        ((evening.local_sidereal_time_hours - morning.local_sidereal_time_hours).rem_euclid(24.0)
            - 6.0)
            .abs()
            < 0.2
    );
}

#[test]
fn validates_location() {
    let err = sky(Args {
        latitude: 120.0,
        longitude: 0.0,
        timestamp_ms: 0,
        min_altitude: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "sky.bad_location");
    let err = sky(Args {
        latitude: 0.0,
        longitude: 500.0,
        timestamp_ms: 0,
        min_altitude: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "sky.bad_location");
}

#[test]
fn dispatches_via_plugin() {
    let out = StargazingGuide::default()
        .call(
            "sky",
            json!({"latitude": 40.0, "longitude": 116.4, "timestampMs": 1760000000000i64}),
        )
        .unwrap();
    assert!(out["stars"].as_array().unwrap().len() >= 3);
    assert_eq!(
        StargazingGuide::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&StargazingGuide::default()).unwrap();
    assert_eq!(checked, 1);
}
