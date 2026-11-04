use super::*;
use serde_json::json;

#[test]
fn generates_robots_with_groups_and_sitemaps() {
    let out = robots(RobotsArgs {
        groups: vec![
            Group {
                user_agent: "*".into(),
                rules: vec!["disallow:/admin".into(), "allow:/".into()],
                crawl_delay: Some(10),
            },
            Group {
                user_agent: "Googlebot".into(),
                rules: vec!["allow:/".into()],
                crawl_delay: None,
            },
        ],
        sitemaps: vec!["https://example.com/sitemap.xml".into()],
    })
    .unwrap();
    assert!(out.text.contains("User-agent: *"));
    assert!(out.text.contains("Disallow: /admin"));
    assert!(out.text.contains("Crawl-delay: 10"));
    assert!(out.text.contains("User-agent: Googlebot"));
    assert!(
        out.text
            .contains("Sitemap: https://example.com/sitemap.xml")
    );
    assert_eq!(out.groups, 2);
}

#[test]
fn robots_rejects_bad_rules() {
    let err = robots(RobotsArgs {
        groups: vec![],
        sitemaps: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "robots.no_groups");
    let err = robots(RobotsArgs {
        groups: vec![Group {
            user_agent: String::new(),
            rules: vec![],
            crawl_delay: None,
        }],
        sitemaps: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "robots.empty_ua");
    let err = robots(RobotsArgs {
        groups: vec![Group {
            user_agent: "*".into(),
            rules: vec!["block:/x".into()],
            crawl_delay: None,
        }],
        sitemaps: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "robots.bad_rule");
}

#[test]
fn sitemap_escapes_and_validates() {
    let out = sitemap(SitemapArgs {
        urls: vec![
            "https://example.com/a?x=1&y=2".into(),
            "relative/path".into(),
        ],
        lastmod: Some("2026-01-01".into()),
        changefreq: Some("weekly".into()),
        priority: Some(0.8),
    })
    .unwrap();
    assert!(out.xml.contains("&amp;"), "& is escaped");
    assert!(
        !out.xml.contains("relative/path"),
        "non-absolute URLs are warned, not included"
    );
    assert_eq!(out.warnings.len(), 1);
    assert_eq!(out.shards, 1);
}

#[test]
fn sitemap_validates_params() {
    let err = sitemap(SitemapArgs {
        urls: vec![],
        lastmod: None,
        changefreq: None,
        priority: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "sitemap.no_urls");
    let err = sitemap(SitemapArgs {
        urls: vec!["https://e.com".into()],
        lastmod: None,
        changefreq: Some("sometimes".into()),
        priority: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "sitemap.bad_changefreq");
    let err = sitemap(SitemapArgs {
        urls: vec!["https://e.com".into()],
        lastmod: None,
        changefreq: None,
        priority: Some(2.0),
    })
    .unwrap_err();
    assert_eq!(err.code, "sitemap.bad_priority");
}

#[test]
fn dispatches_via_plugin() {
    let out = RobotsSitemap::default()
        .call(
            "robots",
            json!({"groups": [{"userAgent": "*", "rules": ["disallow:/"]}]}),
        )
        .unwrap();
    assert!(out["text"].as_str().unwrap().contains("Disallow"));
    assert_eq!(
        RobotsSitemap::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&RobotsSitemap::default()).unwrap();
    assert_eq!(checked, 2);
}
