use std::io::Read;
use std::sync::Mutex;

use encoding_rs::{GBK, SHIFT_JIS};

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
    let dir = std::env::temp_dir().join(format!(
        "tfp-text-encoding-{name}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const ZH: &str = "这是一个用简体中文写的配置文件，里面有数据库地址、用户名和密码等设置。\r\n";

fn fixture(name: &str) -> PathBuf {
    let dir = dir(name);
    let gbk = GBK.encode(&ZH.repeat(3)).0.into_owned();
    std::fs::write(dir.join("gbk.txt"), &gbk).unwrap();
    std::fs::write(dir.join("utf8.md"), "# 标题\n正文\n").unwrap();
    std::fs::write(dir.join("image.bin"), b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").unwrap();
    std::fs::write(dir.join(".hidden"), "secret").unwrap();
    std::fs::write(dir.join("old.txt.bak"), "backup").unwrap();
    std::fs::write(dir.join("old.txt.bak.2"), "backup").unwrap();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub").join("ascii.ini"), "a=1\n").unwrap();
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    std::fs::write(dir.join("node_modules").join("x.js"), "x").unwrap();
    dir
}

fn tool() -> TextEncoding {
    TextEncoding::default()
}

#[test]
fn scans_folders_and_skips_binary_and_hidden_files() {
    let dir = fixture("scan");
    let root = dir.to_string_lossy().into_owned();
    let flat = tool().call("scan", json!({ "paths": [root] })).unwrap();
    let names: Vec<&str> = flat["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["gbk.txt", "utf8.md"]);
    assert_eq!(flat["skipped"]["binary"], 1);
    let gbk = &flat["files"][0];
    assert_eq!(gbk["encoding"], "GBK");
    assert_eq!(gbk["confidence"], "likely");
    assert_eq!(gbk["lineEndings"], "crlf");
    assert_eq!(flat["files"][1]["encoding"], "UTF-8");
    assert_eq!(flat["files"][1]["lineEndings"], "lf");

    let deep = tool()
        .call("scan", json!({ "paths": [root], "recursive": true }))
        .unwrap();
    let names: Vec<&str> = deep["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["gbk.txt", "ascii.ini", "utf8.md"]);
    assert_eq!(deep["files"][1]["ascii"], true);
    // 直接添加的备份文件仍然可以处理
    let bak = dir.join("old.txt.bak").to_string_lossy().into_owned();
    let direct = tool().call("scan", json!({ "paths": [bak] })).unwrap();
    assert_eq!(direct["files"][0]["name"], "old.txt.bak");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn previews_in_any_encoding_and_checks_the_target() {
    let dir = fixture("preview");
    let path = dir.join("gbk.txt").to_string_lossy().into_owned();
    let auto = tool().call("preview", json!({ "path": path })).unwrap();
    assert_eq!(auto["encoding"], "GBK");
    assert!(auto["text"].as_str().unwrap().starts_with("这是一个"));
    assert_eq!(auto["malformed"], false);
    // 按错误的编码读取会出现乱码
    let wrong = tool()
        .call("preview", json!({ "path": path, "encoding": "UTF-8" }))
        .unwrap();
    assert_eq!(wrong["malformed"], true);

    let sjis = SHIFT_JIS.encode("日本語のテキスト\n").0.into_owned();
    let ja = dir.join("ja.txt");
    std::fs::write(&ja, &sjis).unwrap();
    let checked = tool()
        .call(
            "preview",
            json!({ "path": ja.to_string_lossy(), "encoding": "Shift_JIS", "target": "windows-1252" }),
        )
        .unwrap();
    assert_eq!(checked["unmappable"]["char"], "日");
    assert_eq!(checked["unmappable"]["line"], 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn converts_in_place_with_a_backup() {
    let dir = fixture("inplace");
    let gbk = dir.join("gbk.txt");
    let utf8 = dir.join("utf8.md");
    let original = std::fs::read(&gbk).unwrap();
    let ctx = Ctx::default();
    let out = tool()
        .run_task(
            "convert",
            json!({
                "files": [
                    { "path": gbk.to_string_lossy(), "encoding": "GBK" },
                    { "path": utf8.to_string_lossy(), "encoding": "UTF-8" },
                    { "path": dir.join("missing.txt").to_string_lossy(), "encoding": "UTF-8" },
                ],
                "to": "UTF-8",
                "newline": "lf",
                "backup": true,
            }),
            &ctx,
        )
        .unwrap();
    assert_eq!(
        out["summary"],
        json!({ "converted": 1, "unchanged": 1, "failed": 1 })
    );
    assert_eq!(out["items"][2]["error"]["code"], "fs.not_found");
    assert_eq!(
        std::fs::read_to_string(&gbk).unwrap(),
        ZH.repeat(3).replace("\r\n", "\n")
    );
    assert_eq!(std::fs::read(dir.join("gbk.txt.bak")).unwrap(), original);
    assert!(!dir.join("utf8.md.bak").exists());
    assert!(!dir.join(".gbk.txt.tf-part").exists());
    let logs = ctx.0.lock().unwrap().clone();
    assert_eq!(
        logs,
        [
            "encoding.converted",
            "encoding.unchanged",
            "encoding.failed",
            "encoding.done"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn converts_into_another_folder_without_touching_the_originals() {
    let dir = fixture("outdir");
    let out_dir = dir.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("utf8.md"), "existing").unwrap();
    let source = dir.join("utf8.md");
    let out = tool()
        .run_task(
            "convert",
            json!({
                "files": [{ "path": source.to_string_lossy(), "encoding": "UTF-8" }],
                "to": "gb18030",
                "outputDir": out_dir.to_string_lossy(),
            }),
            &Ctx::default(),
        )
        .unwrap();
    assert_eq!(out["items"][0]["status"], "converted");
    let written = out_dir.join("utf8 (1).md");
    assert_eq!(
        out["items"][0]["output"],
        written.to_string_lossy().as_ref()
    );
    let expected = encoding_rs::GB18030.encode("# 标题\n正文\n").0.into_owned();
    assert_eq!(std::fs::read(&written).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "# 标题\n正文\n");
    assert_eq!(
        std::fs::read_to_string(out_dir.join("utf8.md")).unwrap(),
        "existing"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_bad_arguments() {
    let ctx = Ctx::default();
    let err = |args: Value| tool().run_task("convert", args, &ctx).unwrap_err().code;
    assert_eq!(
        err(json!({ "files": [], "to": "UTF-8" })),
        "encoding.no_files"
    );
    assert_eq!(
        err(json!({ "files": [], "to": "UTF-7" })),
        "encoding.unsupported"
    );
    assert_eq!(
        err(json!({ "files": [], "to": "UTF-8", "outputDir": "/definitely/not/here" })),
        "encoding.output_dir_missing"
    );
    let list = tool().call("encodings", json!({})).unwrap();
    assert_eq!(list[0], "UTF-8");
}
