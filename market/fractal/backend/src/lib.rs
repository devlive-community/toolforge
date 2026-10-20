//! 分形探索器：Mandelbrot / Julia 平方迭代、渐变配色与 PNG 导出。

mod fractal;
mod palette;
mod render;

use std::io::Cursor;

use data_encoding::BASE64;
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 导出像素上限标签
pub const MAX_PIXELS_LABEL: &str = "2.5 MP";

pub struct Fractal {
    manifest: Manifest,
}

impl Default for Fractal {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum KindArg {
    Mandelbrot,
    Julia,
}

impl From<KindArg> for fractal::Kind {
    fn from(value: KindArg) -> Self {
        match value {
            KindArg::Mandelbrot => fractal::Kind::Mandelbrot,
            KindArg::Julia => fractal::Kind::Julia,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewArgs {
    fractal: KindArg,
    center_x: f64,
    center_y: f64,
    /// 复平面水平跨度；非正或超范围报错
    span_x: f64,
    #[serde(default)]
    julia_x: f64,
    #[serde(default)]
    julia_y: f64,
    #[serde(default = "default_iter")]
    max_iter: u32,
    #[serde(default = "default_palette")]
    palette: String,
    #[serde(default)]
    brightness: palette::Brightness,
    width: u32,
    height: u32,
}

fn default_iter() -> u32 {
    256
}

fn default_palette() -> String {
    palette::SPECTRUM.to_owned()
}

fn validate(view: &ViewArgs) -> PluginResult<()> {
    if !(0.0..=4.0).contains(&view.julia_x.abs()) && view.fractal == KindArg::Julia {
        return Err(errp("fractal.bad_julia", "value", view.julia_x));
    }
    if view.max_iter == 0 || view.max_iter > render::MAX_ITER_LIMIT {
        return Err(errp("fractal.bad_iter", "max", render::MAX_ITER_LIMIT));
    }
    if !palette::palettes().contains(&view.palette.as_str()) {
        return Err(errp("fractal.bad_palette", "value", view.palette.as_str()));
    }
    Ok(())
}

fn input_from<'a>(
    view: &'a ViewArgs,
    progress: Option<&'a dyn Fn(u32)>,
    is_cancelled: Option<&'a dyn Fn() -> bool>,
) -> render::RenderInput<'a> {
    render::RenderInput {
        kind: view.fractal.into(),
        center_x: view.center_x,
        center_y: view.center_y,
        span_x: view.span_x,
        julia_x: view.julia_x,
        julia_y: view.julia_y,
        width: view.width,
        height: view.height,
        max_iter: view.max_iter,
        palette: &view.palette,
        brightness: view.brightness,
        progress,
        is_cancelled,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewOut {
    /// 预览 PNG Data URI
    image: String,
    #[serde(flatten)]
    stats: render::RenderStats,
}

/// 快速粗预览（非任务）：建议前端用 480 级别宽度先出图
fn preview(args: ViewArgs) -> PluginResult<PreviewOut> {
    validate(&args)?;
    let input = input_from(&args, None, None);
    let out = render::render(&input)?;
    let mut png = Cursor::new(Vec::new());
    out.image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|e| errp("fractal.encode_failed", "detail", e.to_string()))?;
    Ok(PreviewOut {
        image: format!("data:image/png;base64,{}", BASE64.encode(png.get_ref())),
        stats: out.stats,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    #[serde(flatten)]
    view: ViewArgs,
    output_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportReport {
    output: String,
    width: u32,
    height: u32,
    interior_ratio: f64,
    precision_limited: bool,
    elapsed_ms: u64,
}

/// 全尺寸导出：任务内逐行进度、可取消；取消不落盘
fn export(args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<ExportReport> {
    validate(&args.view)?;
    if args.output_path.trim().is_empty() {
        return Err(errp("fractal.output_required", "hint", ""));
    }
    let cancelled = || ctx.is_cancelled();
    let total = args.view.height;
    let progress = |done: u32| ctx.progress(u64::from(done), u64::from(total));
    let input = input_from(&args.view, Some(&progress), Some(&cancelled));
    let out = render::render(&input)?;
    if ctx.is_cancelled() {
        return Err(tf_plugin_api::cancelled());
    }
    let mut png = Cursor::new(Vec::new());
    out.image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|e| errp("fractal.encode_failed", "detail", e.to_string()))?;
    tf_plugin_guest::net::write_file(&args.output_path, png.get_ref()).map_err(|e| {
        errp("fractal.io", "path", args.output_path.as_str()).with("detail", e.to_string())
    })?;
    Ok(ExportReport {
        output: args.output_path,
        width: out.stats.width,
        height: out.stats.height,
        interior_ratio: out.stats.interior_ratio,
        precision_limited: out.stats.precision_limited,
        elapsed_ms: out.stats.elapsed_ms,
    })
}

impl ToolPlugin for Fractal {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "preview" => to_value(preview(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "export" => to_value(export(parse_args(args)?, ctx)?),
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(Fractal);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
