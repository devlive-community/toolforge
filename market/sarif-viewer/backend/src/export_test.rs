use super::*;
use crate::tests::{Context, parsed};
use serde_json::{Value, json};

fn report() -> Report {
    parsed(
        r#"{"version":"2.1.0","runs":[
        {"tool":{"driver":{"name":"eslint","rules":[]}},"artifacts":[{"location":{"uri":"a.ts"}}],
         "results":[
            {"ruleId":"R1","level":"error","message":{"text":"first \"quoted\" |pipe|"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.ts","index":0},"region":{"startLine":5,"startColumn":2}}}]},
            {"ruleId":"R2","level":"note","message":{"text":"note"},"suppressions":[{"kind":"inSource","status":"accepted"}]}
         ]}]}"#,
    )
}

#[test]
fn exports_reflect_filters_and_survive_special_characters() {
    let report = report();
    let ctx = Context::default();
    let filter: Filter = serde_json::from_value(json!({"level":"error"})).unwrap();
    let json_out = render(&report, &filter, "json", &ctx).unwrap();
    let value: Value = serde_json::from_str(&json_out).unwrap();
    assert_eq!(value["counts"]["results"], 1);
    assert_eq!(value["results"][0]["tool"], "eslint");
    assert_eq!(value["results"][0]["startLine"], 5);
    assert_eq!(value["codeFlowsIncluded"], false);

    let csv = render(&report, &filter, "csv", &ctx).unwrap();
    assert!(csv.starts_with("result_id,run,tool,rule_id,level,kind,suppressed,baseline_state,artifact_uri,start_line,start_column,message\r\n"));
    assert!(csv.contains("\"first \"\"quoted\"\" |pipe|\""));
    assert_eq!(csv.lines().count(), 2);

    let markdown = render(&report, &filter, "markdown", &ctx).unwrap();
    assert!(markdown.contains("# SARIF report"));
    assert!(markdown.contains("\\|pipe\\|"));
    assert!(markdown.contains("a.ts:5"));
}

#[test]
fn cancelled_or_unknown_formats_fail_without_output() {
    let report = report();
    let ctx = Context {
        cancel_after: Some(1),
        ..Context::default()
    };
    let filter: Filter = serde_json::from_value(json!({})).unwrap();
    assert_eq!(
        render(&report, &filter, "json", &ctx).err().unwrap().code,
        "task.cancelled"
    );
    let error = render(&report, &filter, "xml", &Context::default())
        .err()
        .unwrap();
    assert_eq!(error.code, "sarif.format");
}
