use super::*;

#[test]
fn parses_directives_and_keeps_first_duplicate() {
    let policies = parse_policies(
        "default-src 'self'; script-src 'self' https://cdn; script-src https://other",
    )
    .unwrap();
    assert_eq!(policies.len(), 1);
    let policy = &policies[0];
    assert!(!policy.report_only);
    assert_eq!(policy.directives.len(), 2);
    let script = policy.directive("script-src").unwrap();
    // 规范：同名指令只保留第一条
    assert_eq!(script.values, vec!["'self'", "https://cdn"]);
    assert!(
        policy
            .diagnostics
            .iter()
            .any(|d| d.code == "csp.duplicate_directive")
    );
}

#[test]
fn keeps_unknown_directives_with_a_note() {
    let policies = parse_policies("default-src 'self'; future-directive 1").unwrap();
    let policy = &policies[0];
    assert!(policy.directive("future-directive").is_some());
    assert!(
        policy
            .diagnostics
            .iter()
            .any(|d| d.code == "csp.unknown_directive")
    );
}

#[test]
fn parses_report_only_prefix() {
    let policies = parse_policies("default-src 'self'\nreport-only: script-src 'none'").unwrap();
    assert!(!policies[0].report_only);
    assert!(policies[1].report_only);
}

#[test]
fn parses_source_expressions() {
    assert_eq!(parse_expr("'none'"), Some(Expr::None));
    assert_eq!(parse_expr("'self'"), Some(Expr::SelfOrigin));
    assert_eq!(parse_expr("'unsafe-inline'"), Some(Expr::UnsafeInline));
    assert_eq!(parse_expr("'strict-dynamic'"), Some(Expr::Other));
    assert_eq!(
        parse_expr("'nonce-abc'"),
        Some(Expr::Nonce("'nonce-abc'".to_owned()))
    );
    assert_eq!(
        parse_expr("'sha256-abc='"),
        Some(Expr::Hash("'sha256-abc='".to_owned()))
    );
    assert_eq!(parse_expr("data:"), Some(Expr::Scheme("data".to_owned())));
    assert_eq!(
        parse_expr("https://cdn.example.com/x/"),
        Some(Expr::Host {
            scheme: Some("https".to_owned()),
            host: "cdn.example.com".to_owned(),
            port: None,
            path: Some("/x/".to_owned()),
        })
    );
    assert_eq!(
        parse_expr("localhost:8080"),
        Some(Expr::Host {
            scheme: None,
            host: "localhost".to_owned(),
            port: Some(8080),
            path: None,
        })
    );
    assert_eq!(
        parse_expr("*"),
        Some(Expr::Host {
            scheme: None,
            host: "*".to_owned(),
            port: None,
            path: None,
        })
    );
}

#[test]
fn matches_self_with_https_upgrade_to_wss() {
    let document = Origin {
        scheme: "https".to_owned(),
        host: "a.com".to_owned(),
        port: 443,
    };
    let ctx = |url| MatchCtx {
        document: Some(&document),
        url: Some(url),
        nonce: None,
        hash: None,
    };
    let parts = url_parts(&Url::parse("https://a.com/x").unwrap());
    assert!(matches(&Expr::SelfOrigin, &ctx(&parts)).unwrap());
    let wss = url_parts(&Url::parse("wss://a.com/socket").unwrap());
    assert!(matches(&Expr::SelfOrigin, &ctx(&wss)).unwrap());
    let other = url_parts(&Url::parse("https://b.com/x").unwrap());
    assert!(!matches(&Expr::SelfOrigin, &ctx(&other)).unwrap());
}

#[test]
fn omitted_port_matches_only_the_default_port() {
    let document = Origin {
        scheme: "https".to_owned(),
        host: "a.com".to_owned(),
        port: 443,
    };
    let ctx = |url| MatchCtx {
        document: Some(&document),
        url: Some(url),
        nonce: None,
        hash: None,
    };
    let expr = parse_expr("https://a.com").unwrap();
    let same = url_parts(&Url::parse("https://a.com/x").unwrap());
    assert!(matches(&expr, &ctx(&same)).unwrap());
    let other_port = url_parts(&Url::parse("https://a.com:8443/x").unwrap());
    assert!(!matches(&expr, &ctx(&other_port)).unwrap());
}

#[test]
fn wildcard_and_path_matching() {
    let document = Origin {
        scheme: "https".to_owned(),
        host: "a.com".to_owned(),
        port: 443,
    };
    let ctx = |url| MatchCtx {
        document: Some(&document),
        url: Some(url),
        nonce: None,
        hash: None,
    };
    let expr = parse_expr("https://*.example.com/assets/").unwrap();
    let cdn = url_parts(&Url::parse("https://cdn.example.com/assets/a.js").unwrap());
    assert!(matches(&expr, &ctx(&cdn)).unwrap());
    let deep = url_parts(&Url::parse("https://a.b.example.com/assets/x.js").unwrap());
    assert!(matches(&expr, &ctx(&deep)).unwrap());
    let apex = url_parts(&Url::parse("https://example.com/assets/a.js").unwrap());
    assert!(!matches(&expr, &ctx(&apex)).unwrap());
    let wrong_path = url_parts(&Url::parse("https://cdn.example.com/img/a.png").unwrap());
    assert!(!matches(&expr, &ctx(&wrong_path)).unwrap());
}

#[test]
fn none_and_scheme_sources() {
    let document = Origin {
        scheme: "https".to_owned(),
        host: "a.com".to_owned(),
        port: 443,
    };
    let ctx = |url| MatchCtx {
        document: Some(&document),
        url: Some(url),
        nonce: None,
        hash: None,
    };
    let none_url = url_parts(&Url::parse("https://a.com/x").unwrap());
    assert!(!matches(&Expr::None, &ctx(&none_url)).unwrap());
    let data = parse_expr("data:").unwrap();
    let data_url = url_parts(&Url::parse("data:image/png;base64,AA").unwrap());
    assert!(matches(&data, &ctx(&data_url)).unwrap());
    let http_url = url_parts(&Url::parse("https://a.com/x").unwrap());
    assert!(!matches(&data, &ctx(&http_url)).unwrap());
}
