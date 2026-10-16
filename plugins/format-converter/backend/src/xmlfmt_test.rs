use serde_json::json;

use super::*;

fn parse_str(input: &str) -> Value {
    parse(input, true).unwrap()
}

#[test]
fn maps_attributes_text_and_repeated_children() {
    let value = parse_str(
        r#"<?xml version="1.0"?>
<!-- 注释 -->
<catalog version="2">
  <book id="b1" lang="zh">
    <title>三体</title>
    <price>23.5</price>
  </book>
  <book id="b2"><title>Rust &amp; WebAssembly</title><zip>01234</zip></book>
  <note type="info">hello <![CDATA[<world>]]></note>
  <empty/>
</catalog>"#,
    );
    assert_eq!(
        value,
        json!({"catalog": {
            "@version": 2,
            "book": [
                {"@id": "b1", "@lang": "zh", "title": "三体", "price": 23.5},
                {"@id": "b2", "title": "Rust & WebAssembly", "zip": "01234"}
            ],
            "note": {"@type": "info", "#text": "hello <world>"},
            "empty": null
        }})
    );
}

#[test]
fn keeps_strings_without_type_inference_and_resolves_char_refs() {
    let value = parse("<a n=\"1\">&#x4E2D;&#25991;</a>", false).unwrap();
    assert_eq!(value, json!({"a": {"@n": "1", "#text": "中文"}}));
}

#[test]
fn reports_syntax_errors() {
    for input in ["<a><b></a>", "<a>", "<a/><b/>", "<a>&nbsp;</a>"] {
        let err = parse(input, true).unwrap_err();
        assert_eq!(err.code, "format.xml_invalid", "{input}");
    }
}

#[test]
fn writes_and_reads_back_losslessly() {
    let value = json!({"catalog": {
        "@version": 2,
        "book": [
            {"@id": "b1", "title": "A <b> & \"c\"", "tags": ["x", "y"]},
            {"@id": "b2", "title": "B", "empty": null}
        ],
        "note": {"@type": "info", "#text": "hello"}
    }});
    let mut warnings = Vec::new();
    let xml = write(&value, 2, &mut warnings).unwrap();
    assert!(xml.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<catalog version=\"2\">\n  <book id=\"b1\">"
    ));
    assert!(xml.contains("<title>A &lt;b&gt; &amp; \"c\"</title>"));
    assert!(xml.contains("<empty/>"));
    assert!(warnings.is_empty());
    assert_eq!(parse_str(&xml), value);
}

#[test]
fn wraps_non_single_roots_and_warns() {
    let mut warnings = Vec::new();
    let xml = write(&json!([{"a": 1}, {"a": 2}]), 0, &mut warnings).unwrap();
    assert!(xml.contains("<root><item><a>1</a></item><item><a>2</a></item></root>"));
    assert_eq!(warnings[0].code, "xml_wrapped_root");

    let mut warnings = Vec::new();
    write(&json!({"r": {"list": [1]}}), 2, &mut warnings).unwrap();
    assert_eq!(warnings, vec![Warning::new("xml_single_array", "$.r.list")]);
}

#[test]
fn rejects_invalid_names() {
    let err = write(&json!({"r": {"1bad": 1}}), 2, &mut Vec::new()).unwrap_err();
    assert_eq!(err.code, "format.xml_name");
    assert_eq!(err.params["path"], "$.r.1bad");
    let err = write(&json!({"r": {"@a": {"x": 1}}}), 2, &mut Vec::new()).unwrap_err();
    assert_eq!(err.code, "format.xml_attribute");
}
