use std::io::Read;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::algo::{Algorithm, MultiHasher};

const CHUNK: usize = 1024 * 1024;

#[derive(Deserialize)]
pub struct Args {
    paths: Vec<String>,
    algorithms: Vec<Algorithm>,
    #[serde(default)]
    uppercase: bool,
}

#[derive(Debug, Serialize)]
pub struct FileResult {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub digests: Map<String, Value>,
    pub error: Option<PluginError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub files: Vec<FileResult>,
    pub total_bytes: u64,
    pub elapsed_ms: u64,
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

fn algorithm_name(algorithm: Algorithm) -> String {
    serde_json::to_value(algorithm)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// 批量计算文件摘要：逐块读取，一次读取同时喂给所有算法；实时输出日志与字节进度，
/// 单个文件失败不影响其他文件，用户取消后立即停止。
pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    if args.paths.is_empty() {
        return Err(PluginError::new("hash.no_files"));
    }
    if args.algorithms.is_empty() {
        return Err(PluginError::new("hash.no_algorithms"));
    }
    let start = Instant::now();

    ctx.stage("hash.scan");
    let sizes: Vec<Option<u64>> = args.paths.iter().map(|p| ctx.file_size(p).ok()).collect();
    let total: u64 = sizes.iter().flatten().sum();
    ctx.log(
        LogLevel::Info,
        "hash.scanned",
        json!({ "count": args.paths.len(), "bytes": total }),
    );

    ctx.stage("hash.compute");
    let mut done = 0u64;
    let mut files = Vec::with_capacity(args.paths.len());
    let mut buffer = vec![0u8; CHUNK];

    for (index, path) in args.paths.iter().enumerate() {
        let name = file_name(path);
        let size = sizes[index].unwrap_or(0);
        ctx.log(
            LogLevel::Info,
            "hash.file_start",
            json!({ "file": name, "index": index + 1, "count": args.paths.len(), "size": size }),
        );
        let file_start = Instant::now();

        let outcome = (|| -> PluginResult<Vec<(Algorithm, String)>> {
            let mut reader = ctx.open_file(path)?;
            let mut hasher = MultiHasher::new(&args.algorithms);
            loop {
                if ctx.is_cancelled() {
                    return Err(cancelled());
                }
                let read = reader.read(&mut buffer).map_err(|e| {
                    PluginError::new("fs.io")
                        .with("path", path.as_str())
                        .with("detail", e.to_string())
                })?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                done += read as u64;
                ctx.progress(done, total.max(done));
            }
            Ok(hasher.finish(args.uppercase))
        })();

        match outcome {
            Ok(digests) => {
                for (algorithm, digest) in &digests {
                    ctx.log(
                        LogLevel::Debug,
                        "hash.digest",
                        json!({ "file": name, "algorithm": algorithm_name(*algorithm), "digest": digest }),
                    );
                }
                ctx.log(
                    LogLevel::Info,
                    "hash.file_done",
                    json!({ "file": name, "ms": file_start.elapsed().as_millis() as u64 }),
                );
                files.push(FileResult {
                    path: path.clone(),
                    name,
                    size,
                    digests: digests
                        .into_iter()
                        .map(|(a, d)| (algorithm_name(a), Value::String(d)))
                        .collect(),
                    error: None,
                });
            }
            Err(err) if err.code == "task.cancelled" => return Err(err),
            Err(err) => {
                ctx.log(
                    LogLevel::Warn,
                    "hash.file_failed",
                    json!({ "file": name, "code": err.code }),
                );
                files.push(FileResult {
                    path: path.clone(),
                    name,
                    size,
                    digests: Map::new(),
                    error: Some(err),
                });
            }
        }
    }

    Ok(Report {
        files,
        total_bytes: done,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
#[path = "files_test.rs"]
mod tests;
