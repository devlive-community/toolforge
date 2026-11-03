use super::*;
use crate::tests::{Context, parsed};

fn sarif(runs: &str) -> String {
    format!(r#"{{"version":"2.1.0","runs":[{runs}]}}"#)
}
fn run(results: &str) -> String {
    format!(r#"{{"tool":{{"driver":{{"name":"scan","rules":[]}}}},"results":[{results}]}}"#)
}
fn result(fields: &str) -> String {
    format!(r#"{{"ruleId":"R","message":{{"text":"m"}},{fields}}}"#)
}

#[test]
fn multiple_runs_keep_tools_columns_and_counts() {
    let report = parsed(&sarif(&format!(
        "{},{}",
        run(&result(r#""level":"warning""#)),
        r#"{"tool":{"driver":{"name":"clippy","version":"1.0","rules":[]}},"columnKind":"unicodeCodePoints","results":[]}"#
    )));
    assert_eq!(report.runs.len(), 2);
    assert_eq!(report.runs[0].tool, "scan");
    assert_eq!(report.runs[1].tool, "clippy");
    assert_eq!(report.runs[1].version.as_deref(), Some("1.0"));
    assert_eq!(report.runs[1].column_kind, "unicodeCodePoints");
    assert_eq!(report.counts.results, 1);
    assert_eq!(report.counts.warnings, 1);
}

#[test]
fn level_precedence_is_level_then_rule_then_kind() {
    // Explicit level wins over the rule default; without one the rule default applies.
    let text = sarif(&format!(
        "{},{}",
        // explicit level wins over the rule default
        r#"{"tool":{"driver":{"name":"s","rules":[{"id":"R","defaultConfiguration":{"level":"note"}}]}},"results":[{"ruleId":"R","level":"warning","message":{"text":"m"}}]}"#,
        // no explicit level: rule default applies
        r#"{"tool":{"driver":{"name":"s","rules":[{"id":"R","defaultConfiguration":{"level":"note"}}]}},"results":[{"ruleId":"R","message":{"text":"m"}}]}"#
    ));
    let report = parsed(&text);
    assert_eq!(report.results[0].level, Level::Warning);
    assert_eq!(report.results[1].level, Level::Note);
    // Missing everywhere: kind=fail defaults to error, kind=pass to none.
    let text = sarif(&format!(
        "{},{}",
        r#"{"tool":{"driver":{"name":"s","rules":[{"id":"R"}]}},"results":[{"ruleId":"R","message":{"text":"m"}}]}"#,
        r#"{"tool":{"driver":{"name":"s","rules":[{"id":"R"}]}},"results":[{"ruleId":"R","kind":"pass","message":{"text":"m"}}]}"#
    ));
    let report = parsed(&text);
    assert_eq!(report.results[0].level, Level::Error);
    assert_eq!(report.results[1].level, Level::None);
    assert_eq!(report.counts.errors, 1);
    assert_eq!(report.counts.none, 1);
}

#[test]
fn rule_index_agreement_mismatch_and_missing_are_handled() {
    let text = sarif(
        r#"{"tool":{"driver":{"name":"s","rules":[{"id":"A"},{"id":"B"}]}},"results":[
        {"ruleId":"A","ruleIndex":0,"message":{"text":"m"}},
        {"ruleId":"B","ruleIndex":0,"message":{"text":"m"}},
        {"ruleId":"A","ruleIndex":7,"message":{"text":"m"}},
        {"ruleId":"ghost","message":{"text":"m"}}
    ]}"#,
    );
    let report = parsed(&text);
    assert_eq!(report.results[0].rule, Some(0));
    // ruleIndex wins; the disagreement is diagnosed but not fatal.
    assert_eq!(report.results[1].rule, Some(0));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.kind == "mismatch" && d.result == Some(1))
    );
    // Out-of-range index keeps the raw ruleId without a resolved rule.
    assert_eq!(report.results[2].rule, None);
    assert_eq!(report.results[2].rule_id, "A");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.kind == "ruleIndex" && d.result == Some(2))
    );
    // Undeclared ruleId is legal and stays unresolved without noise.
    assert_eq!(report.results[3].rule, None);
    assert_eq!(report.results[3].rule_id, "ghost");
}

#[test]
fn artifact_index_resolves_and_out_of_range_keeps_uri() {
    let text = sarif(
        r#"{"tool":{"driver":{"name":"s","rules":[]}},
        "artifacts":[{"location":{"uri":"a.ts"}},{"location":{"uri":"b.ts"}}],
        "results":[
            {"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.ts","index":1},"region":{"startLine":1}}}]},
            {"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"c.ts","index":9},"region":{"startLine":1}}}]}
        ]}"#,
    );
    let report = parsed(&text);
    assert_eq!(report.locations[0].artifact, Some(1));
    assert_eq!(report.artifacts[1].uri, "b.ts");
    assert_eq!(report.locations[1].artifact, None);
    assert_eq!(report.locations[1].uri.as_deref(), Some("c.ts"));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.kind == "artifact" && d.result == Some(1))
    );
}

#[test]
fn suppressions_and_baseline_state_are_kept_visible() {
    let report = parsed(&sarif(&run(&result(
        r#""baselineState":"unchanged","suppressions":[{"kind":"inSource","status":"underReview","justification":"todo"}]"#,
    ))));
    let row = &report.results[0];
    assert!(row.suppressed);
    assert_eq!(row.baseline_state.as_deref(), Some("unchanged"));
    assert_eq!(row.suppressions[0].status.as_deref(), Some("underReview"));
    assert_eq!(report.counts.suppressed, 1);
    // A suppressed result is never silently deleted from counts.
    assert_eq!(report.counts.results, 1);
}

#[test]
fn message_falls_back_to_markdown_then_id_template() {
    let text = sarif(
        r#"{"tool":{"driver":{"name":"s","rules":[{"id":"R","messageStrings":{"default":{"text":"bad {0} at {1}"}}}]}},"results":[
        {"ruleId":"R","message":{"markdown":"**md**"}},
        {"ruleId":"R","message":{"id":"default","arguments":["x","y"]}},
        {"ruleId":"R","message":{"id":"missing"}},
        {"ruleId":"R","message":{}}
    ]}"#,
    );
    let report = parsed(&text);
    assert_eq!(report.results[0].message, "**md**");
    assert_eq!(report.results[1].message, "bad x at y");
    assert_eq!(report.results[2].message, "missing");
    assert_eq!(report.results[3].message, "");
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|d| d.kind == "message")
            .count(),
        2
    );
}

#[test]
fn external_properties_and_extensions_are_flagged_not_resolved() {
    let text = sarif(
        r#"{"tool":{"driver":{"name":"s","rules":[]},"extensions":[{"name":"ext"}]},
        "externalPropertyFileReferences":{"rules":{"location":{"uri":"ext.json"}}},
        "results":[{"ruleId":"R","ruleIndex":5,"message":{"text":"m"}}]}"#,
    );
    let report = parsed(&text);
    assert!(report.runs[0].external_properties);
    assert_eq!(report.runs[0].extension_count, 1);
    assert!(report.diagnostics.iter().any(|d| d.kind == "external"));
    assert!(report.diagnostics.iter().any(|d| d.kind == "extensions"));
    assert!(report.diagnostics.iter().any(|d| d.kind == "ruleIndex"));
}

#[test]
fn unsupported_or_missing_version_is_reported() {
    assert_eq!(
        crate::tests::first_error(
            r#"{"version":"3.0.0","runs":[{"tool":{"driver":{"name":"s","rules":[]}}}]}"#
        )
        .code,
        "sarif.version"
    );
    let report = parsed(r#"{"runs":[{"tool":{"driver":{"name":"s","rules":[]}}}]}"#);
    assert!(report.diagnostics.iter().any(|d| d.kind == "version"));
}

#[test]
fn broken_json_and_oversized_strings_fail_with_reasons() {
    let error = crate::tests::first_error("{ not json");
    assert_eq!(error.code, "sarif.json");
    assert!(error.params.contains_key("detail"));
    let long = "x".repeat(STRING_LIMIT + 1);
    let mut report = Report::default();
    let error = parse(
        &mut report,
        "big.sarif".into(),
        &sarif(&format!(r#"{{"tool":{{"driver":{{"name":"s","rules":[]}}}},"results":[{{"ruleId":"R","message":{{"text":"{long}"}}}}]}}"#)),
        &Context::default(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "sarif.string");
}

#[test]
fn deep_nesting_is_rejected() {
    let mut nested = String::new();
    for _ in 0..70 {
        nested.push_str("{\"a\":");
    }
    nested.push_str("\"x\"");
    for _ in 0..70 {
        nested.push('}');
    }
    let error = crate::tests::first_error(&format!(
        r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"s","properties":{nested}}},"rules":[]}}}}]}}"#
    ));
    assert_eq!(error.code, "sarif.depth");
}

#[test]
fn region_coordinates_out_of_range_are_diagnosed() {
    let report = parsed(&sarif(&run(&result(
        r#""locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.ts"},"region":{"startLine":0,"startColumn":99999999999}}}]"#,
    ))));
    assert_eq!(report.locations[0].region.unwrap().start_line, None);
    assert!(report.diagnostics.iter().any(|d| d.kind == "region"));
}
