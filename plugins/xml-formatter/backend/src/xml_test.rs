use super::*;

fn run(input: &str, mode: Mode, indent: Indent) -> PluginResult<Output> {
    process(
        Args {
            input: input.into(),
            indent,
        },
        mode,
    )
}

fn pretty(input: &str) -> PluginResult<Output> {
    run(input, Mode::Pretty, Indent::Two)
}

#[test]
fn formats_nested_elements_with_attributes() {
    let out = pretty(r#"<?xml version="1.0"?><root a="1"><item id="x">text</item><empty/></root>"#)
        .unwrap();
    assert_eq!(
        out.output,
        "<?xml version=\"1.0\"?>\n<root a=\"1\">\n  <item id=\"x\">text</item>\n  <empty/>\n</root>"
    );
    assert_eq!(out.stats.elements, 3);
    assert_eq!(out.stats.attributes, 2);
    assert_eq!(out.stats.max_depth, 2);
}

#[test]
fn minifies_and_keeps_comments_and_cdata() {
    let input = "<root>\n  <!-- note -->\n  <a><![CDATA[x < y]]></a>\n</root>";
    let out = run(input, Mode::Minify, Indent::Two).unwrap();
    assert_eq!(
        out.output,
        "<root><!-- note --><a><![CDATA[x < y]]></a></root>"
    );
    assert_eq!(out.stats.comments, 1);
}

#[test]
fn supports_tab_and_four_space_indent() {
    let tab = run("<a><b/></a>", Mode::Pretty, Indent::Tab).unwrap();
    assert_eq!(tab.output, "<a>\n\t<b/>\n</a>");
    let four = run("<a><b/></a>", Mode::Pretty, Indent::Four).unwrap();
    assert_eq!(four.output, "<a>\n    <b/>\n</a>");
}

#[test]
fn reports_mismatched_tags_with_position() {
    let err = pretty("<root>\n  <a></b>\n</root>").unwrap_err();
    assert_eq!(err.code, "xml.syntax");
    assert_eq!(err.params["line"], 2);
}

#[test]
fn reports_unclosed_elements() {
    let err = pretty("<root>\n  <a>").unwrap_err();
    assert_eq!(err.code, "xml.unclosed");
    assert_eq!(err.params["tag"], "a");
    assert_eq!(err.params["line"], 2);
}

#[test]
fn rejects_documents_without_single_root() {
    assert_eq!(pretty("<a/><b/>").unwrap_err().code, "xml.multiple_roots");
    assert_eq!(
        pretty("hello <a/>").unwrap_err().code,
        "xml.text_outside_root"
    );
    assert_eq!(pretty("<!-- only -->").unwrap_err().code, "xml.no_root");
    assert_eq!(pretty("  ").unwrap_err().code, "xml.empty");
}

#[test]
fn handles_unicode_positions() {
    let err = pretty("<根>\n  <元素></错>\n</根>").unwrap_err();
    assert_eq!(err.params["line"], 2);
    let out = pretty("<根><名称>中文</名称></根>").unwrap();
    assert!(out.output.contains("<名称>中文</名称>"));
}
