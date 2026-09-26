//! 测试共用的任务上下文与临时目录。

use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde_json::Value;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};

#[derive(Default)]
pub struct Ctx {
    pub logs: Mutex<Vec<String>>,
    pub cancelled: AtomicBool,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

pub fn workspace(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tfp-archive-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

pub fn write(dir: &std::path::Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

/// 测试用的源文件夹：嵌套文件、空文件夹、系统文件
pub fn sample(dir: &std::path::Path) -> PathBuf {
    let root = dir.join("project");
    write(&root, "readme.md", b"# Hello\n");
    write(&root, "src/main.rs", b"fn main() {}\n");
    write(
        &root,
        "assets/data.bin",
        &(0..=255u8).cycle().take(100_000).collect::<Vec<_>>(),
    );
    write(&root, ".DS_Store", b"junk");
    std::fs::create_dir_all(root.join("empty")).unwrap();
    root
}
