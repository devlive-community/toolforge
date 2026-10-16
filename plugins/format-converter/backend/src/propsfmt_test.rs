use serde_json::json;

use super::*;

#[test]
fn parses_java_properties_syntax() {
    let mut warnings = Vec::new();
    let value = parse(
        "# comment\n! another\nserver.port=8080\nserver.host : localhost\napp.name  ToolForge\nmessage = Hello \\\n    World\nunicode=\\u4E2D\\u6587\nkey\\ with\\ space=1\nlist[0]=a\nlist[1]=b\nusers[0].name=Ann\nzip=01234\n",
        true,
        &mut warnings,
    )
    .unwrap();
    assert_eq!(
        value,
        json!({
            "server": {"port": 8080, "host": "localhost"},
            "app": {"name": "ToolForge"},
            "message": "Hello World",
            "unicode": "中文",
            "key with space": 1,
            "list": ["a", "b"],
            "users": [{"name": "Ann"}],
            "zip": "01234"
        })
    );
    assert!(warnings.is_empty());
}

#[test]
fn keeps_flat_keys_on_conflict() {
    let mut warnings = Vec::new();
    let value = parse("a=1\na.b=2\n", true, &mut warnings).unwrap();
    assert_eq!(value, json!({"a": 1, "a.b": 2}));
    assert_eq!(warnings, vec![Warning::new("properties_conflict", "a.b")]);
}

#[test]
fn writes_flattened_and_reads_back() {
    let value = json!({
        "server": {"port": 8080, "path": " /a=b"},
        "list": ["x", {"k": true}],
        "text": "line1\nline2",
        "none": null
    });
    let mut warnings = Vec::new();
    let text = write(&value, &mut warnings).unwrap();
    assert_eq!(
        text,
        "server.port=8080\nserver.path=\\ /a=b\nlist[0]=x\nlist[1].k=true\ntext=line1\\nline2\nnone=\n"
    );
    assert_eq!(parse(&text, true, &mut warnings).unwrap(), value);
    assert_eq!(
        write(&json!(1), &mut warnings).unwrap_err().code,
        "format.properties_root"
    );
}

#[test]
fn rejects_bad_unicode_escape() {
    let err = parse("a=\\u12\n", true, &mut Vec::new()).unwrap_err();
    assert_eq!(err.code, "format.properties_invalid");
}
