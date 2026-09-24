use std::sync::Mutex;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    model: Option<PathBuf>,
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
        self.model
            .clone()
            .ok_or_else(|| PluginError::new("resource.missing").with("id", id))
    }
}

fn args(paths: Vec<String>) -> Args {
    Args {
        paths,
        model: Model::U2netp,
        background: None,
        format: Format::Png,
        output_dir: None,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tfp-bg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parses_background_colors() {
    assert_eq!(parse_color("#fff").unwrap(), Rgb([255, 255, 255]));
    assert_eq!(parse_color("00ff80").unwrap(), Rgb([0, 255, 128]));
    assert_eq!(parse_color("#12").unwrap_err().code, "bg.invalid_color");
    assert_eq!(parse_color("#zzzzzz").unwrap_err().code, "bg.invalid_color");
}

#[test]
fn composes_alpha_or_background() {
    let image = RgbImage::from_pixel(2, 1, Rgb([200, 100, 0]));
    let mask = GrayImage::from_raw(2, 1, vec![255, 0]).unwrap();
    let transparent = compose(&image, &mask, None).to_rgba8();
    assert_eq!(transparent.get_pixel(0, 0).0, [200, 100, 0, 255]);
    assert_eq!(transparent.get_pixel(1, 0).0[3], 0);
    let filled = compose(&image, &mask, Some(Rgb([0, 0, 255]))).to_rgb8();
    assert_eq!(filled.get_pixel(0, 0).0, [200, 100, 0]);
    assert_eq!(filled.get_pixel(1, 0).0, [0, 0, 255]);
}

#[test]
fn output_names_never_overwrite() {
    let dir = temp_dir("names");
    let source = dir.join("cat.jpg");
    assert_eq!(output_path(&source, &dir, "png"), dir.join("cat-nobg.png"));
    std::fs::write(dir.join("cat-nobg.png"), b"x").unwrap();
    assert_eq!(
        output_path(&source, &dir, "png"),
        dir.join("cat-nobg (1).png")
    );
}

#[test]
fn decode_rejects_non_images() {
    assert_eq!(decode(b"hello").unwrap_err().code, "bg.unsupported");
}

#[test]
fn requires_downloaded_model_and_valid_options() {
    let ctx = Ctx::default();
    let sessions = Sessions::default();
    assert_eq!(
        run(args(vec![]), &sessions, &ctx).unwrap_err().code,
        "bg.no_files"
    );
    let err = run(args(vec!["a.png".into()]), &sessions, &ctx).unwrap_err();
    assert_eq!(
        (err.code.as_str(), &err.params["id"]),
        ("resource.missing", &serde_json::json!("u2netp"))
    );

    let mut bad = args(vec!["a.png".into()]);
    bad.background = Some("nope".into());
    assert_eq!(
        run(bad, &sessions, &ctx).unwrap_err().code,
        "bg.invalid_color"
    );
    let mut missing = args(vec!["a.png".into()]);
    missing.output_dir = Some("/nope/dir".into());
    assert_eq!(
        run(missing, &sessions, &ctx).unwrap_err().code,
        "bg.output_dir_missing"
    );
}

/// 需要真实模型：`TOOLFORGE_U2NETP=/path/u2netp.onnx cargo test -p tfp-background-remover -- --ignored`
#[test]
#[ignore]
fn removes_background_with_real_model() {
    let Some(model) = std::env::var_os("TOOLFORGE_U2NETP").map(PathBuf::from) else {
        return;
    };
    let dir = temp_dir("real");
    // 浅色背景上的深色圆形
    let image = RgbImage::from_fn(200, 160, |x, y| {
        let (dx, dy) = (x as i32 - 100, y as i32 - 80);
        if dx * dx + dy * dy < 50 * 50 {
            Rgb([180, 30, 40])
        } else {
            Rgb([235, 235, 240])
        }
    });
    let source = dir.join("circle.png");
    image.save(&source).unwrap();
    let ctx = Ctx {
        model: Some(model),
        ..Ctx::default()
    };
    let sessions = Sessions::default();
    let report = run(
        args(vec![source.to_string_lossy().into_owned()]),
        &sessions,
        &ctx,
    )
    .unwrap();
    assert_eq!(report.succeeded, 1, "{:?}", report.files[0].error);
    let output = image::open(report.files[0].output.as_ref().unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(output.dimensions(), (200, 160));
    assert!(report.files[0].transparent);
    assert!(
        output.get_pixel(100, 80).0[3] > 200,
        "center should be opaque"
    );
    assert!(
        output.get_pixel(3, 3).0[3] < 60,
        "corner should be transparent"
    );
    assert!(
        ctx.logs
            .lock()
            .unwrap()
            .contains(&"bg.model_loaded".to_owned())
    );

    // 第二次复用已加载的模型
    run(
        args(vec![source.to_string_lossy().into_owned()]),
        &sessions,
        &ctx,
    )
    .unwrap();
    assert!(
        ctx.logs
            .lock()
            .unwrap()
            .contains(&"bg.model_reused".to_owned())
    );
}
