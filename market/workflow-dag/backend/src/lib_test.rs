use super::*;
use serde_json::json;

const WORKFLOW: &str = r#"
name: CI
on:
  push:
    branches: [main]
  pull_request:
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build
  test:
    needs: build
    runs-on: ubuntu-latest
    if: github.event_name == 'push'
    steps:
      - run: cargo test
  deploy:
    needs: test
    steps:
      - run: echo deploy
  lint:
    steps:
      - run: cargo clippy
"#;

#[test]
fn parses_jobs_needs_triggers_and_levels() {
    let out = parse_workflow(Args {
        yaml: WORKFLOW.into(),
    })
    .unwrap();
    assert_eq!(out.name.as_deref(), Some("CI"));
    assert!(out.triggers.contains(&"push".to_string()));
    assert!(out.triggers.contains(&"pull_request".to_string()));
    assert_eq!(out.jobs.len(), 4);
    assert_eq!(out.edges, 2);
    let build = out.jobs.iter().find(|j| j.id == "build").unwrap();
    assert_eq!(build.level, 0);
    assert_eq!(build.steps, 2);
    let deploy = out.jobs.iter().find(|j| j.id == "deploy").unwrap();
    assert_eq!(deploy.level, 2, "build → test → deploy");
    let test = out.jobs.iter().find(|j| j.id == "test").unwrap();
    assert_eq!(
        test.condition.as_deref(),
        Some("github.event_name == 'push'")
    );
}

#[test]
fn generates_mermaid() {
    let out = parse_workflow(Args {
        yaml: WORKFLOW.into(),
    })
    .unwrap();
    assert!(out.mermaid.contains("flowchart TD"));
    assert!(out.mermaid.contains("-->"));
}

#[test]
fn detects_cycles_and_errors() {
    let cyclic = r#"
on: push
jobs:
  a:
    needs: b
  b:
    needs: a
"#;
    let out = parse_workflow(Args {
        yaml: cyclic.into(),
    })
    .unwrap();
    assert!(!out.cycles.is_empty(), "a ↔ b cycle");
    // 非法 YAML（tab 缩进）报 bad_yaml
    let err = parse_workflow(Args {
        yaml: "jobs:\n\tbad:\ttab".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "workflow.bad_yaml");
    let err = parse_workflow(Args {
        yaml: "jobs: {}".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "workflow.no_jobs");
}

#[test]
fn dispatches_via_plugin() {
    let out = WorkflowDag::default()
        .call("parse", json!({"yaml": "jobs:\n  a:\n    steps: []"}))
        .unwrap();
    assert_eq!(out["jobs"].as_array().unwrap().len(), 1);
    assert_eq!(
        WorkflowDag::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&WorkflowDag::default()).unwrap();
    assert_eq!(checked, 1);
}
