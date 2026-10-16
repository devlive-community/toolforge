use super::*;

fn keys(entries: &[Entry]) -> Vec<(&str, &str)> {
    entries
        .iter()
        .map(|e| (e.key.as_str(), e.value.as_str()))
        .collect()
}

#[test]
fn flattens_nested_json_and_arrays() {
    let entries = parse(
        "en.json",
        r#"{"app":{"title":"Hi","items":["a","b"]},"count":3,"none":null}"#,
        Format::Json,
    )
    .unwrap();
    assert_eq!(
        keys(&entries),
        [
            ("app.title", "Hi"),
            ("app.items.0", "a"),
            ("app.items.1", "b"),
            ("count", "3"),
            ("none", ""),
        ]
    );
    // 键本身含 . 时保留原始路径段
    let entries = parse("en.json", r#"{"a.b":{"c":"x"}}"#, Format::Json).unwrap();
    assert_eq!(entries[0].path, ["a.b", "c"]);
}

#[test]
fn skips_arb_metadata() {
    let entries = parse(
        "app_en.arb",
        r#"{"@@locale":"en","hello":"Hello {name}","@hello":{"placeholders":{"name":{}}}}"#,
        Format::Arb,
    )
    .unwrap();
    assert_eq!(keys(&entries), [("hello", "Hello {name}")]);
}

#[test]
fn reads_yaml_mappings() {
    let entries = parse(
        "en.yml",
        "en:\n  greeting: Hello # comment\n  list:\n    - one\n  flag: true\n",
        Format::Yaml,
    )
    .unwrap();
    assert_eq!(
        keys(&entries),
        [
            ("en.greeting", "Hello"),
            ("en.list.0", "one"),
            ("en.flag", "true")
        ]
    );
    assert_eq!(parse("e.yml", "", Format::Yaml).unwrap(), []);
    assert_eq!(
        parse("e.yml", "- a\n", Format::Yaml).unwrap_err().code,
        "i18n.not_object"
    );
}

#[test]
fn parses_java_properties() {
    let text = "# comment\n! also\napp.title = Hello\\\n    World\nkey\\ with\\ space:value\nunicode=\\u4F60\\u597D\nempty\n\n  tab\tvalue\n";
    let entries = parse_properties(text);
    assert_eq!(
        keys(&entries),
        [
            ("app.title", "HelloWorld"),
            ("key with space", "value"),
            ("unicode", "你好"),
            ("empty", ""),
            ("tab", "value"),
        ]
    );
    assert_eq!(entries[0].lines, Some((2, 4)));
}

#[test]
fn detects_formats_and_reports_parse_errors() {
    assert_eq!(Format::from_name("zh-CN.JSON"), Some(Format::Json));
    assert_eq!(Format::from_name("a.yml"), Some(Format::Yaml));
    assert_eq!(
        Format::from_name("messages.properties"),
        Some(Format::Properties)
    );
    assert_eq!(Format::from_name("readme.md"), None);
    let err = parse("bad.json", "{", Format::Json).unwrap_err();
    assert_eq!(err.code, "i18n.parse_failed");
    assert_eq!(
        parse("arr.json", "[]", Format::Json).unwrap_err().code,
        "i18n.not_object"
    );
}
