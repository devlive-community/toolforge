use serde_json::json;

use super::*;

/// 测试用的最小 JSON Patch 应用器，用来验证生成的补丁能把左侧变成右侧
fn pointer_parts(path: &str) -> Vec<String> {
    path.split('/')
        .skip(1)
        .map(|t| t.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn parent<'a>(doc: &'a mut Value, parts: &[String]) -> &'a mut Value {
    parts.iter().fold(doc, |node, part| match node {
        Value::Object(map) => map.get_mut(part).unwrap(),
        Value::Array(items) => &mut items[part.parse::<usize>().unwrap()],
        _ => panic!("bad path"),
    })
}

fn remove(doc: &mut Value, path: &str) -> Value {
    let parts = pointer_parts(path);
    let (last, init) = parts.split_last().unwrap();
    match parent(doc, init) {
        Value::Object(map) => map.shift_remove(last).unwrap(),
        Value::Array(items) => items.remove(last.parse().unwrap()),
        _ => panic!("bad parent"),
    }
}

fn add(doc: &mut Value, path: &str, value: Value) {
    let parts = pointer_parts(path);
    let Some((last, init)) = parts.split_last() else {
        *doc = value;
        return;
    };
    match parent(doc, init) {
        Value::Object(map) => {
            map.insert(last.clone(), value);
        }
        Value::Array(items) => items.insert(last.parse().unwrap(), value),
        _ => panic!("bad parent"),
    }
}

fn apply(mut doc: Value, ops: &Value) -> Value {
    for op in ops.as_array().unwrap() {
        let path = op["path"].as_str().unwrap();
        match op["op"].as_str().unwrap() {
            "add" => add(&mut doc, path, op["value"].clone()),
            "remove" => {
                remove(&mut doc, path);
            }
            "replace" => {
                if path.is_empty() {
                    doc = op["value"].clone();
                } else {
                    *parent(&mut doc, &pointer_parts(path)) = op["value"].clone();
                }
            }
            "move" => {
                let value = remove(&mut doc, op["from"].as_str().unwrap());
                add(&mut doc, path, value);
            }
            other => panic!("unexpected op {other}"),
        }
    }
    doc
}

fn apply_merge(target: &Value, patch: &Value) -> Value {
    let Value::Object(patch) = patch else {
        return patch.clone();
    };
    let mut out = target.as_object().cloned().unwrap_or_default();
    for (key, value) in patch {
        if value.is_null() {
            out.shift_remove(key);
        } else {
            let merged = apply_merge(out.get(key).unwrap_or(&Value::Null), value);
            out.insert(key.clone(), merged);
        }
    }
    Value::Object(out)
}

fn patch_of(left: &Value, right: &Value) -> Value {
    let mut ops = Vec::new();
    json_patch(left, right, "", &mut ops);
    Value::Array(ops)
}

#[test]
fn json_patch_roundtrips_mixed_changes() {
    let left = json!({
        "name": "a", "old": 1, "tags": ["x", "y", "z"], "nested": {"k": [1, {"v": true}]},
        "a/b": 1, "t~": 2, "type": 1
    });
    let right = json!({
        "name": "b", "renamed": 1, "tags": ["x", "q"], "nested": {"k": [1, {"v": false}, 3]},
        "a/b": 2, "t~": 2, "type": "1", "extra": null
    });
    let ops = patch_of(&left, &right);
    assert_eq!(apply(left.clone(), &ops), right);
    let text = ops.to_string();
    assert!(
        ops.as_array()
            .unwrap()
            .contains(&json!({"op": "move", "from": "/old", "path": "/renamed"}))
    );
    assert!(text.contains(r#""path":"/a~1b""#));
    assert!(text.contains(r#""path":"/tags/2""#));
}

#[test]
fn json_patch_handles_root_replacement_and_no_changes() {
    assert_eq!(patch_of(&json!([1]), &json!([1])), json!([]));
    let ops = patch_of(&json!({"a": 1}), &json!([1]));
    assert_eq!(ops, json!([{"op": "replace", "path": "", "value": [1]}]));
}

#[test]
fn merge_patch_matches_rfc_example() {
    // RFC 7386 §3 示例
    let target = json!({"title": "Goodbye!", "author": {"givenName": "John", "familyName": "Doe"}, "tags": ["example", "sample"], "content": "This will be unchanged"});
    let result = json!({"title": "Hello!", "author": {"givenName": "John"}, "tags": ["example"], "content": "This will be unchanged", "phoneNumber": "+01-123-456-7890"});
    let mut nulls = Vec::new();
    let patch = merge_patch(&target, &result, "", &mut nulls).unwrap();
    assert_eq!(
        patch,
        json!({"title": "Hello!", "author": {"familyName": null}, "tags": ["example"], "phoneNumber": "+01-123-456-7890"})
    );
    assert_eq!(apply_merge(&target, &patch), result);
    assert!(nulls.is_empty());
}

#[test]
fn merge_patch_reports_unrepresentable_nulls() {
    let mut nulls = Vec::new();
    merge_patch(
        &json!({"a": 1}),
        &json!({"a": null, "b": {"c": null}, "d": [null]}),
        "",
        &mut nulls,
    );
    assert_eq!(nulls, vec!["/a", "/b/c"]);
}

#[test]
fn run_formats_output_and_counts() {
    let out = run(Args {
        left: r#"{"a":1}"#.into(),
        right: r#"{"a":2}"#.into(),
        mode: Mode::Json,
        kind: Kind::Patch,
    })
    .unwrap();
    assert_eq!(out.operations, Some(1));
    assert!(out.output.starts_with("[\n  {"));
    let err = run(Args {
        left: "{".into(),
        right: "{}".into(),
        mode: Mode::Json,
        kind: Kind::Merge,
    })
    .unwrap_err();
    assert_eq!(err.params["side"], "left");
}
