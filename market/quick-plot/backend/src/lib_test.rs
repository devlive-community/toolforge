use std::io::Read;

use serde_json::json;

use super::*;

struct Ctx(Vec<u8>);

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Ok(Box::new(std::io::Cursor::new(self.0.clone())))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Ok(self.0.len() as u64)
    }
}

#[test]
fn parses_suggests_and_plots() {
    let tool = QuickPlot::default();
    let summary = tool
        .call("parse", json!({ "text": "t,qps,p99,host\n2024-01-01 00:00:00,1,2,a\n2024-01-01 00:01:00,3,4,b\n" }))
        .unwrap();
    assert_eq!(summary["format"], "csv");
    assert_eq!(summary["delimiter"], "comma");
    assert_eq!(summary["rows"], 2);
    assert_eq!(summary["columns"][0]["kind"], "time");
    assert_eq!(summary["columns"][3]["kind"], "text");
    assert_eq!(summary["columns"][1]["min"], 1.0);
    assert!(summary["columns"][3]["min"].is_null());
    assert_eq!(summary["preview"][1][3], "b");
    assert_eq!(
        summary["suggestion"],
        json!({ "chart": "line", "x": 0, "series": [1, 2] })
    );
    let plot = tool
        .call(
            "plot",
            json!({ "token": summary["token"], "chart": "line", "x": 0, "series": [{ "column": 1 }, { "column": 2, "axis": "right" }] }),
        )
        .unwrap();
    assert_eq!(plot["x"]["kind"], "time");
    assert!(plot["right"]["ticks"].is_array());
    assert_eq!(
        plot["series"][1]["points"][1],
        json!([1_704_067_260_000.0 + offset_ms(), 4.0])
    );
}

/// 本地时区相对 UTC 的偏移（毫秒）：不带偏移的时间按本地解释
fn offset_ms() -> f64 {
    infer::datetime("2024-01-01 00:00:00").unwrap() - 1_704_067_200_000.0
}

#[test]
fn suggests_bars_for_short_category_tables() {
    let tool = QuickPlot::default();
    let summary = tool
        .call(
            "parse",
            json!({ "text": "[{\"lang\":\"rust\",\"stars\":5},{\"lang\":\"go\",\"stars\":3}]" }),
        )
        .unwrap();
    assert_eq!(summary["format"], "json");
    assert_eq!(
        summary["suggestion"],
        json!({ "chart": "bar", "x": 0, "series": [1] })
    );
    // 首列递增的数值列作 X 轴；否则用行号
    let increasing = tool
        .call("parse", json!({ "text": "n,v\n1,5\n2,3\n4,9\n" }))
        .unwrap();
    assert_eq!(increasing["suggestion"]["x"], 0);
    let shuffled = tool
        .call("parse", json!({ "text": "a,b\n3,5\n1,3\n" }))
        .unwrap();
    assert!(shuffled["suggestion"]["x"].is_null());
    assert_eq!(shuffled["suggestion"]["series"], json!([0, 1]));
}

#[test]
fn tokens_expire_when_a_new_table_is_parsed() {
    let tool = QuickPlot::default();
    let first = tool.call("parse", json!({ "text": "v\n1\n2\n" })).unwrap();
    tool.call("parse", json!({ "text": "v\n3\n4\n" })).unwrap();
    let err = tool
        .call(
            "plot",
            json!({ "token": first["token"], "chart": "line", "series": [{ "column": 0 }] }),
        )
        .unwrap_err();
    assert_eq!(err.code, "plot.expired");
}

#[test]
fn loads_files_by_extension() {
    let tool = QuickPlot::default();
    let jsonl = Ctx(b"{\"a\":1}\n{\"a\":2}\n".to_vec());
    let summary = tool
        .run_task("load", json!({ "path": "/tmp/data.ndjson" }), &jsonl)
        .unwrap();
    assert_eq!(
        (summary["format"].as_str(), summary["file"].as_str()),
        (Some("jsonl"), Some("data.ndjson"))
    );
    let tsv = Ctx(b"a\tb\n1\t2\n".to_vec());
    assert_eq!(
        tool.run_task("load", json!({ "path": "x.tsv" }), &tsv)
            .unwrap()["format"],
        "tsv"
    );
    let bad = Ctx(vec![b'a', b',', 0xff, b'\n']);
    let err = tool
        .run_task("load", json!({ "path": "x.csv" }), &bad)
        .unwrap_err();
    assert_eq!(err.code, "plot.not_utf8");
    assert_eq!(format_for("A.JSON", Format::Auto), Format::Json);
    assert_eq!(format_for("a.json", Format::Csv), Format::Csv);
}

#[test]
fn saves_only_png_data() {
    let err = save_png(SaveArgs {
        path: "/tmp/x.png".into(),
        data: "data:image/png;base64,aGVsbG8=".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "plot.png_invalid");
    let err = save_png(SaveArgs {
        path: "/tmp/x.png".into(),
        data: "!!".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "plot.png_invalid");
    let dir = std::env::temp_dir().join(format!("tf-quick-plot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("chart.png");
    let png = data_encoding::BASE64.encode(b"\x89PNG\r\n\x1a\nrest");
    save_png(SaveArgs {
        path: path.to_string_lossy().into_owned(),
        data: format!("data:image/png;base64,{png}"),
    })
    .unwrap();
    assert!(std::fs::read(&path).unwrap().starts_with(b"\x89PNG"));
    std::fs::remove_dir_all(dir).unwrap();
}
