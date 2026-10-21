use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tf_plugin_api::{LogLevel, PluginError};
#[derive(Default)]
struct Context {
    cancelled: AtomicBool,
    cancel_on: Option<&'static str>,
}
impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, s: &str) {
        if self.cancel_on == Some(s) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, p: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(p)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| PluginError::new("test.file"))
    }
    fn file_size(&self, p: &str) -> PluginResult<u64> {
        std::fs::metadata(p)
            .map(|m| m.len())
            .map_err(|_| PluginError::new("test.file"))
    }
}
fn fixture(name: &str) -> String {
    format!("{}/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"))
}
fn snapshot(text: &str) -> Snapshot {
    model::parse("fixture".into(), text, &Context::default()).unwrap()
}
fn reports() -> Reports {
    Reports {
        before: snapshot(include_str!("../fixtures/before.json")),
        after: Some(snapshot(include_str!("../fixtures/after.json"))),
    }
}
fn query(kind: &str) -> analysis::Query {
    serde_json::from_value(json!({"handle":"1","kind":kind})).unwrap()
}
fn small() -> Value {
    json!({"inputs":{"src/a.js":{"bytes":100,"imports":[]}},"outputs":{"out.js":{"bytes":20,"imports":[],"exports":[],"entryPoint":"src/a.js","inputs":{"src/a.js":{"bytesInOutput":12}}}}})
}

#[test]
fn real_metafile_agrees_with_official_analyze_byte_counts() {
    let r = reports();
    assert_eq!(r.before.totals.bytes, 460);
    assert_eq!(r.before.totals.attributed, 248);
    assert_eq!(r.before.totals.unattributed, 212);
    let out = &r.before.meta.outputs["dist/a-LX4P3RKJ.js"];
    assert_eq!(out.bytes, 194);
    assert_eq!(out.inputs["src/a.js"].bytes_in_output, 134);
    assert_ne!(r.before.meta.inputs["src/a.js"].bytes, 134);
    let official = include_str!("../fixtures/before.analyze.txt");
    assert!(official.contains("194b"));
    assert!(official.contains("134b"));
    assert!(official.contains("69.1%"));
    let a = r.after.unwrap();
    assert_eq!(a.totals.bytes, 476);
    assert_eq!(a.totals.attributed, 264);
    assert_eq!(a.totals.unattributed, 212);
}
#[test]
fn shared_chunks_and_external_edges_have_verified_import_traces() {
    let r = reports();
    let ctx = Context::default();
    let trace =
        analysis::trace(&r.before, "dist/chunk-VSRC2VDR.js", "src/shared.js", &ctx).unwrap();
    assert_eq!(trace["state"], "found");
    for edge in trace["edges"].as_array().unwrap() {
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        assert!(
            r.before.meta.inputs[from]
                .imports
                .iter()
                .any(|e| !e.external && e.path == to)
        );
    }
    let mut q = query("imports");
    q.output = Some("dist/a-LX4P3RKJ.js".into());
    let edges = analysis::rows(&r, &q, &ctx).unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["path"] == "remote-pkg" && e["external"] == true && e["resolved"] == false)
    );
}
#[test]
fn one_module_in_two_outputs_is_not_deduplicated_or_replaced_by_source_size() {
    let s = snapshot(include_str!("../fixtures/unsplit.json"));
    let count = s
        .meta
        .outputs
        .values()
        .filter(|o| o.inputs.contains_key("src/shared.js"))
        .count();
    assert_eq!(count, 2);
    let contributions: u64 = s
        .meta
        .outputs
        .values()
        .filter_map(|o| o.inputs.get("src/shared.js"))
        .map(|c| c.bytes_in_output)
        .sum();
    assert!(contributions > 0);
    assert_ne!(contributions, s.meta.inputs["src/shared.js"].bytes);
}
#[test]
fn hash_names_are_added_removed_until_explicitly_paired() {
    let r = reports();
    let ctx = Context::default();
    let mut q = query("compare");
    let auto = analysis::comparison(&r, &q, &ctx).unwrap();
    assert_eq!(auto.len(), 8);
    assert_eq!(auto.iter().filter(|v| v["mode"] == "exact").count(), 2);
    q.pairs = vec![analysis::Pair {
        before: "dist/chunk-VSRC2VDR.js".into(),
        after: "dist/chunk-GDODLRL4.js".into(),
    }];
    let paired = analysis::comparison(&r, &q, &ctx).unwrap();
    assert_eq!(paired.len(), 7);
    assert!(
        paired
            .iter()
            .any(|v| v["mode"] == "manual" && v["delta"] == 16)
    );
    q.kind = "diffModules".into();
    q.output = Some(q.pairs[0].before.clone());
    q.other = Some(q.pairs[0].after.clone());
    let modules = analysis::diff_modules(&r, &q, &ctx).unwrap();
    assert_eq!(modules[0]["path"], "src/shared.js");
    assert_eq!(modules[0]["beforeBytes"], 22);
    assert_eq!(modules[0]["afterBytes"], 38);
    assert_eq!(modules[0]["delta"], 16);
    q.pairs.push(q.pairs[0].clone());
    assert!(analysis::comparison(&r, &q, &ctx).is_err());
}
#[test]
fn over_attribution_and_unknown_references_stay_explicit() {
    let mut value = small();
    value["outputs"]["out.js"]["inputs"]["src/a.js"]["bytesInOutput"] = json!(30);
    value["outputs"]["out.js"]["inputs"]["unknown.js"] = json!({"bytesInOutput":2});
    let s = snapshot(&value.to_string());
    assert_eq!(s.totals.over_attributed, 12);
    assert_eq!(s.totals.unattributed, 0);
    assert!(s.diagnostics.iter().any(|d| d.kind == "unknownInput"));
    let mut q = query("tree");
    q.output = Some("out.js".into());
    let chart = treemap::tree(&s, &q, &Context::default()).unwrap();
    assert_eq!(chart["basis"], 32);
    assert_eq!(chart["overAttributed"], 12);
}
#[test]
fn zero_missing_contributions_and_unattributed_output_are_not_tree_shaking_claims() {
    let mut value = small();
    value["outputs"]["out.js"]["inputs"]["src/a.js"]["bytesInOutput"] = json!(0);
    let s = snapshot(&value.to_string());
    let mut q = query("tree");
    q.output = Some("out.js".into());
    let tree = treemap::tree(&s, &q, &Context::default()).unwrap();
    assert_eq!(tree["items"].as_array().unwrap().len(), 1);
    assert_eq!(tree["items"][0]["kind"], "unattributed");
    assert_eq!(tree["items"][0]["bytes"], 20);
    let empty = snapshot("{\"inputs\":{},\"outputs\":{}}");
    assert_eq!(empty.totals.bytes, 0);
}
#[test]
fn missing_negative_fractional_unsafe_and_duplicate_counts_are_rejected() {
    let ctx = Context::default();
    for bytes in [json!(-1), json!(1.5), json!(SAFE + 1)] {
        let mut v = small();
        v["outputs"]["out.js"]["bytes"] = bytes;
        assert!(model::parse("x".into(), &v.to_string(), &ctx).is_err());
    }
    let mut v = small();
    v["outputs"]["out.js"]
        .as_object_mut()
        .unwrap()
        .remove("bytes");
    assert!(model::parse("x".into(), &v.to_string(), &ctx).is_err());
    assert!(model::parse("x".into(),r#"{"inputs":{"x":{"bytes":1,"imports":[]},"x":{"bytes":2,"imports":[]}},"outputs":{}}"#,&ctx).is_err());
    assert!(model::parse("x".into(), "{}", &ctx).is_err());
    let mut v = small();
    v["inputs"]["src/a.js"]["bytes"] = json!(SAFE);
    v["inputs"]["other"] = json!({"bytes":1,"imports":[]});
    assert!(model::parse("x".into(), &v.to_string(), &ctx).is_err());
}
#[test]
fn json_size_depth_and_edge_budgets_fail_explicitly() {
    let ctx = Context::default();
    assert!(model::parse("x".into(), &" ".repeat(FILE_LIMIT + 1), &ctx).is_err());
    let deep = format!("{}{}", "[".repeat(65), "]".repeat(65));
    assert_eq!(
        model::parse("x".into(), &deep, &ctx).err().unwrap().code,
        "bundle.depth"
    );
    let mut v = small();
    v["inputs"]["src/a.js"]["imports"] = json!(vec![
        json!({"path":"x","kind":"import-statement","external":true});
        100001
    ]);
    assert_eq!(
        model::parse("x".into(), &v.to_string(), &ctx)
            .err()
            .unwrap()
            .code,
        "bundle.entries"
    );
}
#[test]
fn charts_are_bounded_area_preserving_and_directory_scopes_match_tables() {
    let mut inputs = serde_json::Map::new();
    let mut contributions = serde_json::Map::new();
    for i in 0..300 {
        let path = format!("f{i:03}.js");
        inputs.insert(path.clone(), json!({"bytes":1,"imports":[]}));
        contributions.insert(path, json!({"bytesInOutput":1}));
    }
    let s=snapshot(&json!({"inputs":inputs,"outputs":{"out":{"bytes":300,"imports":[],"exports":[],"inputs":contributions}}}).to_string());
    let mut q = query("tree");
    q.output = Some("out".into());
    let tree = treemap::tree(&s, &q, &Context::default()).unwrap();
    assert_eq!(tree["items"].as_array().unwrap().len(), 256);
    assert_eq!(tree["limited"], true);
    assert_eq!(tree["basis"], 300);
    let area: f64 = tree["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["width"].as_f64().unwrap() * v["height"].as_f64().unwrap())
        .sum();
    assert!((area - 600000.0).abs() < 0.001);
    let r = reports();
    let mut q = query("modules");
    q.output = Some("dist/a-LX4P3RKJ.js".into());
    q.directory = vec!["missing".into()];
    assert!(
        analysis::rows(&r, &q, &Context::default())
            .unwrap()
            .is_empty()
    );
}
#[test]
fn import_cycles_terminate_and_depth_limits_are_not_reported_as_absence() {
    let mut v = small();
    v["inputs"]["src/a.js"]["imports"] = json!([{"path":"src/a.js","kind":"import-statement"}]);
    let s = snapshot(&v.to_string());
    assert_eq!(
        analysis::trace(&s, "out.js", "src/a.js", &Context::default()).unwrap()["state"],
        "found"
    );
    let mut inputs = serde_json::Map::new();
    for i in 0..70 {
        inputs.insert(format!("m{i}"),json!({"bytes":1,"imports":if i<69{vec![json!({"path":format!("m{}",i+1),"kind":"import-statement"})]}else{vec![]}}));
    }
    let s=snapshot(&json!({"inputs":inputs,"outputs":{"out":{"bytes":1,"imports":[],"exports":[],"entryPoint":"m0","inputs":{"m69":{"bytesInOutput":1}}}}}).to_string());
    assert_eq!(
        analysis::trace(&s, "out", "m69", &Context::default()).unwrap()["state"],
        "limited"
    );
}
#[test]
fn load_query_export_and_cancellation_preserve_cached_identity() {
    let p = BundleAnalyzer::default();
    let ctx = Context::default();
    let summary = p
        .run_task(
            "load",
            json!({"beforePath":fixture("before"),"afterPath":fixture("after")}),
            &ctx,
        )
        .unwrap();
    assert_eq!(summary["totalDelta"], 16);
    let h = &summary["handle"];
    let a = p
        .run_task(
            "query",
            json!({"handle":h,"kind":"outputs","limit":2,"offset":2}),
            &ctx,
        )
        .unwrap();
    assert_eq!(a["total"], 5);
    assert_eq!(a["items"].as_array().unwrap().len(), 2);
    assert!(
        p.run_task(
            "load",
            json!({"beforePath":fixture("after")}),
            &Context {
                cancel_on: Some("parse"),
                ..Context::default()
            }
        )
        .is_err()
    );
    assert!(p.reports(h.as_str().unwrap()).is_ok());
    let directory = std::env::temp_dir().join(format!("tf-bundle-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for format in ["csv", "json", "markdown"] {
        let path = directory.join(format);
        p.run_task("export",json!({"query":{"handle":h,"kind":"modules","output":"dist/a-LX4P3RKJ.js"},"format":format,"path":path}),&ctx).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("src/a.js"));
        assert!(text.contains("134"));
    }
    let path = directory.join("cancelled");
    assert!(
        p.run_task(
            "export",
            json!({"query":{"handle":h,"kind":"outputs"},"format":"json","path":path}),
            &Context {
                cancel_on: Some("export"),
                ..Context::default()
            }
        )
        .is_err()
    );
    assert!(!path.exists());
    std::fs::remove_dir_all(directory).unwrap();
    let next = p
        .run_task("load", json!({"beforePath":fixture("after")}), &ctx)
        .unwrap();
    assert_ne!(&next["handle"], h);
    assert_eq!(
        p.reports(h.as_str().unwrap()).err().unwrap().code,
        "bundle.expired"
    );
}
#[test]
fn exports_escape_formula_and_markup_paths_and_keep_numeric_deltas() {
    let mut v = small();
    let contribution = v["outputs"]["out.js"]["inputs"].as_object_mut().unwrap();
    let item = contribution.remove("src/a.js").unwrap();
    contribution.insert("=SUM(1)<x>|a".into(), item);
    let r = Reports {
        before: snapshot(&v.to_string()),
        after: None,
    };
    let mut q = query("modules");
    q.output = Some("out.js".into());
    let csv = export::render(&r, &q, "csv", &Context::default()).unwrap();
    assert!(csv.contains("'=SUM"));
    let md = export::render(&r, &q, "markdown", &Context::default()).unwrap();
    assert!(!md.contains("<x>"));
    assert!(md.contains("&lt;x&gt;\\|a"));
}

#[test]
fn source_metafiles_cannot_be_selected_as_export_destinations() {
    let plugin = BundleAnalyzer::default();
    let ctx = Context::default();
    let source = fixture("before");
    let original = std::fs::read(&source).unwrap();
    let loaded = plugin
        .run_task("load", json!({"beforePath":source}), &ctx)
        .unwrap();
    let result = plugin.run_task(
        "export",
        json!({"query":{"handle":loaded["handle"],"kind":"outputs"},"format":"json","path":source}),
        &ctx,
    );
    assert_eq!(result.err().unwrap().code, "bundle.source");
    assert_eq!(std::fs::read(source).unwrap(), original);
}
#[test]
fn trace_reports_and_form_pairings_use_bounded_backend_pages() {
    let r = reports();
    let ctx = Context::default();
    let mut q = query("trace");
    q.output = Some("dist/chunk-VSRC2VDR.js".into());
    q.module = Some("src/shared.js".into());
    let text = export::render(&r, &q, "json", &ctx).unwrap();
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["rows"][0]["state"], "found");
    assert_eq!(report["rows"][0]["to"], "src/shared.js");
    let mut q = query("pairings");
    q.pairs = vec![analysis::Pair {
        before: "not-yet-valid".into(),
        after: "edit-me".into(),
    }];
    let rows = analysis::rows(&r, &q, &ctx).unwrap();
    assert_eq!(rows[0]["id"], 0);
    q.limit = 0;
    assert!(analysis::rows(&r, &q, &ctx).is_err());
    q.limit = 101;
    assert!(q.validate().is_err());
}
