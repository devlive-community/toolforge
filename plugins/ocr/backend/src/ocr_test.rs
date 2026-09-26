use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;
use tf_plugin_api::{LogLevel, TaskContext};

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    models: Option<PathBuf>,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _: u64, _: u64) {}
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
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        let file = match id {
            DETECTOR => "ch_PP-OCRv4_det_infer.onnx",
            _ => "ch_PP-OCRv4_rec_infer.onnx",
        };
        self.models
            .as_ref()
            .map(|dir| dir.join(file))
            .ok_or_else(|| PluginError::new("resource.missing").with("id", id))
    }
}

fn line(text: &str, x: f32, y: f32) -> Line {
    Line {
        text: text.into(),
        score: 0.9,
        region: Region {
            x,
            y,
            w: 50.0,
            h: 20.0,
            score: 0.9,
        },
    }
}

#[test]
fn joins_rows_with_spaces_and_newlines() {
    let lines = [
        line("姓名", 0.0, 0.0),
        line("Ada", 100.0, 2.0),
        line("Line two", 0.0, 40.0),
    ];
    assert_eq!(join(&lines), "姓名 Ada\nLine two");
    assert_eq!(join(&[]), "");
}

#[test]
fn requires_models_and_valid_images() {
    let engines = Engines::default();
    let ctx = Ctx::default();
    let args = Args {
        source: Source::File {
            path: "/nope.png".into(),
        },
    };
    assert_eq!(
        recognize(args, &engines, &ctx).unwrap_err().code,
        "resource.missing"
    );

    let dir = std::env::temp_dir().join(format!("tfp-ocr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ctx = Ctx {
        models: Some(dir.clone()),
        ..Default::default()
    };
    let bad = dir.join("bad.png");
    std::fs::write(&bad, b"not an image").unwrap();
    let args = Args {
        source: Source::File {
            path: bad.to_string_lossy().into_owned(),
        },
    };
    assert_eq!(
        recognize(args, &engines, &ctx).unwrap_err().code,
        "ocr.unsupported"
    );
}

#[test]
fn parses_sources() {
    let file: Args = serde_json::from_value(
        serde_json::json!({ "source": { "kind": "file", "path": "/a.png" } }),
    )
    .unwrap();
    assert!(matches!(file.source, Source::File { ref path } if path == "/a.png"));
    let clip: Args =
        serde_json::from_value(serde_json::json!({ "source": { "kind": "clipboard" } })).unwrap();
    assert!(matches!(clip.source, Source::Clipboard));
}

/// 需要真实模型：`TOOLFORGE_OCR_MODELS=<含两个 onnx 的目录> TOOLFORGE_OCR_SAMPLE=<图片> cargo test -p tfp-ocr -- --ignored`
#[test]
#[ignore]
fn recognizes_text_with_real_models() {
    let (Some(models), Some(sample)) = (
        std::env::var_os("TOOLFORGE_OCR_MODELS"),
        std::env::var_os("TOOLFORGE_OCR_SAMPLE"),
    ) else {
        return;
    };
    let ctx = Ctx {
        models: Some(PathBuf::from(models)),
        ..Default::default()
    };
    let engines = Engines::default();
    let args = Args {
        source: Source::File {
            path: sample.to_string_lossy().into_owned(),
        },
    };
    let output = recognize(args, &engines, &ctx).unwrap();
    println!("{}\n({} ms)", output.text, output.elapsed_ms);
    // 识别模型通常会省略中英文之间的空格
    assert!(output.text.contains("开发者工具箱"), "{}", output.text);
    assert!(output.text.contains("Version 0.1.5"), "{}", output.text);
    assert!(output.text.contains("English"), "{}", output.text);
    assert!(output.preview.starts_with("data:image/jpeg;base64,"));
    assert_eq!(
        *ctx.logs.lock().unwrap(),
        vec!["ocr.model_loaded", "ocr.regions", "ocr.done"]
    );
    // 第二次复用已加载的模型与已编译的执行计划
    let args = Args {
        source: Source::File {
            path: sample.to_string_lossy().into_owned(),
        },
    };
    let again = recognize(args, &engines, &ctx).unwrap();
    println!("second run: {} ms", again.elapsed_ms);
    assert_eq!(again.text, output.text);
}
