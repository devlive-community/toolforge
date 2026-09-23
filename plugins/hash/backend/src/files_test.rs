use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    progress: Mutex<Vec<(u64, u64)>>,
    cancel_after_first_chunk: AtomicBool,
    cancel: AtomicBool,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, done: u64, total: u64) {
        self.progress.lock().unwrap().push((done, total));
        if self.cancel_after_first_chunk.load(Ordering::Relaxed) {
            self.cancel.store(true, Ordering::Relaxed);
        }
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
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

fn temp_file(name: &str, content: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("tfp-hash-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

fn args(paths: Vec<String>) -> Args {
    Args {
        paths,
        algorithms: vec![Algorithm::Md5, Algorithm::Sha256],
        uppercase: false,
    }
}

#[test]
fn hashes_files_and_reports_progress() {
    let path = temp_file("abc.txt", b"abc");
    let ctx = Ctx::default();
    let report = run(args(vec![path]), &ctx).unwrap();
    assert_eq!(report.total_bytes, 3);
    assert_eq!(report.files[0].name, "abc.txt");
    assert_eq!(
        report.files[0].digests["md5"],
        "900150983cd24fb0d6963f7d28e17f72"
    );
    assert_eq!(*ctx.progress.lock().unwrap().last().unwrap(), (3, 3));
    let logs = ctx.logs.lock().unwrap();
    assert!(logs.contains(&"hash.file_start".to_owned()));
    assert!(logs.contains(&"hash.file_done".to_owned()));
}

#[test]
fn missing_files_fail_individually() {
    let ok = temp_file("ok.txt", b"x");
    let ctx = Ctx::default();
    let report = run(args(vec!["/definitely/missing".into(), ok]), &ctx).unwrap();
    assert_eq!(report.files[0].error.as_ref().unwrap().code, "fs.not_found");
    assert!(report.files[1].error.is_none());
    assert!(
        ctx.logs
            .lock()
            .unwrap()
            .contains(&"hash.file_failed".to_owned())
    );
}

#[test]
fn stops_when_cancelled() {
    let big = temp_file("big.bin", &vec![7u8; CHUNK * 3]);
    let ctx = Ctx::default();
    ctx.cancel_after_first_chunk.store(true, Ordering::Relaxed);
    let err = run(args(vec![big]), &ctx).unwrap_err();
    assert_eq!(err.code, "task.cancelled");
    assert_eq!(ctx.progress.lock().unwrap().len(), 1);
}

#[test]
fn validates_arguments() {
    let ctx = Ctx::default();
    assert_eq!(run(args(vec![]), &ctx).unwrap_err().code, "hash.no_files");
    let mut no_algo = args(vec!["a".into()]);
    no_algo.algorithms.clear();
    assert_eq!(run(no_algo, &ctx).unwrap_err().code, "hash.no_algorithms");
}
