use super::*;
use serde_json::json;
use tf_plugin_api::{PluginResult, unknown_function};

struct Echo(Manifest);

impl ToolPlugin for Echo {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "echo" | "hidden" | "long" => Ok(args),
            other => Err(unknown_function(other)),
        }
    }
}

fn registry() -> PluginRegistry {
    let manifest = Manifest::from_static(
        r#"{"id":"echo","version":"1.0.0","name":"i18n:name","description":"i18n:d",
            "category":"dev","functions":{"echo":{},"long":{"task":true}}}"#,
    );
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(Echo(manifest)));
    registry
}

#[test]
fn calls_declared_function() {
    let out = registry().call("echo", "echo", json!({"a": 1})).unwrap();
    assert_eq!(out, json!({"a": 1}));
}

#[test]
fn rejects_undeclared_function() {
    let err = registry().call("echo", "hidden", json!(null)).unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}

#[test]
fn rejects_unknown_plugin() {
    let err = registry().call("missing", "echo", json!(null)).unwrap_err();
    assert_eq!(err.code, "plugin.not_found");
}

#[test]
fn task_functions_cannot_be_called_directly() {
    let err = registry().call("echo", "long", json!(null)).unwrap_err();
    assert_eq!(err.code, "plugin.requires_task");
}

#[test]
fn only_task_functions_can_run_as_tasks() {
    assert!(registry().ensure_task("echo", "long").is_ok());
    assert_eq!(
        registry().ensure_task("echo", "echo").unwrap_err().code,
        "plugin.not_a_task"
    );
}

struct Picky(Manifest, &'static str, u8);

impl ToolPlugin for Picky {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn call(&self, function: &str, _: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        text.contains(self.1)
            .then(|| tf_plugin_api::Detection::new(self.2, "hit").with("len", text.len()))
    }
}

fn picky(id: &str, needle: &'static str, score: u8) -> Arc<dyn ToolPlugin> {
    let manifest = Manifest::from_static(&format!(
        r#"{{"id":"{id}","version":"1.0.0","name":"n","description":"d","category":"dev","functions":{{}}}}"#
    ));
    Arc::new(Picky(manifest, needle, score))
}

#[test]
fn ranks_detections_and_drops_weak_ones() {
    let mut registry = registry();
    registry.register(picky("low", "a", 20));
    registry.register(picky("mid", "a", 60));
    registry.register(picky("high", "ab", 90));
    registry.register(picky("tie", "a", 60));
    let ids: Vec<String> = registry
        .detect("  ab  ")
        .into_iter()
        .map(|s| s.plugin_id)
        .collect();
    assert_eq!(ids, vec!["high", "mid", "tie"]);
    let first = &registry.detect("ab")[0];
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        json!({"pluginId": "high", "score": 90, "label": "hit", "params": {"len": 2}})
    );
    assert!(registry.detect("   ").is_empty());
    assert!(
        registry
            .detect(&"a".repeat(tf_plugin_api::DETECT_LIMIT + 1))
            .is_empty()
    );
}

#[test]
fn previews_text_on_one_line() {
    assert_eq!(preview("  a\n\tb   c ", 10), "a b c");
    assert_eq!(preview("abcdef", 3), "abc…");
    assert_eq!(preview("abc", 3), "abc");
    assert_eq!(preview("ab cd", 4), "ab c…");
}
