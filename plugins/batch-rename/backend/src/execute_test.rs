use std::io::Read;
use std::sync::Mutex;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx(Mutex<Vec<String>>);

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.0.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
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

fn dir(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tfp-rename-exec-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn step(dir: &Path, from: &str, to: &str) -> Move {
    Move {
        from: dir.join(from).to_string_lossy().into_owned(),
        to: dir.join(to).to_string_lossy().into_owned(),
    }
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn swaps_cycles_and_undoes() {
    let dir = dir("swap");
    for name in ["a", "b", "c"] {
        std::fs::write(dir.join(name), name).unwrap();
    }
    let ctx = Ctx::default();
    let journal = execute(
        &[
            step(&dir, "a", "b"),
            step(&dir, "b", "c"),
            step(&dir, "c", "a"),
        ],
        &ctx,
    )
    .unwrap();
    assert_eq!(
        (read(&dir, "a"), read(&dir, "b"), read(&dir, "c")),
        ("c".into(), "a".into(), "b".into())
    );
    assert_eq!(
        listing(&dir),
        ["a", "b", "c"],
        "no temporary files are left behind"
    );
    assert_eq!(ctx.0.lock().unwrap().len(), 3);

    undo(&journal, &ctx).unwrap();
    assert_eq!(
        (read(&dir, "a"), read(&dir, "b"), read(&dir, "c")),
        ("a".into(), "b".into(), "c".into())
    );
}

#[test]
fn changes_only_the_case() {
    let dir = dir("case");
    std::fs::write(dir.join("photo.jpg"), "x").unwrap();
    let journal = execute(&[step(&dir, "photo.jpg", "Photo.JPG")], &Ctx::default()).unwrap();
    assert_eq!(listing(&dir), ["Photo.JPG"]);
    undo(&journal, &Ctx::default()).unwrap();
    assert_eq!(listing(&dir), ["photo.jpg"]);
}

#[test]
fn rolls_back_when_a_step_fails() {
    let dir = dir("rollback");
    std::fs::write(dir.join("one"), "1").unwrap();
    std::fs::write(dir.join("two"), "2").unwrap();
    std::fs::write(dir.join("blocker"), "b").unwrap();
    let ctx = Ctx::default();
    // 第二个目标在执行时已存在：整个批次回滚
    let err = execute(
        &[step(&dir, "one", "uno"), step(&dir, "two", "blocker")],
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, "rename.target_appeared");
    assert_eq!(listing(&dir), ["blocker", "one", "two"]);
    assert_eq!(
        (read(&dir, "one"), read(&dir, "blocker")),
        ("1".into(), "b".into())
    );
    // 源文件不存在：第一阶段失败
    let err = execute(&[step(&dir, "one", "uno"), step(&dir, "ghost", "x")], &ctx).unwrap_err();
    assert_eq!(err.code, "rename.failed");
    assert_eq!(listing(&dir), ["blocker", "one", "two"]);
}

#[test]
fn refuses_unsafe_undo() {
    let dir = dir("undo");
    std::fs::write(dir.join("new"), "n").unwrap();
    std::fs::write(dir.join("old"), "someone else").unwrap();
    let journal = vec![step(&dir, "old", "new")];
    assert_eq!(
        undo(&journal, &Ctx::default()).unwrap_err().code,
        "rename.undo_occupied"
    );
    let gone = vec![step(&dir, "x", "missing")];
    assert_eq!(
        undo(&gone, &Ctx::default()).unwrap_err().code,
        "rename.undo_missing"
    );
    assert_eq!(read(&dir, "old"), "someone else");
}
