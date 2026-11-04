use super::*;
use serde_json::json;

#[test]
fn flags_latest_root_add_and_healthcheck() {
    let content = "FROM ubuntu\nADD ./app /app\nUSER root\nRUN apt-get install -y curl\n";
    let out = lint(Args {
        content: content.into(),
    })
    .unwrap();
    let rules: Vec<&str> = out.issues.iter().map(|i| i.rule).collect();
    assert!(rules.contains(&"pin_tag"), "FROM ubuntu has no tag");
    assert!(rules.contains(&"use_copy"), "ADD for a local file");
    assert!(rules.contains(&"no_root"), "USER root");
    assert!(rules.contains(&"apt_cache_cleanup"), "no apt lists cleanup");
    assert!(rules.contains(&"no_healthcheck"));
    // 行号
    let add = out.issues.iter().find(|i| i.rule == "use_copy").unwrap();
    assert_eq!(add.line, 2);
    assert!(add.fix.contains("COPY ./app /app"));
}

#[test]
fn clean_dockerfile_passes() {
    let content = "FROM alpine:3.19 AS build\nCOPY . /src\nRUN apk add --no-cache build-base\nUSER appuser\nHEALTHCHECK CMD true\n";
    let out = lint(Args {
        content: content.into(),
    })
    .unwrap();
    assert_eq!(out.stages, 1);
    assert!(
        out.issues
            .iter()
            .all(|i| i.rule == "no_healthcheck" || i.rule != "pin_tag"),
        "tag is pinned"
    );
    assert!(!out.issues.iter().any(|i| i.rule == "no_root"));
    assert!(!out.issues.iter().any(|i| i.rule == "use_copy"));
    assert!(!out.issues.iter().any(|i| i.rule == "apt_cache_cleanup"));
    assert!(!out.issues.iter().any(|i| i.rule == "pin_tag"));
}

#[test]
fn multiline_run_continuation_is_parsed() {
    let content = "FROM debian:12\nRUN apt-get update && apt-get install -y \\\n  curl \\\n  && rm -rf /var/lib/apt/lists/*\n";
    let out = lint(Args {
        content: content.into(),
    })
    .unwrap();
    assert!(
        !out.issues.iter().any(|i| i.rule == "apt_cache_cleanup"),
        "cleanup present in continuation"
    );
}

#[test]
fn validates_input() {
    let err = lint(Args {
        content: String::new(),
    })
    .unwrap_err();
    assert_eq!(err.code, "dockerfile.empty");
}

#[test]
fn dispatches_via_plugin() {
    let out = DockerfileLint::default()
        .call("lint", json!({"content": "FROM node:20"}))
        .unwrap();
    assert!(
        out["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule"] == "no_healthcheck")
    );
    assert_eq!(
        DockerfileLint::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&DockerfileLint::default()).unwrap();
    assert_eq!(checked, 1);
}
