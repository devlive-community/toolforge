//! 文档转换插件后端：Excel → Word、Word → Excel 用纯 Rust 完成；
//! Word → PDF、PDF → Word 以及旧格式（.doc / .rtf / .odt）的预转换调用本机的 LibreOffice。

mod engine;
mod excel;
#[cfg(test)]
mod test_support;
mod word;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use engine::Engine;

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Mode {
    ExcelToWord,
    WordToExcel,
    WordToPdf,
    PdfToWord,
}

const EXCEL: &[&str] = &["xlsx", "xlsm", "xlsb", "xls", "ods"];
const WORD: &[&str] = &["docx", "doc", "rtf", "odt", "wps", "dotx"];

impl Mode {
    fn accepts(self, ext: &str) -> bool {
        match self {
            Mode::ExcelToWord => EXCEL.contains(&ext),
            Mode::WordToExcel | Mode::WordToPdf => WORD.contains(&ext),
            Mode::PdfToWord => ext == "pdf",
        }
    }

    fn output_ext(self) -> &'static str {
        match self {
            Mode::ExcelToWord | Mode::PdfToWord => "docx",
            Mode::WordToExcel => "xlsx",
            Mode::WordToPdf => "pdf",
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvertArgs {
    mode: Mode,
    inputs: Vec<String>,
    /// 为空时保存到原文件所在文件夹
    #[serde(default)]
    output_dir: Option<String>,
    #[serde(default = "yes")]
    header_row: bool,
    #[serde(default = "yes")]
    sheet_titles: bool,
    /// Word → Excel 工作表名称（由界面按语言传入）
    #[serde(default)]
    table_name: Option<String>,
    #[serde(default)]
    text_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    input: String,
    output: Option<String>,
    error: Option<PluginError>,
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into())
}

/// 不覆盖已有文件：name.ext → name (1).ext
fn unique(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// 每次转换用独立的临时文件夹，结束后删除
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> PluginResult<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("toolforge-doc-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| PluginError::new("doc.write_failed").with("detail", e.to_string()))?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn need_engine(engine: Option<&Engine>) -> PluginResult<&Engine> {
    engine.ok_or_else(|| PluginError::new("doc.needs_libreoffice"))
}

/// 移动 LibreOffice 的输出到目标位置（跨磁盘时复制）
fn place(produced: &Path, target: &Path) -> PluginResult<()> {
    if std::fs::rename(produced, target).is_err() {
        std::fs::copy(produced, target)
            .map_err(|e| PluginError::new("doc.write_failed").with("detail", e.to_string()))?;
    }
    Ok(())
}

fn convert_one(
    args: &ConvertArgs,
    input: &Path,
    engine: Option<&Engine>,
    ctx: &dyn TaskContext,
) -> PluginResult<PathBuf> {
    if !input.is_file() {
        return Err(PluginError::new("fs.not_found").with("path", input.to_string_lossy().as_ref()));
    }
    let ext = ext_of(input);
    if !args.mode.accepts(&ext) {
        return Err(PluginError::new("doc.unsupported_input").with("ext", ext));
    }
    let dir = match &args.output_dir {
        Some(d) => PathBuf::from(d),
        None => input.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let target = unique(&dir, &stem_of(input), args.mode.output_ext());
    let scratch = Scratch::new()?;
    match args.mode {
        Mode::ExcelToWord => {
            let sheets = excel::to_word(
                input,
                &target,
                &excel::Options {
                    header_row: args.header_row,
                    sheet_titles: args.sheet_titles,
                },
            )?;
            ctx.log(LogLevel::Debug, "doc.sheets", json!({ "count": sheets }));
        }
        Mode::WordToExcel => {
            // 旧格式先用 LibreOffice 转成 docx
            let docx = if ext == "docx" {
                input.to_path_buf()
            } else {
                engine::convert(
                    need_engine(engine)?,
                    input,
                    &scratch.0,
                    "docx:MS Word 2007 XML",
                    None,
                    ctx,
                )?
            };
            let options = word::Options {
                header_row: args.header_row,
                table_name: args.table_name.clone().unwrap_or_else(|| "Table".into()),
                text_name: args.text_name.clone().unwrap_or_else(|| "Text".into()),
            };
            let (count, text_only) = word::to_excel(&docx, &target, &options)?;
            if text_only {
                ctx.log(
                    LogLevel::Warn,
                    "doc.no_tables",
                    json!({ "name": stem_of(input) }),
                );
            } else {
                ctx.log(LogLevel::Debug, "doc.tables", json!({ "count": count }));
            }
        }
        Mode::WordToPdf => {
            let produced =
                engine::convert(need_engine(engine)?, input, &scratch.0, "pdf", None, ctx)?;
            place(&produced, &target)?;
        }
        Mode::PdfToWord => {
            let produced = engine::convert(
                need_engine(engine)?,
                input,
                &scratch.0,
                "docx:MS Word 2007 XML",
                Some("writer_pdf_import"),
                ctx,
            )?;
            place(&produced, &target)?;
        }
    }
    Ok(target)
}

fn run_convert(args: ConvertArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    if args.inputs.is_empty() {
        return Err(PluginError::new("doc.no_files"));
    }
    if let Some(dir) = &args.output_dir
        && !Path::new(dir).is_dir()
    {
        return Err(PluginError::new("doc.output_dir_missing").with("path", dir.as_str()));
    }
    let needs_engine = matches!(args.mode, Mode::WordToPdf | Mode::PdfToWord)
        || (args.mode == Mode::WordToExcel
            && args.inputs.iter().any(|p| ext_of(Path::new(p)) != "docx"));
    let engine = if needs_engine {
        ctx.stage("doc.engine");
        engine::find()
    } else {
        None
    };
    if needs_engine && engine.is_none() && args.mode != Mode::WordToExcel {
        return Err(PluginError::new("doc.needs_libreoffice"));
    }
    ctx.stage("doc.convert");
    let mut items = Vec::new();
    for (i, input) in args.inputs.iter().enumerate() {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let path = Path::new(input);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        ctx.log(LogLevel::Info, "doc.converting", json!({ "name": name }));
        let item = match convert_one(&args, path, engine.as_ref(), ctx) {
            Ok(output) => {
                let output_name = output
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ctx.log(
                    LogLevel::Info,
                    "doc.converted",
                    json!({ "name": name, "output": output_name }),
                );
                Item {
                    input: input.clone(),
                    output: Some(output.to_string_lossy().into_owned()),
                    error: None,
                }
            }
            Err(e) if e.code == "task.cancelled" => return Err(e),
            Err(e) => {
                ctx.log(
                    LogLevel::Error,
                    "doc.failed",
                    json!({ "name": name, "code": e.code }),
                );
                Item {
                    input: input.clone(),
                    output: None,
                    error: Some(e),
                }
            }
        };
        items.push(item);
        ctx.progress(i as u64 + 1, args.inputs.len() as u64);
    }
    let converted = items.iter().filter(|i| i.output.is_some()).count();
    ctx.log(
        LogLevel::Info,
        "doc.done",
        json!({ "converted": converted, "failed": items.len() - converted }),
    );
    Ok(json!({ "items": items, "converted": converted, "failed": items.len() - converted }))
}

pub struct DocConverter {
    manifest: Manifest,
}

impl Default for DocConverter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DocConverter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, _args: Value) -> PluginResult<Value> {
        match function {
            "engine" => to_value(engine::find()),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "convert" => run_convert(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
