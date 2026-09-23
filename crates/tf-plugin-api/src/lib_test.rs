use super::*;

#[test]
fn error_serializes_code_and_params() {
    let err = PluginError::new("json.syntax").with("line", 3);
    let value = serde_json::to_value(&err).unwrap();
    assert_eq!(value["code"], "json.syntax");
    assert_eq!(value["params"]["line"], 3);
}

#[test]
fn empty_params_are_omitted() {
    let value = serde_json::to_value(PluginError::new("x")).unwrap();
    assert!(value.get("params").is_none());
}

#[test]
fn manifest_defaults_optional_fields() {
    let manifest = Manifest::from_static(
        r#"{"id":"a","version":"1.0.0","name":"i18n:name","description":"i18n:d","category":"dev"}"#,
    );
    assert!(manifest.functions.is_empty());
    assert!(!manifest.sensitive);
}
