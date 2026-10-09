use super::*;

#[test]
fn validates_single_values_and_json_lines() {
    assert_eq!(validate(r#"{"a": 1}"#).unwrap(), 1);
    assert_eq!(validate("{\"a\":1}\n{\"a\":2}\n[3]").unwrap(), 3);
    let err = validate("{\n  \"a\": 1,\n  \"b\": }").unwrap_err();
    assert_eq!(err.code, "query.invalid_json");
    assert_eq!(err.params["line"], 3);
    assert_eq!(validate("  ").unwrap_err().code, "query.empty_input");
}

#[test]
fn loads_files() {
    let path = std::env::temp_dir().join(format!("tfp-json-query-{}.json", std::process::id()));
    std::fs::write(&path, "[1, 2]").unwrap();
    let text = load(&Source::Path {
        path: path.to_string_lossy().into_owned(),
    })
    .unwrap();
    assert_eq!(text, "[1, 2]");
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        load(&Source::Path {
            path: "/no/such.json".into()
        })
        .unwrap_err()
        .code,
        "fs.not_found"
    );
}
