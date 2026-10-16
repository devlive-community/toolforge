use super::*;

fn resource(kind: &str, url: &str) -> ResourceInput {
    ResourceInput {
        kind: kind.to_owned(),
        url: Some(url.to_owned()),
        nonce: None,
        hash: None,
    }
}

fn run_with(policies: &str, document: &str, resources: Vec<ResourceInput>) -> Report {
    run(policies, Some(document), &resources).unwrap()
}

#[test]
fn explicit_directive_overrides_default_src() {
    let report = run_with(
        "default-src 'none'; img-src 'self'",
        "https://a.com/",
        vec![
            resource("image", "https://a.com/logo.png"),
            resource("connect", "https://a.com/api"),
        ],
    );
    let image = &report.resources[0];
    assert_eq!(image.status, "allowed");
    assert_eq!(image.directive.as_deref(), Some("img-src"));
    assert_eq!(image.matched.as_deref(), Some("'self'"));
    let connect = &report.resources[1];
    assert_eq!(connect.status, "blocked");
    assert_eq!(connect.directive.as_deref(), Some("default-src"));
    assert_eq!(connect.matched.as_deref(), Some("'none'"));
}

#[test]
fn every_enforcing_policy_must_allow() {
    let policies = "script-src https://cdn.example.com\ndefault-src 'self'";
    let report = run_with(
        policies,
        "https://a.com/",
        vec![resource("script", "https://cdn.example.com/a.js")],
    );
    // 第二条策略没有 script-src，回退到 default-src 'self'，cdn 不在自身来源
    assert_eq!(report.resources[0].status, "blocked");
    let per_policy: Vec<&str> = report.resources[0]
        .policies
        .iter()
        .map(|p| p.status)
        .collect();
    assert_eq!(per_policy, vec!["allowed", "blocked"]);

    let report = run_with(
        "script-src https://a.com\ndefault-src 'self'",
        "https://a.com/",
        vec![resource("script", "https://a.com/a.js")],
    );
    // 两条策略都允许
    assert_eq!(report.resources[0].status, "allowed");
}

#[test]
fn report_only_reports_without_blocking() {
    let report = run_with(
        "report-only: script-src 'none'",
        "https://a.com/",
        vec![resource("script", "https://a.com/a.js")],
    );
    let verdict = &report.resources[0];
    // 只有 Report-Only 策略时不拦截
    assert_eq!(verdict.status, "unrestricted");
    assert!(
        verdict
            .reasons
            .iter()
            .any(|d| d.code == "csp.report_only_would_block")
    );
    assert_eq!(verdict.policies[0].status, "blocked");
    assert!(verdict.policies[0].report_only);
}

#[test]
fn strict_dynamic_is_undetermined_without_static_evidence() {
    let report = run_with(
        "script-src 'strict-dynamic' 'nonce-abc' https://cdn.example.com",
        "https://a.com/",
        vec![resource("script", "https://cdn.example.com/a.js")],
    );
    // 主机来源在 strict-dynamic 下不可静态判定
    assert_eq!(report.resources[0].status, "undetermined");
}

#[test]
fn nonce_allows_inline_resources() {
    let report = run_with(
        "script-src 'nonce-abc'",
        "https://a.com/",
        vec![
            ResourceInput {
                kind: "script".to_owned(),
                url: None,
                nonce: Some("abc".to_owned()),
                hash: None,
            },
            ResourceInput {
                kind: "script".to_owned(),
                url: None,
                nonce: None,
                hash: None,
            },
        ],
    );
    assert_eq!(report.resources[0].status, "allowed");
    assert_eq!(report.resources[0].matched.as_deref(), Some("'nonce-abc'"));
    // 没有内联元数据时静态无法判定
    assert_eq!(report.resources[1].status, "undetermined");
    assert!(
        report.resources[1]
            .reasons
            .iter()
            .any(|d| d.code == "csp.undetermined")
    );
}

#[test]
fn worker_falls_back_through_child_src() {
    let report = run_with(
        "default-src 'none'; child-src 'self'",
        "https://a.com/",
        vec![resource("worker", "https://a.com/w.js")],
    );
    assert_eq!(report.resources[0].status, "allowed");
    assert_eq!(report.resources[0].directive.as_deref(), Some("child-src"));
}

#[test]
fn upgrade_insecure_requests_rewrites_before_matching() {
    let report = run_with(
        "img-src https://cdn.example.com; upgrade-insecure-requests",
        "https://a.com/",
        vec![resource("image", "http://cdn.example.com/x.png")],
    );
    assert_eq!(report.resources[0].status, "allowed");
}

#[test]
fn self_without_document_is_undetermined() {
    let report = run(
        "img-src 'self'",
        None,
        &[resource("image", "https://a.com/x.png")],
    )
    .unwrap();
    assert_eq!(report.resources[0].status, "undetermined");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "csp.self_without_document")
    );
}

#[test]
fn unknown_resource_type_is_not_fatal() {
    let report = run_with(
        "default-src 'none'",
        "https://a.com/",
        vec![resource("websocket", "wss://a.com/x")],
    );
    assert_eq!(report.resources[0].status, "unrestricted");
    assert!(
        report.resources[0]
            .reasons
            .iter()
            .any(|d| d.code == "csp.unknown_type")
    );
}

#[test]
fn markdown_report_lists_policies_and_verdicts() {
    let report = run_with(
        "default-src 'self'; img-src 'none'",
        "https://a.com/",
        vec![resource("image", "https://a.com/x.png")],
    );
    let md = markdown(&report);
    assert!(md.contains("# CSP Inspection"));
    assert!(md.contains("## Policy 1"));
    assert!(md.contains("**blocked**"));
}
