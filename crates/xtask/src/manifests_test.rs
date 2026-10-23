use serde_json::json;

use super::*;

const PATH: &str = "market/demo/manifest.json";

#[test]
fn accepts_plain_versions_only() {
    assert!(valid_version("0.3.0"));
    assert!(valid_version("10.20.30"));
    assert!(!valid_version("0.3"));
    assert!(!valid_version("0.3.0-beta"));
    assert!(!valid_version("v0.3.0"));
    assert!(!valid_version("0..1"));
}

#[test]
fn untouched_legacy_plugins_pass() {
    let manifest = json!({ "version": "0.1.2" });
    assert!(check(PATH, &manifest, Baseline::Version("0.1.2")).is_none());
    assert!(check(PATH, &manifest, Baseline::Unknown).is_none());
}

#[test]
fn updated_or_new_plugins_need_min_app_version() {
    let manifest = json!({ "version": "0.1.3" });
    assert!(check(PATH, &manifest, Baseline::Version("0.1.2")).is_some());
    assert!(check(PATH, &manifest, Baseline::Missing).is_some());

    let declared = json!({ "version": "0.1.3", "minAppVersion": "0.2.0" });
    assert!(check(PATH, &declared, Baseline::Version("0.1.2")).is_none());
    assert!(check(PATH, &declared, Baseline::Missing).is_none());
}

#[test]
fn malformed_min_app_version_is_reported_even_without_baseline() {
    let manifest = json!({ "version": "0.1.0", "minAppVersion": "0.2" });
    let violation = check(PATH, &manifest, Baseline::Unknown).unwrap();
    assert_eq!(violation.rule, "plugin-min-app-version");
    let number = json!({ "version": "0.1.0", "minAppVersion": 2 });
    assert!(check(PATH, &number, Baseline::Unknown).is_some());
}

#[test]
fn interfaces_are_required_for_changed_plugins_except_grandfathered_versions() {
    let bumped = json!({ "version": "0.1.3" });
    assert!(require_interfaces(
        "plugins/demo",
        &bumped,
        Baseline::Version("0.1.2")
    ));
    assert!(!require_interfaces(
        "plugins/demo",
        &bumped,
        Baseline::Version("0.1.3")
    ));
    assert!(!require_interfaces(
        "plugins/demo",
        &bumped,
        Baseline::Unknown
    ));

    let legacy = json!({ "version": "0.1.0" });
    assert!(!require_interfaces(
        "market/http-cache",
        &legacy,
        Baseline::Missing
    ));
    let next = json!({ "version": "0.1.1" });
    assert!(require_interfaces(
        "market/http-cache",
        &next,
        Baseline::Missing
    ));
}
