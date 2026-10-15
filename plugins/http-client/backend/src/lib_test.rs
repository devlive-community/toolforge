use super::*;
use serde_json::json;

#[test]
fn send_is_a_task_and_curl_is_a_call() {
    let tool = HttpClient::default();
    assert!(tool.manifest().functions["send"].task);
    let curl = tool
        .call("to_curl", json!({"method": "GET", "url": "https://x.dev"}))
        .unwrap();
    assert_eq!(curl, "curl \\\n  'https://x.dev/' \\\n  -L");
}

#[test]
fn calls_resolve_templates_and_keep_legacy_arguments_compatible() {
    let tool = HttpClient::default();
    let curl = tool
        .call(
            "to_curl",
            json!({
                "method": "GET", "url": "{{base}}/users",
                "params": [{ "key": "token", "value": "{{token}}" }],
                "variables": [
                    { "key": "base", "value": "https://example.com" },
                    { "key": "token", "value": "has spaces & /", "secret": true }
                ]
            }),
        )
        .unwrap();
    assert!(
        curl.as_str()
            .unwrap()
            .contains("https://example.com/users?token=has+spaces+%26+%2F")
    );
    let resolved = tool
        .call(
            "resolve_request",
            json!({
                "request": { "method": "GET", "url": "{{base}}" },
                "variables": [{ "key": "base", "value": "https://example.com" }]
            }),
        )
        .unwrap();
    assert_eq!(resolved["url"], "https://example.com");
    assert_eq!(resolved["timeoutMs"], 30_000);
    assert_eq!(resolved["body"]["kind"], "none");
    let imported = tool
        .call(
            "import_curl",
            json!({ "command": "curl https://example.com" }),
        )
        .unwrap();
    assert_eq!(imported["request"]["url"], "https://example.com");
    assert!(imported["warnings"].is_array());
    assert_ne!(
        tool.call("new_id", json!({})).unwrap(),
        tool.call("new_id", json!({})).unwrap()
    );
}

#[test]
fn workspace_api_rejects_malformed_shapes_and_returns_a_safe_copy() {
    let tool = HttpClient::default();
    let state = json!({
        "version": 1,
        "collections": [], "requests": [],
        "environments": [{
            "id": "env", "name": "Local",
            "variables": [{ "key": "token", "value": "private", "secret": true }]
        }],
        "activeEnvironmentId": "env"
    });
    let saved = tool
        .call("workspace_validate", json!({ "workspace": state.clone() }))
        .unwrap();
    assert_eq!(saved["environments"][0]["variables"][0]["value"], "");
    assert_eq!(state["environments"][0]["variables"][0]["value"], "private");
    for workspace in [
        json!({}),
        json!({ "version": "1", "collections": [], "requests": [], "environments": [] }),
    ] {
        assert_eq!(
            tool.call("workspace_validate", json!({ "workspace": workspace }))
                .unwrap_err()
                .code,
            "plugin.invalid_args"
        );
    }
}
