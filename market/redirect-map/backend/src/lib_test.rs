use super::*;
use serde_json::json;

#[test]
fn generates_all_formats() {
    let mappings = vec![
        ("/old".to_string(), "/new".to_string()),
        ("/blog/2020".to_string(), "/posts/2020".to_string()),
    ];
    let out = generate(Args {
        mappings: mappings.clone(),
        format: "nginx".into(),
    })
    .unwrap();
    assert!(out.rules.contains("location = /old { return 301 /new; }"));
    let out = generate(Args {
        mappings: mappings.clone(),
        format: "apache".into(),
    })
    .unwrap();
    assert!(out.rules.contains("RewriteRule ^old$ /new [R=301,L]"));
    assert!(out.rules.contains("RewriteEngine On"));
    let out = generate(Args {
        mappings,
        format: "cloudflare".into(),
    })
    .unwrap();
    assert!(out.rules.contains("/old,/new"));
}

#[test]
fn flattens_chains() {
    let mappings = vec![
        ("/a".to_string(), "/b".to_string()),
        ("/b".to_string(), "/c".to_string()),
    ];
    let out = generate(Args {
        mappings,
        format: "nginx".into(),
    })
    .unwrap();
    assert_eq!(
        out.chains_flattened, 2,
        "/a and /b both walk the chain; /a skips /b, /b points at its own target"
    );
    // /a 直达 /c（跳过 /b）
    let a = out.redirects.iter().find(|r| r.from == "/a").unwrap();
    assert_eq!(a.to, "/c");
    assert!(out.cycles.is_empty());
}

#[test]
fn detects_cycles_and_conflicts() {
    let mappings = vec![
        ("/x".to_string(), "/y".to_string()),
        ("/y".to_string(), "/x".to_string()),
    ];
    let out = generate(Args {
        mappings,
        format: "nginx".into(),
    })
    .unwrap();
    assert_eq!(out.cycles.len(), 1, "x→y→x cycle detected");
    // 环路条目不生成规则
    assert!(
        out.redirects.is_empty()
            || out
                .redirects
                .iter()
                .all(|r| r.from != "/x" && r.from != "/y")
    );

    let mappings = vec![
        ("/old".to_string(), "/new1".to_string()),
        ("/old".to_string(), "/new2".to_string()),
    ];
    let out = generate(Args {
        mappings,
        format: "nginx".into(),
    })
    .unwrap();
    assert_eq!(out.conflicts, vec!["/old".to_string()]);
    assert_eq!(
        out.redirects.iter().find(|r| r.from == "/old").unwrap().to,
        "/new1",
        "first mapping wins"
    );
}

#[test]
fn validates_params() {
    let err = generate(Args {
        mappings: vec![],
        format: "nginx".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "redirect.no_mappings");
    let err = generate(Args {
        mappings: vec![("/a".into(), "/b".into())],
        format: "iis".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "redirect.bad_format");
    let err = generate(Args {
        mappings: vec![(String::new(), "/b".into())],
        format: "nginx".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "redirect.empty_mapping");
}

#[test]
fn dispatches_via_plugin() {
    let out = RedirectMap::default()
        .call(
            "generate",
            json!({"mappings": [["/a", "/b"]], "format": "nginx"}),
        )
        .unwrap();
    assert!(out["rules"].as_str().unwrap().contains("301"));
    assert_eq!(
        RedirectMap::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&RedirectMap::default()).unwrap();
    assert_eq!(checked, 1);
}
