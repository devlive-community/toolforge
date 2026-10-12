use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::RgbaImage;
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
    let dir = std::env::temp_dir().join(format!("tfp-stitch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 生成一张纯色（可带透明度）的 PNG
fn sample_png(dir: &Path, name: &str, width: u32, height: u32, color: [u8; 4]) -> String {
    let image = RgbaImage::from_pixel(width, height, image::Rgba(color));
    let path = dir.join(name);
    image.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn base_args(paths: Vec<String>, dir: &Path) -> Args {
    Args {
        paths,
        layout: Layout::Horizontal,
        align: Align::Center,
        columns: 2,
        gap: 0,
        background: Background::Transparent,
        format: Format::Png,
        quality: 85,
        output_dir: Some(dir.to_string_lossy().into_owned()),
        name: Some("stitched".to_owned()),
    }
}

fn sizes(widths: &[(u32, u32)]) -> Vec<(u32, u32)> {
    widths.to_vec()
}

#[test]
fn lays_out_horizontally_with_alignment() {
    let mut args = base_args(vec![], Path::new("/tmp"));
    args.layout = Layout::Horizontal;
    args.gap = 5;

    args.align = Align::Center;
    let canvas = layout(&sizes(&[(20, 10), (10, 20)]), &args).unwrap();
    assert_eq!((canvas.width, canvas.height), (35, 20));
    assert_eq!(canvas.placements, vec![(0, 5), (25, 0)]);

    args.align = Align::Start;
    let canvas = layout(&sizes(&[(20, 10), (10, 20)]), &args).unwrap();
    assert_eq!(canvas.placements, vec![(0, 0), (25, 0)]);

    args.align = Align::End;
    let canvas = layout(&sizes(&[(20, 10), (10, 20)]), &args).unwrap();
    assert_eq!(canvas.placements, vec![(0, 10), (25, 0)]);
}

#[test]
fn lays_out_vertically_with_alignment() {
    let mut args = base_args(vec![], Path::new("/tmp"));
    args.layout = Layout::Vertical;
    args.gap = 5;
    args.align = Align::Center;

    let canvas = layout(&sizes(&[(20, 10), (10, 20)]), &args).unwrap();
    assert_eq!((canvas.width, canvas.height), (20, 35));
    assert_eq!(canvas.placements, vec![(0, 0), (5, 15)]);

    args.align = Align::End;
    let canvas = layout(&sizes(&[(20, 10), (10, 20)]), &args).unwrap();
    // 第一张图就是全宽，右对齐后仍在原位；第二张靠右
    assert_eq!(canvas.placements, vec![(0, 0), (10, 15)]);
}

#[test]
fn lays_out_a_grid_with_centered_cells() {
    let mut args = base_args(vec![], Path::new("/tmp"));
    args.layout = Layout::Grid;
    args.columns = 2;
    args.gap = 4;

    // 3 张图 2 列：第一行两张（最高 30），第二行一张
    let canvas = layout(&sizes(&[(20, 10), (10, 30), (15, 12)]), &args).unwrap();
    assert_eq!((canvas.width, canvas.height), (34, 46));
    assert_eq!(canvas.placements, vec![(0, 10), (24, 0), (2, 34)]);
}

#[test]
fn rejects_oversized_canvases() {
    let mut args = base_args(vec![], Path::new("/tmp"));
    args.gap = 1000;
    let err = layout(&sizes(&[(5000, 5000); 100]), &args).unwrap_err();
    assert_eq!(err.code, "stitch.pixels_too_large");
}

#[test]
fn parses_colors_and_rejects_bad_hex() {
    assert_eq!(
        Background::Color {
            hex: "#ff8800".into()
        }
        .rgba()
        .unwrap(),
        [255, 136, 0, 255]
    );
    assert_eq!(
        Background::Color {
            hex: "33669988".into()
        }
        .rgba()
        .unwrap(),
        [51, 102, 153, 136]
    );
    assert_eq!(Background::Transparent.rgba().unwrap(), [0, 0, 0, 0]);
    assert_eq!(
        Background::Color {
            hex: "#12345".into()
        }
        .rgba()
        .unwrap_err()
        .code,
        "stitch.invalid_color"
    );
}

#[test]
fn output_path_avoids_collisions() {
    let dir = temp_dir("output");
    let first = output_path(&dir, "stitched", "png");
    assert_eq!(first, dir.join("stitched.png"));
    std::fs::write(&first, b"x").unwrap();
    assert_eq!(
        output_path(&dir, "stitched", "png"),
        dir.join("stitched (1).png")
    );
    // 名字里的扩展名会被剥掉
    assert_eq!(
        output_path(&dir, "result.png", "jpg"),
        dir.join("result.jpg")
    );
    // 空名字回退为 stitched（用新目录避免撞上已写入的文件）
    let fresh = temp_dir("output-fresh");
    assert_eq!(output_path(&fresh, "", "png"), fresh.join("stitched.png"));
}

#[test]
fn stitches_two_pngs_horizontally_keeping_transparency() {
    let dir = temp_dir("h");
    let red = sample_png(&dir, "red.png", 10, 20, [255, 0, 0, 255]);
    let blue = sample_png(&dir, "blue.png", 10, 20, [0, 0, 255, 128]);
    let ctx = Ctx::default();
    let report = run(base_args(vec![red, blue], &dir), &ctx).unwrap();
    assert_eq!((report.width, report.height), (20, 20));

    let image = image::open(&report.output).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (20, 20));
    // 左边不透明红、右边半透明蓝（透明背景不填充）
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(19, 0).0, [0, 0, 255, 128]);
}

#[test]
fn stitches_a_grid_with_gap_and_background() {
    let dir = temp_dir("grid");
    let a = sample_png(&dir, "a.png", 10, 10, [255, 0, 0, 255]);
    let b = sample_png(&dir, "b.png", 10, 10, [0, 255, 0, 255]);
    let c = sample_png(&dir, "c.png", 10, 10, [0, 0, 255, 255]);
    let d = sample_png(&dir, "d.png", 10, 10, [255, 255, 0, 255]);
    let mut args = base_args(vec![a, b, c, d], &dir);
    args.layout = Layout::Grid;
    args.columns = 2;
    args.gap = 10;
    args.background = Background::Color {
        hex: "#101010".into(),
    };
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    assert_eq!((report.width, report.height), (30, 30));

    let image = image::open(&report.output).unwrap().into_rgba8();
    // 间距处是背景色
    assert_eq!(image.get_pixel(10, 0).0, [16, 16, 16, 255]);
    assert_eq!(image.get_pixel(0, 10).0, [16, 16, 16, 255]);
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
}

#[test]
fn stitches_vertically_onto_white_for_jpeg() {
    let dir = temp_dir("jpeg");
    let red = sample_png(&dir, "red.png", 10, 10, [255, 0, 0, 255]);
    let transparent = sample_png(&dir, "t.png", 10, 10, [0, 0, 0, 0]);
    let mut args = base_args(vec![red, transparent], &dir);
    args.layout = Layout::Vertical;
    args.format = Format::Jpeg;
    args.quality = 90;
    args.background = Background::Transparent; // JPEG 输出应回退为白底
    args.gap = 5;
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    assert!(report.output.ends_with(".jpg"));

    let image = image::open(&report.output).unwrap().into_rgb8();
    assert_eq!(image.dimensions(), (10, 25));
    // 第二张图全透明，落在白底上
    assert_eq!(image.get_pixel(0, 20).0, [255, 255, 255]);
}

#[test]
fn writes_webp_output() {
    let dir = temp_dir("webp");
    let a = sample_png(&dir, "a.png", 10, 10, [255, 0, 0, 255]);
    let b = sample_png(&dir, "b.png", 10, 10, [0, 0, 255, 255]);
    let mut args = base_args(vec![a, b], &dir);
    args.format = Format::Webp;
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    assert!(report.output.ends_with(".webp"));
    assert!(report.thumbnail.starts_with("data:image/png;base64,"));
}

#[test]
fn fails_the_task_when_a_file_is_broken() {
    let dir = temp_dir("broken");
    let good = sample_png(&dir, "good.png", 10, 10, [255, 0, 0, 255]);
    let bad = dir.join("bad.png");
    std::fs::write(&bad, b"not an image").unwrap();
    let ctx = Ctx::default();
    let err = run(
        base_args(vec![good, bad.to_string_lossy().into_owned()], &dir),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, "stitch.decode_failed");
}

#[test]
fn refuses_too_few_images_and_missing_output_dir() {
    let dir = temp_dir("few");
    let a = sample_png(&dir, "a.png", 10, 10, [255, 0, 0, 255]);
    let ctx = Ctx::default();
    assert_eq!(
        run(base_args(vec![a.clone()], &dir), &ctx)
            .unwrap_err()
            .code,
        "stitch.too_few"
    );

    let mut args = base_args(vec![a.clone(), a], &dir);
    args.output_dir = Some(dir.join("nowhere").to_string_lossy().into_owned());
    assert_eq!(
        run(args, &ctx).unwrap_err().code,
        "stitch.output_dir_missing"
    );
}

#[test]
fn outputs_next_to_the_first_image_by_default() {
    let dir = temp_dir("default-dir");
    let a = sample_png(&dir, "a.png", 10, 10, [255, 0, 0, 255]);
    let b = sample_png(&dir, "b.png", 10, 10, [0, 0, 255, 255]);
    let mut args = base_args(vec![a, b], &dir);
    args.output_dir = None;
    args.name = None;
    let ctx = Ctx::default();
    let report = run(args, &ctx).unwrap();
    assert_eq!(Path::new(&report.output).parent(), Some(dir.as_path()));
    assert!(report.output.contains("stitched.png"));
}

#[test]
fn reports_progress_and_logs() {
    let dir = temp_dir("logs");
    let a = sample_png(&dir, "a.png", 10, 10, [255, 0, 0, 255]);
    let b = sample_png(&dir, "b.png", 10, 10, [0, 0, 255, 255]);
    let ctx = Ctx::default();
    run(base_args(vec![a, b], &dir), &ctx).unwrap();
    let logs = ctx.logs.lock().unwrap();
    assert!(logs.contains(&"stitch.image_loaded".to_owned()));
    assert!(logs.contains(&"stitch.done".to_owned()));
    let progress = ctx.progress.lock().unwrap();
    assert_eq!(progress.last(), Some(&(4, 4)));
}
