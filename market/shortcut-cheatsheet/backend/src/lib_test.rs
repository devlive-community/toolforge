use super::*;
use serde_json::json;

#[test]
fn lists_all_and_by_app() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, ENTRIES.len());
    assert!(out.apps.contains(&"vscode"));
    let out = list(ListArgs {
        app: Some("chrome".into()),
        query: String::new(),
    })
    .unwrap();
    assert!(out.total >= 10);
    assert!(out.entries.iter().all(|e| e.app == "chrome"));
}

#[test]
fn searches_by_action_and_keys() {
    let out = list(ListArgs {
        app: None,
        query: "split".into(),
    })
    .unwrap();
    assert!(out.entries.iter().any(|e| e.action == "ac_split"));
    let out = list(ListArgs {
        app: None,
        query: "cmd+shift+t".into(),
    })
    .unwrap();
    assert!(out.entries.iter().any(|e| e.action == "ac_reopen_tab"));
    assert!(
        list(ListArgs {
            app: None,
            query: "不存在的功能".into()
        })
        .is_err()
    );
}

#[test]
fn dispatches_via_plugin() {
    let out = ShortcutCheatsheet::default()
        .call("list", json!({"app": "vscode"}))
        .unwrap();
    assert!(out["total"].as_u64().unwrap() >= 15);
    assert_eq!(
        ShortcutCheatsheet::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&ShortcutCheatsheet::default())
            .unwrap();
    assert_eq!(checked, 1);
}
