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
    assert!(manifest.resources.is_empty());
}

#[test]
fn manifest_parses_resources() {
    let manifest = Manifest::from_static(
        r#"{"id":"a","version":"1.0.0","name":"n","description":"d","category":"image",
            "resources":[{"id":"u2netp","urls":["https://x/m.onnx"],"sha256":"ab","size":3}]}"#,
    );
    assert_eq!(manifest.resources[0].id, "u2netp");
    assert_eq!(manifest.resources[0].size, 3);
    assert!(manifest.resources[0].license.is_none());
}

#[test]
fn resource_ids_are_path_safe() {
    assert!(ResourceSpec::valid_id("isnet-general-use"));
    assert!(ResourceSpec::valid_id("model_v1.2"));
    assert!(!ResourceSpec::valid_id(""));
    assert!(!ResourceSpec::valid_id("../x"));
    assert!(!ResourceSpec::valid_id(".hidden"));
    assert!(!ResourceSpec::valid_id("Upper"));
    assert!(!ResourceSpec::valid_id("a/b"));
}

struct Noop;

impl TaskContext for Noop {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Ok(0)
    }
}

struct Upper(Manifest);

impl ToolPlugin for Upper {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn call(&self, _: &str, args: Value) -> PluginResult<Value> {
        Ok(Value::String(
            args.as_str().unwrap_or_default().to_uppercase(),
        ))
    }
}

#[test]
fn run_task_defaults_to_call() {
    let manifest = Manifest::from_static(
        r#"{"id":"a","version":"1.0.0","name":"n","description":"d","category":"dev"}"#,
    );
    let out = Upper(manifest)
        .run_task("x", Value::from("ab"), &Noop)
        .unwrap();
    assert_eq!(out, "AB");
}

#[test]
fn log_level_serializes_lowercase() {
    assert_eq!(serde_json::to_value(LogLevel::Warn).unwrap(), "warn");
}

#[test]
fn resources_are_missing_by_default() {
    let err = Noop.resource_path("u2netp").unwrap_err();
    assert_eq!(err.code, "resource.missing");
    assert_eq!(err.params["id"], "u2netp");
}
