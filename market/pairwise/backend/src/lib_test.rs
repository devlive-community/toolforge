use super::*;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tf_plugin_api::{LogLevel, PluginError};

#[derive(Default)]
pub struct Context {
    pub cancelled: AtomicBool,
    pub delay: bool,
    pub cancel_after: Option<usize>,
    pub checks: AtomicUsize,
}

impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, _: &str) {}
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        if self.delay {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        self.cancelled.load(Ordering::Relaxed)
            || self
                .cancel_after
                .is_some_and(|limit| self.checks.fetch_add(1, Ordering::Relaxed) >= limit)
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| PluginError::new("test.file"))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|_| PluginError::new("test.file"))
    }
}

fn input() -> Value {
    json!({"text":r#"{"factors":[{"name":"OS","values":["Linux","Windows"]},{"name":"Role","values":["guest","admin"]}]}"#, "seed":"review"})
}

#[test]
fn generates_pages_and_traces_pairs_to_their_actual_cases() {
    let plugin = Pairwise::default();
    let summary = plugin
        .run_task("generate", input(), &Context::default())
        .unwrap();
    assert_eq!(summary["status"], "complete");
    let handle = &summary["handle"];
    let a = plugin
        .call("cases", json!({"handle":handle,"limit":2}))
        .unwrap();
    let b = plugin
        .call("cases", json!({"handle":handle,"offset":2,"limit":2}))
        .unwrap();
    assert_eq!(a["total"], 4);
    assert_eq!(a["items"][0]["id"], 1);
    assert_eq!(b["items"][0]["id"], 3);
    let pairs = plugin
        .call("pairs", json!({"handle":handle,"state":"covered"}))
        .unwrap();
    assert_eq!(pairs["total"], 4);
    let first = &pairs["items"][0];
    let witnesses = plugin
        .call("cases", json!({"handle":handle,"pairId":first["id"]}))
        .unwrap();
    assert_eq!(witnesses["total"], first["caseCount"]);
    assert_eq!(witnesses["items"][0]["cells"][0], first["leftValue"]);
    assert_eq!(witnesses["items"][0]["cells"][1], first["rightValue"]);
    for bad in [
        json!({"handle":handle,"limit":101}),
        json!({"handle":handle,"pairId":9999}),
        json!({"handle":handle,"state":"fake"}),
    ] {
        let function = if bad.get("state").is_some() {
            "pairs"
        } else {
            "cases"
        };
        assert!(plugin.call(function, bad).is_err());
    }
}

#[test]
fn export_import_recomputes_coverage_and_preserves_cases() {
    let plugin = Pairwise::default();
    let ctx = Context::default();
    let summary = plugin.run_task("generate", input(), &ctx).unwrap();
    let directory =
        std::env::temp_dir().join(format!("tf-pairwise-{}-roundtrip", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("matrix.json");
    plugin
        .run_task(
            "export",
            json!({"handle":summary["handle"],"format":"json","path":path}),
            &ctx,
        )
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let imported = plugin
        .run_task("generate", json!({"path":path}), &ctx)
        .unwrap();
    assert_eq!(imported["status"], "complete");
    let before = plugin
        .call("cases", json!({"handle":summary["handle"]}))
        .unwrap();
    let after = plugin
        .call("cases", json!({"handle":imported["handle"]}))
        .unwrap();
    assert_eq!(before, after);
    let mut tampered: Value = serde_json::from_str(&text).unwrap();
    tampered["cases"] = json!([]);
    tampered["evidence"]["coveragePercent"] = json!(100);
    let partial = plugin
        .run_task("generate", json!({"text":tampered.to_string()}), &ctx)
        .unwrap();
    assert_eq!(partial["status"], "incomplete");
    assert_eq!(partial["uncovered"], 4);
    assert_eq!(partial["caseCount"], 0);
    let csv_path = directory.join("matrix.csv");
    assert!(
        plugin
            .run_task(
                "export",
                json!({"handle":partial["handle"],"format":"csv","path":csv_path}),
                &ctx
            )
            .is_err()
    );
    assert!(!csv_path.exists());
    plugin
        .run_task(
            "export",
            json!({"handle":partial["handle"],"format":"json","path":path,"allowPartial":true}),
            &ctx,
        )
        .unwrap();
    let partial: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(partial["evidence"]["status"], "incomplete");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cached_handles_do_not_export_a_different_or_expired_report() {
    let plugin = Pairwise::default();
    let ctx = Context::default();
    let first = plugin.run_task("generate", input(), &ctx).unwrap();
    for _ in 0..3 {
        plugin.run_task("generate", input(), &ctx).unwrap();
    }
    assert_eq!(
        plugin
            .call("cases", json!({"handle":first["handle"]}))
            .err()
            .unwrap()
            .code,
        "pairwise.expired"
    );
    assert!(plugin.call("missing", json!({})).is_err());
}

#[test]
fn bundle_versions_invalid_cases_and_json_are_rejected() {
    let plugin = Pairwise::default();
    let ctx = Context::default();
    let mut bundle = json!({"formatVersion":1,"algorithm":ALGORITHM,"seed":"1","model": {"factors":[{"name":"A","values":[0]},{"name":"B","values":[0]}]},"limits":{},"cases":[{"A":0,"B":1}],"evidence":{}});
    assert_eq!(
        plugin
            .run_task("generate", json!({"text":bundle.to_string()}), &ctx)
            .err()
            .unwrap()
            .code,
        "pairwise.invalid_case"
    );
    bundle["algorithm"] = json!("future-v2");
    assert_eq!(
        plugin
            .run_task("generate", json!({"text":bundle.to_string()}), &ctx)
            .err()
            .unwrap()
            .code,
        "pairwise.version"
    );
    assert!(
        plugin
            .run_task("generate", json!({"text":"{"}), &ctx)
            .is_err()
    );
    assert!(
        plugin
            .run_task(
                "generate",
                json!({"text":"{}","draft":{"factors":[],"forbiddenText":"[]"}}),
                &ctx
            )
            .is_err()
    );
}

#[test]
fn typed_csv_and_model_preparation_preserve_large_numbers_and_quotes() {
    let plugin = Pairwise::default();
    let input = json!({"text":r#"{"factors":[{"name":"A","values":[9007199254740993,"=1+1","a,\"b\n"]},{"name":"B","values":[true]}]}"#});
    let prepared = plugin.call("prepare", input.clone()).unwrap();
    assert!(
        prepared["modelText"]
            .as_str()
            .unwrap()
            .contains("9007199254740993")
    );
    let result = plugin
        .run_task("generate", input, &Context::default())
        .unwrap();
    let report = plugin.report(result["handle"].as_str().unwrap()).unwrap();
    let csv = csv(&report);
    assert!(csv.starts_with("\"A\",\"B\"\r\n"));
    assert!(csv.contains("9007199254740993"));
    assert!(csv.contains("\"\"=1+1\"\""));
    assert!(csv.contains("\\n"));
}
