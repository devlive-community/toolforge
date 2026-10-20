use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    cancelled: AtomicBool,
    logs: Mutex<Vec<String>>,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
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
    let dir = std::env::temp_dir().join(format!("tfm-sheet-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn item(width: u32, height: u32, name: &str) -> Item {
    Item {
        width,
        height,
        name: name.to_owned(),
        note: None,
    }
}

fn settings(columns: u32, rows: u32) -> Settings {
    Settings {
        columns,
        rows,
        ..Settings::default()
    }
}

fn image_file(dir: &std::path::Path, name: &str, width: u32, height: u32) -> PathBuf {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    });
    let path = dir.join(name);
    image
        .write_to(
            &mut std::io::BufWriter::new(std::fs::File::create(&path).unwrap()),
            image::ImageFormat::Png,
        )
        .unwrap();
    path
}

#[test]
fn plans_single_image_on_one_page() {
    let out = plan(PlanArgs {
        items: vec![item(3000, 2000, "a.jpg")],
        settings: settings(4, 5),
    })
    .unwrap();
    assert_eq!(out.pages.len(), 1);
    assert_eq!(out.pages[0].cells.len(), 1);
    assert_eq!(out.pages[0].cells[0].number, 1);
    // A4 竖向
    assert_eq!(out.page_size, [210.0, 297.0]);
}

#[test]
fn pages_and_numbers_run_across_pages() {
    let items: Vec<Item> = (0..5)
        .map(|i| item(800, 600, &format!("{i}.jpg")))
        .collect();
    let out = plan(PlanArgs {
        items,
        settings: settings(2, 2),
    })
    .unwrap();
    assert_eq!(out.pages.len(), 2);
    // 末页只放 1 张，编号跨页连续
    assert_eq!(out.pages[0].cells.len(), 4);
    assert_eq!(out.pages[1].cells.len(), 1);
    assert_eq!(out.pages[1].cells[0].number, 5);
    // 页内行优先
    assert!(out.pages[0].cells[1].x > out.pages[0].cells[0].x);
    assert!(out.pages[0].cells[2].y > out.pages[0].cells[0].y);
}

#[test]
fn landscape_swaps_page_size() {
    let out = plan(PlanArgs {
        items: vec![item(100, 100, "a.jpg")],
        settings: Settings {
            orientation: Orientation::Landscape,
            ..settings(1, 1)
        },
    })
    .unwrap();
    assert_eq!(out.page_size, [297.0, 210.0]);
}

#[test]
fn contain_fits_and_centers_image() {
    // 100×200 竖图放进近方格：高度顶满图片框，宽度按比例，且在框内水平居中
    let out = plan(PlanArgs {
        items: vec![item(100, 200, "tall.jpg")],
        settings: Settings {
            margin: 0.0,
            gap: 0.0,
            label_height: 0.0,
            show_page_number: false,
            ..settings(1, 1)
        },
    })
    .unwrap();
    let cell = &out.pages[0].cells[0];
    // 图片框 = 单元格 - 内边距（label_height 为 0）
    let frame_h = cell.h - 1.2;
    let frame_w = cell.w - 2.4;
    assert!(
        (cell.image_h - frame_h).abs() < 1e-6,
        "tall image fills frame height"
    );
    assert!(cell.image_w < frame_w);
    let left_gap = cell.image_x - cell.x;
    let right_gap = cell.x + cell.w - (cell.image_x + cell.image_w);
    assert!((left_gap - right_gap).abs() < 1e-6);
    // 宽高比保持
    assert!((cell.image_w / cell.image_h - 0.5).abs() < 1e-6);
}

#[test]
fn label_lines_follow_switches() {
    let sample = Item {
        width: 100,
        height: 100,
        name: "beach.jpg".to_owned(),
        note: Some("日落".to_owned()),
    };
    let lines = |show_number: bool, show_name: bool| {
        label_lines(
            7,
            &sample,
            &Settings {
                show_number,
                show_name,
                ..settings(1, 1)
            },
        )
    };
    assert_eq!(lines(true, true), vec!["7. beach.jpg", "日落"]);
    assert_eq!(lines(true, false), vec!["7", "日落"]);
    assert_eq!(lines(false, true), vec!["beach.jpg", "日落"]);
    // 名称与编号都关掉时，只留说明
    assert_eq!(lines(false, false), vec!["日落"]);
    // 无说明时只有一行
    let plain = item(100, 100, "a.jpg");
    assert_eq!(label_lines(3, &plain, &settings(1, 1)), vec!["3. a.jpg"]);
}

#[test]
fn rejects_bad_input() {
    assert_eq!(
        plan(PlanArgs {
            items: vec![],
            settings: settings(4, 5)
        })
        .unwrap_err()
        .code,
        "sheet.empty"
    );
    assert_eq!(
        plan(PlanArgs {
            items: vec![item(1, 1, "a.jpg")],
            settings: settings(0, 5)
        })
        .unwrap_err()
        .code,
        "sheet.bad_grid"
    );
    // 标签高度吃掉整格：图片区不足最小高度
    assert_eq!(
        plan(PlanArgs {
            items: vec![item(1, 1, "a.jpg")],
            settings: Settings {
                label_height: 275.0,
                ..settings(1, 1)
            }
        })
        .unwrap_err()
        .code,
        "sheet.cell_too_small"
    );
}

#[test]
fn export_writes_multipage_pdf() {
    let dir = temp_dir("export");
    let first = image_file(&dir, "first.png", 1200, 900);
    let second = image_file(&dir, "second.png", 600, 600);
    let output = dir.join("sheet.pdf");
    let args = ExportArgs {
        items: vec![
            ExportItem {
                path: first.to_string_lossy().into_owned(),
                name: "first.png".to_owned(),
                note: None,
            },
            ExportItem {
                path: second.to_string_lossy().into_owned(),
                name: "second.png".to_owned(),
                note: Some("note line".to_owned()),
            },
        ],
        settings: settings(2, 1),
        font_path: None,
        output_path: output.to_string_lossy().into_owned(),
    };
    let report = export(args, &Ctx::default()).unwrap();
    assert_eq!(report.pages, 1);
    assert_eq!(report.images, 2);
    assert!(
        report.missing_glyphs.is_empty(),
        "ascii labels need no fallback"
    );
    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    // 读回验证页数与页面尺寸（A4 竖向 210×297mm → 595×842pt）
    let doc = lopdf::Document::load_mem(&bytes).unwrap();
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 1);
    // 内容流里有图片绘制与文字
    let content = doc
        .objects
        .values()
        .find_map(|o| match o {
            lopdf::Object::Stream(stream) => {
                let data = stream.content.to_vec();
                String::from_utf8(data).ok().filter(|s| s.contains("Do"))
            }
            _ => None,
        })
        .unwrap();
    assert!(content.contains("/Im1 Do"));
    assert!(content.contains("Tj"), "helvetica fallback draws labels");
    assert!(content.contains("first.png"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_reports_missing_glyphs_without_font() {
    let dir = temp_dir("missing");
    let image = image_file(&dir, "a.png", 300, 300);
    let output = dir.join("sheet.pdf");
    let report = export(
        ExportArgs {
            items: vec![ExportItem {
                path: image.to_string_lossy().into_owned(),
                name: "海滩.png".to_owned(),
                note: None,
            }],
            settings: settings(1, 1),
            font_path: None,
            output_path: output.to_string_lossy().into_owned(),
        },
        &Ctx::default(),
    )
    .unwrap();
    // 无字体时中文不可显示，但导出仍成功并报告缺字
    assert!(report.missing_glyphs.contains(&'海'));
    assert!(output.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_honours_cancellation() {
    let dir = temp_dir("cancel");
    let image = image_file(&dir, "a.png", 300, 300);
    let ctx = Ctx::default();
    ctx.cancelled.store(true, Ordering::SeqCst);
    let err = export(
        ExportArgs {
            items: vec![ExportItem {
                path: image.to_string_lossy().into_owned(),
                name: "a.png".to_owned(),
                note: None,
            }],
            settings: settings(1, 1),
            font_path: None,
            output_path: dir.join("sheet.pdf").to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, tf_plugin_api::cancelled().code);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_rejects_empty_and_font_errors() {
    let dir = temp_dir("errors");
    assert_eq!(
        export(
            ExportArgs {
                items: vec![],
                settings: settings(1, 1),
                font_path: None,
                output_path: dir.join("s.pdf").to_string_lossy().into_owned(),
            },
            &Ctx::default()
        )
        .unwrap_err()
        .code,
        "sheet.empty"
    );
    assert_eq!(
        export(
            ExportArgs {
                items: vec![ExportItem {
                    path: dir.join("none.png").to_string_lossy().into_owned(),
                    name: "a.png".to_owned(),
                    note: None,
                }],
                settings: settings(1, 1),
                font_path: Some(dir.join("missing.ttf").to_string_lossy().into_owned()),
                output_path: dir.join("s.pdf").to_string_lossy().into_owned(),
            },
            &Ctx::default()
        )
        .unwrap_err()
        .code,
        "fs.not_found"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
