use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use super::*;
use crate::test_support::{dir, write};

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

#[test]
fn compares_diffs_and_syncs_two_folders() {
    let base = dir("lib");
    let (l, r) = (base.join("l"), base.join("r"));
    write(&l, "app.conf", "port=80\nhost=a\n");
    write(&r, "app.conf", "port=8080\nhost=a\n");
    write(&l, ".git/HEAD", "ref");
    write(&r, "new/file.txt", "n");
    let (ls, rs) = (
        l.to_string_lossy().into_owned(),
        r.to_string_lossy().into_owned(),
    );
    let tool = FolderCompare::default();
    let ctx = Ctx::default();
    let out = tool
        .run_task(
            "compare",
            json!({ "left": ls, "right": rs, "method": "content" }),
            &ctx,
        )
        .unwrap();
    // .git 默认被忽略
    let paths: Vec<&str> = out["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["new", "new/file.txt", "app.conf"]);
    assert_eq!(out["summary"]["changed"], 1);
    assert_eq!(out["items"][2]["status"], "changed");

    let diff = tool
        .call(
            "diff",
            json!({ "left": ls, "right": rs, "path": "app.conf" }),
        )
        .unwrap();
    assert_eq!(diff["rows"][0]["tag"], "change");
    assert_eq!(diff["added"], 1);
    assert_eq!(
        tool.call("diff", json!({ "left": ls, "right": rs, "path": "../x" }))
            .unwrap_err()
            .code,
        "fc.invalid_path"
    );

    let copied = tool
        .run_task(
            "copy",
            json!({ "left": ls, "right": rs, "paths": ["new", "new/file.txt"], "direction": "toLeft" }),
            &ctx,
        )
        .unwrap();
    assert_eq!(copied["files"], 1, "folder and its child are copied once");
    assert!(l.join("new").join("file.txt").is_file());
    let again = tool
        .run_task(
            "compare",
            json!({ "left": ls, "right": rs }),
            &Ctx::default(),
        )
        .unwrap();
    assert_eq!(again["summary"]["rightOnly"], 0);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn rejects_bad_folders() {
    let base = dir("lib-errors");
    let s = base.to_string_lossy().into_owned();
    let ctx = Ctx::default();
    let err = |args: Value| {
        FolderCompare::default()
            .run_task("compare", args, &ctx)
            .unwrap_err()
            .code
    };
    assert_eq!(err(json!({ "left": s, "right": s })), "fc.same_folder");
    assert_eq!(
        err(json!({ "left": s, "right": "/no/such/dir" })),
        "fc.not_a_folder"
    );
    std::fs::remove_dir_all(base).unwrap();
}
