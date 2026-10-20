//! 低多边形照片艺术：特征点 + Delaunay 剖分 + 原图采样 + PNG/SVG 导出。

mod delaunay;
mod points;
mod render;
mod sample;

use std::io::{Cursor, Read};

use data_encoding::BASE64;
use image::{ImageDecoder, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 输入像素上限
const MAX_INPUT_PIXELS: u64 = 24_000_000;
/// 工作图长边
const WORK_EDGE: u32 = 1600;
/// 点数上限（含四角锚点）
pub const MAX_POINTS: usize = 2000;
/// 导出缩放上限
const MAX_SCALE: f64 = 3.0;

pub struct LowPoly {
    manifest: Manifest,
}

impl Default for LowPoly {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 解码为 RGBA8 并按 EXIF 方向摆正
fn decode(bytes: &[u8]) -> PluginResult<RgbaImage> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| errp("lowpoly.decode_failed", "detail", e.to_string()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| errp("lowpoly.decode_failed", "detail", e.to_string()))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| errp("lowpoly.decode_failed", "detail", e.to_string()))?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
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
        (Some(data), _) => {
            // 支持 Data URI（data:image/...;base64,）与裸 base64
            let clean = match data.rsplit_once("base64,") {
                Some((_, payload)) => payload,
                None => data,
            };
            BASE64
                .decode(clean.trim().as_bytes())
                .map_err(|e| errp("lowpoly.bad_data", "detail", e.to_string()))?
        }
        (None, Some(path)) => {
            let mut bytes = Vec::new();
            match ctx {
                Some(ctx) => {
                    let size = ctx.file_size(path)?;
                    if size > MAX_INPUT_PIXELS * 8 {
                        return Err(errp("lowpoly.file_too_large", "limit", "192 MB"));
                    }
                    ctx.open_file(path)?.read_to_end(&mut bytes).map_err(|e| {
                        errp("lowpoly.io", "path", path).with("detail", e.to_string())
                    })?;
                }
                None => {
                    let size = tf_plugin_guest::net::file_size(path)?;
                    bytes =
                        tf_plugin_guest::net::read_file(path, 0, size.min(u32::MAX as u64) as u32)?
                }
            }
            bytes
        }
        _ => return Err(PluginError::new("lowpoly.source_required")),
    };
    let image = decode(&bytes)?;
    if u64::from(image.width()) * u64::from(image.height()) > MAX_INPUT_PIXELS {
        return Err(errp(
            "lowpoly.pixels_too_large",
            "limit",
            format!("{} MP", MAX_INPUT_PIXELS / 1_000_000),
        ));
    }
    Ok(image)
}

/// 受限工作图：长边缩到 WORK_EDGE（不放大）
fn work_image(image: &RgbaImage) -> RgbaImage {
    let long = image.width().max(image.height());
    if long <= WORK_EDGE {
        return image.clone();
    }
    let scale = WORK_EDGE as f64 / long as f64;
    let w = ((image.width() as f64 * scale).round() as u32).max(1);
    let h = ((image.height() as f64 * scale).round() as u32).max(1);
    image::imageops::resize(image, w, h, image::imageops::FilterType::Triangle)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareArgs {
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepareOut {
    width: u32,
    height: u32,
    /// 工作图 PNG Data URI（前端保存到项目状态，后续调用复用）
    work_data: String,
}

/// 生成受限工作图；后续 generate/remesh/export 都用它的 base64，原图移动不影响
fn prepare(args: PrepareArgs, ctx: Option<&dyn TaskContext>) -> PluginResult<PrepareOut> {
    let image = load_image(&args.data, &args.path, ctx)?;
    let work = work_image(&image);
    let mut png = Cursor::new(Vec::new());
    work.write_to(&mut png, ImageFormat::Png)
        .map_err(|e| errp("lowpoly.encode_failed", "detail", e.to_string()))?;
    Ok(PrepareOut {
        width: work.width(),
        height: work.height(),
        work_data: format!("data:image/png;base64,{}", BASE64.encode(png.get_ref())),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageInput {
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateArgs {
    image: ImageInput,
    seed: u64,
    /// 期望点数（含四角锚点）
    count: u32,
    /// 细节权重 0-100（越高越贴合边缘）
    #[serde(default = "default_detail")]
    detail: u32,
}

fn default_detail() -> u32 {
    50
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshOut {
    points: Vec<[f64; 2]>,
    triangles: Vec<[usize; 3]>,
}

/// 自动生成特征点并剖分（不取色；取色在 remesh/export 阶段统一做）
fn generate(args: GenerateArgs, ctx: Option<&dyn TaskContext>) -> PluginResult<MeshOut> {
    let image = load_image(&args.image.data, &args.image.path, ctx)?;
    let work = work_image(&image);
    let count = (args.count.min(MAX_POINTS as u32) as usize).max(4);
    let detail = args.detail.clamp(0, 100) as f64 / 100.0;
    let pts = points::generate(&work, args.seed, count, detail);
    let triangles = delaunay::triangulate(&pts);
    if triangles.is_empty() {
        return Err(PluginError::new("lowpoly.degenerate"));
    }
    Ok(MeshOut {
        points: pts,
        triangles,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemeshArgs {
    image: ImageInput,
    /// 手动点集（含四角锚点；越界点会被钳制回画布）
    points: Vec<[f64; 2]>,
    #[serde(default = "default_brightness")]
    brightness: i32,
}

fn default_brightness() -> i32 {
    0
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemeshOut {
    points: Vec<[f64; 2]>,
    triangles: Vec<[usize; 3]>,
    /// 每个三角的采样色（#RRGGBB，已应用亮度）
    colors: Vec<String>,
}

fn clamp_points(points: &[[f64; 2]], width: f64, height: f64) -> Vec<[f64; 2]> {
    points
        .iter()
        .map(|p| [p[0].clamp(0.0, width), p[1].clamp(0.0, height)])
        .collect()
}

/// 手动点集重剖分 + 取色
fn remesh(args: RemeshArgs, ctx: Option<&dyn TaskContext>) -> PluginResult<RemeshOut> {
    if args.points.len() > MAX_POINTS {
        return Err(errp("lowpoly.too_many_points", "limit", MAX_POINTS as u64));
    }
    let image = load_image(&args.image.data, &args.image.path, ctx)?;
    let work = work_image(&image);
    let clamped = clamp_points(
        &args.points,
        f64::from(work.width() - 1),
        f64::from(work.height() - 1),
    );
    let triangles = delaunay::triangulate(&clamped);
    if triangles.is_empty() {
        return Err(PluginError::new("lowpoly.degenerate"));
    }
    let colors = sample::colors(&work, &clamped, &triangles, args.brightness);
    Ok(RemeshOut {
        points: clamped,
        triangles,
        colors,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    image: ImageInput,
    points: Vec<[f64; 2]>,
    /// 导出缩放（1-3，相对工作图）
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_brightness")]
    brightness: i32,
    /// 轮廓线宽（像素，0 为无轮廓）
    #[serde(default)]
    stroke_width: u32,
    #[serde(default = "default_stroke_color")]
    stroke_color: String,
    /// 透明区域底色 #RRGGBB
    #[serde(default = "default_background")]
    background: String,
    format: Format,
    output_path: String,
}

fn default_scale() -> f64 {
    1.0
}

fn default_stroke_color() -> String {
    "111111".to_owned()
}

fn default_background() -> String {
    "ffffff".to_owned()
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Format {
    Png,
    Svg,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportReport {
    output: String,
    width: u32,
    height: u32,
    triangles: usize,
    bytes: u64,
}

/// 导出 PNG/SVG；任务内逐阶段可取消
fn export(args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<ExportReport> {
    if args.output_path.trim().is_empty() {
        return Err(PluginError::new("lowpoly.output_required"));
    }
    if !(0.1..=MAX_SCALE).contains(&args.scale) {
        return Err(errp("lowpoly.bad_scale", "max", MAX_SCALE));
    }
    if args.points.len() > MAX_POINTS {
        return Err(errp("lowpoly.too_many_points", "limit", MAX_POINTS as u64));
    }
    let image = load_image(&args.image.data, &args.image.path, Some(ctx))?;
    let work = work_image(&image);
    let clamped = clamp_points(
        &args.points,
        f64::from(work.width() - 1),
        f64::from(work.height() - 1),
    );
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let triangles = delaunay::triangulate(&clamped);
    if triangles.is_empty() {
        return Err(PluginError::new("lowpoly.degenerate"));
    }
    ctx.stage("lowpoly.sample");
    let colors = sample::colors(&work, &clamped, &triangles, args.brightness);
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let background = sample::parse_color(&args.background)
        .ok_or_else(|| errp("lowpoly.bad_color", "value", args.background.as_str()))?;
    let stroke = sample::parse_color(&args.stroke_color)
        .ok_or_else(|| errp("lowpoly.bad_color", "value", args.stroke_color.as_str()))?;
    ctx.stage("lowpoly.render");
    let bytes = match args.format {
        Format::Png => render::render_png(
            &work,
            &clamped,
            &triangles,
            &colors,
            args.scale,
            args.stroke_width,
            stroke,
            background,
        )?,
        Format::Svg => render::render_svg(
            &work,
            &clamped,
            &triangles,
            &colors,
            args.scale,
            args.stroke_width,
            args.stroke_color.clone(),
            format!(
                "#{:02x}{:02x}{:02x}",
                background[0], background[1], background[2]
            ),
        )
        .into_bytes(),
    };
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let out_w = ((f64::from(work.width()) * args.scale).round() as u32).max(1);
    let out_h = ((f64::from(work.height()) * args.scale).round() as u32).max(1);
    tf_plugin_guest::net::write_file(&args.output_path, &bytes).map_err(|e| {
        errp("lowpoly.io", "path", args.output_path.as_str()).with("detail", e.to_string())
    })?;
    ctx.log(
        LogLevel::Info,
        "lowpoly.exported",
        serde_json::json!({ "triangles": triangles.len(), "bytes": bytes.len() }),
    );
    Ok(ExportReport {
        output: args.output_path,
        width: out_w,
        height: out_h,
        triangles: triangles.len(),
        bytes: bytes.len() as u64,
    })
}

impl ToolPlugin for LowPoly {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "prepare" => to_value(prepare(parse_args(args)?, None)?),
            "generate" => to_value(generate(parse_args(args)?, None)?),
            "remesh" => to_value(remesh(parse_args(args)?, None)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "export" => to_value(export(parse_args(args)?, ctx)?),
            "prepare" => to_value(prepare(parse_args(args)?, Some(ctx))?),
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(LowPoly);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
