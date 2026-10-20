use super::*;
use std::io::Read;

fn view(width: u32, height: u32) -> ViewArgs {
    ViewArgs {
        fractal: KindArg::Mandelbrot,
        center_x: -0.6,
        center_y: 0.0,
        span_x: 3.2,
        julia_x: 0.0,
        julia_y: 0.0,
        max_iter: 128,
        palette: palette::SPECTRUM.to_owned(),
        brightness: palette::Brightness::Normal,
        width,
        height,
    }
}

#[test]
fn preview_returns_png_data_uri_and_stats() {
    let out = preview(view(120, 80)).unwrap();
    assert!(out.image.starts_with("data:image/png;base64,"));
    assert_eq!((out.stats.width, out.stats.height), (120, 80));
    assert_eq!(out.stats.max_iter, 128);
    assert!(!out.stats.precision_limited);
    // 主心形占比明显大于 0，但不是全图
    assert!(out.stats.interior_ratio > 0.05);
    assert!(out.stats.interior_ratio < 0.9);
}

#[test]
fn preview_works_for_julia() {
    let mut args = view(100, 100);
    args.fractal = KindArg::Julia;
    args.julia_x = -0.8;
    args.julia_y = 0.156;
    args.center_x = 0.0;
    args.center_y = 0.0;
    args.span_x = 3.0;
    let out = preview(args).unwrap();
    assert!(out.stats.interior_ratio > 0.0);
}

#[test]
fn rejects_invalid_args() {
    // span 非正
    let mut args = view(60, 40);
    args.span_x = 0.0;
    let error = preview(args).unwrap_err();
    assert_eq!(error.code, "fractal.bad_span");
    // 迭代越界
    let mut args = view(60, 40);
    args.max_iter = 0;
    let error = preview(args).unwrap_err();
    assert_eq!(error.code, "fractal.bad_iter");
    let mut args = view(60, 40);
    args.max_iter = render::MAX_ITER_LIMIT + 1;
    let error = preview(args).unwrap_err();
    assert_eq!(error.code, "fractal.bad_iter");
    // 调色板不在白名单
    let mut args = view(60, 40);
    args.palette = "rainbow".to_owned();
    let error = preview(args).unwrap_err();
    assert_eq!(error.code, "fractal.bad_palette");
    // Julia 参数超界
    let mut args = view(60, 40);
    args.fractal = KindArg::Julia;
    args.julia_x = 5.0;
    let error = preview(args).unwrap_err();
    assert_eq!(error.code, "fractal.bad_julia");
    // 像素预算超限（1600×1600 > 2.5 MP）
    let error = preview(view(1600, 1600)).unwrap_err();
    assert_eq!(error.code, "fractal.pixels_too_large");
}

#[test]
fn preview_marks_precision_limit() {
    let mut args = view(80, 60);
    args.span_x = fractal::PRECISION_LIMIT / 10.0;
    let out = preview(args).unwrap();
    assert!(out.stats.precision_limited);
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tfm-fractal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 可取消的任务上下文
struct Ctx {
    cancelled: std::sync::atomic::AtomicBool,
    rows: std::sync::atomic::AtomicU64,
}

impl Ctx {
    fn new() -> Self {
        Self {
            cancelled: std::sync::atomic::AtomicBool::new(false),
            rows: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl TaskContext for Ctx {
    fn log(&self, _: tf_plugin_api::LogLevel, _: &str, _: Value) {}
    fn progress(&self, done: u64, _: u64) {
        self.rows.store(done, std::sync::atomic::Ordering::SeqCst);
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
}

#[test]
fn export_writes_png_file() {
    let dir = temp_dir("export");
    let output = dir.join("mandelbrot.png");
    let report = export(
        ExportArgs {
            view: view(160, 120),
            output_path: output.to_string_lossy().into_owned(),
        },
        &Ctx::new(),
    )
    .unwrap();
    assert_eq!((report.width, report.height), (160, 120));
    assert_eq!(report.output, output.to_string_lossy());
    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_reports_progress_and_cancels_cleanly() {
    let dir = temp_dir("cancel");
    let output = dir.join("x.png");
    let ctx = Ctx::new();
    let report = export(
        ExportArgs {
            view: view(120, 90),
            output_path: output.to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap();
    // 逐行进度回报到最后一行
    assert_eq!(ctx.rows.load(std::sync::atomic::Ordering::SeqCst), 90);
    assert_eq!(report.width, 120);

    // 预先取消：不落盘
    std::fs::remove_file(&output).unwrap();
    let ctx = Ctx::new();
    ctx.cancelled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let error = export(
        ExportArgs {
            view: view(120, 90),
            output_path: output.to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap_err();
    assert_eq!(error.code, tf_plugin_api::cancelled().code);
    assert!(!output.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_requires_output_path() {
    let error = export(
        ExportArgs {
            view: view(60, 40),
            output_path: "   ".to_owned(),
        },
        &Ctx::new(),
    )
    .unwrap_err();
    assert_eq!(error.code, "fractal.output_required");
}
