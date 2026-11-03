use super::*;
use crate::tests::{Context, parsed};
use serde_json::json;

fn report() -> Report {
    // Two runs: eslint (1 error, 1 suppressed warning, 1 unresolved-level result)
    // and clippy (1 note).
    parsed(
        r#"{"version":"2.1.0","runs":[
        {"tool":{"driver":{"name":"eslint","rules":[{"id":"R1","defaultConfiguration":{"level":"warning"}}]}},"artifacts":[{"location":{"uri":"a.ts"}}],
         "results":[
            {"ruleId":"R1","level":"error","message":{"text":"first issue"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.ts","index":0},"region":{"startLine":5}}}]},
            {"ruleId":"R1","message":{"text":"hidden issue"},"suppressions":[{"kind":"inSource","status":"accepted"}],"baselineState":"new"},
            {"ruleId":"R2","message":{"text":"third issue"},"baselineState":"unchanged"}
         ]},
        {"tool":{"driver":{"name":"clippy","rules":[]}},"results":[
            {"ruleId":"C1","level":"note","message":{"text":"clippy note"}}
         ]}
    ]}"#,
    )
}

fn browse_args(kind: &str, filter: Value) -> BrowseArgs {
    let mut value = json!({"handle":"1","kind":kind,"offset":0,"limit":25});
    if !filter.is_null() {
        value["filter"] = filter;
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn filters_narrow_results_across_every_axis() {
    let report = report();
    let all = browse(
        &report,
        &browse_args("results", json!({})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(all["total"], 4);
    let errors = browse(
        &report,
        &browse_args("results", json!({"level":"error"})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(errors["total"], 2);
    assert_eq!(errors["counts"]["results"], 2);
    let suppressed = browse(
        &report,
        &browse_args("results", json!({"suppressed":true})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(suppressed["total"], 1);
    assert_eq!(suppressed["items"][0]["baselineState"], "new");
    let rule = browse(
        &report,
        &browse_args("results", json!({"ruleId":"R1"})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(rule["total"], 2);
    let baseline = browse(
        &report,
        &browse_args("results", json!({"baseline":"unchanged"})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(baseline["total"], 1);
    let run = browse(
        &report,
        &browse_args("results", json!({"run":1})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(run["total"], 1);
    let query = browse(
        &report,
        &browse_args("results", json!({"query":"CLIPPY"})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(query["total"], 1);
    // Artifact filtering goes through the result's primary location.
    let artifact = browse(
        &report,
        &browse_args("results", json!({"artifact":0})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(artifact["total"], 1);
}

#[test]
fn browse_kinds_project_each_table() {
    let report = report();
    assert_eq!(
        browse(
            &report,
            &browse_args("runs", json!({})),
            &Context::default()
        )
        .unwrap()["total"],
        2
    );
    let rules = browse(
        &report,
        &browse_args("rules", json!({})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(rules["total"], 1);
    assert_eq!(rules["items"][0]["ruleId"], "R1");
    let artifacts = browse(
        &report,
        &browse_args("artifacts", json!({})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(artifacts["total"], 1);
    let diagnostics = browse(
        &report,
        &browse_args("diagnostics", json!({})),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(diagnostics["total"], 0);
}

#[test]
fn pagination_and_unknown_kinds_fail_cleanly() {
    let report = report();
    for args in [
        json!({"handle":"1","kind":"results","offset":0,"limit":0}),
        json!({"handle":"1","kind":"results","offset":0,"limit":101}),
        json!({"handle":"1","kind":"nope"}),
        json!({"handle":"1","kind":"results","filter":{"run":9}}),
    ] {
        let args: BrowseArgs = serde_json::from_value(args).unwrap();
        assert_eq!(
            browse(&report, &args, &Context::default())
                .err()
                .unwrap()
                .code,
            "sarif.page"
        );
    }
}

#[test]
fn result_scan_is_cancellable() {
    let report = report();
    let ctx = Context {
        cancel_after: Some(2),
        ..Context::default()
    };
    let filter: Filter = serde_json::from_value(json!({})).unwrap();
    assert_eq!(
        results(&report, &filter, &ctx).err().unwrap().code,
        "task.cancelled"
    );
}
