use std::sync::Mutex;

use image::{Rgba, RgbaImage};
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
    let dir = std::env::temp_dir().join(format!("tfp-image-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 生成一张带半透明像素的 PNG 测试图
fn sample_png(dir: &Path, width: u32, height: u32) -> String {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([
            (x * 7) as u8,
            (y * 5) as u8,
            120,
            if x < 4 { 0 } else { 255 },
        ])
    });
    let path = dir.join("sample.png");
    image.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn args(paths: Vec<String>, target: Target) -> Args {
    Args {
        paths,
        output_dir: None,
        target,
        quality: 80,
        resize: Resize::None,
        suffix: String::new(),
    }
}

#[test]
fn target_size_modes() {
    assert_eq!(target_size(400, 200, Resize::None), (400, 200));
    assert_eq!(
        target_size(400, 200, Resize::Scale { percent: 50 }),
        (200, 100)
    );
    assert_eq!(
        target_size(
            400,
            200,
            Resize::Fit {
                max_width: 100,
                max_height: 0
            }
        ),
        (100, 50)
    );
    // Fit 不放大
    assert_eq!(
        target_size(
            40,
            20,
            Resize::Fit {
                max_width: 100,
                max_height: 100
            }
        ),
        (40, 20)
    );
    assert_eq!(target_size(3, 3, Resize::Scale { percent: 1 }), (1, 1));
}

#[test]
fn output_path_never_overwrites() {
    let dir = temp_dir("names");
    let source = dir.join("a.png");
    std::fs::write(&source, b"x").unwrap();
    // 同目录同扩展名时不能覆盖源文件
    assert_eq!(output_path(&source, &dir, "", "png"), dir.join("a (1).png"));
    assert_eq!(
        output_path(&source, &dir, "-min", "jpg"),
        dir.join("a-min.jpg")
    );
    std::fs::write(dir.join("a-min.jpg"), b"x").unwrap();
    assert_eq!(
        output_path(&source, &dir, "-min", "jpg"),
        dir.join("a-min (1).jpg")
    );
}

#[test]
fn converts_png_to_resized_jpeg() {
    let dir = temp_dir("jpeg");
    let source = sample_png(&dir, 40, 20);
    let ctx = Ctx::default();
    let mut args = args(vec![source], Target::Jpeg);
    args.resize = Resize::Scale { percent: 50 };
    args.suffix = "-small".into();
    let report = run(args, &ctx).unwrap();

    assert_eq!(report.succeeded, 1);
    let file = &report.files[0];
    assert_eq!((file.width, file.height), (40, 20));
    assert_eq!((file.new_width, file.new_height), (20, 10));
    assert_eq!(file.format, Some(Target::Jpeg));
    let output = file.output.as_deref().unwrap();
    assert!(output.ends_with("sample-small.jpg"));
    let (decoded, format) = decode(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(format, ImageFormat::Jpeg);
    assert_eq!((decoded.width(), decoded.height()), (20, 10));
    assert!(
        file.thumbnail
            .as_deref()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert_eq!(*ctx.progress.lock().unwrap(), vec![(1, 1)]);
    assert_eq!(
        *ctx.logs.lock().unwrap(),
        vec!["image.file_start", "image.file_done"]
    );
}

#[test]
fn every_target_round_trips() {
    let dir = temp_dir("targets");
    let source = sample_png(&dir, 12, 8);
    let (image, _) = decode(&std::fs::read(&source).unwrap()).unwrap();
    for target in [
        Target::Png,
        Target::Jpeg,
        Target::Webp,
        Target::Gif,
        Target::Bmp,
        Target::Ico,
        Target::Tiff,
    ] {
        let bytes = encode(&image, target, 90).unwrap();
        let (decoded, format) = decode(&bytes).unwrap();
        assert_eq!(format, target.format(), "{target:?}");
        assert_eq!((decoded.width(), decoded.height()), (12, 8), "{target:?}");
    }
}

#[test]
fn keep_format_and_ico_is_fitted() {
    let dir = temp_dir("keep");
    let source = sample_png(&dir, 300, 150);
    let ctx = Ctx::default();
    let report = run(args(vec![source.clone()], Target::Keep), &ctx).unwrap();
    assert_eq!(report.files[0].format, Some(Target::Png));
    assert!(
        report.files[0]
            .output
            .as_deref()
            .unwrap()
            .ends_with("sample (1).png")
    );

    let report = run(args(vec![source], Target::Ico), &ctx).unwrap();
    let file = &report.files[0];
    assert_eq!((file.new_width, file.new_height), (256, 128));
    assert!(
        ctx.logs
            .lock()
            .unwrap()
            .contains(&"image.ico_fitted".to_owned())
    );
}

#[test]
fn failures_are_per_file() {
    let dir = temp_dir("fail");
    let good = sample_png(&dir, 4, 4);
    let bad = dir.join("broken.png");
    std::fs::write(&bad, b"not an image").unwrap();
    let ctx = Ctx::default();
    let report = run(
        args(
            vec![
                bad.to_string_lossy().into_owned(),
                "/nope/x.png".into(),
                good,
            ],
            Target::Webp,
        ),
        &ctx,
    )
    .unwrap();
    assert_eq!(report.succeeded, 1);
    let codes: Vec<_> = report
        .files
        .iter()
        .map(|f| f.error.as_ref().map(|e| e.code.as_str()))
        .collect();
    assert_eq!(
        codes,
        vec![Some("image.unsupported"), Some("fs.not_found"), None]
    );
}

#[test]
fn validates_arguments() {
    let ctx = Ctx::default();
    assert_eq!(
        run(args(vec![], Target::Png), &ctx).unwrap_err().code,
        "image.no_files"
    );
    let mut missing = args(vec!["x.png".into()], Target::Png);
    missing.output_dir = Some("/nope/dir".into());
    assert_eq!(
        run(missing, &ctx).unwrap_err().code,
        "image.output_dir_missing"
    );
}

#[test]
fn jpeg_flattens_transparency_onto_white() {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0])));
    assert_eq!(flatten(&image).get_pixel(0, 0).0, [255, 255, 255]);
}
