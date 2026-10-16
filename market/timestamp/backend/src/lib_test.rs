use super::*;
use serde_json::json;

#[test]
fn dispatches_every_declared_function() {
    let tool = Timestamp::default();
    for (function, args) in [
        ("now", json!({})),
        ("from_timestamp", json!({"value": "0"})),
        ("to_timestamp", json!({"input": "1970-01-01T00:00:00Z"})),
        ("timezones", json!({})),
    ] {
        assert!(tool.manifest().functions.contains_key(function));
        tool.call(function, args).unwrap();
    }
}
