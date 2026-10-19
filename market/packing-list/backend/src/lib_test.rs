use super::*;
use tf_plugin_api::PluginError;
use tf_plugin_api::ToolPlugin;

#[test]
fn generate_and_summarize_via_plugin() {
    let plugin = PackingList::default();
    let items = plugin
        .call(
            "generate",
            serde_json::json!({
                "template": [
                    {"name": "袜子", "base": 2, "perPerson": true, "category": "衣物", "unit": "双"},
                    {"name": "充电宝", "base": 1, "category": "电子", "unit": "个", "weight": 300}
                ],
                "people": 2,
                "days": 3
            }),
        )
        .unwrap();
    assert_eq!(items[0]["target"], 4);
    assert_eq!(items[1]["target"], 1);

    let out = plugin
        .call(
            "summary",
            serde_json::json!({ "people": 2, "days": 3, "bags": [], "items": items }),
        )
        .unwrap();
    assert_eq!(out["target"], 5);
    assert_eq!(out["packed"], 0);
    assert_eq!(out["categories"].as_array().unwrap().len(), 2);
}

#[test]
fn export_requires_summary_and_renders() {
    let plugin = PackingList::default();
    let err: PluginError = plugin
        .call(
            "export",
            serde_json::json!({"format": "text", "summary": {"broken": true}, "tripName": "x"}),
        )
        .unwrap_err();
    assert_eq!(err.code, "plc.serialize");
    let summary = serde_json::json!({
        "target": 3, "packed": 1, "excluded": 0, "unknownWeightItems": 1, "totalWeight": 800,
        "categories": [], "bags": [],
        "unpacked": [{"id": "b", "name": "牙刷", "category": "洗漱", "remaining": 1}]
    });
    let out = plugin
        .call(
            "export",
            serde_json::json!({"format": "text", "summary": summary, "tripName": "出行"}),
        )
        .unwrap();
    assert_eq!(out["ext"], "txt");
    assert!(out["content"].as_str().unwrap().contains("[洗漱]"));
}

#[test]
fn unknown_function_is_rejected() {
    let err = PackingList::default()
        .call("send", Value::Null)
        .unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}
