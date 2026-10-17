use std::path::PathBuf;

use serde_json::Value;
use tf_plugin_api::LogLevel;

use super::*;
use crate::testkit::{ECHO_PROTO, TYPES_PROTO, pool};

/// 从真实文件系统读取文件
pub struct FsCtx;

impl TaskContext for FsCtx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| PluginError::new("fs.not_found").with("detail", e.to_string()))
    }
}

fn layout(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("tf-grpc-{name}-{}", std::process::id()));
    let api = root.join("api");
    let shared = root.join("shared");
    std::fs::create_dir_all(&api).unwrap();
    std::fs::create_dir_all(shared.join("common")).unwrap();
    std::fs::write(api.join("echo.proto"), ECHO_PROTO).unwrap();
    std::fs::write(shared.join("common").join("types.proto"), TYPES_PROTO).unwrap();
    (root, shared)
}

#[test]
fn compiles_proto_with_include_directories() {
    let (root, shared) = layout("ok");
    let main = root.join("api").join("echo.proto");
    let pool = load_proto(
        &main.to_string_lossy(),
        &[shared.to_string_lossy().into_owned()],
        &FsCtx,
    )
    .unwrap();
    let catalog = catalog(&pool);
    assert_eq!(catalog.services.len(), 1);
    let echo = &catalog.services[0];
    assert_eq!(echo.name, "demo.v1.Echo");
    let watch = echo.methods.iter().find(|m| m.name == "Watch").unwrap();
    assert!(watch.server_streaming && watch.supported);
    assert_eq!(watch.path, "/demo.v1.Echo/Watch");
    assert_eq!(
        (watch.input.as_str(), watch.output.as_str()),
        ("demo.v1.EchoRequest", "demo.v1.EchoReply")
    );
    let upload = echo.methods.iter().find(|m| m.name == "Upload").unwrap();
    assert!(upload.client_streaming && !upload.supported);
    // google/protobuf/timestamp.proto 由内置定义提供
    assert!(catalog.files >= 3);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reports_the_missing_import_and_where_it_was_searched() {
    let (root, _) = layout("missing");
    let main = root.join("api").join("echo.proto");
    let err = load_proto(&main.to_string_lossy(), &[], &FsCtx).unwrap_err();
    assert_eq!(err.code, "grpc.import_missing");
    assert_eq!(err.params["import"], "common/types.proto");
    assert_eq!(err.params["from"], "echo.proto");
    assert!(err.params["searched"].as_str().unwrap().contains("api"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reports_syntax_errors_with_the_file() {
    let root = std::env::temp_dir().join(format!("tf-grpc-bad-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let main = root.join("bad.proto");
    std::fs::write(&main, "syntax = \"proto3\";\nmessage {").unwrap();
    let err = load_proto(&main.to_string_lossy(), &[], &FsCtx).unwrap_err();
    assert_eq!(err.code, "grpc.proto_invalid");
    assert_eq!(err.params["file"], "bad.proto");
    let err = load_proto(&root.join("none.proto").to_string_lossy(), &[], &FsCtx).unwrap_err();
    assert_eq!(err.code, "grpc.read_failed");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn loads_descriptor_sets() {
    let root = std::env::temp_dir().join(format!("tf-grpc-fds-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("all.pb");
    std::fs::write(&path, pool().encode_to_vec()).unwrap();
    let loaded = load_descriptor(&path.to_string_lossy(), &FsCtx).unwrap();
    assert_eq!(catalog(&loaded), catalog(&pool()));
    std::fs::write(&path, b"\xff\xff not a descriptor").unwrap();
    let err = load_descriptor(&path.to_string_lossy(), &FsCtx).unwrap_err();
    assert_eq!(err.code, "grpc.descriptor_invalid");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finds_methods_by_path() {
    let pool = pool();
    assert_eq!(
        find_method(&pool, "/demo.v1.Echo/Unary").unwrap().name(),
        "Unary"
    );
    assert_eq!(
        find_method(&pool, "demo.v1.Echo/Unary").unwrap().name(),
        "Unary"
    );
    for bad in ["demo.v1.Echo/Nope", "Unary", "/x.Y/Unary"] {
        assert_eq!(
            find_method(&pool, bad).unwrap_err().code,
            "grpc.method_not_found"
        );
    }
}
