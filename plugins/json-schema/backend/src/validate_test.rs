use std::sync::atomic::Ordering;

use serde_json::json;

use super::*;
use crate::tests::Context;

fn validate(instance: &str, schema: &str, draft: Draft, formats: bool) -> Report {
    run(
        Args {
            instance: instance.into(),
            schema: schema.into(),
            draft,
            validate_formats: formats,
        },
        &Context::default(),
    )
    .unwrap()
}

#[test]
fn validates_booleans_scalars_arrays_and_objects() {
    for instance in ["null", "true", "1", "\"text\"", "[]", "{}"] {
        assert!(validate(instance, "true", Draft::Draft202012, false).valid);
        assert!(!validate(instance, "false", Draft::Draft7, false).valid);
    }
    let report = validate(
        r#"{"name":"Alice","roles":["dev"]}"#,
        r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"roles":{"type":"array","items":{"type":"string"}}}}"#,
        Draft::Draft202012,
        false,
    );
    assert!(report.valid);
    assert!(report.schema_valid);
    assert!(!report.truncated);
}

#[test]
fn schema_legality_and_both_syntax_errors_are_reported() {
    let report = validate("{", "{", Draft::Draft202012, false);
    assert_eq!(report.diagnostics.len(), 2);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|d| d.code == "schema.invalid_json" && d.line == Some(1))
    );
    let report = validate(
        "{}",
        r#"{"type":"nonsense","minLength":-1}"#,
        Draft::Draft202012,
        false,
    );
    assert!(!report.schema_valid);
    assert!(report.diagnostics.iter().all(|d| d.target == "schema"));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.instance_path == "/type")
    );
}

#[test]
fn explicit_drafts_have_distinct_tuple_rules_and_mismatch_is_not_silent() {
    assert!(
        validate(
            "[1]",
            r#"{"items":[{"type":"integer"}],"additionalItems":false}"#,
            Draft::Draft7,
            false
        )
        .valid
    );
    assert!(
        !validate(
            "[1]",
            r#"{"items":[{"type":"integer"}]}"#,
            Draft::Draft202012,
            false
        )
        .schema_valid
    );
    assert!(
        validate(
            "[1]",
            r#"{"prefixItems":[{"type":"integer"}],"items":false}"#,
            Draft::Draft202012,
            false
        )
        .valid
    );
    let report = validate(
        "{}",
        r#"{"$schema":"http://json-schema.org/draft-07/schema#"}"#,
        Draft::Draft202012,
        false,
    );
    assert_eq!(report.diagnostics[0].code, "schema.draft_mismatch");
    assert_eq!(report.diagnostics[0].instance_path, "/$schema");
    let report = validate(
        "{}",
        r#"{"$schema":"file:///unknown-schema"}"#,
        Draft::Draft202012,
        false,
    );
    assert_eq!(report.diagnostics[0].code, "schema.draft_mismatch");
}

#[test]
fn format_checks_are_explicit_in_both_drafts() {
    for draft in [Draft::Draft7, Draft::Draft202012] {
        assert!(
            validate(
                r#""not-an-email""#,
                r#"{"type":"string","format":"email"}"#,
                draft,
                false
            )
            .valid
        );
        assert!(
            !validate(
                r#""not-an-email""#,
                r#"{"type":"string","format":"email"}"#,
                draft,
                true
            )
            .valid
        );
        assert!(
            validate(
                r#""alice@example.com""#,
                r#"{"type":"string","format":"email"}"#,
                draft,
                true
            )
            .valid
        );
        assert!(
            validate(
                r#""text""#,
                r#"{"format":"custom-unregistered"}"#,
                draft,
                true
            )
            .valid
        );
    }
}

#[test]
fn local_references_return_canonical_schema_and_escaped_instance_paths() {
    let schema = r##"{"$id":"https://example.test/root","$defs":{"positive":{"type":"integer","minimum":0}},"properties":{"a/b~c":{"$ref":"#/$defs/positive"}}}"##;
    let report = validate(r#"{"a/b~c":-1}"#, schema, Draft::Draft202012, false);
    assert!(report.schema_valid);
    assert_eq!(report.diagnostics[0].instance_path, "/a~1b~0c");
    assert_eq!(report.diagnostics[0].schema_path, "/$defs/positive/minimum");
    assert!(validate(r#"{"a/b~c":2}"#, schema, Draft::Draft202012, false).valid);
    let anchored =
        r##"{"$defs":{"integer":{"$anchor":"integer","type":"integer"}},"$ref":"#integer"}"##;
    assert!(validate("42", anchored, Draft::Draft202012, false).valid);
}

#[test]
fn external_and_missing_references_never_read_files() {
    let context = Context::default();
    for schema in [
        r#"{"$ref":"file:///etc/passwd"}"#,
        r#"{"$ref":"https://127.0.0.1:1/no-network"}"#,
        r#"{"$ref":"../schema.json"}"#,
        r##"{"$ref":"#/$defs/missing"}"##,
        r##"{"$id":"https://example.test/root","$defs":{"branch":{"$id":"file:///etc/passwd","$ref":"#/definitions/secret"}},"$ref":"#/$defs/branch"}"##,
    ] {
        let outcome = run(
            Args {
                instance: "{}".into(),
                schema: schema.into(),
                draft: Draft::Draft202012,
                validate_formats: false,
            },
            &context,
        );
        match outcome {
            Ok(report) => {
                assert!(!report.valid);
                assert!(!report.schema_valid);
            }
            Err(error) => assert_eq!(error.code, "schema.reference_target"),
        }
    }
    assert_eq!(context.file_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn diagnostics_truncate_and_large_values_are_masked() {
    let instance = serde_json::to_string(&vec!["secret"; 250]).unwrap();
    let report = validate(
        &instance,
        r#"{"items":{"type":"integer"}}"#,
        Draft::Draft202012,
        false,
    );
    assert_eq!(report.diagnostics.len(), MAX_DIAGNOSTICS);
    assert!(report.truncated);
    assert!(!report.diagnostics[0].detail.contains("secret"));
}

#[test]
fn preserves_arbitrary_precision_and_exact_decimal_constraints() {
    assert!(
        validate(
            "18446744073709551616",
            r#"{"type":"integer","minimum":18446744073709551616}"#,
            Draft::Draft202012,
            false
        )
        .valid
    );
    assert!(
        !validate(
            "18446744073709551615",
            r#"{"minimum":18446744073709551616}"#,
            Draft::Draft202012,
            false
        )
        .valid
    );
    assert!(validate("0.3", r#"{"multipleOf":0.1}"#, Draft::Draft202012, false).valid);
}

#[test]
fn regex_failures_and_cancellation_are_structured() {
    let report = validate(
        r#""abc""#,
        r#"{"pattern":"[invalid"}"#,
        Draft::Draft202012,
        false,
    );
    assert!(!report.schema_valid);
    let report = validate(
        r#""abc""#,
        r#"{"pattern":"a{10000000}"}"#,
        Draft::Draft202012,
        false,
    );
    assert!(!report.schema_valid);
    let ctx = Context::default();
    ctx.cancelled.store(true, Ordering::SeqCst);
    let error = run(
        Args {
            instance: "{}".into(),
            schema: "{}".into(),
            draft: Draft::Draft202012,
            validate_formats: false,
        },
        &ctx,
    )
    .unwrap_err();
    assert_eq!(error.code, "task.cancelled");
}

#[test]
fn schema_examples_are_data_even_when_they_look_like_references() {
    let data = json!({"$ref":"file:///example"});
    let schema = json!({"const":data, "examples":[data]});
    assert!(
        validate(
            &data.to_string(),
            &schema.to_string(),
            Draft::Draft202012,
            false
        )
        .valid
    );
}

#[test]
fn regex_backtracking_budget_fails_closed() {
    // The ambiguous alternatives force VM backtracking. A nested a+ followed
    // by a backreference can be rejected by a regex prefilter without touching
    // the VM budget, so assert the engine's structured failure as a precondition.
    let instance = json!(format!("{}b", "a".repeat(200)));
    let schema = json!({"pattern":r"^(a|aa)+\1$"});
    let validator = jsonschema::options()
        .with_pattern_options(PatternOptions::fancy_regex().backtrack_limit(10_000))
        .build(&schema)
        .unwrap();
    let error = validator.validate(&instance).unwrap_err();
    assert!(
        matches!(
            error.kind(),
            ValidationErrorKind::BacktrackLimitExceeded { .. }
        ),
        "expected an exhausted backtracking budget, got {:?}",
        error.kind()
    );
    let report = validate(
        &instance.to_string(),
        &schema.to_string(),
        Draft::Draft202012,
        false,
    );
    assert!(report.schema_valid);
    assert!(!report.valid);
    assert_eq!(
        report.diagnostics[0].code, "schema.regex_budget",
        "{report:?}"
    );
}

#[test]
fn recursive_document_refs_validate_progressively_nested_data() {
    let schema = r##"{"$defs":{"node":{"type":"object","required":["value"],"properties":{"value":{"type":"integer"},"next":{"$ref":"#/$defs/node"}}}},"$ref":"#/$defs/node"}"##;
    assert!(
        validate(
            r#"{"value":1,"next":{"value":2}}"#,
            schema,
            Draft::Draft202012,
            false
        )
        .valid
    );
    let report = validate(
        r#"{"value":1,"next":{"value":"wrong"}}"#,
        schema,
        Draft::Draft202012,
        false,
    );
    assert!(report.schema_valid);
    assert!(!report.valid);
    assert_eq!(report.diagnostics[0].instance_path, "/next/value");
}

#[test]
fn no_progress_self_references_terminate_without_infinite_expansion() {
    for schema in [
        r##"{"$ref":"#"}"##,
        r##"{"$defs":{"a":{"$ref":"#/$defs/b"},"b":{"$ref":"#/$defs/a"}},"$ref":"#/$defs/a"}"##,
    ] {
        for instance in ["42", "null", r#"{"nested":[1,2,3]}"#] {
            assert!(validate(instance, schema, Draft::Draft202012, false).valid);
        }
    }
}
