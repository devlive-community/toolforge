use super::*;
use serde_json::json;

#[test]
fn dispatches_declared_functions() {
    let tool = XmlFormatter::default();
    for function in ["format", "minify", "validate"] {
        assert!(tool.manifest().functions.contains_key(function));
        let out = tool
            .call(function, json!({"input": "<a><b/></a>"}))
            .unwrap();
        assert_eq!(out["stats"]["elements"], 2);
    }
}

struct TestTask;
impl tf_plugin_api::TaskContext for TestTask {
    fn log(&self, _: tf_plugin_api::LogLevel, _: &str, _: serde_json::Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}
#[test]
fn invalid_or_cancelled_queries_invalidate_the_previous_cached_export() {
    let tool = XmlFormatter::default();
    let first = tool
        .run_task(
            "query",
            json!({"input":"<r><i>one</i></r>","expression":"//i","revision":5}),
            &TestTask,
        )
        .unwrap();
    let handle = first["handle"].as_u64().unwrap();
    assert_eq!(
        tool.call("copy", json!({"handle":handle,"index":0}))
            .unwrap(),
        "one"
    );
    assert!(
        tool.run_task("query", json!({"input":"<r/>","expression":"["}), &TestTask)
            .is_err()
    );
    assert_eq!(
        tool.call("page", json!({"handle":handle}))
            .unwrap_err()
            .code,
        "xpath.expired"
    );
    assert_eq!(
        tool.call("copy", json!({"handle":handle,"index":0}))
            .unwrap_err()
            .code,
        "xpath.expired"
    );
}
#[test]
fn plugin_migration_preserves_id_and_query_is_an_explicit_task() {
    let tool = XmlFormatter::default();
    assert_eq!(tool.manifest().id, "org.devlive.toolforge.xml-formatter");
    assert!(tool.manifest().functions["query"].task);
    assert!(tool.manifest().functions["export"].task);
    assert!(tool.detect("<r><i/></r>").is_some());
}
#[test]
fn complete_export_writes_every_result_without_source_mutation() {
    let tool = XmlFormatter::default();
    let input = format!("<r>{}</r>", "<i>x</i>".repeat(42));
    let first = tool
        .run_task(
            "query",
            json!({"input":input,"expression":"//i"}),
            &TestTask,
        )
        .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 20);
    let handle = first["handle"].as_u64().unwrap();
    let path = std::env::temp_dir().join(format!(
        "toolforge-xpath-{}-{}.json",
        std::process::id(),
        handle
    ));
    tool.run_task("export", json!({"handle":handle,"path":path}), &TestTask)
        .unwrap();
    let output: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(output["items"].as_array().unwrap().len(), 42);
    assert!(output.get("input").is_none());
    std::fs::remove_file(path).unwrap();
}
