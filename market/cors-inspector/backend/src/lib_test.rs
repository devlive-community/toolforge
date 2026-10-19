use super::*;
use tf_plugin_api::ToolPlugin;

fn base_args() -> serde_json::Value {
    json!({
        "documentUrl": "https://app.example.com/page",
        "targetUrl": "https://api.example.com/data",
        "method": "GET",
        "credentials": "same-origin",
        "headers": [],
    })
}

#[test]
fn inspect_returns_handle_and_stages() {
    let plugin = CorsInspector::default();
    let first = plugin.call("inspect", base_args()).unwrap();
    assert_eq!(first["handle"], 1);
    let second = plugin.call("inspect", base_args()).unwrap();
    assert_eq!(second["handle"], 2);
    // 跨源简单请求：无需预检；实际响应未填 → 整体「尚无法判断」
    assert_eq!(second["request"]["preflightNeeded"], false);
    assert_eq!(second["request"]["origin"], "https://app.example.com");
    assert_eq!(second["preflight"]["status"], "skipped");
    assert_eq!(second["reading"]["status"], "undetermined");
    assert_eq!(second["overall"], "undetermined");
    assert_eq!(second["outcome"], "undetermined");
}

#[test]
fn inspect_camel_case_matches_frontend_types() {
    let args = json!({
        "documentUrl": "https://app.example.com/page",
        "targetUrl": "https://api.example.com/data",
        "method": "POST",
        "credentials": "same-origin",
        "headers": [{"name": "X-Token", "value": "a"}],
        "preflight": {"status": 204, "headers": [
            {"name": "Access-Control-Allow-Origin", "value": "*"},
            {"name": "Access-Control-Allow-Methods", "value": "POST"},
            {"name": "Access-Control-Allow-Headers", "value": "X-Token"},
        ]},
    });
    let plugin = CorsInspector::default();
    let out = plugin.call("inspect", args).unwrap();
    assert_eq!(out["request"]["preflightNeeded"], true);
    assert_eq!(out["request"]["unsafeHeaderNames"], json!(["x-token"]));
    assert_eq!(
        out["request"]["options"]["headers"][2]["name"],
        "Access-Control-Request-Headers"
    );
    assert_eq!(out["preflight"]["status"], "allowed");
    assert_eq!(out["preflight"]["conditions"][0]["id"], "status");
    assert_eq!(out["reading"]["status"], "undetermined");
    assert_eq!(out["overall"], "undetermined");
    assert_eq!(out["outcome"], "undetermined");
}

#[test]
fn export_requires_a_previous_inspect() {
    let plugin = CorsInspector::default();
    let err = plugin
        .call("export", json!({"format": "markdown"}))
        .unwrap_err();
    assert_eq!(err.code, "cors.no_result");
}

#[test]
fn export_renders_markdown_and_json() {
    let plugin = CorsInspector::default();
    plugin.call("inspect", base_args()).unwrap();
    let out = plugin
        .call("export", json!({"format": "markdown"}))
        .unwrap();
    assert_eq!(out["ext"], "md");
    assert!(
        out["content"]
            .as_str()
            .unwrap()
            .contains("# CORS Preflight Inspection")
    );
    let out = plugin.call("export", json!({"format": "json"})).unwrap();
    assert_eq!(out["ext"], "json");
    let parsed: Value = serde_json::from_str(out["content"].as_str().unwrap()).unwrap();
    assert_eq!(parsed["request"]["method"], "GET");
    assert_eq!(parsed["overall"], "undetermined");
}

#[test]
fn inspect_reports_limit_errors() {
    let mut args = base_args();
    let headers: Vec<Value> = (0..65)
        .map(|i| json!({"name": format!("x-{i}"), "value": ""}))
        .collect();
    args["headers"] = json!(headers);
    let err = CorsInspector::default().call("inspect", args).unwrap_err();
    assert_eq!(err.code, "cors.header_too_many");
    assert_eq!(err.params["max"], 64);
}

#[test]
fn unknown_function_is_rejected() {
    let err = CorsInspector::default()
        .call("send", json!({}))
        .unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}
