//! 字符画工坊：把照片转换为怀旧电脑风格的字符画。
//!
//! 输出始终包含无颜色控制字符的纯文本；可选 24 位 ANSI 前景色彩色版。
//! 完整结果留在插件内缓存（有界 LRU），对外只返回有界预览与统计，
//! 复制 / 保存通过 fetch / save 按 id 取，避免大文本挤爆 IPC。

mod charset;
mod render;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;

use data_encoding::BASE64;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};
use tf_plugin_guest::net;

use crate::render::{Options, Output};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 结果缓存上限（份）
const CACHE_LIMIT: usize = 8;
/// 预览文本上限：行数与字节数双重有界
const PREVIEW_MAX_ROWS: usize = 400;
const PREVIEW_MAX_BYTES: usize = 64_000;

pub struct AsciiArt {
    manifest: Manifest,
}

impl Default for AsciiArt {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 结果缓存：WASM 单线程，thread_local 足够；FIFO 淘汰最旧结果
struct CacheSlot {
    plain: String,
    ansi: String,
}

thread_local! {
    static CACHE: RefCell<VecDeque<(String, CacheSlot)>> = const { RefCell::new(VecDeque::new()) };
    static CACHE_SEQ: RefCell<u64> = const { RefCell::new(0) };
}

fn cache_put(plain: String, ansi: String) -> String {
    CACHE_SEQ.with(|seq| {
        let id = {
            let mut seq = seq.borrow_mut();
            *seq += 1;
            let serial: u64 = *seq;
            format!("a{serial:012x}")
        };
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() >= CACHE_LIMIT {
                cache.pop_front();
            }
            cache.push_back((id.clone(), CacheSlot { plain, ansi }));
        });
        id
    })
}

fn cache_get(id: &str) -> Option<CacheSlot> {
    CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, slot)| CacheSlot {
                plain: slot.plain.clone(),
                ansi: slot.ansi.clone(),
            })
    })
}

/// 解码为 RGBA8 并按 EXIF 方向摆正
fn decode(bytes: &[u8]) -> PluginResult<RgbaImage> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| errp("ascii.decode_failed", "detail", e.to_string()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| errp("ascii.decode_failed", "detail", e.to_string()))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)
        .map_err(|e| errp("ascii.decode_failed", "detail", e.to_string()))?;
    img.apply_orientation(orientation);
    Ok(img.into_rgba8())
}

/// 从 data URI（或裸 base64）解码出原始字节
fn decode_data(data: &str) -> PluginResult<Vec<u8>> {
    let clean = match data.rsplit_once("base64,") {
        Some((_, payload)) => payload,
        None => data,
    };
    BASE64
        .decode(clean.trim().as_bytes())
        .map_err(|e| errp("ascii.bad_data", "detail", e.to_string()))
}

/// 读取插件输入的图片：data（base64）优先，其次路径（task 内用 ctx，非 task 用 guest）
fn load_image(
    data: &Option<String>,
    path: &Option<String>,
    ctx: Option<&dyn TaskContext>,
) -> PluginResult<RgbaImage> {
    let bytes = match (
        data.as_deref().map(str::trim).filter(|d| !d.is_empty()),
        path.as_deref().map(str::trim).filter(|p| !p.is_empty()),
    ) {
        (Some(data), _) => decode_data(data)?,
        (None, Some(path)) => {
            let limit = render::MAX_INPUT_PIXELS * 4;
            match ctx {
                Some(ctx) => {
                    let mut buf = Vec::new();
                    ctx.open_file(path)?
                        .read_to_end(&mut buf)
                        .map_err(|e| errp("ascii.io", "detail", e.to_string()))?;
                    buf
                }
                None => net::read_to_end(path, limit)?,
            }
        }
        _ => {
            return Err(errp(
                "ascii.bad_params",
                "reason",
                "需要 path 或 data / need path or data",
            ));
        }
    };
    // 像素预算在解码前按文件大小粗查 + 解码后精确检查
    if bytes.len() as u64 > render::MAX_INPUT_PIXELS * 4 {
        return Err(errp("ascii.too_large", "limit", render::MAX_INPUT_PIXELS));
    }
    let img = decode(&bytes)?;
    let (w, h) = img.dimensions();
    if u64::from(w) * u64::from(h) > render::MAX_INPUT_PIXELS {
        return Err(errp("ascii.too_large", "limit", render::MAX_INPUT_PIXELS));
    }
    Ok(img)
}

/// 解析底色：6 位十六进制（无 # 前缀，允许带 #）
fn parse_bg(bg: &Option<String>) -> PluginResult<Rgba<u8>> {
    let raw = bg.as_deref().map(str::trim).unwrap_or_default();
    let hex = raw.strip_prefix('#').unwrap_or(raw);
    if hex.is_empty() {
        return Ok(Rgba([255, 255, 255, 255]));
    }
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(errp("ascii.bad_bg", "value", raw.to_string()));
    }
    let value =
        u32::from_str_radix(hex, 16).map_err(|_| errp("ascii.bad_bg", "value", raw.to_string()))?;
    Ok(Rgba([
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
        255,
    ]))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrepareArgs {
    path: String,
    /// 工作图长边上限
    #[serde(default = "default_work_edge")]
    work_edge: u32,
}

fn default_work_edge() -> u32 {
    1600
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepareOut {
    width: u32,
    height: u32,
    /// 受限工作图的 PNG Data URI，原图移动后仍可继续
    work_data: String,
}

fn prepare(args: PrepareArgs, ctx: Option<&dyn TaskContext>) -> PluginResult<PrepareOut> {
    let img = load_image(&None, &Some(args.path), ctx)?;
    let (w, h) = img.dimensions();
    // 缩到工作图长边，降低后续重复解码成本
    let scale = (args.work_edge as f64 / w.max(h) as f64).min(1.0);
    let work = if scale < 1.0 {
        let nw = ((w as f64) * scale).round().max(1.0) as u32;
        let nh = ((h as f64) * scale).round().max(1.0) as u32;
        image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let (nw, nh) = work.dimensions();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut png))
        .write_image(work.as_raw(), nw, nh, ExtendedColorType::Rgba8)
        .map_err(|e| errp("ascii.decode_failed", "detail", e.to_string()))?;
    Ok(PrepareOut {
        width: nw,
        height: nh,
        work_data: format!("data:image/png;base64,{}", BASE64.encode(&png)),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConvertArgs {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    data: Option<String>,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default)]
    chars: Option<String>,
    #[serde(default = "default_preset")]
    preset: String,
    #[serde(default)]
    brightness: f64,
    #[serde(default = "default_contrast")]
    contrast: f64,
    #[serde(default)]
    invert: bool,
    #[serde(default)]
    color: bool,
    #[serde(default = "default_aspect")]
    aspect: f64,
    #[serde(default = "default_alpha_mode")]
    alpha_mode: String,
    #[serde(default)]
    bg: Option<String>,
}

fn default_width() -> u32 {
    80
}
fn default_preset() -> String {
    "classic".to_owned()
}
fn default_contrast() -> f64 {
    1.0
}
fn default_aspect() -> f64 {
    render::DEFAULT_ASPECT
}
fn default_alpha_mode() -> String {
    "keep".to_owned()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConvertOut {
    /// 结果 id，供 fetch / save 取完整文本
    id: String,
    /// 有界预览（≤ 400 行且 ≤ 64 KB，按整行截断）
    text: String,
    /// 预览是否为完整结果
    complete: bool,
    cols: u32,
    rows: u32,
    /// 输出总字符数（含换行）
    chars: u64,
    warnings: Vec<String>,
}

fn convert(args: ConvertArgs) -> PluginResult<ConvertOut> {
    let img = load_image(&args.data, &args.path, None)?;
    // 梯度：自定义优先，其次预设
    let mut warnings: Vec<String> = Vec::new();
    let chars = match args
        .chars
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        Some(custom) => {
            let (normalized, stats) = charset::normalize_custom(custom)?;
            if stats.duplicates > 0 {
                warnings.push("duplicateChars".to_owned());
            }
            if stats.skipped > 0 {
                warnings.push("skippedChars".to_owned());
            }
            normalized
        }
        None => charset::preset_chars(&args.preset)
            .ok_or_else(|| {
                errp(
                    "ascii.bad_params",
                    "reason",
                    format!("未知符号组 {}", args.preset),
                )
            })?
            .to_owned(),
    };
    let keep_alpha = match args.alpha_mode.as_str() {
        "keep" => true,
        "fill" => false,
        other => {
            return Err(errp(
                "ascii.bad_params",
                "reason",
                format!("未知透明处理 {other}"),
            ));
        }
    };
    if !(0.3..=1.0).contains(&args.aspect) {
        return Err(errp(
            "ascii.bad_params",
            "reason",
            "aspect 需在 0.3 到 1 之间",
        ));
    }
    if !(-1.0..=1.0).contains(&args.brightness) {
        return Err(errp(
            "ascii.bad_params",
            "reason",
            "brightness 需在 -1 到 1 之间",
        ));
    }
    if !(0.2..=3.0).contains(&args.contrast) {
        return Err(errp(
            "ascii.bad_params",
            "reason",
            "contrast 需在 0.2 到 3 之间",
        ));
    }
    let opts = Options {
        cols: args.width,
        chars,
        brightness: args.brightness,
        contrast: args.contrast,
        invert: args.invert,
        color: args.color,
        aspect: args.aspect,
        keep_alpha,
        bg: parse_bg(&args.bg)?,
    };
    let Output {
        plain,
        ansi,
        cols,
        rows,
    } = render::convert(&img, &opts)?;
    let total_chars = plain.chars().count() as u64;
    let (preview, complete) = bounded_preview(&plain);
    let id = cache_put(plain, ansi);
    Ok(ConvertOut {
        id,
        text: preview,
        complete,
        cols,
        rows,
        chars: total_chars,
        warnings,
    })
}

/// 截取有界预览：整行截断，同时受行数与字节限制
fn bounded_preview(text: &str) -> (String, bool) {
    let mut lines = text.lines().count();
    if !text.ends_with('\n') {
        lines += 1;
    }
    if lines <= PREVIEW_MAX_ROWS && text.len() <= PREVIEW_MAX_BYTES {
        return (text.to_owned(), true);
    }
    let mut taken = 0usize;
    let mut bytes = 0usize;
    for line in text.lines() {
        if taken >= PREVIEW_MAX_ROWS || bytes + line.len() + 1 > PREVIEW_MAX_BYTES {
            break;
        }
        bytes += line.len() + 1;
        taken += 1;
    }
    let mut preview = text.lines().take(taken).collect::<Vec<_>>().join("\n");
    preview.push('\n');
    (preview, false)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FetchArgs {
    id: String,
    /// true 取彩色版，false 取纯文本
    #[serde(default)]
    ansi: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchOut {
    text: String,
}

fn fetch(args: FetchArgs) -> PluginResult<FetchOut> {
    let slot = cache_get(&args.id).ok_or_else(|| PluginError::new("ascii.not_found"))?;
    Ok(FetchOut {
        text: if args.ansi { slot.ansi } else { slot.plain },
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveArgs {
    id: String,
    #[serde(default)]
    ansi: bool,
    /// 保存目标路径（来自宿主保存对话框）
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveOut {
    bytes: u64,
}

fn save(args: SaveArgs) -> PluginResult<SaveOut> {
    let slot = cache_get(&args.id).ok_or_else(|| PluginError::new("ascii.not_found"))?;
    let text = if args.ansi { slot.ansi } else { slot.plain };
    net::write_file(&args.path, text.as_bytes())
        .map_err(|e| errp("ascii.io", "detail", e.to_string()))?;
    Ok(SaveOut {
        bytes: text.len() as u64,
    })
}

impl ToolPlugin for AsciiArt {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "prepare" => to_value(prepare(parse_args(args)?, None)?),
            "convert" => to_value(convert(parse_args(args)?)?),
            "fetch" => to_value(fetch(parse_args(args)?)?),
            "save" => to_value(save(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "prepare" => to_value(prepare(parse_args(args)?, Some(ctx))?),
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(AsciiArt);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
