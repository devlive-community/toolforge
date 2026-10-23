use std::sync::Mutex;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    last_progress: Mutex<Option<(u64, u64)>>,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, done: u64, total: u64) {
        *self.last_progress.lock().unwrap() = Some((done, total));
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
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

fn temp(name: &str, content: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("tfp-encoder-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn encodes_file_as_data_uri_with_mime() {
    let path = temp("dot.png", b"foobar");
    let ctx = Ctx::default();
    let out = encode(
        Args {
            path,
            format: Format::DataUri,
            wrap: false,
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(out.output, "data:image/png;base64,Zm9vYmFy");
    assert_eq!(
        (out.mime.as_str(), out.bytes, out.name.as_str()),
        ("image/png", 6, "dot.png")
    );
    assert_eq!(*ctx.last_progress.lock().unwrap(), Some((6, 6)));
    assert_eq!(
        *ctx.logs.lock().unwrap(),
        vec!["encode.file_start", "encode.file_done"]
    );
}

#[test]
fn plain_base64_and_unknown_mime() {
    let path = temp("blob.bin", b"fooba");
    let out = encode(
        Args {
            path,
            format: Format::Base64,
            wrap: false,
        },
        &Ctx::default(),
    )
    .unwrap();
    assert_eq!(out.output, "Zm9vYmE=");
    assert_eq!(out.mime, "application/octet-stream");
}

#[test]
fn missing_file_fails() {
    let err = encode(
        Args {
            path: "/nope/x.png".into(),
            format: Format::Base64,
            wrap: false,
        },
        &Ctx::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, "fs.not_found");
}

fn output(len: usize) -> Output {
    Output {
        output: "A".repeat(len),
        name: "big.bin".into(),
        mime: "application/octet-stream".into(),
        bytes: len as u64,
    }
}

#[test]
fn large_results_return_a_bounded_preview() {
    let results = Results::default();
    let summary = results.store(output(PREVIEW * 3));
    assert!(summary.truncated);
    assert_eq!(summary.preview.len(), PREVIEW);
    assert_eq!(summary.chars, PREVIEW * 3);

    let full = results.full(ResultArgs { id: summary.id }).unwrap();
    assert_eq!(full.output.len(), PREVIEW * 3);

    let small = results.store(output(10));
    assert!(!small.truncated);
    assert_eq!(small.preview.len(), 10);
}

#[test]
fn older_results_expire() {
    let results = Results::default();
    let first = results.store(output(4));
    let second = results.store(output(8));
    assert_eq!(
        results.full(ResultArgs { id: first.id }).unwrap_err().code,
        "encode.result_expired"
    );
    assert_eq!(
        results
            .full(ResultArgs { id: second.id })
            .unwrap()
            .output
            .len(),
        8
    );
}

#[test]
fn saves_full_output_to_disk() {
    let results = Results::default();
    let summary = results.store(output(PREVIEW + 1));
    let path = temp("saved.txt", b"");
    results
        .save(SaveArgs {
            id: summary.id,
            path: path.clone(),
        })
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap().len(), PREVIEW + 1);
}
