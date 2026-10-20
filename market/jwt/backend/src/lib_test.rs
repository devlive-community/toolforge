use super::*;
use serde_json::json;

#[test]
fn dispatches_and_is_marked_sensitive() {
    let tool = Jwt::default();
    assert!(tool.manifest().sensitive);
    let signed = tool
        .call(
            "sign",
            json!({"algorithm": "HS256", "payload": "{\"a\":1}", "key": "k"}),
        )
        .unwrap();
    let token = signed["token"].as_str().unwrap();
    let decoded = tool.call("decode", json!({"token": token})).unwrap();
    assert_eq!(decoded["algorithm"], "HS256");
    let verified = tool
        .call("verify", json!({"token": token, "key": "k"}))
        .unwrap();
    assert_eq!(verified["valid"], true);
}
