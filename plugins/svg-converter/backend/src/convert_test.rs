use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    progress: Mutex<Vec<(u64, u64)>>,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, done: u64, total: u64) {
        self.progress.lock().unwrap().push((done, total));
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

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tfp-svg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 一个 20×10 的绿色方块 SVG
fn sample_svg(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##).unwrap();
    path.to_string_lossy().into_owned()
}

/// 一个半透明红色方块 SVG（左半透明、右半不透明）
fn alpha_svg(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    std::fs::write(
        &path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="10" height="10" fill="#ff0000" fill-opacity="0"/><rect x="10" width="10" height="10" fill="#ff0000"/></svg>"##,
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

fn base_args(paths: Vec<String>, dir: &Path) -> Args {
    Args {
        paths,
        output_dir: Some(dir.to_string_lossy().into_owned()),
        format: Format::Png,
        quality: 85,
        resize: Resize::None,
        suffix: String::new(),
    }
}

#[test]
fn target_size_scales_and_fits() {
    assert_eq!(target_size(20, 10, Resize::None), (20, 10));
    assert_eq!(
        target_size(20, 10, Resize::Scale { percent: 250 }),
        (50, 25)
    );
    // fit 允许放大（矢量不失真），0 表示该方向不限制
    assert_eq!(
        target_size(
            20,
            10,
            Resize::Fit {
                max_width: 200,
                max_height: 0
            }
        ),
        (200, 100)
    );
    assert_eq!(
        target_size(
            20,
            10,
            Resize::Fit {
                max_width: 0,
                max_height: 100
            }
        ),
        (200, 100)
    );
    // 两个方向都受限时取更紧的比例
    assert_eq!(
        target_size(
            20,
            10,
            Resize::Fit {
                max_width: 30,
                max_height: 100
            }
        ),
        (30, 15)
    );
    // 宽高都为 0 视为不缩放
    assert_eq!(
        target_size(
            20,
            10,
            Resize::Fit {
                max_width: 0,
                max_height: 0
            }
        ),
        (20, 10)
    );
    assert_eq!(target_size(20, 10, Resize::Scale { percent: 0 }), (1, 1));
}

#[test]
fn output_path_never_overwrites() {
    let dir = temp_dir("output-path");
    let source = dir.join("icon.svg");
    std::fs::write(&source, "<svg/>").unwrap();
    let first = output_path(&source, &dir, "", "png");
    assert_eq!(first, dir.join("icon.png"));
    std::fs::write(&first, b"x").unwrap();
    let second = output_path(&source, &dir, "", "png");
    assert_eq!(second, dir.join("icon (1).png"));
    std::fs::write(&second, b"x").unwrap();
    let third = output_path(&source, &dir, "", "png");
    assert_eq!(third, dir.join("icon (2).png"));
}

#[test]
fn converts_to_png_keeping_transparency() {
    let dir = temp_dir("png");
    let svg = alpha_svg(&dir, "alpha.svg");
    let ctx = Ctx::default();
    let report = run(base_args(vec![svg.clone()], &dir), &ctx).unwrap();
    assert_eq!(report.succeeded, 1);
    let output = report.files[0].output.as_deref().unwrap();
    let image = image::open(output).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (20, 10));
    // 左半透明、右半不透明红色
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(image.get_pixel(19, 0).0, [255, 0, 0, 255]);
}

#[test]
fn converts_to_jpeg_on_white() {
    let dir = temp_dir("jpeg");
    let svg = alpha_svg(&dir, "alpha.svg");
    let mut args = base_args(vec![svg], &dir);
    args.format = Format::Jpeg;
    args.quality = 90;
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    let output = report.files[0].output.as_deref().unwrap();
    assert!(output.ends_with(".jpg"));
    let image = image::open(output).unwrap().into_rgb8();
    // 透明区合成到白底
    assert_eq!(image.get_pixel(0, 0).0, [255, 255, 255]);
}

#[test]
fn converts_to_webp() {
    let dir = temp_dir("webp");
    let svg = sample_svg(&dir, "sample.svg");
    let mut args = base_args(vec![svg], &dir);
    args.format = Format::Webp;
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    let output = report.files[0].output.as_deref().unwrap();
    assert!(output.ends_with(".webp"));
    let image = image::open(output).unwrap().into_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [0, 255, 0, 255]);
}

#[test]
fn applies_the_requested_resize() {
    let dir = temp_dir("resize");
    let svg = sample_svg(&dir, "sample.svg");
    let mut args = base_args(vec![svg], &dir);
    args.resize = Resize::Fit {
        max_width: 200,
        max_height: 0,
    };
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    let file = &report.files[0];
    assert_eq!((file.new_width, file.new_height), (200, 100));
}

#[test]
fn rejects_oversized_outputs() {
    let dir = temp_dir("pixels");
    let svg = sample_svg(&dir, "sample.svg");
    let mut args = base_args(vec![svg], &dir);
    args.resize = Resize::Fit {
        max_width: 200_000,
        max_height: 0,
    };
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    let error = report.files[0].error.as_ref().unwrap();
    assert_eq!(error.code, "svg.pixels_too_large");
}

#[test]
fn reports_broken_files_without_stopping_the_batch() {
    let dir = temp_dir("broken");
    let good = sample_svg(&dir, "good.svg");
    let bad = dir.join("bad.svg");
    std::fs::write(&bad, "definitely not svg").unwrap();
    let ctx = Ctx::default();
    let report = run(
        base_args(vec![bad.to_string_lossy().into_owned(), good], &dir),
        &ctx,
    )
    .unwrap();
    assert_eq!(report.succeeded, 1);
    assert_eq!(report.files[0].error.as_ref().unwrap().code, "svg.invalid");
    assert!(report.files[1].output.is_some());
}

#[test]
fn refuses_an_empty_batch_and_missing_output_dir() {
    let dir = temp_dir("empty");
    let ctx = Ctx::default();
    assert_eq!(
        run(base_args(vec![], &dir), &ctx).unwrap_err().code,
        "svg.no_files"
    );

    let svg = sample_svg(&dir, "sample.svg");
    let mut args = base_args(vec![svg], &dir);
    args.output_dir = Some(dir.join("nowhere").to_string_lossy().into_owned());
    assert_eq!(run(args, &ctx).unwrap_err().code, "svg.output_dir_missing");
}

#[test]
fn writes_output_next_to_the_source_by_default() {
    let dir = temp_dir("default-dir");
    let svg = sample_svg(&dir, "sample.svg");
    let mut args = base_args(vec![svg], &dir);
    args.output_dir = None;
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    let output = report.files[0].output.as_deref().unwrap();
    assert_eq!(Path::new(output).parent(), Some(dir.as_path()));
}

#[test]
fn logs_progress_per_file() {
    let dir = temp_dir("logs");
    let svg = sample_svg(&dir, "sample.svg");
    let ctx = Ctx::default();
    run(base_args(vec![svg], &dir), &ctx).unwrap();
    let logs = ctx.logs.lock().unwrap();
    assert!(logs.contains(&"svg.file_start".to_owned()));
    assert!(logs.contains(&"svg.file_done".to_owned()));
    assert_eq!(ctx.progress.lock().unwrap().last(), Some(&(1, 1)));
}
