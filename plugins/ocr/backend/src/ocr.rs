//! 识别流程：读取图片（文件或剪贴板）→ 检测文字区域 → 逐行识别 → 按阅读顺序拼接文本。

use std::io::{Cursor, Read};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbImage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::detect::{self, Region};
use crate::engine::{Engine, Engines, run};
use crate::recognize::{self, MIN_SCORE};

pub const DETECTOR: &str = "ppocr-v4-det";
pub const RECOGNIZER: &str = "ppocr-v4-rec";
const MAX_FILE: u64 = 50 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
const PREVIEW: u32 = 1600;
/// 并行识别的线程数上限
const MAX_WORKERS: usize = 8;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Source {
    File { path: String },
    Clipboard,
}

#[derive(Debug, Deserialize)]
pub struct Args {
    pub source: Source,
}

#[derive(Debug, Serialize)]
pub struct Line {
    pub text: String,
    pub score: f32,
    #[serde(flatten)]
    pub region: Region,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub preview: String,
    pub lines: Vec<Line>,
    pub text: String,
    pub elapsed_ms: u64,
}

fn decode_failed(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("ocr.decode_failed").with("detail", err.to_string())
}

fn check_pixels(width: u32, height: u32) -> PluginResult<()> {
    if width as u64 * height as u64 > MAX_PIXELS {
        return Err(PluginError::new("ocr.too_many_pixels")
            .with("width", width)
            .with("height", height));
    }
    Ok(())
}

fn load_file(path: &str, ctx: &dyn TaskContext) -> PluginResult<RgbImage> {
    if ctx.file_size(path)? > MAX_FILE {
        return Err(PluginError::new("ocr.file_too_large").with("limit", "50 MB"));
    }
    let mut bytes = Vec::new();
    ctx.open_file(path)?
        .read_to_end(&mut bytes)
        .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(decode_failed)?;
    if reader.format().is_none() {
        return Err(PluginError::new("ocr.unsupported"));
    }
    let mut decoder = reader.into_decoder().map_err(decode_failed)?;
    let (width, height) = decoder.dimensions();
    check_pixels(width, height)?;
    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_failed)?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    Ok(image.to_rgb8())
}

fn load_clipboard() -> PluginResult<RgbImage> {
    let empty = || PluginError::new("ocr.clipboard_empty");
    let data = arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_image())
        .map_err(|_| empty())?;
    check_pixels(data.width as u32, data.height as u32)?;
    let rgba = image::RgbaImage::from_raw(
        data.width as u32,
        data.height as u32,
        data.bytes.into_owned(),
    )
    .ok_or_else(empty)?;
    // 透明区域按白底合成，截图中的透明背景不影响识别
    Ok(RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        image::Rgb([blend(r), blend(g), blend(b)])
    }))
}

fn preview(image: &RgbImage) -> String {
    let small = if image.width().max(image.height()) > PREVIEW {
        DynamicImage::ImageRgb8(image.clone()).resize(PREVIEW, PREVIEW, FilterType::Triangle)
    } else {
        DynamicImage::ImageRgb8(image.clone())
    };
    let mut buffer = Vec::new();
    if small
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Jpeg)
        .is_err()
    {
        return String::new();
    }
    format!(
        "data:image/jpeg;base64,{}",
        data_encoding::BASE64.encode(&buffer)
    )
}

/// 同一行的片段用空格连接，不同行换行
pub fn join(lines: &[Line]) -> String {
    let mut text = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            let separator = if detect::same_row(&lines[i - 1].region, &line.region) {
                " "
            } else {
                "\n"
            };
            text.push_str(separator);
        }
        text.push_str(&line.text);
    }
    text
}

/// 识别单个区域；置信度过低或没有文字时返回 None
fn recognize_line(
    engine: &Engine,
    image: &RgbImage,
    region: Region,
    keys: &[&str],
) -> PluginResult<Option<Line>> {
    let (width, height) = image.dimensions();
    let (x, y) = (region.x.floor() as u32, region.y.floor() as u32);
    let w = (region.w.ceil() as u32).min(width - x).max(1);
    let h = (region.h.ceil() as u32).min(height - y).max(1);
    let crop = image::imageops::crop_imm(image, x, y, w, h).to_image();
    let (tensor, bucket) = recognize::input(&crop);
    let (probs, shape) = run(&engine.recognizer(bucket)?, tensor)?;
    let (text, score) = recognize::decode(&probs, shape[1], shape[2], keys);
    Ok((!text.is_empty() && score >= MIN_SCORE).then(|| Line {
        text,
        score: (score * 1000.0).round() / 1000.0,
        region,
    }))
}

pub fn recognize(args: Args, engines: &Engines, ctx: &dyn TaskContext) -> PluginResult<Output> {
    let start = Instant::now();
    let detector = ctx.resource_path(DETECTOR)?;
    let recognizer = ctx.resource_path(RECOGNIZER)?;

    ctx.stage("ocr.load");
    let (image, name) = match &args.source {
        Source::File { path } => (
            load_file(path, ctx)?,
            std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned()),
        ),
        Source::Clipboard => (load_clipboard()?, None),
    };
    let (engine, fresh) = engines.get(&detector, &recognizer)?;
    if fresh {
        ctx.log(
            LogLevel::Info,
            "ocr.model_loaded",
            json!({ "ms": start.elapsed().as_millis() as u64 }),
        );
    }

    ctx.stage("ocr.detect");
    let (width, height) = image.dimensions();
    let (det_w, det_h) = detect::input_size(width, height);
    let (prob, shape) = run(
        &engine.detector(det_w, det_h)?,
        detect::input(&image, det_w, det_h),
    )?;
    let (map_h, map_w) = (shape[2], shape[3]);
    let regions = detect::regions(&prob, map_w, map_h, width, height);
    ctx.log(
        LogLevel::Info,
        "ocr.regions",
        json!({ "count": regions.len() }),
    );
    if ctx.is_cancelled() {
        return Err(cancelled());
    }

    ctx.stage("ocr.recognize");
    let keys = recognize::characters();
    let total = regions.len() as u64;
    // 各行相互独立：多线程并行识别，执行计划可以在线程间共享
    let workers = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(1, MAX_WORKERS)
        .min(regions.len().max(1));
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failure: Mutex<Option<PluginError>> = Mutex::new(None);
    let recognized: Mutex<Vec<(usize, Line)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(region) = regions.get(index).copied() else {
                        break;
                    };
                    if ctx.is_cancelled()
                        || failure.lock().unwrap_or_else(|e| e.into_inner()).is_some()
                    {
                        break;
                    }
                    match recognize_line(&engine, &image, region, keys) {
                        Ok(Some(line)) => recognized
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push((index, line)),
                        Ok(None) => {}
                        Err(err) => {
                            failure
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .get_or_insert(err);
                        }
                    }
                    let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                    ctx.progress(finished as u64, total);
                }
            });
        }
    });
    if let Some(err) = failure.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(err);
    }
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let mut recognized = recognized.into_inner().unwrap_or_else(|e| e.into_inner());
    recognized.sort_by_key(|(index, _)| *index);
    let lines: Vec<Line> = recognized.into_iter().map(|(_, line)| line).collect();
    ctx.progress(total, total);
    let elapsed_ms = start.elapsed().as_millis() as u64;
    ctx.log(
        LogLevel::Info,
        "ocr.done",
        json!({ "lines": lines.len(), "ms": elapsed_ms }),
    );
    Ok(Output {
        name,
        width,
        height,
        preview: preview(&image),
        text: join(&lines),
        lines,
        elapsed_ms,
    })
}

#[cfg(test)]
#[path = "ocr_test.rs"]
mod tests;
