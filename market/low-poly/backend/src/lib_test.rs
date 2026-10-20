use super::*;
use image::Rgba;

fn png_data(width: u32, height: u32) -> String {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x * 3 % 256) as u8, (y * 5 % 256) as u8, 128, 255])
    });
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    format!("data:image/png;base64,{}", BASE64.encode(out.get_ref()))
}

#[test]
fn prepare_scales_long_edge_to_work_size() {
    let out = prepare(
        PrepareArgs {
            data: Some(png_data(3200, 1600)),
            path: None,
        },
        None,
    )
    .unwrap();
    assert_eq!((out.width, out.height), (1600, 800));
    assert!(out.work_data.starts_with("data:image/png;base64,"));
}

#[test]
fn prepare_keeps_small_images() {
    let out = prepare(
        PrepareArgs {
            data: Some(png_data(64, 48)),
            path: None,
        },
        None,
    )
    .unwrap();
    assert_eq!((out.width, out.height), (64, 48));
}

#[test]
fn generate_includes_corners_and_remesh_clamps() {
    let data = png_data(160, 120);
    let mesh = generate(
        GenerateArgs {
            image: ImageInput {
                data: Some(data.clone()),
                path: None,
            },
            seed: 5,
            count: 40,
            detail: 50,
        },
        None,
    )
    .unwrap();
    // 锚点：左上与右上（坐标缩到工作图尺寸）
    assert!(mesh.points.iter().any(|p| p[0] == 0.0 && p[1] == 0.0));
    assert!(mesh.points.iter().any(|p| p[0] == 159.0 && p[1] == 0.0));
    assert!(!mesh.triangles.is_empty());

    // 越界点被钳制回画布
    let out = remesh(
        RemeshArgs {
            image: ImageInput {
                data: Some(data),
                path: None,
            },
            points: vec![[0.0, 0.0], [500.0, 0.0], [159.0, 119.0], [0.0, 119.0]],
            brightness: 0,
        },
        None,
    )
    .unwrap();
    assert!(out.points.iter().all(|p| p[0] <= 159.0 && p[1] <= 119.0));
    assert_eq!(out.colors.len(), out.triangles.len());
    assert!(
        out.colors
            .iter()
            .all(|c| c.starts_with('#') && c.len() == 7)
    );
}

#[test]
fn rejects_bad_input() {
    // 无来源
    let error = prepare(
        PrepareArgs {
            data: None,
            path: None,
        },
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, "lowpoly.source_required");
    // 坏 base64
    let error = prepare(
        PrepareArgs {
            data: Some("!!!not-base64!!!".to_owned()),
            path: None,
        },
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, "lowpoly.bad_data");
    // 全共线点集
    let error = remesh(
        RemeshArgs {
            image: ImageInput {
                data: Some(png_data(64, 48)),
                path: None,
            },
            points: vec![[0.0, 0.0], [63.0, 47.0]],
            brightness: 0,
        },
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, "lowpoly.degenerate");
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tfm-lowpoly-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 可取消的任务上下文
struct Ctx {
    cancelled: std::sync::atomic::AtomicBool,
}

impl Ctx {
    fn new() -> Self {
        Self {
            cancelled: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl TaskContext for Ctx {
    fn log(&self, _: tf_plugin_api::LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
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
fn export_writes_png_and_svg() {
    let data = png_data(160, 120);
    let dir = temp_dir("export");
    for format in [Format::Png, Format::Svg] {
        let output = dir.join(match format {
            Format::Png => "out.png",
            Format::Svg => "out.svg",
        });
        let report = export(
            ExportArgs {
                image: ImageInput {
                    data: Some(data.clone()),
                    path: None,
                },
                points: vec![
                    [0.0, 0.0],
                    [159.0, 0.0],
                    [159.0, 119.0],
                    [0.0, 119.0],
                    [80.0, 60.0],
                ],
                scale: 2.0,
                brightness: 0,
                stroke_width: 2,
                stroke_color: "111111".to_owned(),
                background: "ffffff".to_owned(),
                format,
                output_path: output.to_string_lossy().into_owned(),
            },
            &Ctx::new(),
        )
        .unwrap();
        assert_eq!((report.width, report.height), (320, 240));
        assert!(report.triangles >= 3);
        let bytes = std::fs::read(&output).unwrap();
        match format {
            Format::Png => assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G'])),
            Format::Svg => {
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.contains("<polygon"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_rejects_bad_scale_and_cancellation() {
    let data = png_data(64, 48);
    let dir = temp_dir("bad");
    let error = export(
        ExportArgs {
            image: ImageInput {
                data: Some(data.clone()),
                path: None,
            },
            points: vec![[0.0, 0.0], [63.0, 0.0], [0.0, 47.0]],
            scale: 9.0,
            brightness: 0,
            stroke_width: 0,
            stroke_color: "000000".to_owned(),
            background: "ffffff".to_owned(),
            format: Format::Png,
            output_path: dir.join("x.png").to_string_lossy().into_owned(),
        },
        &Ctx::new(),
    )
    .unwrap_err();
    assert_eq!(error.code, "lowpoly.bad_scale");

    let ctx = Ctx::new();
    ctx.cancelled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let error = export(
        ExportArgs {
            image: ImageInput {
                data: Some(data),
                path: None,
            },
            points: vec![[0.0, 0.0], [63.0, 0.0], [0.0, 47.0]],
            scale: 1.0,
            brightness: 0,
            stroke_width: 0,
            stroke_color: "000000".to_owned(),
            background: "ffffff".to_owned(),
            format: Format::Png,
            output_path: dir.join("x.png").to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap_err();
    assert_eq!(error.code, tf_plugin_api::cancelled().code);
    let _ = std::fs::remove_dir_all(&dir);
}
