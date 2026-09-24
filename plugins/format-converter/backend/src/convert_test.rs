use super::*;

fn convert(input: &str, from: Format, to: Format) -> PluginResult<Output> {
    run(Args {
        input: input.into(),
        from,
        to,
        options: Options::default(),
    })
}

fn out(input: &str, from: Format, to: Format) -> String {
    convert(input, from, to).unwrap().output
}

#[test]
fn json_to_yaml_and_back_keeps_key_order() {
    let yaml = out(
        r#"{"name":"ToolForge","tags":["a","b"],"meta":{"z":1,"a":2}}"#,
        Format::Json,
        Format::Yaml,
    );
    assert_eq!(
        yaml,
        "name: ToolForge\ntags:\n- a\n- b\nmeta:\n  z: 1\n  a: 2\n"
    );
    let json = out(&yaml, Format::Yaml, Format::Json);
    assert_eq!(
        json,
        "{\n  \"name\": \"ToolForge\",\n  \"tags\": [\n    \"a\",\n    \"b\"\n  ],\n  \"meta\": {\n    \"z\": 1,\n    \"a\": 2\n  }\n}"
    );
}

#[test]
fn toml_roundtrip_with_tables_and_dates() {
    let toml_text = "title = \"demo\"\nreleased = 2024-02-29T08:30:00Z\n\n[server]\nport = 8080\nhosts = [\"a\", \"b\"]\n";
    let json = out(toml_text, Format::Toml, Format::Json);
    assert!(json.contains("\"released\": \"2024-02-29T08:30:00Z\""));
    let back = out(&json, Format::Json, Format::Toml);
    assert!(back.contains("[server]\nport = 8080"));
}

#[test]
fn toml_cannot_hold_null_or_non_table_root() {
    let err = convert(r#"{"a":{"b":null}}"#, Format::Json, Format::Toml).unwrap_err();
    assert_eq!(err.code, "format.toml_null");
    assert_eq!(err.params["path"], "$.a.b");
    assert_eq!(
        convert("[1,2]", Format::Json, Format::Toml)
            .unwrap_err()
            .code,
        "format.toml_root"
    );
}

#[test]
fn csv_to_json_infers_types() {
    let json = out(
        "id,name,active,zip,score\n1,Alice,true,01234,9.5\n2,Bob,false,,7",
        Format::Csv,
        Format::Json,
    );
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value[0]["id"], 1);
    assert_eq!(value[0]["active"], true);
    assert_eq!(value[0]["zip"], "01234");
    assert_eq!(value[0]["score"], 9.5);
    assert_eq!(value[1]["zip"], Value::Null);
}

#[test]
fn json_to_csv_flattens_and_unions_headers() {
    let csv_text = out(
        r#"[{"id":1,"user":{"name":"A"},"tags":["x"]},{"id":2,"extra":"y, z"}]"#,
        Format::Json,
        Format::Csv,
    );
    assert_eq!(
        csv_text,
        "id,user.name,tags,extra\n1,A,\"[\"\"x\"\"]\",\n2,,,\"y, z\"\n"
    );
}

#[test]
fn csv_options_delimiter_and_no_header() {
    let options = Options {
        delimiter: Delimiter::Semicolon,
        header: false,
        infer_types: false,
        ..Options::default()
    };
    let output = run(Args {
        input: "a;1\nb;2".into(),
        from: Format::Csv,
        to: Format::Json,
        options,
    })
    .unwrap();
    let value: Value = serde_json::from_str(&output.output).unwrap();
    assert_eq!(value, serde_json::json!([["a", "1"], ["b", "2"]]));
    assert_eq!(output.records, Some(2));
}

#[test]
fn multi_document_yaml_becomes_array() {
    let json = out("a: 1\n---\na: 2\n", Format::Yaml, Format::Json);
    assert_eq!(
        serde_json::from_str::<Value>(&json).unwrap(),
        serde_json::json!([{"a": 1}, {"a": 2}])
    );
}

#[test]
fn detects_formats() {
    assert_eq!(detect("  {\"a\": 1}"), Format::Json);
    assert_eq!(detect("[server]\nport = 1"), Format::Toml);
    assert_eq!(detect("name = \"x\""), Format::Toml);
    assert_eq!(detect("id,name\n1,a"), Format::Csv);
    assert_eq!(detect("name: x\nlist:\n  - a"), Format::Yaml);
    assert_eq!(detect("- a\n- b"), Format::Yaml);
    let output = convert("a: 1", Format::Auto, Format::Json).unwrap();
    assert_eq!(output.from, Format::Yaml);
}

#[test]
fn errors_report_positions() {
    let json = convert("{\n  \"a\": ,\n}", Format::Json, Format::Yaml).unwrap_err();
    assert_eq!(
        (json.code.as_str(), &json.params["line"]),
        ("format.json_invalid", &serde_json::json!(2))
    );
    let yaml = convert("a: [1, 2\nb: 3", Format::Yaml, Format::Json).unwrap_err();
    assert_eq!(yaml.code, "format.yaml_invalid");
    assert!(yaml.params.contains_key("line"));
    let toml_err = convert("a = 1\nb = \n", Format::Toml, Format::Json).unwrap_err();
    assert_eq!(toml_err.code, "format.toml_invalid");
    assert_eq!(toml_err.params["line"], 2);
    assert_eq!(
        convert("   ", Format::Json, Format::Yaml).unwrap_err().code,
        "format.empty"
    );
}

#[test]
fn minified_json_output() {
    let output = run(Args {
        input: "a: 1\nb: [x]".into(),
        from: Format::Yaml,
        to: Format::Json,
        options: Options {
            minify: true,
            ..Options::default()
        },
    })
    .unwrap();
    assert_eq!(output.output, r#"{"a":1,"b":["x"]}"#);
}
