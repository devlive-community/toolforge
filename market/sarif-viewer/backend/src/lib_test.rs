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
pub fn parsed(text: &str) -> Report {
    let mut report = Report::default();
    parser::parse(
        &mut report,
        "fixture.sarif".into(),
        text,
        &Context::default(),
    )
    .unwrap();
    report
}
pub fn fixture(name: &str) -> String {
    format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}
pub fn first_error(text: &str) -> tf_plugin_api::PluginError {
    let mut report = Report::default();
    parser::parse(
        &mut report,
        "fixture.sarif".into(),
        text,
        &Context::default(),
    )
    .err()
    .unwrap()
}

#[test]
fn multiple_files_keep_runs_and_files_distinct() {
    let plugin = SarifViewer::default();
    let ctx = Context::default();
    let result = plugin
        .run_task(
            "parse",
            json!({"paths":[fixture("eslint.sarif"),fixture("eslint.sarif")]}),
            &ctx,
        )
        .unwrap();
    assert_eq!(result["fileCount"], 2);
    assert_eq!(result["runCount"], 2);
    let report = plugin.report(result["handle"].as_str().unwrap()).unwrap();
    assert_eq!(report.runs[0].file, 0);
    assert_eq!(report.runs[1].file, 1);
    assert_eq!(report.runs[1].id, 1);
}

#[test]
fn failed_or_cancelled_parse_does_not_replace_last_good_report() {
    let plugin = SarifViewer::default();
    let ctx = Context::default();
    let result = plugin
        .run_task("parse", json!({"paths":[fixture("eslint.sarif")]}), &ctx)
        .unwrap();
    let handle = result["handle"].as_str().unwrap();
    assert_eq!(
        plugin
            .run_task("parse", json!({"paths":[fixture("broken.sarif")]}), &ctx)
            .err()
            .unwrap()
            .code,
        "sarif.json"
    );
    assert_eq!(
        plugin
            .run_task(
                "parse",
                json!({"paths":[fixture("eslint.sarif")]}),
                &Context {
                    cancel_after: Some(5),
                    ..Context::default()
                }
            )
            .err()
            .unwrap()
            .code,
        "task.cancelled"
    );
    assert_eq!(plugin.report(handle).unwrap().counts.results, 3);
    let next = plugin
        .run_task("parse", json!({"paths":[fixture("eslint.sarif")]}), &ctx)
        .unwrap();
    assert_ne!(next["handle"], result["handle"]);
    assert_eq!(plugin.report(handle).err().unwrap().code, "sarif.expired");
}

#[test]
fn detail_returns_rule_locations_and_flows_without_raw_indexes() {
    let plugin = SarifViewer::default();
    let ctx = Context::default();
    let result = plugin
        .run_task("parse", json!({"paths":[fixture("eslint.sarif")]}), &ctx)
        .unwrap();
    let handle = result["handle"].as_str().unwrap();
    let detail = plugin
        .run_task("detail", json!({"handle":handle,"id":2}), &ctx)
        .unwrap();
    assert_eq!(detail["row"]["id"], 2);
    assert_eq!(detail["row"]["ruleId"], "no-flow");
    assert!(detail["row"].get("locations").is_none());
    assert_eq!(detail["rule"]["ruleId"], "no-flow");
    assert_eq!(detail["locations"].as_array().unwrap().len(), 2);
    assert_eq!(detail["flows"].as_array().unwrap().len(), 1);
    assert_eq!(detail["flows"][0]["steps"].as_array().unwrap().len(), 2);
    assert_eq!(
        plugin
            .run_task("detail", json!({"handle":handle,"id":99}), &ctx)
            .err()
            .unwrap()
            .code,
        "sarif.page"
    );
}

#[test]
fn input_limits_and_unknown_functions_fail_explicitly() {
    let plugin = SarifViewer::default();
    let ctx = Context::default();
    let many: Vec<String> = std::iter::repeat_n(fixture("eslint.sarif"), 33).collect();
    for paths in [json!([]), json!(many)] {
        assert_eq!(
            plugin
                .run_task("parse", json!({"paths":paths}), &ctx)
                .err()
                .unwrap()
                .code,
            "sarif.files"
        );
    }
    assert!(plugin.call("parse", json!({})).is_err());
    assert_eq!(
        plugin
            .run_task("missing", json!({}), &ctx)
            .err()
            .unwrap()
            .code,
        "plugin.function_not_found"
    );
    assert!(plugin.manifest().sensitive);
}
