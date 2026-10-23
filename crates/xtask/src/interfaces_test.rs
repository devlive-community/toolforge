use serde_json::json;

use super::*;

fn locales() -> Vec<(String, Value)> {
    let messages = json!({
        "api": {
            "run": { "summary": "Run", "input": "Input", "paths": "Files", "out": "Output" },
            "col": "Column"
        }
    });
    vec![
        ("en-US".to_owned(), messages.clone()),
        ("zh-CN".to_owned(), messages),
    ]
}

fn manifest(interface: Value) -> Value {
    json!({
        "id": "org.devlive.toolforge.demo",
        "permissions": ["fs:user-selected", "clipboard:write"],
        "functions": { "run": { "interface": interface } }
    })
}

fn valid() -> Value {
    json!({
        "expose": ["cli", "agent"],
        "summary": "i18n:api.run.summary",
        "effect": "read",
        "uses": ["fs:user-selected"],
        "params": {
            "type": "object",
            "required": ["paths"],
            "properties": {
                "input": { "type": "string", "description": "i18n:api.run.input", "x-stdin": true },
                "paths": {
                    "type": "array",
                    "description": "i18n:api.run.paths",
                    "items": { "type": "string", "format": "path", "x-access": "read" },
                    "x-positional": 0
                }
            }
        },
        "output": { "table": "/results", "columns": [{ "key": "digest", "label": "i18n:api.col" }] },
        "examples": ["echo hi | tfp-demo run"]
    })
}

fn violations(manifest: &Value, require_decided: bool) -> Vec<String> {
    let locales = locales();
    let plugin = Plugin {
        relative: "plugins/demo/manifest.json",
        manifest,
        locales: &locales,
    };
    check(&plugin, require_decided)
        .into_iter()
        .map(|v| v.message)
        .collect()
}

/// 设置或新增 pointer 指向的字段
fn with(mut interface: Value, pointer: &str, value: Value) -> Value {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    interface.pointer_mut(parent).unwrap()[key] = value;
    interface
}

#[test]
fn accepts_a_complete_declaration() {
    assert_eq!(violations(&manifest(valid()), true), Vec::<String>::new());
}

#[test]
fn undecided_functions_fail_only_when_required() {
    let undecided = json!({ "id": "x.demo", "functions": { "run": {} } });
    assert!(violations(&undecided, false).is_empty());
    assert_eq!(violations(&undecided, true).len(), 1);
    assert!(violations(&manifest(json!(false)), true).is_empty());
}

#[test]
fn rejects_unknown_channels_effects_and_permissions() {
    assert!(!violations(&manifest(with(valid(), "/expose", json!(["web"]))), false).is_empty());
    assert!(
        !violations(
            &manifest(with(valid(), "/effect", json!("teleport"))),
            false
        )
        .is_empty()
    );
    assert!(!violations(&manifest(with(valid(), "/uses", json!(["network"]))), false).is_empty());
}

#[test]
fn requires_i18n_keys_in_every_locale() {
    let missing = with(valid(), "/summary", json!("i18n:api.run.nothing"));
    assert_eq!(violations(&manifest(missing), false).len(), 2);
    let plain = with(valid(), "/summary", json!("Run it"));
    assert_eq!(violations(&manifest(plain), false).len(), 1);
}

#[test]
fn path_params_need_fs_permission_and_matching_effect() {
    let no_uses = with(valid(), "/uses", json!([]));
    assert!(violations(&manifest(no_uses), false)[0].contains("fs:user-selected"));

    let write = with(
        valid(),
        "/params/properties/paths/items/x-access",
        json!("write"),
    );
    assert!(violations(&manifest(write.clone()), false)[0].contains("effect write"));
    assert!(violations(&manifest(with(write, "/effect", json!("write"))), false).is_empty());

    let no_access = with(
        valid(),
        "/params/properties/paths/items/x-access",
        json!("rw"),
    );
    assert!(!violations(&manifest(no_access), false).is_empty());
}

#[test]
fn rejects_nested_or_unsupported_schema() {
    let nested = with(valid(), "/params/properties/input/type", json!("object"));
    assert!(!violations(&manifest(nested), false).is_empty());
    let keyword = with(valid(), "/params/properties/input/pattern", json!("^a"));
    assert!(
        violations(&manifest(keyword), false)
            .iter()
            .any(|m| m.contains("pattern"))
    );
    let required = with(valid(), "/params/required", json!(["ghost"]));
    assert!(!violations(&manifest(required), false).is_empty());
}

#[test]
fn positional_and_stdin_rules() {
    let gap = with(valid(), "/params/properties/input/x-positional", json!(2));
    assert!(
        violations(&manifest(gap), false)
            .iter()
            .any(|m| m.contains("without gaps"))
    );
    let stdin_on_array = with(valid(), "/params/properties/paths/x-stdin", json!(true));
    assert!(!violations(&manifest(stdin_on_array), false).is_empty());
}

#[test]
fn output_and_examples_are_checked() {
    let text = with(valid(), "/output", json!({ "text": "/digest" }));
    assert!(violations(&manifest(text), false).is_empty());
    let bad_pointer = with(valid(), "/output", json!({ "text": "digest" }));
    assert!(!violations(&manifest(bad_pointer), false).is_empty());
    let wrong_command = with(valid(), "/examples", json!(["tfp-other run"]));
    assert!(violations(&manifest(wrong_command), false)[0].contains("tfp-demo"));
}
