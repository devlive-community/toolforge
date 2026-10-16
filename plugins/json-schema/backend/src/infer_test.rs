use serde_json::json;

use super::*;

fn infer(input: &str, split: Split) -> Value {
    let out = run(Args {
        input: input.into(),
        split,
        draft: Draft::Draft202012,
        enums: true,
    })
    .unwrap();
    serde_json::from_str(&out.schema).unwrap()
}

fn assert_valid(schema: &Value, instances: &[Value]) {
    let validator = jsonschema::validator_for(schema).unwrap();
    for instance in instances {
        assert!(
            validator.is_valid(instance),
            "{instance} should match {schema}"
        );
    }
}

#[test]
fn merges_samples_into_types_required_and_formats() {
    let samples = [
        json!({"id": 1, "name": "Ann", "email": "ann@example.com", "created": "2026-10-01T08:00:00Z", "score": 9, "tags": ["a"], "role": "admin"}),
        json!({"id": 2, "name": "Bob", "email": "bob@example.org", "created": "2026-10-02T09:30:00+08:00", "score": 7.5, "tags": [], "role": "user", "nick": null}),
        json!({"id": 3, "name": null, "email": "c@example.net", "created": "2026-10-03T00:00:00.123Z", "score": 8, "tags": ["b", "c"], "role": "user", "nick": "cc"}),
    ];
    let input = Value::Array(samples.to_vec()).to_string();
    let schema = infer(&input, Split::Array);
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["type"], "object");
    assert_eq!(
        schema["required"],
        json!(["id", "name", "email", "created", "score", "tags", "role"])
    );
    let props = &schema["properties"];
    assert_eq!(props["id"], json!({"type": "integer"}));
    assert_eq!(props["name"], json!({"type": ["string", "null"]}));
    assert_eq!(props["email"], json!({"type": "string", "format": "email"}));
    assert_eq!(
        props["created"],
        json!({"type": "string", "format": "date-time"})
    );
    assert_eq!(props["score"], json!({"type": "number"}));
    assert_eq!(
        props["tags"],
        json!({"type": "array", "items": {"type": "string"}})
    );
    assert_eq!(
        props["role"],
        json!({"type": "string", "enum": ["admin", "user"]})
    );
    assert_eq!(props["nick"], json!({"type": ["string", "null"]}));
    assert_valid(&schema, &samples);
}

#[test]
fn mixed_complex_types_use_any_of() {
    let schema = infer(
        "{\"v\": 1}\n{\"v\": {\"a\": true}}\n{\"v\": [1, \"x\"]}\n",
        Split::Lines,
    );
    let any_of = schema["properties"]["v"]["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 3);
    assert_eq!(
        any_of[1],
        json!({"type": "array", "items": {"type": ["string", "integer"]}})
    );
    assert_valid(
        &schema,
        &[
            json!({"v": 1}),
            json!({"v": {"a": true}}),
            json!({"v": [1, "x"]}),
        ],
    );
}

#[test]
fn detects_string_formats() {
    let cases = [
        ("2026-10-15", Some("date")),
        ("08:30:00Z", Some("time")),
        ("https://toolforge.devlive.org/a?b=1", Some("uri")),
        ("123e4567-e89b-12d3-a456-426614174000", Some("uuid")),
        ("192.168.1.1", Some("ipv4")),
        ("2001:db8::1", Some("ipv6")),
        ("hello world", None),
        ("2026-13-01", None),
    ];
    for (text, expected) in cases {
        assert_eq!(detect_format(text).map(Format::name), expected, "{text}");
    }
}

#[test]
fn single_document_and_enum_switch() {
    let schema = infer(
        r#"{"a":"x","list":[{"k":1},{"k":2,"j":true}]}"#,
        Split::Document,
    );
    assert_eq!(schema["properties"]["a"], json!({"type": "string"}));
    assert_eq!(
        schema["properties"]["list"]["items"],
        json!({"type": "object", "properties": {"k": {"type": "integer"}, "j": {"type": "boolean"}}, "required": ["k"]})
    );
    let out = run(Args {
        input: r#"["a","a","b"]"#.into(),
        split: Split::Array,
        draft: Draft::Draft7,
        enums: false,
    })
    .unwrap();
    let schema: Value = serde_json::from_str(&out.schema).unwrap();
    assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
    assert_eq!(schema.get("enum"), None);
    assert_eq!(out.samples, 3);
}

#[test]
fn reports_input_errors() {
    let err = |input: &str, split| {
        run(Args {
            input: input.into(),
            split,
            draft: Draft::Draft202012,
            enums: true,
        })
        .unwrap_err()
        .code
    };
    assert_eq!(err("{}\n{oops}\n", Split::Lines), "schema.infer_syntax");
    assert_eq!(err("{}", Split::Array), "schema.infer_not_array");
    assert_eq!(err("[]", Split::Array), "schema.infer_empty");
    assert_eq!(err("  ", Split::Document), "schema.infer_empty");
}
