use std::io::{Cursor, Read};
use std::sync::Mutex;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
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
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tfp-compress-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_image(path: &Path, format: ImageFormat, quality_noise: bool) {
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(200, 120, |x, y| {
        let noise = if quality_noise {
            ((x * 7 + y * 13) % 11) as u8
        } else {
            0
        };
        Rgb([(x as u8).saturating_add(noise), y as u8, 90])
    }));
    let mut buffer = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut buffer), format)
        .unwrap();
    std::fs::write(path, buffer).unwrap();
}

fn args(paths: Vec<String>) -> Args {
    serde_json::from_value(serde_json::json!({ "paths": paths })).unwrap()
}

#[test]
fn compresses_next_to_the_source_without_overwriting() {
    let dir = workspace("batch");
    write_image(&dir.join("a.png"), ImageFormat::Png, true);
    write_image(&dir.join("b.JPEG"), ImageFormat::Jpeg, true);
    std::fs::write(dir.join("a-min.png"), b"existing").unwrap();
    std::fs::write(dir.join("broken.png"), b"\x89PNG\r\n\x1a\nbroken").unwrap();
    let paths = ["a.png", "b.JPEG", "broken.png", "missing.png"]
        .map(|n| dir.join(n).to_string_lossy().into_owned())
        .to_vec();
    let ctx = Ctx::default();
    let report = run(args(paths), &ctx).unwrap();

    assert_eq!(report.files.len(), 4);
    assert_eq!(report.succeeded, 2);
    let a = &report.files[0];
    assert!(
        a.output.as_deref().unwrap().ends_with("a-min (1).png"),
        "{a:?}"
    );
    assert!(a.after_bytes < a.before_bytes);
    assert_eq!(std::fs::read(dir.join("a-min.png")).unwrap(), b"existing");
    // 原扩展名大小写保持不变
    assert!(
        report.files[1]
            .output
            .as_deref()
            .unwrap()
            .ends_with("b-min.JPEG")
    );
    assert_eq!(
        report.files[2].error.as_ref().unwrap().code,
        "image.decode_failed"
    );
    assert_eq!(report.files[3].error.as_ref().unwrap().code, "fs.not_found");
    let logs = ctx.logs.lock().unwrap();
    assert_eq!(
        logs.iter()
            .filter(|c| c.as_str() == "image.file_failed")
            .count(),
        2
    );
    assert_eq!(logs.last().map(String::as_str), Some("image.summary"));
}

#[test]
fn keeps_the_original_when_compression_would_grow_it() {
    let dir = workspace("grow");
    // 已经高度压缩的小 JPEG，以更高质量重新编码只会更大
    let image = DynamicImage::ImageRgb8(RgbImage::from_fn(64, 64, |x, y| {
        Rgb([x as u8 * 4, y as u8 * 4, 0])
    }));
    let jpeg = engine::encode_jpeg(&image, 10, None, None).unwrap();
    let source = dir.join("tiny.jpg");
    std::fs::write(&source, &jpeg).unwrap();
    let mut args = args(vec![source.to_string_lossy().into_owned()]);
    args.options.quality = 100;
    let report = run(args, &Ctx::default()).unwrap();
    let file = &report.files[0];
    assert!(file.kept_original, "{file:?}");
    assert_eq!(std::fs::read(file.output.as_ref().unwrap()).unwrap(), jpeg);
}

#[test]
fn writes_to_an_output_folder_and_converts_formats() {
    let dir = workspace("out");
    let out = dir.join("out");
    std::fs::create_dir(&out).unwrap();
    write_image(&dir.join("photo.jpg"), ImageFormat::Jpeg, true);
    let mut args = args(vec![dir.join("photo.jpg").to_string_lossy().into_owned()]);
    args.output_dir = Some(out.to_string_lossy().into_owned());
    args.suffix = String::new();
    args.options.format = Format::Webp;
    let report = run(args, &Ctx::default()).unwrap();
    assert_eq!(
        PathBuf::from(report.files[0].output.as_ref().unwrap()),
        out.join("photo.webp")
    );
    assert_eq!(report.files[0].format, Some(Format::Webp));
}

#[test]
fn validates_arguments() {
    let ctx = Ctx::default();
    assert_eq!(run(args(vec![]), &ctx).unwrap_err().code, "image.no_files");
    let mut same_place = args(vec!["/x.png".into()]);
    same_place.suffix = " ".into();
    assert_eq!(
        run(same_place, &ctx).unwrap_err().code,
        "image.suffix_required"
    );
    let mut missing = args(vec!["/x.png".into()]);
    missing.output_dir = Some("/definitely/not/here".into());
    assert_eq!(
        run(missing, &ctx).unwrap_err().code,
        "image.output_dir_missing"
    );
}

#[test]
fn scans_folders_for_images() {
    let dir = workspace("scan");
    std::fs::create_dir_all(dir.join("nested/.hidden")).unwrap();
    for name in [
        "a.PNG",
        "nested/b.webp",
        "nested/c.jpeg",
        "nested/.hidden/d.png",
        "notes.txt",
        ".e.jpg",
    ] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    let names: Vec<String> = scan(&[dir.to_string_lossy().into_owned()])
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["a.PNG", "b.webp", "c.jpeg"]);
    let single = scan(&[
        dir.join("a.PNG").to_string_lossy().into_owned(),
        "/nope".into(),
    ]);
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].size, 1);
}
