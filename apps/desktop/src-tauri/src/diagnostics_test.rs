use std::io::Read;

use serde_json::json;

use super::*;

#[test]
fn keeps_only_safe_prefs() {
    let prefs = json!({ "theme": "dark", "globalShortcut": null, "collapsed": { "dev": true }, "secret": "x" });
    let safe = safe_prefs(&prefs);
    assert_eq!(
        Value::Object(safe),
        json!({ "theme": "dark", "globalShortcut": null })
    );
}

#[test]
fn writes_report_and_logs() {
    let dir = std::env::temp_dir().join(format!("tf-diag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("app.log");
    std::fs::write(&log, "2026-01-01T00:00:00.000Z INFO  toolforge started\n").unwrap();
    let report = Report {
        generated_at: "2026-01-01T00:00:00.000Z".into(),
        app: AppInfo::test_value(),
        prefs: safe_prefs(&json!({ "theme": "light" })),
        launcher: LauncherStatus::default(),
        plugins: vec![PluginEntry {
            id: "org.devlive.toolforge.hash".into(),
            version: "0.1.0".into(),
        }],
        tasks: Vec::new(),
    };
    let path = dir.join("diag.zip");
    let bytes = write_zip(&path, &report, &[log, dir.join("missing.log")]).unwrap();
    assert!(bytes > 0);

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let mut report_json = String::new();
    archive
        .by_name("report.json")
        .unwrap()
        .read_to_string(&mut report_json)
        .unwrap();
    let value: Value = serde_json::from_str(&report_json).unwrap();
    assert_eq!(value["prefs"], json!({ "theme": "light" }));
    assert_eq!(value["plugins"][0]["id"], "org.devlive.toolforge.hash");
    let mut log_text = String::new();
    archive
        .by_name("logs/app.log")
        .unwrap()
        .read_to_string(&mut log_text)
        .unwrap();
    assert!(log_text.contains("toolforge started"));
}
