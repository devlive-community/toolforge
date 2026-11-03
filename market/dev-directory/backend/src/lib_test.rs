use super::*;
use serde_json::json;

#[test]
fn lists_all_and_filters() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, SITES.len());
    let out = list(ListArgs {
        query: String::new(),
        category: Some("docs".into()),
    })
    .unwrap();
    assert!(out.total >= 10);
    assert!(out.sites.iter().all(|s| s.category == "docs"));
    let out = list(ListArgs {
        query: "regex".into(),
        category: None,
    })
    .unwrap();
    assert!(out.sites.iter().any(|s| s.name == "Regex101"));
    assert!(
        list(ListArgs {
            query: "zzz".into(),
            category: None
        })
        .is_err()
    );
}

#[test]
fn dispatches_via_plugin() {
    let out = DevDirectory::default()
        .call("list", json!({"query": "mdn"}))
        .unwrap();
    assert!(out["total"].as_u64().unwrap() >= 1);
    assert_eq!(
        DevDirectory::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&DevDirectory::default()).unwrap();
    assert_eq!(checked, 1);
}
