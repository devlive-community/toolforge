//! PDF 工具插件后端：查看页面与文档信息、生成缩略图，按页面列表合并 / 提取 / 重排 / 旋转，
//! 拆分为多个文件，以及把图片合成为 PDF。

mod compose;
mod images;
mod render;
mod source;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use source::Source;

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct ThumbnailArgs {
    #[serde(flatten)]
    source: Source,
    pages: Vec<u32>,
    #[serde(default = "default_width")]
    width: u32,
}

fn default_width() -> u32 {
    160
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComposeArgs {
    sources: Vec<Source>,
    pages: Vec<compose::PageRef>,
    output: String,
    #[serde(flatten)]
    options: compose::Options,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SplitArgs {
    #[serde(flatten)]
    source: Source,
    #[serde(flatten)]
    split: compose::Split,
    #[serde(default)]
    ranges: String,
    /// 输出目录；为空时为源文件所在目录
    #[serde(default)]
    output_dir: Option<String>,
}

fn check_output(output: &str) -> PluginResult<PathBuf> {
    let path = PathBuf::from(output);
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() || parent.is_dir() => Ok(path),
        _ => Err(PluginError::new("pdf.output_dir_missing").with("path", output)),
    }
}

pub struct PdfTools {
    manifest: Manifest,
}

impl Default for PdfTools {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl PdfTools {
    fn compose(&self, args: ComposeArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let output = check_output(&args.output)?;
        ctx.stage("pdf.compose");
        let doc = compose::compose(&args.sources, &args.pages, &args.options)?;
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let size = compose::save(doc, &output)?;
        ctx.log(
            LogLevel::Info,
            "pdf.saved",
            json!({ "path": args.output, "pages": args.pages.len(), "size": size }),
        );
        Ok(json!({ "output": args.output, "pages": args.pages.len(), "size": size }))
    }

    fn split(&self, args: SplitArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let info = source::info(&args.source)?;
        let count = info.pages.len() as u32;
        let parts = compose::split_parts(args.split, &args.ranges, count)?;
        let source_path = Path::new(&args.source.path);
        let dir = match args.output_dir.as_deref().filter(|d| !d.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => source_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        };
        if !dir.is_dir() {
            return Err(PluginError::new("pdf.output_dir_missing")
                .with("path", dir.to_string_lossy().as_ref()));
        }
        let stem = source_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document".into());
        ctx.stage("pdf.split");
        let sources = [args.source.clone()];
        let mut outputs = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let pages: Vec<compose::PageRef> = (part.0..=part.1)
                .map(|page| compose::PageRef {
                    source: 0,
                    page,
                    rotate: 0,
                })
                .collect();
            let doc = compose::compose(&sources, &pages, &compose::Options::default())?;
            let path = compose::part_path(&dir, &stem, *part);
            let size = compose::save(doc, &path)?;
            ctx.log(LogLevel::Info, "pdf.part_saved", json!({ "path": path.to_string_lossy(), "from": part.0, "to": part.1, "size": size }));
            outputs.push(path.to_string_lossy().into_owned());
            ctx.progress(index as u64 + 1, parts.len() as u64);
        }
        Ok(json!({ "outputs": outputs, "dir": dir.to_string_lossy() }))
    }

    fn images(&self, args: images::Args, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let output = check_output(&args.output)?;
        ctx.stage("pdf.images");
        let total = args.paths.len() as u64;
        let doc = images::images_to_pdf(
            &args,
            |done| ctx.progress(done as u64, total),
            || ctx.is_cancelled(),
        )?;
        let size = compose::save(doc, &output)?;
        ctx.log(
            LogLevel::Info,
            "pdf.saved",
            json!({ "path": args.output, "pages": total, "size": size }),
        );
        Ok(json!({ "output": args.output, "pages": total, "size": size }))
    }
}

impl ToolPlugin for PdfTools {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => to_value(source::info(&parse_args(args)?)?),
            "thumbnails" => {
                let args: ThumbnailArgs = parse_args(args)?;
                to_value(render::thumbnails(&args.source, &args.pages, args.width)?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "compose" => self.compose(parse_args(args)?, ctx),
            "split" => self.split(parse_args(args)?, ctx),
            "images" => self.images(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
