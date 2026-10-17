use super::*;
use url::Url;

fn parse(link: &str) -> Option<String> {
    install_id(&Url::parse(link).unwrap())
}

#[test]
fn parses_install_links() {
    assert_eq!(
        parse("toolforge://plugin/install/org.devlive.toolforge.uscc").as_deref(),
        Some("org.devlive.toolforge.uscc")
    );
    // 尾随斜杠与多余路径段都忽略
    assert_eq!(
        parse("toolforge://plugin/install/org.devlive.toolforge.uscc/").as_deref(),
        Some("org.devlive.toolforge.uscc")
    );
    assert_eq!(
        parse("toolforge://plugin/install/org.devlive.toolforge.uscc/extra").as_deref(),
        Some("org.devlive.toolforge.uscc")
    );
}

#[test]
fn rejects_unsupported_links() {
    for link in [
        "https://toolforge.devlive.org/zh/tools/uscc/",
        "toolforge://plugins/install/org.devlive.toolforge.uscc",
        "toolforge://plugin/open/org.devlive.toolforge.uscc",
        "toolforge://plugin/install/",
        "toolforge://plugin/install/a%20b",
        "toolforge://plugin/install/org.devlive.toolforge.uscc?x=1",
    ] {
        // 查询串不影响路径解析，带 ? 的链接仍取到同一 id
        let expected = link
            .contains("?x=1")
            .then_some("org.devlive.toolforge.uscc");
        assert_eq!(parse(link).as_deref(), expected, "{link}");
    }
}

#[test]
fn finds_the_matching_index_entry() {
    let index = json!({
        "plugins": [
            { "id": "org.devlive.toolforge.uscc", "version": "0.1.2" },
            { "id": "org.devlive.toolforge.semver", "version": "0.2.0" }
        ]
    });
    let entry = find_entry(&index, "org.devlive.toolforge.semver").unwrap();
    assert_eq!(entry.get("version").and_then(Value::as_str), Some("0.2.0"));
    assert!(find_entry(&index, "org.devlive.toolforge.unknown").is_none());
    assert!(find_entry(&json!({}), "org.devlive.toolforge.uscc").is_none());
}
