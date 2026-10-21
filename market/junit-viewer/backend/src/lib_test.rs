use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub struct Context {
    pub checks: AtomicUsize,
    pub cancel_after: Option<usize>,
}
impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, _: &str) {}
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancel_after
            .is_some_and(|n| self.checks.fetch_add(1, Ordering::Relaxed) >= n)
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| error("read"))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|_| error("read"))
    }
}
pub fn parsed(xml: &str) -> Report {
    let mut report = Report::default();
    parser::parse(&mut report, "fixture.xml".into(), xml, &Context::default()).unwrap();
    report
}
fn fixture(name: &str) -> String {
    format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn duplicate_file_imports_keep_sources_and_cases_distinct() {
    let plugin = JunitViewer::default();
    let ctx = Context::default();
    let result = plugin
        .run_task(
            "load",
            json!({"paths":[fixture("pytest.xml"),fixture("pytest.xml")]}),
            &ctx,
        )
        .unwrap();
    assert_eq!(result["sourceCount"], 2);
    assert_eq!(result["counts"]["tests"], 8);
    let report = plugin.report(result["handle"].as_str().unwrap()).unwrap();
    assert_eq!(report.cases[0].source, 0);
    assert_eq!(report.cases[4].source, 1);
    assert_ne!(report.cases[0].id, report.cases[4].id);
}
#[test]
fn failed_or_cancelled_load_does_not_replace_last_good_report() {
    let plugin = JunitViewer::default();
    let ctx = Context::default();
    let result = plugin
        .run_task("load", json!({"paths":[fixture("pytest.xml")]}), &ctx)
        .unwrap();
    let handle = result["handle"].as_str().unwrap();
    assert!(
        plugin
            .run_task("load", json!({"paths":[fixture("missing.xml")]}), &ctx)
            .is_err()
    );
    assert!(
        plugin
            .run_task(
                "load",
                json!({"paths":[fixture("pytest.xml")]}),
                &Context {
                    cancel_after: Some(5),
                    ..Context::default()
                }
            )
            .is_err()
    );
    assert_eq!(plugin.report(handle).unwrap().counts.tests, 4);
    let next = plugin
        .run_task("load", json!({"paths":[fixture("pytest.xml")]}), &ctx)
        .unwrap();
    assert_ne!(next["handle"], result["handle"]);
    assert_eq!(plugin.report(handle).err().unwrap().code, "junit.expired");
}
#[test]
fn logs_are_requested_in_unicode_character_pages() {
    let plugin = JunitViewer::default();
    let text = "甲🧶<&>".repeat(10_000);
    let report = parsed(&format!(
        "<testsuite><testcase name='case'><system-out><![CDATA[{text}]]></system-out></testcase></testsuite>"
    ));
    plugin.cache.lock().unwrap().current = Some(("1".into(), Arc::new(report)));
    let ctx = Context::default();
    let detail = plugin
        .run_task("detail", json!({"handle":"1","kind":"case","id":0}), &ctx)
        .unwrap();
    assert_eq!(detail["items"][0]["characters"], 50_000);
    assert!(detail.to_string().len() < 1000);
    assert!(detail["row"].get("content").is_none());
    let a = plugin
        .run_task("content", json!({"handle":"1","id":0,"limit":7}), &ctx)
        .unwrap();
    let b = plugin
        .run_task(
            "content",
            json!({"handle":"1","id":0,"offset":7,"limit":7}),
            &ctx,
        )
        .unwrap();
    assert_eq!(
        format!(
            "{}{}",
            a["text"].as_str().unwrap(),
            b["text"].as_str().unwrap()
        ),
        text.chars().take(14).collect::<String>()
    );
    assert!(
        plugin
            .run_task("content", json!({"handle":"1","id":0,"limit":8193}), &ctx)
            .is_err()
    );
}
#[test]
fn file_exports_preserve_identity_and_exclude_full_logs() {
    let plugin = JunitViewer::default();
    let ctx = Context::default();
    let result = plugin
        .run_task("load", json!({"paths":[fixture("pytest.xml")]}), &ctx)
        .unwrap();
    let dir = std::env::temp_dir().join(format!("tf-junit-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for format in ["json", "csv", "markdown"] {
        let path = dir.join(format);
        plugin.run_task("export",json!({"handle":result["handle"],"path":path,"format":format,"filter":{"status":"failure"}}),&ctx).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("test_values[2]") || text.contains("test\\_values\\[2\\]"));
        assert!(!text.contains("assert value == 1"));
        if format == "json" {
            let value: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(value["cases"].as_array().unwrap().len(), 1);
            assert_eq!(value["logsIncluded"], false);
        }
    }
    let cancelled = dir.join("cancelled");
    assert!(
        plugin
            .run_task(
                "export",
                json!({"handle":result["handle"],"path":cancelled,"format":"json"}),
                &Context {
                    cancel_after: Some(2),
                    ..Context::default()
                }
            )
            .is_err()
    );
    assert!(!cancelled.exists());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn input_limits_and_unknown_functions_fail_explicitly() {
    let plugin = JunitViewer::default();
    let ctx = Context::default();
    for paths in [vec![], vec![fixture("pytest.xml"); 33]] {
        assert_eq!(
            plugin
                .run_task("load", json!({"paths":paths}), &ctx)
                .err()
                .unwrap()
                .code,
            "junit.files"
        );
    }
    assert!(plugin.call("load", json!({})).is_err());
    assert!(plugin.run_task("missing", json!({}), &ctx).is_err());
    assert!(plugin.manifest().sensitive);
}
