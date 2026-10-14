use super::*;
use serde_json::json;

struct Dir(std::path::PathBuf);

impl ServiceContext for Dir {
    fn data_dir(&self) -> &std::path::Path {
        &self.0
    }
}

fn range(range: &str) -> RangeArgs {
    RangeArgs {
        range: Some(range.into()),
        from: None,
        to: None,
    }
}

#[test]
fn resolves_preset_and_custom_ranges() {
    let today = Date::constant(2026, 3, 1);
    let resolve = |r: &str| range(r).resolve(today).unwrap();
    assert_eq!(resolve("today"), ("2026-03-01".into(), "2026-03-01".into()));
    assert_eq!(
        resolve("yesterday"),
        ("2026-02-28".into(), "2026-02-28".into())
    );
    assert_eq!(resolve("week"), ("2026-02-23".into(), "2026-03-01".into()));
    assert_eq!(resolve("month").0, "2026-01-31");
    assert_eq!(resolve("all").0, "1970-01-01");
    assert_eq!(
        range("decade").resolve(today).unwrap_err().code,
        "input_stats.invalid_range"
    );
    // 自定义范围顺序颠倒时自动交换
    let custom = RangeArgs {
        range: None,
        from: Some("2026-05-02".into()),
        to: Some("2026-04-30".into()),
    };
    assert_eq!(
        custom.resolve(today).unwrap(),
        ("2026-04-30".into(), "2026-05-02".into())
    );
    let bad = RangeArgs {
        range: None,
        from: Some("2026-13-01".into()),
        to: Some("2026-01-01".into()),
    };
    assert_eq!(
        bad.resolve(today).unwrap_err().code,
        "input_stats.invalid_date"
    );
}

#[test]
fn calls_need_the_service() {
    let plugin = InputStats::default();
    assert!(plugin.manifest().service);
    assert_eq!(
        plugin.call("status", json!({})).unwrap_err().code,
        "input_stats.not_started"
    );
}

#[test]
fn service_lifecycle_without_capturing() {
    let dir = std::env::temp_dir().join(format!("tf-input-stats-lib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let plugin = InputStats::default();
    plugin.start_service(Arc::new(Dir(dir.clone()))).unwrap();
    assert!(dir.join(DB_FILE).is_file());

    // 默认不采集，需要用户主动开启
    let status = plugin.call("status", json!({})).unwrap();
    assert_eq!(status["enabled"], false);
    assert_eq!(status["capturing"], false);
    assert_eq!(status["today"]["keys"], 0);

    let excluded = plugin
        .call(
            "set_excluded",
            json!({"apps": ["com.bank.app", "com.bank.app", ""]}),
        )
        .unwrap();
    assert_eq!(
        excluded,
        json!([{"id": "com.bank.app", "name": "com.bank.app"}])
    );
    let apps = plugin.call("apps", json!({})).unwrap();
    assert_eq!(apps["apps"], json!([]));
    assert_eq!(apps["excluded"][0]["id"], "com.bank.app");

    let summary = plugin.call("summary", json!({"range": "week"})).unwrap();
    assert_eq!(summary["hours"].as_array().unwrap().len(), 24);
    assert_eq!(plugin.call("click_map", json!({})).unwrap(), json!([]));
    plugin.call("clear", json!({})).unwrap();
    assert_eq!(
        plugin.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );

    plugin.stop_service();
    plugin.stop_service();
    drop(plugin);
    // 设置在重新启动后仍然有效
    let plugin = InputStats::default();
    plugin.start_service(Arc::new(Dir(dir.clone()))).unwrap();
    let status = plugin.call("status", json!({})).unwrap();
    assert_eq!(status["excluded"][0]["id"], "com.bank.app");
    plugin.stop_service();
    std::fs::remove_dir_all(&dir).unwrap();
}
