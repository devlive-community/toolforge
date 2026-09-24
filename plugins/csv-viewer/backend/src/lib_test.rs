use std::io::Read;
use std::sync::Mutex;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    progress: Mutex<Vec<(u64, u64)>>,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, done: u64, total: u64) {
        self.progress.lock().unwrap().push((done, total));
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| PluginError::new("fs.not_found"))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|_| PluginError::new("fs.not_found"))
    }
}

fn temp(name: &str, content: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("tfp-csv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn opens_pages_filters_and_exports() {
    let tool = CsvViewer::default();
    let ctx = Ctx::default();
    // GBK 编码、分号分隔
    let path = temp("people.csv", b"\xC3\xFB\xB3\xC6;age\nAda;36\nBob;7\nCy;\n");
    let info = tool
        .run_task("open", json!({ "path": path }), &ctx)
        .unwrap();
    assert_eq!(info["rows"], 3);
    assert_eq!(info["delimiter"], ";");
    assert_eq!(info["encoding"], "gb18030");
    assert_eq!(info["hasHeader"], true);
    assert_eq!(info["columns"][0]["name"], "名称");
    assert_eq!(info["columns"][1]["kind"], "integer");
    assert_eq!(info["name"], "people.csv");
    assert_eq!(*ctx.logs.lock().unwrap(), vec!["csv.loaded"]);
    let (done, total) = *ctx.progress.lock().unwrap().last().unwrap();
    assert_eq!(done, total);
    let id = info["id"].as_u64().unwrap();

    let page = tool
        .call(
            "rows",
            json!({ "id": id, "sort": [{ "column": 1, "desc": true }], "limit": 2 }),
        )
        .unwrap();
    assert_eq!(page["total"], 3);
    assert_eq!(
        page["rows"][0],
        json!({ "index": 0, "cells": ["Ada", "36"] })
    );
    assert_eq!(page["rows"].as_array().unwrap().len(), 2);

    let filtered =
        json!({ "id": id, "filters": [{ "column": 1, "op": "notEmpty" }], "query": "b" });
    let page = tool.call("rows", filtered.clone()).unwrap();
    assert_eq!(page["rows"], json!([{ "index": 1, "cells": ["Bob", "7"] }]));

    let stats = tool
        .call("stats", json!({ "id": id, "column": 1 }))
        .unwrap();
    assert_eq!(
        (stats["count"].as_u64(), stats["empty"].as_u64()),
        (Some(3), Some(1))
    );

    let out = temp("out.json", b"");
    let mut args = filtered;
    args["path"] = json!(out);
    args["format"] = json!("json");
    let result = tool.run_task("export", args, &Ctx::default()).unwrap();
    assert_eq!(result["rows"], 1);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "[\n  {\"名称\":\"Bob\",\"age\":7}\n]\n"
    );
}

#[test]
fn parses_pasted_text_and_reports_errors() {
    let tool = CsvViewer::default();
    let info = tool
        .call(
            "parse_text",
            json!({ "text": "a\tb\n1\t2", "header": false }),
        )
        .unwrap();
    assert_eq!(
        (info["rows"].as_u64(), info["delimiter"].as_str()),
        (Some(2), Some("\\t"))
    );
    assert_eq!(info["path"], Value::Null);
    assert_eq!(
        tool.call("parse_text", json!({ "text": " " }))
            .unwrap_err()
            .code,
        "csv.empty"
    );
    assert_eq!(
        tool.call("rows", json!({ "id": 999 })).unwrap_err().code,
        "csv.table_expired"
    );
    let missing = tool.run_task("open", json!({ "path": "/nope.csv" }), &Ctx::default());
    assert_eq!(missing.unwrap_err().code, "fs.not_found");
}

#[test]
fn keeps_only_recent_tables() {
    let tool = CsvViewer::default();
    let ids: Vec<u64> = (0..4)
        .map(|_| {
            tool.call("parse_text", json!({ "text": "a,b\n1,2" }))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(
        tool.call("rows", json!({ "id": ids[0] })).unwrap_err().code,
        "csv.table_expired"
    );
    assert!(tool.call("rows", json!({ "id": ids[3] })).is_ok());
}
