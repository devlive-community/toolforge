use super::*;
use serde_json::json;

#[test]
fn hash_text_returns_requested_algorithms_in_order() {
    let out = Hash::default()
        .call(
            "hash_text",
            json!({"input": "abc", "algorithms": ["sha1", "md5"], "uppercase": true}),
        )
        .unwrap();
    assert_eq!(out["results"][0]["algorithm"], "sha1");
    assert_eq!(
        out["results"][1]["digest"],
        "900150983CD24FB0D6963F7D28E17F72"
    );
    assert_eq!(out["bytes"], 3);
}

#[test]
fn hash_text_supports_utf8() {
    let out = Hash::default()
        .call("hash_text", json!({"input": "中文", "algorithms": ["md5"]}))
        .unwrap();
    assert_eq!(
        out["results"][0]["digest"],
        "a7bac2239fcdcb3a067903d8077c4a07"
    );
}

#[test]
fn manifest_marks_file_hashing_as_task() {
    let plugin = Hash::default();
    assert!(plugin.manifest().functions["hash_files"].task);
    assert!(!plugin.manifest().functions["hash_text"].task);
}
