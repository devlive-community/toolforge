use super::*;

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    let plugin = ReadingJournal::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}

#[test]
fn declared_interfaces_are_accepted_by_backend() {
    // recap 有路径参数，检查会跳过实际调用；这里确保声明本身能被解析
    let plugin = ReadingJournal::default();
    tf_plugin_api::interface::check_declared_interfaces(&plugin)
        .expect("declared interfaces must match the backend");
}

#[test]
fn ui_flow_round_trips_as_json() {
    let plugin = ReadingJournal::default();
    let journal = plugin.call("empty", json!({})).unwrap();
    let created = plugin
        .call("apply", json!({ "journal": journal, "opId": "op-a", "revision": 0, "today": "2026-10-28",
            "op": { "kind": "createWork", "status": "want", "fields": { "kind": "film", "title": "Stalker", "creator": "Tarkovsky", "total": 162 } } }))
        .unwrap();
    let id = created["createdId"].as_str().unwrap().to_string();
    let started = plugin
        .call("apply", json!({ "journal": created["journal"], "opId": "op-b", "revision": 1, "today": "2026-10-28",
            "op": { "kind": "start", "workId": id, "start": "2026-10-27" } }))
        .unwrap();
    let view = plugin
        .call("view", json!({ "journal": started["journal"] }))
        .unwrap();
    assert_eq!(view["works"][0]["status"], json!("doing"));
    assert_eq!(view["works"][0]["progress"], json!(0));
    assert_eq!(view["undo"], json!("start"));
}
