//! 联系印样：纸张分页布局（plan）与多页 PDF 导出（task）。
//! 布局全部以毫米计算（原点在页面左上角），PDF 侧换算为点；缩略图等比完整放入
//! 格内（不裁剪、不拉伸）；编号跨页连续；标签行与说明跟图片索引绑定。

use std::io::Read;
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::pdf;
use crate::stitch;

/// 印样数量上限（缩略图很小，比拼接上限放宽）
pub const MAX_IMAGES: usize = 300;
/// 页数上限（防止极端行列 × 大清单生成超大文档）
const MAX_PAGES: usize = 200;
/// 导出缩略图分辨率
const DPI: f64 = 150.0;
/// 缩略图 JPEG 质量
const THUMB_QUALITY: u8 = 85;
/// 单元格内边距（mm）
const CELL_PADDING: f64 = 1.2;
/// 缩略图区最小高度（mm），低于此值说明标签设置挤压了图片区
const MIN_FRAME: f64 = 5.0;

pub(crate) const MM_TO_PT: f64 = 72.0 / 25.4;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 纸张尺寸（毫米，竖向）
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Paper {
    A4,
    Letter,
}

impl Paper {
    /// 竖向尺寸（宽, 高）
    fn size(self) -> (f64, f64) {
        match self {
            Self::A4 => (210.0, 297.0),
            Self::Letter => (215.9, 279.4),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// 页面设置；plan 与 export 共用
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub paper: Paper,
    pub orientation: Orientation,
    /// 页边距（mm）
    pub margin: f64,
    pub columns: u32,
    pub rows: u32,
    /// 间距（mm）
    pub gap: f64,
    /// 标签区高度（mm）
    pub label_height: f64,
    /// 标签字号（pt）
    pub font_size: f64,
    /// 整份标题；为空时不占位
    pub title: Option<String>,
    pub show_number: bool,
    pub show_name: bool,
    pub show_page_number: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            orientation: Orientation::Portrait,
            margin: 10.0,
            columns: 4,
            rows: 5,
            gap: 4.0,
            label_height: 8.0,
            font_size: 7.0,
            title: None,
            show_number: true,
            show_name: true,
            show_page_number: true,
        }
    }
}

/// 参与布局的图片条目（plan 用尺寸，export 额外带路径）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub width: u32,
    pub height: u32,
    pub name: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// 导出条目：路径用于读取原图
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanArgs {
    pub items: Vec<Item>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    /// 全局连续编号（1 起）
    pub number: usize,
    /// 单元格（含标签区），页面左上原点毫米
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// 缩略图等比放入后的实际框
    pub image_x: f64,
    pub image_y: f64,
    pub image_w: f64,
    pub image_h: f64,
    /// 标签行文本
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub number: usize,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOut {
    /// 页面尺寸（毫米）
    pub page_size: [f64; 2],
    /// 标签字号（pt），PDF 与前端预览共用
    pub label_font_size: f64,
    /// 整份标题；为空时不占位
    pub title: Option<String>,
    pub title_font_size: Option<f64>,
    pub page_font_size: Option<f64>,
    pub pages: Vec<Page>,
}

impl PlanOut {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

fn validate(settings: &Settings, count: usize) -> PluginResult<()> {
    if count == 0 {
        return Err(PluginError::new("sheet.empty"));
    }
    if count > MAX_IMAGES {
        return Err(errp("sheet.too_many_images", "limit", MAX_IMAGES as u64));
    }
    let (w, _) = page_size(settings);
    if settings.margin < 0.0 || settings.gap < 0.0 || settings.label_height < 0.0 {
        return Err(PluginError::new("sheet.negative"));
    }
    if !(4.0..=48.0).contains(&settings.font_size) {
        return Err(errp("sheet.bad_font_size", "value", settings.font_size));
    }
    if settings.columns == 0 || settings.rows == 0 {
        return Err(PluginError::new("sheet.bad_grid"));
    }
    if w <= settings.margin * 2.0 {
        return Err(PluginError::new("sheet.margin_too_wide"));
    }
    Ok(())
}

/// 页面尺寸（毫米）
fn page_size(settings: &Settings) -> (f64, f64) {
    let (w, h) = settings.paper.size();
    match settings.orientation {
        Orientation::Portrait => (w, h),
        Orientation::Landscape => (h, w),
    }
}

/// 标签文本行：编号 + 文件名同行，说明单独一行
fn label_lines(number: usize, item: &Item, settings: &Settings) -> Vec<String> {
    let first = match (settings.show_number, settings.show_name) {
        (true, true) => format!("{number}. {}", item.name),
        (true, false) => number.to_string(),
        (false, true) => item.name.clone(),
        (false, false) => String::new(),
    };
    let mut lines = Vec::new();
    if !first.is_empty() {
        lines.push(first);
    }
    if let Some(note) = item
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        lines.push(note.to_owned());
    }
    lines
}

/// 分页布局：编号跨页连续，末页空位留空
pub fn plan(args: PlanArgs) -> PluginResult<PlanOut> {
    let settings = &args.settings;
    validate(settings, args.items.len())?;
    let (page_w, page_h) = page_size(settings);
    let per_page = (settings.columns as usize).saturating_mul(settings.rows as usize);
    let page_count = args.items.len().div_ceil(per_page).max(1);
    if page_count > MAX_PAGES {
        return Err(errp("sheet.too_many_pages", "limit", MAX_PAGES as u64));
    }
    // 标题区与页码区高度（mm）
    let title_size = settings.title.as_ref().map(|_| settings.font_size * 1.8);
    let title_height = title_size.map(|s| s * MM_TO_PT * 1.8).unwrap_or(0.0);
    let page_size_pt = settings.show_page_number.then_some(settings.font_size);
    let page_height = page_size_pt.map(|s| s * MM_TO_PT * 2.0).unwrap_or(0.0);

    let (margin, gap, columns, rows) = (
        settings.margin,
        settings.gap,
        settings.columns,
        settings.rows,
    );
    let usable_w = page_w - margin * 2.0;
    let usable_h = page_h - margin * 2.0 - title_height - page_height;
    let cell_w = (usable_w - gap * (columns - 1) as f64) / columns as f64;
    let cell_h = (usable_h - gap * (rows - 1) as f64) / rows as f64;
    let frame_w = cell_w - CELL_PADDING * 2.0;
    let frame_h = cell_h - settings.label_height - CELL_PADDING;
    if frame_h < MIN_FRAME || frame_w < MIN_FRAME {
        return Err(PluginError::new("sheet.cell_too_small"));
    }

    let mut pages = Vec::with_capacity(page_count);
    for page_number in 0..page_count {
        let chunk = &args.items
            [page_number * per_page..((page_number + 1) * per_page).min(args.items.len())];
        let mut cells = Vec::with_capacity(chunk.len());
        for (slot, item) in chunk.iter().enumerate() {
            let col = slot % columns as usize;
            let row = slot / columns as usize;
            let x = margin + col as f64 * (cell_w + gap);
            let y = margin + title_height + row as f64 * (cell_h + gap);
            // 等比完整放入（contain），在框内居中
            let scale = (frame_w / item.width as f64).min(frame_h / item.height as f64);
            let image_w = item.width as f64 * scale;
            let image_h = item.height as f64 * scale;
            cells.push(Cell {
                number: page_number * per_page + slot + 1,
                x,
                y,
                w: cell_w,
                h: cell_h,
                image_x: x + (cell_w - image_w) / 2.0,
                image_y: y + (frame_h - image_h) / 2.0,
                image_w,
                image_h,
                lines: label_lines(page_number * per_page + slot + 1, item, settings),
            });
        }
        pages.push(Page {
            number: page_number + 1,
            cells,
        });
    }

    Ok(PlanOut {
        page_size: [page_w, page_h],
        label_font_size: settings.font_size,
        title: settings.title.clone().filter(|t| !t.trim().is_empty()),
        title_font_size: title_size,
        page_font_size: page_size_pt,
        pages,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub items: Vec<ExportItem>,
    pub settings: Settings,
    /// 标签字体（TTF/OTF）；缺省时用 PDF 标准 Helvetica（仅 ASCII）
    #[serde(default)]
    pub font_path: Option<String>,
    pub output_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub output: String,
    pub pages: usize,
    pub images: usize,
    pub bytes: u64,
    pub elapsed_ms: u64,
    /// 标签中无法显示的字符（去重）；空表示全部渲染
    pub missing_glyphs: Vec<char>,
}

/// 解码为 RGB（白底），EXIF 方向已摆正
fn decode_rgb(bytes: &[u8]) -> PluginResult<RgbImage> {
    let rgba: RgbaImage = stitch::decode(bytes)?;
    Ok(RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        // 印样页白底：半透明像素与白底合成
        let blend = |fg: u8, bg: u8| {
            ((u16::from(fg) * u16::from(a) + u16::from(bg) * u16::from(255 - a)) / 255) as u8
        };
        image::Rgb([blend(r, 255), blend(g, 255), blend(b, 255)])
    }))
}

/// 等比缩放到目标框内（contain）
fn fit(image: &RgbImage, box_w: u32, box_h: u32) -> RgbImage {
    let scale = (box_w as f64 / image.width() as f64).min(box_h as f64 / image.height() as f64);
    let w = ((image.width() as f64 * scale).round() as u32).max(1);
    let h = ((image.height() as f64 * scale).round() as u32).max(1);
    image::imageops::resize(image, w, h, image::imageops::FilterType::Triangle)
}

/// 导出：读图解码（逐图进度，可取消）→ 布局 → 缩略图 JPEG → 组装 PDF → 写盘
pub fn export(args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let start = Instant::now();
    if args.items.is_empty() {
        return Err(PluginError::new("sheet.empty"));
    }
    if args.items.len() > MAX_IMAGES {
        return Err(errp("sheet.too_many_images", "limit", MAX_IMAGES as u64));
    }
    if args.output_path.trim().is_empty() {
        return Err(PluginError::new("sheet.output_required"));
    }
    // 字体字节先行读取，解析失败直接报错而不是半成品
    let font_data = match args
        .font_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(path) => {
            let mut bytes = Vec::new();
            ctx.open_file(path)?
                .read_to_end(&mut bytes)
                .map_err(|e| errp("sheet.font_io", "path", path).with("detail", e.to_string()))?;
            Some(bytes)
        }
        None => None,
    };

    let total_steps = args.items.len() + 2;
    ctx.stage("sheet.decode");
    let mut images = Vec::with_capacity(args.items.len());
    for (index, item) in args.items.iter().enumerate() {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let size = ctx.file_size(&item.path)?;
        let mut bytes = Vec::with_capacity(size.min(64 << 20) as usize);
        ctx.open_file(&item.path)?
            .read_to_end(&mut bytes)
            .map_err(|e| {
                PluginError::new("stitch.io")
                    .with("path", item.path.as_str())
                    .with("detail", e.to_string())
            })?;
        let image = decode_rgb(&bytes)?;
        images.push(image);
        ctx.progress(index as u64 + 1, total_steps as u64);
    }
    if ctx.is_cancelled() {
        return Err(cancelled());
    }

    ctx.stage("sheet.compose");
    let items = args
        .items
        .iter()
        .zip(&images)
        .map(|(item, image)| Item {
            width: image.width(),
            height: image.height(),
            name: item.name.clone(),
            note: item.note.clone(),
        })
        .collect::<Vec<_>>();
    let layout = plan(PlanArgs {
        items,
        settings: args.settings.clone(),
    })?;

    // 按布局把缩略图缩放到目标像素（DPI 换算），JPEG 编码后交给 PDF 组装
    let mut thumbs = Vec::with_capacity(args.items.len());
    for page in &layout.pages {
        for cell in &page.cells {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let image = &images[cell.number - 1];
            let box_w = (cell.image_w * DPI / 25.4).round() as u32;
            let box_h = (cell.image_h * DPI / 25.4).round() as u32;
            let fitted = fit(image, box_w.max(1), box_h.max(1));
            let mut jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpeg, THUMB_QUALITY)
                .write_image(
                    fitted.as_raw(),
                    fitted.width(),
                    fitted.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|e| errp("sheet.encode_failed", "detail", e.to_string()))?;
            thumbs.push((fitted.width(), fitted.height(), jpeg));
        }
    }
    ctx.progress(args.items.len() as u64 + 1, total_steps as u64);

    ctx.stage("sheet.pdf");
    let (document, missing) = pdf::build(&layout, &thumbs, font_data.as_deref())?;
    let mut missing_glyphs = missing;
    missing_glyphs.sort_unstable();
    missing_glyphs.dedup();
    tf_plugin_guest::net::write_file(&args.output_path, &document).map_err(|e| {
        PluginError::new("sheet.io")
            .with("path", args.output_path.as_str())
            .with("detail", e.to_string())
    })?;
    ctx.progress(total_steps as u64, total_steps as u64);
    if !missing_glyphs.is_empty() {
        ctx.log(
            LogLevel::Warn,
            "sheet.missing_glyphs",
            json!({ "count": missing_glyphs.len() }),
        );
    }

    Ok(Report {
        output: args.output_path,
        pages: layout.page_count(),
        images: args.items.len(),
        bytes: document.len() as u64,
        elapsed_ms: start.elapsed().as_millis() as u64,
        missing_glyphs,
    })
}

#[cfg(test)]
#[path = "contact_test.rs"]
mod tests;
