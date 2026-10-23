use serde_json::json;

use super::*;

fn manifest(sensitive: bool) -> Manifest {
    Manifest::from_static(&format!(
        r#"{{"id":"p","version":"1.0.0","name":"n","description":"d","category":"dev","sensitive":{sensitive},
            "functions":{{"kept":{{"history":true}},"skipped":{{}}}}}}"#
    ))
}

#[test]
fn follows_the_same_rules_as_the_app() {
    let store = Store::open_in_memory().unwrap();
    let args = json!({ "input": "hello" });
    record(
        &store,
        "cli",
        &manifest(false),
        "kept",
        &args,
        &json!({ "output": "x" }),
    );
    record(
        &store,
        "cli",
        &manifest(false),
        "skipped",
        &args,
        &json!({}),
    );
    record(&store, "agent", &manifest(true), "kept", &args, &json!({}));
    let entries = store.history_list("", 10, 0).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        (entries[0].source.as_str(), entries[0].input.as_str()),
        ("cli", "hello")
    );

    store.kv_set(ENABLED_KEY, &json!(false)).unwrap();
    record(&store, "cli", &manifest(false), "kept", &args, &json!({}));
    assert_eq!(
        store.history_count().unwrap(),
        1,
        "disabled history records nothing"
    );
}
