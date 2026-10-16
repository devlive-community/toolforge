use serde_json::json;

use super::*;

#[test]
fn parses_sections_nested_sections_and_arrays() {
    let value = parse(
        r#"; 全局
name = ToolForge
debug = true

[server]
host = 127.0.0.1 ; 行尾注释
port: 8080
path = "/a;b"

[server.tls]
enabled = false
ciphers[] = AES
ciphers[] = CHACHA
code = '007'
"#,
        true,
    )
    .unwrap();
    assert_eq!(
        value,
        json!({
            "name": "ToolForge",
            "debug": true,
            "server": {
                "host": "127.0.0.1",
                "port": 8080,
                "path": "/a;b",
                "tls": {"enabled": false, "ciphers": ["AES", "CHACHA"], "code": "007"}
            }
        })
    );
}

#[test]
fn reports_invalid_lines() {
    let err = parse("[a]\njust text\n", true).unwrap_err();
    assert_eq!(err.code, "format.ini_invalid");
    assert_eq!(err.params["line"], 2);
    let err = parse("a = 1\n[a]\n", true).unwrap_err();
    assert_eq!(err.params["line"], 2);
}

#[test]
fn writes_and_reads_back() {
    let value = json!({
        "name": "ToolForge",
        "version": "1.0",
        "empty": "",
        "nothing": null,
        "server": {"port": 8080, "hosts": ["a", "b"], "tls": {"enabled": true}},
        "deep": {"only": {"leaf": 1}}
    });
    let mut warnings = Vec::new();
    let ini = write(&value, &mut warnings).unwrap();
    assert_eq!(
        ini,
        "name = ToolForge\nversion = \"1.0\"\nempty = \"\"\nnothing = \n\n[server]\nport = 8080\nhosts[] = a\nhosts[] = b\n\n[server.tls]\nenabled = true\n\n[deep.only]\nleaf = 1\n"
    );
    assert!(warnings.is_empty());
    assert_eq!(parse(&ini, true).unwrap(), value);
}

#[test]
fn warns_about_lossy_shapes() {
    let mut warnings = Vec::new();
    let ini = write(
        &json!({"items": [{"a": 1}, {"a": 2}], "e": []}),
        &mut warnings,
    )
    .unwrap();
    assert!(ini.contains("[items.0]\na = 1"));
    assert!(warnings.contains(&Warning::new("ini_array_objects", "$.items")));
    assert!(warnings.contains(&Warning::new("empty_container", "$.e")));
    assert_eq!(
        write(&json!([1]), &mut warnings).unwrap_err().code,
        "format.ini_root"
    );
}
