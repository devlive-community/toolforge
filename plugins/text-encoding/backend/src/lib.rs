//! 文本编码转换插件后端：批量识别文件编码（含 BOM、UTF-16、GBK / Big5 / Shift_JIS 等）与换行符，
//! 预览指定编码下的内容，并转换为目标编码与换行符。

mod convert;
mod detect;

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use convert::{LineEndings, NewLine, Target};
use detect::{Confidence, Detected};

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_FILES: usize = 5_000;
const MAX_SIZE: u64 = 64 * 1024 * 1024;
const PREVIEW_CHARS: usize = 64 * 1024;
/// 扫描文件夹时跳过的目录
const SKIP_DIRS: &[&str] = &["node_modules", "target", "dist", "build", "__pycache__"];

#[derive(Deserialize)]
struct ScanArgs {
    paths: Vec<String>,
    #[serde(default)]
    recursive: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {
    path: String,
    name: String,
    size: u64,
    encoding: Option<&'static str>,
    bom: bool,
    ascii: bool,
    confidence: Option<Confidence>,
    line_endings: Option<LineEndings>,
    /// 无法读取或过大时的错误
    error: Option<PluginError>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Skipped {
    binary: usize,
    too_many: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Scanned {
    files: Vec<FileInfo>,
    skipped: Skipped,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewArgs {
    path: String,
    /// 按此编码读取；为空时自动识别
    #[serde(default)]
    encoding: Option<String>,
    /// 检查能否转换成此编码
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    path: String,
    encoding: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvertArgs {
    files: Vec<Source>,
    to: String,
    #[serde(default)]
    bom: bool,
    #[serde(default)]
    newline: NewLine,
    /// 为空时覆盖原文件
    #[serde(default)]
    output_dir: Option<String>,
    /// 覆盖原文件前保留一份 .bak
    #[serde(default)]
    backup: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    Converted,
    Unchanged,
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    path: String,
    status: Outcome,
    output: Option<String>,
    error: Option<PluginError>,
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

/// 本工具生成的备份（name.bak、name.bak.1）；扫描文件夹时跳过，避免反复转换
fn is_backup(path: &Path) -> bool {
    let name = name_of(path);
    name.ends_with(".bak")
        || name
            .rsplit_once(".bak.")
            .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn walk(path: &Path, recursive: bool, top: bool, out: &mut Vec<PathBuf>, skipped: &mut Skipped) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.is_dir() {
        let skip = path
            .file_name()
            .is_some_and(|n| SKIP_DIRS.contains(&n.to_string_lossy().as_ref()));
        if !top && (!recursive || skip) {
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| !is_hidden(p))
            .collect();
        children.sort();
        for child in children {
            walk(&child, recursive, false, out, skipped);
        }
    } else if !top && is_backup(path) {
    } else if out.len() < MAX_FILES {
        out.push(path.to_path_buf());
    } else {
        skipped.too_many += 1;
    }
}

fn read(path: &Path) -> PluginResult<Vec<u8>> {
    let meta = std::fs::metadata(path).map_err(|_| {
        PluginError::new("fs.not_found").with("path", path.to_string_lossy().as_ref())
    })?;
    if meta.is_dir() {
        return Err(PluginError::new("encoding.not_a_file"));
    }
    if meta.len() > MAX_SIZE {
        return Err(PluginError::new("encoding.too_large").with("size", meta.len()));
    }
    std::fs::read(path).map_err(|e| {
        PluginError::new("encoding.read_failed")
            .with("path", path.to_string_lossy().as_ref())
            .with("detail", e.to_string())
    })
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 识别一个文件；二进制文件返回 None
fn inspect(path: &Path) -> Option<FileInfo> {
    let mut info = FileInfo {
        path: path.to_string_lossy().into_owned(),
        name: name_of(path),
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        encoding: None,
        bom: false,
        ascii: false,
        confidence: None,
        line_endings: None,
        error: None,
    };
    let bytes = match read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            info.error = Some(error);
            return Some(info);
        }
    };
    let Detected::Text {
        encoding,
        bom,
        ascii,
        confidence,
    } = detect::detect(&bytes)
    else {
        return None;
    };
    let (text, _) = convert::decode_lossy(&bytes, encoding);
    info.encoding = Some(encoding.name());
    info.bom = bom;
    info.ascii = ascii;
    info.confidence = Some(confidence);
    info.line_endings = Some(convert::line_endings(&text));
    Some(info)
}

fn scan(args: &ScanArgs) -> Scanned {
    let mut paths = Vec::new();
    let mut skipped = Skipped::default();
    for path in &args.paths {
        walk(
            Path::new(path),
            args.recursive,
            true,
            &mut paths,
            &mut skipped,
        );
    }
    let infos: Vec<Option<FileInfo>> = paths.par_iter().map(|p| inspect(p)).collect();
    skipped.binary = infos.iter().filter(|i| i.is_none()).count();
    Scanned {
        files: infos.into_iter().flatten().collect(),
        skipped,
    }
}

fn preview(args: &PreviewArgs) -> PluginResult<Value> {
    let bytes = read(Path::new(&args.path))?;
    let encoding = match &args.encoding {
        Some(name) => convert::encoding(name)?,
        None => match detect::detect(&bytes) {
            Detected::Text { encoding, .. } => encoding,
            Detected::Binary => return Err(PluginError::new("encoding.binary")),
        },
    };
    let (text, malformed) = convert::decode_lossy(&bytes, encoding);
    let unmappable = match &args.target {
        Some(name) => convert::first_unmappable(&text, convert::encoding(name)?)
            .map(|(c, line)| json!({ "char": c.to_string(), "code": format!("U+{:04X}", c as u32), "line": line })),
        None => None,
    };
    let end = text
        .char_indices()
        .nth(PREVIEW_CHARS)
        .map_or(text.len(), |(i, _)| i);
    Ok(json!({
        "encoding": encoding.name(),
        "text": &text[..end],
        "truncated": end < text.len(),
        "malformed": malformed,
        "lineEndings": convert::line_endings(&text),
        "unmappable": unmappable,
    }))
}

/// 生成不覆盖已有文件的路径：name.ext → name (1).ext
fn unique(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(path)
}

fn write_failed(path: &Path, e: std::io::Error) -> PluginError {
    PluginError::new("encoding.write_failed")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", e.to_string())
}

/// 先写临时文件再改名，保留原文件权限
fn replace(path: &Path, bytes: &[u8]) -> PluginResult<()> {
    let tmp = path.with_file_name(format!(".{}.tf-part", name_of(path)));
    std::fs::write(&tmp, bytes).map_err(|e| write_failed(&tmp, e))?;
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        write_failed(path, e)
    })
}

fn backup_path(path: &Path) -> PathBuf {
    let base = format!("{}.bak", name_of(path));
    (0..)
        .map(|n| {
            path.with_file_name(if n == 0 {
                base.clone()
            } else {
                format!("{base}.{n}")
            })
        })
        .find(|p| !p.exists())
        .expect("a free backup name")
}

fn convert_one(source: &Source, target: Target, args: &ConvertArgs) -> PluginResult<Item> {
    let path = Path::new(&source.path);
    let bytes = read(path)?;
    let from = convert::encoding(&source.encoding)?;
    let converted = convert::transcode(&bytes, from, target)?;
    let item = |status, output: Option<&Path>| Item {
        path: source.path.clone(),
        status,
        output: output.map(|p| p.to_string_lossy().into_owned()),
        error: None,
    };
    match &args.output_dir {
        // 输出到文件夹时，没有变化的文件也复制过去，保证输出完整
        Some(dir) => {
            let out = unique(Path::new(dir).join(name_of(path)));
            let status = if converted.is_some() {
                Outcome::Converted
            } else {
                Outcome::Unchanged
            };
            std::fs::write(&out, converted.as_deref().unwrap_or(&bytes))
                .map_err(|e| write_failed(&out, e))?;
            Ok(item(status, Some(&out)))
        }
        None => match converted {
            None => Ok(item(Outcome::Unchanged, None)),
            Some(out) => {
                if args.backup {
                    let bak = backup_path(path);
                    std::fs::copy(path, &bak).map_err(|e| write_failed(&bak, e))?;
                }
                replace(path, &out)?;
                Ok(item(Outcome::Converted, Some(path)))
            }
        },
    }
}

fn run_convert(args: ConvertArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let target = Target {
        encoding: convert::encoding(&args.to)?,
        bom: args.bom,
        newline: args.newline,
    };
    if let Some(dir) = &args.output_dir
        && !Path::new(dir).is_dir()
    {
        return Err(PluginError::new("encoding.output_dir_missing").with("path", dir.as_str()));
    }
    if args.files.is_empty() {
        return Err(PluginError::new("encoding.no_files"));
    }
    ctx.stage("encoding.convert");
    let total = args.files.len() as u64;
    let mut items = Vec::with_capacity(args.files.len());
    for source in &args.files {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let item = convert_one(source, target, &args).unwrap_or_else(|error| Item {
            path: source.path.clone(),
            status: Outcome::Failed,
            output: None,
            error: Some(error),
        });
        let name = name_of(Path::new(&source.path));
        match (&item.status, &item.error) {
            (Outcome::Converted, _) => ctx.log(
                LogLevel::Info,
                "encoding.converted",
                json!({ "name": name, "from": source.encoding, "to": target.encoding.name() }),
            ),
            (Outcome::Unchanged, _) => ctx.log(
                LogLevel::Debug,
                "encoding.unchanged",
                json!({ "name": name }),
            ),
            (Outcome::Failed, Some(error)) => ctx.log(
                LogLevel::Error,
                "encoding.failed",
                json!({ "name": name, "code": error.code, "params": error.params }),
            ),
            (Outcome::Failed, None) => {}
        }
        items.push(item);
        ctx.progress(items.len() as u64, total);
    }
    let count = |want: fn(&Outcome) -> bool| items.iter().filter(|i| want(&i.status)).count();
    let summary = json!({
        "converted": count(|s| matches!(s, Outcome::Converted)),
        "unchanged": count(|s| matches!(s, Outcome::Unchanged)),
        "failed": count(|s| matches!(s, Outcome::Failed)),
    });
    ctx.log(LogLevel::Info, "encoding.done", summary.clone());
    Ok(json!({ "items": items, "summary": summary }))
}

pub struct TextEncoding {
    manifest: Manifest,
}

impl Default for TextEncoding {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TextEncoding {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "encodings" => to_value(convert::ENCODINGS),
            "scan" => to_value(scan(&parse_args(args)?)),
            "preview" => preview(&parse_args(args)?),
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
