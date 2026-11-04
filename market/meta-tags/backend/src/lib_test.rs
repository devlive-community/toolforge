use super::*;
use serde_json::json;

fn args() -> Args {
    Args {
        title: "Demo Page".into(),
        description: Some("A demo".into()),
        keywords: Some("demo, test".into()),
        author: Some("Alice".into()),
        image: Some("https://example.com/cover.png".into()),
        url: Some("https://example.com/demo".into()),
        og_type: Some("website".into()),
        site_name: Some("Example".into()),
        twitter_card: Some("summary_large_image".into()),
        locale: Some("zh_CN".into()),
        json_ld: Some("website".into()),
    }
}

#[test]
fn generates_full_head_block() {
    let out = generate(args()).unwrap();
    let html = &out.html;
    assert!(html.contains("<title>Demo Page</title>"));
    assert!(html.contains("<meta name=\"description\" content=\"A demo\">"));
    assert!(html.contains("og:title"));
    assert!(html.contains("og:image"));
    assert!(html.contains("twitter:card"));
    assert!(html.contains("canonical"));
    assert!(html.contains("hreflang=\"zh_CN\""));
    assert!(html.contains("application/ld+json"));
    assert!(html.contains("\"@type\":\"WebSite\""));
    assert_eq!(out.warnings.len(), 0);
}

#[test]
fn escapes_html_specials() {
    let mut custom = args();
    custom.title = "A <b> & \"quote\"".into();
    let out = generate(custom).unwrap();
    assert!(out.html.contains("A &lt;b&gt; &amp; &quot;quote&quot;"));
    assert!(!out.html.contains("<b>"));
}

#[test]
fn length_warnings() {
    let mut custom = args();
    custom.title = "长".repeat(61);
    custom.description = Some("描".repeat(161));
    let out = generate(custom).unwrap();
    assert!(
        out.warnings
            .iter()
            .any(|w| w.rule == "title_too_long" && w.actual == 61)
    );
    assert!(
        out.warnings
            .iter()
            .any(|w| w.rule == "description_too_long" && w.actual == 161)
    );
}

#[test]
fn validates_inputs() {
    let err = generate(Args {
        title: String::new(),
        description: None,
        keywords: None,
        author: None,
        image: None,
        url: None,
        og_type: None,
        site_name: None,
        twitter_card: None,
        locale: None,
        json_ld: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "meta.empty_title");
    let mut custom = args();
    custom.og_type = Some("book".into());
    let err = generate(custom).unwrap_err();
    assert_eq!(err.code, "meta.bad_og_type");
    let mut custom = args();
    custom.json_ld = Some("recipe".into());
    let err = generate(custom).unwrap_err();
    assert_eq!(err.code, "meta.bad_jsonld");
}

#[test]
fn dispatches_via_plugin() {
    let out = MetaTags::default()
        .call("generate", json!({"title": "T"}))
        .unwrap();
    assert!(out["html"].as_str().unwrap().contains("<title>T</title>"));
    assert_eq!(
        MetaTags::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&MetaTags::default()).unwrap();
    assert_eq!(checked, 1);
}
