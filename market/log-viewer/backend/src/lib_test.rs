use super::*;

/// 测试用的任务上下文：search 经 guest 文件接口读取，这里只需要进度与取消
struct Ctx;

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, _code: &str, _params: Value) {}
    fn progress(&self, _done: u64, _total: u64) {}
    fn stage(&self, _code: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Err(PluginError::new("fs.not_found").with("path", path))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found").with("path", path))
    }
}

#[test]
fn search_returns_matching_lines_in_one_call() {
    let path = std::env::temp_dir().join(format!("tf-log-search-{}.log", std::process::id()));
    std::fs::write(
        &path,
        "INFO start\nERROR disk full\nWARN slow\nERROR disk gone\n",
    )
    .unwrap();
    let result = LogViewer::default()
        .run_task(
            "search",
            json!({ "path": path, "query": "disk", "levels": ["error"], "limit": 1 }),
            &Ctx,
        )
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(result["total"], 2);
    assert_eq!(
        result["lines"],
        json!([{ "n": 2, "level": "error", "text": "ERROR disk full" }])
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&LogViewer::default()).unwrap();
    assert_eq!(checked, 0);
}
