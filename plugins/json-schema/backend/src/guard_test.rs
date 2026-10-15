use serde_json::json;

use super::*;

#[test]
fn rejects_oversize_and_deep_before_parsing_but_ignores_brackets_in_strings() {
    assert_eq!(
        text(&" ".repeat(MAX_BYTES + 1), "instance")
            .unwrap_err()
            .code,
        "schema.too_large"
    );
    assert_eq!(
        text(
            &format!(
                "{}0{}",
                "[".repeat(MAX_DEPTH + 1),
                "]".repeat(MAX_DEPTH + 1)
            ),
            "instance"
        )
        .unwrap_err()
        .code,
        "schema.too_deep"
    );
    text(&format!("\"{}\"", "[".repeat(200)), "instance").unwrap();
    text(r#"{"x":"[\"[\\["}"#, "instance").unwrap();
}

#[test]
fn accepts_local_refs_and_rejects_all_nonfragment_forms() {
    for reference in [
        "file:///etc/passwd",
        "https://example.com/s.json",
        "http://localhost/s",
        "../s.json",
        "/tmp/s.json",
        "s.json#/$defs/s",
        "",
        "//host/schema",
    ] {
        let issue = schema(
            &json!({"properties":{"a":{"$ref":reference}}}),
            Draft::Draft202012,
        )
        .unwrap_err();
        assert_eq!(issue.code, "schema.external_ref");
        assert_eq!(issue.path, "/properties/a/$ref");
    }
    schema(
        &json!({"$defs":{"x":{"type":"string"}}, "$ref":"#/$defs/x"}),
        Draft::Draft202012,
    )
    .unwrap();
    schema(&json!({"$ref":"#anchor"}), Draft::Draft202012).unwrap();
}

#[test]
fn scans_all_schema_bearing_keywords_without_interpreting_instance_data() {
    schema(
        &json!({"const":{"$ref":"file:///data"}, "examples":[{"$ref":"https://data"}]}),
        Draft::Draft202012,
    )
    .unwrap();
    for value in [
        json!({"$defs":{"x":{"$dynamicRef":"https://remote"}}}),
        json!({"definitions":{"x":{"$ref":"relative"}}}),
        json!({"dependencies":{"x":{"$ref":"relative"}}}),
        json!({"items":[{"$ref":"relative"}]}),
        json!({"additionalProperties":{"$ref":"relative"}}),
        json!({"allOf":[{"$ref":"relative"}]}),
        json!({"if":{"$ref":"relative"}}),
        json!({"unevaluatedItems":{"$ref":"relative"}}),
    ] {
        assert_eq!(
            schema(&value, Draft::Draft202012).unwrap_err().code,
            "schema.external_ref"
        );
    }
}

#[test]
fn id_cannot_disguise_external_reference_as_embedded_resource() {
    let issue = schema(
        &json!({"$id":"file:///etc/secret", "$ref":"file:///etc/secret"}),
        Draft::Draft202012,
    )
    .unwrap_err();
    assert_eq!(issue.code, "schema.external_ref");
    let issue = schema(&json!({"$id":"https://example.test/root", "$defs":{"inner":{"$id":"child", "$ref":"child#"}}}), Draft::Draft202012).unwrap_err();
    assert_eq!(issue.code, "schema.external_ref");
}

#[test]
fn bounds_patterns_branches_nodes_and_preserves_pointer_escaping() {
    assert_eq!(
        schema(
            &json!({"pattern":"a".repeat(MAX_PATTERN_BYTES + 1)}),
            Draft::Draft202012
        )
        .unwrap_err()
        .code,
        "schema.pattern_length"
    );
    assert_eq!(
        schema(
            &json!({"anyOf": vec![json!(true); MAX_BRANCHES + 1]}),
            Draft::Draft202012
        )
        .unwrap_err()
        .code,
        "schema.branch_budget"
    );
    assert_eq!(
        nodes(&json!(vec![0; MAX_NODES]), "instance")
            .unwrap_err()
            .code,
        "schema.node_budget"
    );
    let issue = schema(
        &json!({"properties":{"a/b~c":{"$ref":"bad"}}}),
        Draft::Draft202012,
    )
    .unwrap_err();
    assert_eq!(issue.path, "/properties/a~1b~0c/$ref");
}

#[test]
fn bounds_arbitrary_precision_numeric_work_without_rounding_normal_numbers() {
    let huge: Value = serde_json::from_str(&"1".repeat(MAX_NUMBER_BYTES + 1)).unwrap();
    assert_eq!(
        nodes(&huge, "instance").unwrap_err().code,
        "schema.numeric_budget"
    );
    for literal in ["1e10001", "1e-10001", "1e999999999999999999999"] {
        let value: Value = serde_json::from_str(literal).unwrap();
        assert_eq!(
            nodes(&value, "schema").unwrap_err().code,
            "schema.numeric_budget"
        );
    }
    for literal in [
        "18446744073709551616",
        "0.0000000000000000000000001",
        "1e10000",
    ] {
        nodes(&serde_json::from_str(literal).unwrap(), "instance").unwrap();
    }
}

#[test]
fn bounds_json_keys_and_full_pointer_length_before_engine_allocations() {
    let value = json!({"a".repeat(MAX_KEY_BYTES + 1): 0});
    assert_eq!(
        nodes(&value, "instance").unwrap_err().code,
        "schema.path_budget"
    );
    let key = "k".repeat(MAX_KEY_BYTES);
    let mut nested = json!(0);
    for _ in 0..5 {
        nested = json!({key.clone(): nested});
    }
    assert_eq!(
        nodes(&nested, "schema").unwrap_err().code,
        "schema.path_budget"
    );
}

#[test]
fn promoted_reference_targets_share_the_pattern_budget() {
    let first = json!({"allOf": vec![json!({"pattern":"a"}); 100]});
    let second = json!({"allOf": vec![json!({"pattern":"b"}); 100]});
    let issue = schema_many(
        &[(&first, "/const".into()), (&second, "/payload".into())],
        Draft::Draft202012,
    )
    .unwrap_err();
    assert_eq!(issue.code, "schema.pattern_budget");
}
