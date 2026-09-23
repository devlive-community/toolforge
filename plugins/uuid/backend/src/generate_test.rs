use super::*;
use serde_json::json;

fn args(value: serde_json::Value) -> Args {
    serde_json::from_value(value).unwrap()
}

#[test]
fn generates_requested_count_of_unique_v4() {
    let out = run(args(json!({"kind": "v4", "count": 50}))).unwrap();
    assert_eq!(out.ids.len(), 50);
    let unique: std::collections::HashSet<_> = out.ids.iter().collect();
    assert_eq!(unique.len(), 50);
    assert!(
        out.ids
            .iter()
            .all(|id| Uuid::parse_str(id).unwrap().get_version_num() == 4)
    );
}

#[test]
fn v5_matches_rfc_example_and_is_deterministic() {
    let out = run(args(
        json!({"kind": "v5", "count": 10, "namespace": "dns", "name": "example.com"}),
    ))
    .unwrap();
    assert_eq!(out.ids, vec!["cfbff0d1-9375-5685-968c-48ce8b15ae17"]);
    assert!(out.deterministic);
}

#[test]
fn v3_matches_known_value() {
    let out = run(args(
        json!({"kind": "v3", "namespace": "dns", "name": "example.com"}),
    ))
    .unwrap();
    assert_eq!(out.ids[0], "9073926b-929f-31c2-abc9-fad77ae3e8eb");
}

#[test]
fn applies_uuid_formatting_options() {
    let out = run(args(
        json!({"kind": "v7", "uppercase": true, "hyphens": false, "braces": true}),
    ))
    .unwrap();
    let id = &out.ids[0];
    assert!(id.starts_with('{') && id.ends_with('}'));
    assert_eq!(id.len(), 34);
    assert_eq!(id.to_ascii_uppercase(), *id);
}

#[test]
fn v7_ids_are_time_ordered() {
    let out = run(args(json!({"kind": "v7", "count": 20}))).unwrap();
    let mut sorted = out.ids.clone();
    sorted.sort();
    assert_eq!(sorted, out.ids);
}

#[test]
fn nanoid_respects_size_and_alphabet() {
    let out = run(args(
        json!({"kind": "nanoid", "count": 5, "nanoidSize": 8, "nanoidAlphabet": "ab"}),
    ))
    .unwrap();
    assert!(
        out.ids
            .iter()
            .all(|id| id.len() == 8 && id.chars().all(|c| c == 'a' || c == 'b'))
    );
    let err = run(args(json!({"kind": "nanoid", "nanoidAlphabet": "a"}))).unwrap_err();
    assert_eq!(err.code, "uuid.invalid_alphabet");
}

#[test]
fn ulid_casing() {
    let upper = run(args(json!({"kind": "ulid", "uppercase": true}))).unwrap();
    assert_eq!(upper.ids[0].len(), 26);
    assert_eq!(upper.ids[0].to_ascii_uppercase(), upper.ids[0]);
    let lower = run(args(json!({"kind": "ulid"}))).unwrap();
    assert_eq!(lower.ids[0].to_ascii_lowercase(), lower.ids[0]);
}

#[test]
fn validates_count_and_custom_namespace() {
    assert_eq!(
        run(args(json!({"kind": "v4", "count": 0})))
            .unwrap_err()
            .code,
        "uuid.invalid_count"
    );
    assert_eq!(
        run(args(json!({"kind": "v4", "count": 1001})))
            .unwrap_err()
            .code,
        "uuid.invalid_count"
    );
    let err = run(args(
        json!({"kind": "v5", "namespace": "custom", "customNamespace": "nope"}),
    ))
    .unwrap_err();
    assert_eq!(err.code, "uuid.invalid_namespace");
}
