use super::*;

#[test]
fn strips_tracking_parameters() {
    let out = UrlParser::default()
        .call("strip_tracking", json!({ "input": "https://shop.example/item?id=42&utm_source=mail&spm=a1.b2&fbclid=xyz#reviews" }))
        .unwrap();
    assert_eq!(
        out,
        json!({ "href": "https://shop.example/item?id=42#reviews", "removed": 3 })
    );
}

#[test]
fn parses_and_builds_through_calls() {
    let tool = UrlParser::default();
    let parsed = tool
        .call("parse", json!({ "input": "https://a.example/?k=v" }))
        .unwrap();
    assert_eq!(parsed["params"][0]["key"], "k");
    assert_eq!(parsed["hostKind"], "domain");
    let built = tool
        .call(
            "build",
            json!({ "input": "https://a.example/?k=v", "params": [{ "key": "k", "value": "w" }] }),
        )
        .unwrap();
    assert_eq!(built["href"], "https://a.example/?k=w");
}

#[test]
fn detects_links_on_the_clipboard() {
    let tool = UrlParser::default();
    let with_query = tool.detect("https://example.com/a?b=1").unwrap();
    assert_eq!((with_query.score, with_query.label.as_str()), (70, "url"));
    assert_eq!(tool.detect("https://example.com/").unwrap().score, 40);
    assert!(tool.detect("just some text").is_none());
    assert!(tool.detect("https://example.com and more").is_none());
}
