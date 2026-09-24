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
