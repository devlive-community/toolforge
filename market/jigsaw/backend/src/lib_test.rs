use super::*;
use data_encoding::BASE64;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::{Cursor, Read};

fn png_data(width: u32, height: u32) -> String {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x * 5 % 256) as u8, (y * 9 % 256) as u8, 130, 255])
    });
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    format!("data:image/png;base64,{}", BASE64.encode(out.get_ref()))
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tfm-jigsaw-{}-{name}", std::process::id()));
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

fn game_args(data: &str, crop: (u32, u32, u32, u32), cols: u32, rows: u32) -> NewGameArgs {
    NewGameArgs {
        work_data: data.to_owned(),
        crop_x: crop.0,
        crop_y: crop.1,
        crop_w: crop.2,
        crop_h: crop.3,
        cols,
        rows,
        seed: 7,
    }
}

#[test]
fn new_game_builds_consistent_setup() {
    let setup = new_game(
        game_args(&png_data(400, 300), (0, 0, 400, 300), 4, 4),
        &Ctx::new(),
    )
    .unwrap();
    assert_eq!((setup.width, setup.height), (400, 300));
    assert_eq!((setup.cols, setup.rows), (4, 4));
    assert_eq!(setup.edges.eh.len(), 3);
    assert_eq!(setup.edges.ev.len(), 4);
    assert_eq!(setup.edges.ev[0].len(), 3);
    assert_eq!(setup.tray.len(), 16);
    // 散落位置在画板外围环带
    for [x, y] in &setup.tray {
        let outside = *x < 0.0 || *x > 1.0 || *y < 0.0 || *y > 1.0;
        assert!(
            outside,
            "tray position must start outside the board: {x},{y}"
        );
    }
    // 格位尺寸除得尽画板
    assert!((setup.cell_w * 4.0 - f64::from(setup.width)).abs() < 1e-6);
}

#[test]
fn new_game_crops_and_validates() {
    // 裁剪一半
    let setup = new_game(
        game_args(&png_data(400, 300), (100, 75, 200, 150), 3, 3),
        &Ctx::new(),
    )
    .unwrap();
    assert_eq!((setup.width, setup.height), (200, 150));
    // 规格不在白名单
    let error = new_game(
        game_args(&png_data(400, 300), (0, 0, 400, 300), 5, 5),
        &Ctx::new(),
    )
    .unwrap_err();
    assert_eq!(error.code, "jigsaw.bad_grid");
    // 裁剪范围过小
    let error = new_game(
        game_args(&png_data(400, 300), (0, 0, 60, 60), 3, 3),
        &Ctx::new(),
    )
    .unwrap_err();
    assert_eq!(error.code, "jigsaw.crop_too_small");
}

#[test]
fn new_game_is_cancellable() {
    let ctx = Ctx::new();
    ctx.cancelled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let error = new_game(game_args(&png_data(400, 300), (0, 0, 400, 300), 3, 3), &ctx).unwrap_err();
    assert_eq!(error.code, tf_plugin_api::cancelled().code);
}

#[test]
fn verify_rejects_fill_without_identity() {
    // 每片都在正确位置且恰好一次 → 通过
    let placements: Vec<[f64; 5]> = (0..9u32)
        .map(|i| {
            let (row, col) = (i / 3, i % 3);
            [
                f64::from(row),
                f64::from(col),
                f64::from(col) * 100.0,
                f64::from(row) * 80.0,
                1.0,
            ]
        })
        .collect();
    let out = verify_fn(VerifyArgs {
        cols: 3,
        rows: 3,
        cell_w: 100.0,
        cell_h: 80.0,
        placements: placements.clone(),
    })
    .unwrap();
    assert!(out.valid);
    assert_eq!(out.placed, 9);

    // 一片偏了 2px：画面看着满了也不算完成
    let mut drifted = placements.clone();
    drifted[4][2] += 2.0;
    let out = verify_fn(VerifyArgs {
        cols: 3,
        rows: 3,
        cell_w: 100.0,
        cell_h: 80.0,
        placements: drifted,
    })
    .unwrap();
    assert!(!out.valid);

    // 只放 8 片：不可能通过
    let mut partial = placements.clone();
    partial[8][4] = 0.0;
    let out = verify_fn(VerifyArgs {
        cols: 3,
        rows: 3,
        cell_w: 100.0,
        cell_h: 80.0,
        placements: partial,
    })
    .unwrap();
    assert!(!out.valid);
    assert_eq!(out.placed, 8);

    // 数量不对
    let error = verify_fn(VerifyArgs {
        cols: 3,
        rows: 3,
        cell_w: 100.0,
        cell_h: 80.0,
        placements: placements[..8].to_vec(),
    })
    .unwrap_err();
    assert_eq!(error.code, "jigsaw.piece_count");

    // 身份重复
    let mut duplicated = placements.clone();
    duplicated[1][0] = 0.0;
    duplicated[1][1] = 0.0;
    let error = verify_fn(VerifyArgs {
        cols: 3,
        rows: 3,
        cell_w: 100.0,
        cell_h: 80.0,
        placements: duplicated,
    })
    .unwrap_err();
    assert_eq!(error.code, "jigsaw.bad_identity");
}

#[test]
fn place_fn_judges_by_identity() {
    let out = place_fn(PlaceArgs {
        row: 1,
        col: 2,
        x: 205.0,
        y: 81.0,
        cell_w: 100.0,
        cell_h: 80.0,
    });
    assert!(out.placed);
    assert_eq!((out.x, out.y), (200.0, 80.0));
    let out = place_fn(PlaceArgs {
        row: 1,
        col: 2,
        x: 300.0,
        y: 200.0,
        cell_w: 100.0,
        cell_h: 80.0,
    });
    assert!(!out.placed);
    assert_eq!((out.x, out.y), (300.0, 200.0));
}

#[test]
fn prepare_task_roundtrip_via_path() {
    // 走文件路径：guest net 在原生分支用 std::fs
    let dir = temp_dir("prepare");
    let path = dir.join("photo.png");
    std::fs::write(&path, test_png_bytes(300, 200)).unwrap();
    let out = prepare::prepare(&ImageInput {
        data: None,
        path: Some(path.to_string_lossy().into_owned()),
    })
    .unwrap();
    assert_eq!((out.width, out.height), (300, 200));
    let _ = std::fs::remove_dir_all(&dir);
}

fn test_png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]));
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}
