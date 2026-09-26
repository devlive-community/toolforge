//! 批量压缩任务：多个文件并行处理，逐个输出实时日志；单个文件失败不影响其他文件。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::engine::{self, Format, Options};

const MAX_FILE: u64 = 200 * 1024 * 1024;
const MAX_WORKERS: usize = 4;
pub const EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

fn default_suffix() -> String {
    "-min".to_owned()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub paths: Vec<String>,
    #[serde(default)]
    pub options: Options,
    /// 输出目录；为空时输出到源文件所在目录
    #[serde(default)]
    pub output_dir: Option<String>,
    /// 追加到输出文件名（扩展名之前）
    #[serde(default = "default_suffix")]
    pub suffix: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub path: String,
    pub name: String,
    pub output: Option<String>,
    pub format: Option<Format>,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub width: u32,
    pub height: u32,
    /// 压缩后反而更大，已原样保留原图
    pub kept_original: bool,
    pub ms: u64,
    pub error: Option<PluginError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub files: Vec<FileResult>,
    pub succeeded: usize,
    pub total_before: u64,
    pub total_after: u64,
    pub elapsed_ms: u64,
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 在输出目录中生成不与已有文件（包括源文件）冲突的路径
pub fn output_path(source: &Path, dir: &Path, suffix: &str, extension: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_owned());
    let mut candidate = dir.join(format!("{stem}{suffix}.{extension}"));
    let mut index = 1;
    while candidate.exists() || candidate == source {
        candidate = dir.join(format!("{stem}{suffix} ({index}).{extension}"));
        index += 1;
    }
    candidate
}

/// 格式不变时沿用源文件的扩展名（如 .jpeg、.JPG）
fn extension_for(source: &Path, source_format: Format, format: Format) -> String {
    let original = source
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    if format == source_format && !original.is_empty() {
        original
    } else {
        format.extension().to_owned()
    }
}

fn io_error(path: &Path, err: std::io::Error) -> PluginError {
    PluginError::new("fs.io")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", err.to_string())
}

pub fn read_source(path: &Path, ctx: &dyn TaskContext) -> PluginResult<Vec<u8>> {
    let raw = path.to_string_lossy();
    if ctx.file_size(&raw)? > MAX_FILE {
        return Err(PluginError::new("image.too_large").with("limit", "200 MB"));
    }
    let mut bytes = Vec::new();
    ctx.open_file(&raw)?
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(path, e))?;
    Ok(bytes)
}

fn compress_one(source: &Path, args: &Args, ctx: &dyn TaskContext) -> PluginResult<FileResult> {
    let started = Instant::now();
    let original = read_source(source, ctx)?;
    let before = original.len() as u64;
    let source_format = engine::decode_format(&original)?;
    let compressed = engine::compress(&original, &args.options)?;
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    // 格式与尺寸都没变但结果更大时保留原图，压缩永远不会让文件变大
    let unchanged = compressed.format == source_format && args.options.max_size == 0;
    let kept_original = unchanged && compressed.bytes.len() as u64 >= before;
    let bytes = if kept_original {
        &original
    } else {
        &compressed.bytes
    };

    let dir = match args.output_dir.as_deref().filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => source.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let extension = extension_for(source, source_format, compressed.format);
    let output = output_path(source, &dir, &args.suffix, &extension);
    std::fs::write(&output, bytes).map_err(|e| io_error(&output, e))?;

    Ok(FileResult {
        path: source.to_string_lossy().into_owned(),
        name: file_name(source),
        output: Some(output.to_string_lossy().into_owned()),
        format: Some(compressed.format),
        before_bytes: before,
        after_bytes: bytes.len() as u64,
        width: compressed.width,
        height: compressed.height,
        kept_original,
        ms: started.elapsed().as_millis() as u64,
        error: None,
    })
}

pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    if args.paths.is_empty() {
        return Err(PluginError::new("image.no_files"));
    }
    let output_dir = args.output_dir.as_deref().filter(|d| !d.is_empty());
    if let Some(dir) = output_dir
        && !Path::new(dir).is_dir()
    {
        return Err(PluginError::new("image.output_dir_missing").with("path", dir));
    }
    if output_dir.is_none() && args.suffix.trim().is_empty() {
        // 同目录且无后缀时 output_path 会自动编号，但用户通常以为是覆盖原图，这里明确拒绝
        return Err(PluginError::new("image.suffix_required"));
    }
    let started = Instant::now();
    let count = args.paths.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<FileResult>>> = Mutex::new((0..count).map(|_| None).collect());
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .min(MAX_WORKERS)
        .min(count);

    ctx.stage("image.compress");
    ctx.progress(0, count as u64);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= count || ctx.is_cancelled() {
                        break;
                    }
                    let source = Path::new(&args.paths[index]);
                    let name = file_name(source);
                    let result = match compress_one(source, &args, ctx) {
                        Ok(result) => {
                            ctx.log(
                                LogLevel::Info,
                                if result.kept_original {
                                    "image.file_kept"
                                } else {
                                    "image.file_done"
                                },
                                json!({
                                    "file": name,
                                    "output": result.output.as_deref().map(|o| file_name(Path::new(o))),
                                    "before": result.before_bytes,
                                    "after": result.after_bytes,
                                    "ms": result.ms,
                                }),
                            );
                            result
                        }
                        Err(error) => {
                            if error.code != "task.cancelled" {
                                ctx.log(
                                    LogLevel::Error,
                                    "image.file_failed",
                                    json!({ "file": name, "code": error.code }),
                                );
                            }
                            FileResult {
                                path: args.paths[index].clone(),
                                name,
                                output: None,
                                format: None,
                                before_bytes: ctx.file_size(&args.paths[index]).unwrap_or(0),
                                after_bytes: 0,
                                width: 0,
                                height: 0,
                                kept_original: false,
                                ms: 0,
                                error: Some(error),
                            }
                        }
                    };
                    if let Ok(mut results) = results.lock() {
                        results[index] = Some(result);
                    }
                    let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                    ctx.progress(finished as u64, count as u64);
                }
            });
        }
    });
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let files: Vec<FileResult> = results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect();
    let ok = files.iter().filter(|f| f.error.is_none());
    let report = Report {
        succeeded: ok.clone().count(),
        total_before: ok.clone().map(|f| f.before_bytes).sum(),
        total_after: ok.map(|f| f.after_bytes).sum(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        files,
    };
    ctx.log(
        LogLevel::Info,
        "image.summary",
        json!({
            "succeeded": report.succeeded,
            "count": count,
            "before": report.total_before,
            "after": report.total_after,
            "ms": report.elapsed_ms,
        }),
    );
    Ok(report)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub size: u64,
}

const MAX_SCAN: usize = 5000;

fn is_image(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|e| EXTENSIONS.contains(&e.as_str()))
}

fn walk(path: &Path, out: &mut Vec<Entry>) {
    if out.len() >= MAX_SCAN {
        return;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| !file_name(p).starts_with('.'))
            .collect();
        children.sort();
        for child in children {
            walk(&child, out);
        }
    } else if is_image(path) {
        out.push(Entry {
            path: path.to_string_lossy().into_owned(),
            name: file_name(path),
            size: meta.len(),
        });
    }
}

/// 展开文件与文件夹（递归）中的图片，最多 5000 个
pub fn scan(paths: &[String]) -> Vec<Entry> {
    let mut out = Vec::new();
    for path in paths {
        walk(Path::new(path), &mut out);
    }
    out
}

#[cfg(test)]
#[path = "batch_test.rs"]
mod tests;
