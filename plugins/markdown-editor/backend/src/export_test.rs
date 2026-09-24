use super::*;

#[test]
fn builds_standalone_document() {
    let doc = export_html(ExportArgs {
        source: "# Hello <World>\n\ntext".into(),
        title: None,
    })
    .unwrap()
    .html;
    assert!(doc.starts_with("<!DOCTYPE html>"));
    assert!(doc.contains("<title>Hello &lt;World&gt;</title>"), "{doc}");
    assert!(doc.contains("<style>\n:root"));
    assert!(doc.contains("<p>text</p>"));

    let named = export_html(ExportArgs {
        source: "no heading".into(),
        title: Some("Notes \"1\"".into()),
    })
    .unwrap()
    .html;
    assert!(named.contains("<title>Notes &quot;1&quot;</title>"));
    let untitled = export_html(ExportArgs {
        source: "x".into(),
        title: Some("  ".into()),
    })
    .unwrap()
    .html;
    assert!(untitled.contains("<title></title>"));
}
