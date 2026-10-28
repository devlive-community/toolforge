use super::*;

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    // 宿主按 manifest.functions 放行调用，漏声明会在界面报「插件不支持该操作」
    let plugin = CollectionLedger::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}

#[test]
fn ui_flow_create_lend_view_round_trips_as_json() {
    let plugin = CollectionLedger::default();
    let ledger = plugin.call("empty", json!({})).unwrap();
    let created = plugin
        .call(
            "apply",
            json!({ "ledger": ledger, "opId": "op-a", "revision": 0, "today": "2026-10-28",
                    "op": { "kind": "create", "owned": 2, "fields": { "name": "Gundam RX-78", "series": "MG", "location": "Box 1" } } }),
        )
        .unwrap();
    let id = created["createdId"].as_str().unwrap().to_string();
    let lent = plugin
        .call(
            "apply",
            json!({ "ledger": created["ledger"], "opId": "op-b", "revision": 1, "today": "2026-10-28",
                    "op": { "kind": "lend", "itemId": id, "borrower": "Lee", "qty": 1, "date": "2026-10-28", "due": "2026-11-05" } }),
        )
        .unwrap();
    let view = plugin
        .call(
            "view",
            json!({ "ledger": lent["ledger"], "today": "2026-10-28" }),
        )
        .unwrap();
    assert_eq!(view["summary"]["available"], json!(1));
    assert_eq!(view["pending"][0]["borrower"], json!("Lee"));
    assert_eq!(view["pending"][0]["dueState"], json!("ok"));
    assert_eq!(view["items"][0]["outstanding"], json!(1));
    assert_eq!(view["undo"], json!("lend"));
}
