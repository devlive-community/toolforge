use super::*;

/// 插件级链路：review → queue；撤销数据（prev 状态）由前端用返回值拼装
#[test]
fn plugin_pipeline() {
    let plugin = Flashcards::default();
    let today = "2026-10-19";
    // 新卡首次通过
    let out = plugin
        .call(
            "review",
            serde_json::json!({ "stage": 0, "today": today, "remembered": true }),
        )
        .unwrap();
    assert_eq!(out["stage"], 1);
    assert_eq!(out["due"], "2026-10-20");
    // 队列：到期 + 新卡
    let out = plugin
        .call(
            "queue",
            serde_json::json!({
                "cards": [
                    { "id": "a", "deck": "d", "stage": 1, "due": "2026-10-19" },
                    { "id": "b", "deck": "d", "stage": 0, "due": "" }
                ],
                "today": today,
                "limit": 10,
                "newLimit": 10
            }),
        )
        .unwrap();
    assert_eq!(out["dueIds"], serde_json::json!(["a"]));
    assert_eq!(out["newIds"], serde_json::json!(["b"]));
    // new_id
    let first = plugin.call("new_id", Value::Null).unwrap();
    let second = plugin.call("new_id", Value::Null).unwrap();
    assert_ne!(first["id"], second["id"]);
    // 未知函数
    let err = plugin.call("study", Value::Null).unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}

#[test]
fn csv_via_plugin() {
    let plugin = Flashcards::default();
    let out = plugin
        .call(
            "import_csv",
            serde_json::json!({ "csv": "Q,A,,\n", "defaultDeck": "d" }),
        )
        .unwrap();
    assert_eq!(out["cards"].as_array().unwrap().len(), 1);
}

#[test]
fn backup_roundtrip_via_plugin() {
    let plugin = Flashcards::default();
    let exported = plugin
        .call(
            "backup",
            serde_json::json!({ "cards": [{ "id": "1", "deck": "d", "question": "Q", "answer": "A", "stage": 2, "due": "2026-10-22" }] }),
        )
        .unwrap();
    let json = exported["json"].as_str().unwrap();
    let preview = plugin
        .call(
            "restore_preview",
            serde_json::json!({ "json": json, "current": [] }),
        )
        .unwrap();
    assert_eq!(preview["incoming"], 1);
    let applied = plugin
        .call(
            "restore_apply",
            serde_json::json!({ "json": json, "current": [], "mode": "replace" }),
        )
        .unwrap();
    assert_eq!(applied["cards"][0]["stage"], 2);
}
