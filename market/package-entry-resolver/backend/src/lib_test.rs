use super::*;
use serde_json::json;

fn resolve_json(exports: &str, subpath: &str, conditions: &[&str], listing: &[&str]) -> Output {
    resolve(Args {
        package_json: format!(r#"{{"name":"demo","exports":{exports}}}"#),
        subpath: subpath.into(),
        conditions: conditions.iter().map(|c| c.to_string()).collect(),
        listing: listing.iter().map(|c| c.to_string()).collect(),
    })
    .unwrap()
}

#[test]
fn string_shorthand_only_dot() {
    let out = resolve_json(r#""./dist/index.js""#, ".", &["import"], &[]);
    assert_eq!(out.status, "resolved");
    assert_eq!(out.target.as_deref(), Some("./dist/index.js"));
    let out = resolve_json(r#""./dist/index.js""#, "./other", &["import"], &[]);
    assert_eq!(out.status, "no_match");
}

#[test]
fn conditions_pick_in_order_and_skip_disabled() {
    let exports = r#"{
        ".": {
            "types": "./types.d.ts",
            "import": "./dist/esm/index.mjs",
            "require": "./dist/cjs/index.cjs",
            "default": "./dist/index.js"
        }
    }"#;
    let out = resolve_json(exports, ".", &["import"], &[]);
    assert_eq!(out.target.as_deref(), Some("./dist/esm/index.mjs"));
    assert!(
        out.steps
            .iter()
            .any(|s| s.key.as_deref() == Some("types") && s.outcome == "condition_skipped")
    );
    let out = resolve_json(exports, ".", &["require"], &[]);
    assert_eq!(out.target.as_deref(), Some("./dist/cjs/index.cjs"));
}

#[test]
fn default_ending_hides_later_conditions() {
    // default 之后写 require 也不会命中（键序保留，顺序匹配）
    let exports = r#"{ ".": { "default": "./fallback.js", "require": "./should-not-hit.cjs" } }"#;
    let out = resolve_json(exports, ".", &["require"], &[]);
    assert_eq!(out.target.as_deref(), Some("./fallback.js"));
}

#[test]
fn nested_conditions_continue_after_miss() {
    let exports = r#"{ ".": { "import": { "browser": "./browser.mjs", "default": "./node.mjs" }, "require": "./index.cjs" } }"#;
    let out = resolve_json(exports, ".", &["import", "node"], &[]);
    assert_eq!(
        out.target.as_deref(),
        Some("./node.mjs"),
        "browser not enabled, default inside import hits"
    );
    let out = resolve_json(exports, ".", &["require"], &[]);
    assert_eq!(out.target.as_deref(), Some("./index.cjs"));
}

#[test]
fn patterns_expand_with_longest_prefix() {
    let exports = r#"{
        "./features/*": "./src/features/*.js",
        "./features/premium/*": "./src/premium/*.js"
    }"#;
    let out = resolve_json(exports, "./features/a/b", &["import"], &[]);
    assert_eq!(out.target.as_deref(), Some("./src/features/a/b.js"));
    let out = resolve_json(exports, "./features/premium/x", &["import"], &[]);
    assert_eq!(
        out.target.as_deref(),
        Some("./src/premium/x.js"),
        "more specific pattern wins"
    );
}

#[test]
fn null_blocks_and_arrays_fall_back() {
    let exports = r#"{ "./private": null }"#;
    let out = resolve_json(exports, "./private", &["import"], &[]);
    assert_eq!(out.status, "blocked");

    // 数组：非法目标继续尝试，合法的命中
    let exports = r#"{ ".": ["not-a-target", "./works.js"] }"#;
    let out = resolve_json(exports, ".", &["import"], &[]);
    assert_eq!(out.target.as_deref(), Some("./works.js"));

    // 数组全部非法 → invalid_target（规范不回退到 no_match）
    let exports = r#"{ ".": ["nope", "also-nope"] }"#;
    let out = resolve_json(exports, ".", &["import"], &[]);
    assert_eq!(out.status, "invalid_target");
}

#[test]
fn outside_paths_and_missing_targets_fail() {
    let exports = r#"{ ".": "../escape.js" }"#;
    let out = resolve_json(exports, ".", &["import"], &[]);
    assert_eq!(out.status, "invalid_target");

    // 清单核对：解析目标不在清单 → not_in_listing；在 → resolved 且 listed=true
    let exports = r#"{ ".": "./dist/index.js" }"#;
    let out = resolve_json(exports, ".", &["import"], &["dist/index.js"]);
    assert_eq!(out.status, "resolved");
    assert_eq!(out.listed, Some(true));
    let out = resolve_json(exports, ".", &["import"], &["dist/other.js"]);
    assert_eq!(out.status, "not_in_listing");
    assert_eq!(out.listed, Some(false));
    // 没有清单：只输出目标，存在性未检查
    let out = resolve_json(exports, ".", &["import"], &[]);
    assert_eq!(out.status, "resolved");
    assert_eq!(out.listed, None);
}

#[test]
fn rejects_bad_input_shapes() {
    let err = resolve(Args {
        package_json: r#"{"name":"demo"}"#.into(),
        subpath: ".".into(),
        conditions: vec![],
        listing: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "resolver.no_exports");
    let err = resolve(Args {
        package_json: r#"{"name":"demo","exports":{}}"#.into(),
        subpath: "lib".into(),
        conditions: vec![],
        listing: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "resolver.bad_subpath");
    let err = resolve(Args {
        package_json: "not json".into(),
        subpath: ".".into(),
        conditions: vec![],
        listing: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "resolver.bad_json");
}

#[test]
fn dispatches_via_plugin() {
    let tool = PackageEntryResolver::default();
    let out = tool
        .call(
            "resolve",
            json!({
                "packageJson": r#"{"name":"demo","exports":"./index.js"}"#,
                "subpath": ".",
                "conditions": ["import"]
            }),
        )
        .unwrap();
    assert_eq!(out["status"], "resolved");
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PackageEntryResolver::default())
            .unwrap();
    assert_eq!(checked, 1);
}
