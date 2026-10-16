//! 校验和清单：为目录生成 sha256sum / md5sum 兼容的清单，并按清单批量校验文件。
//!
//! 生成格式为 GNU 文本格式 `摘要␠␠相对路径`；读取时同时支持 GNU（含 `*` 二进制标记、
//! 反斜杠转义）与 BSD 格式 `SHA256 (路径) = 摘要`。

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::algo::{Algorithm, MultiHasher};

const CHUNK: usize = 1024 * 1024;
/// 结果列表最多返回的条目数（失败与缺失优先），完整结果写在日志中
const MAX_ENTRIES: usize = 5000;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

pub fn extension(algorithm: Algorithm) -> Option<&'static str> {
    Some(match algorithm {
        Algorithm::Md5 => "md5",
        Algorithm::Sha1 => "sha1",
        Algorithm::Sha224 => "sha224",
        Algorithm::Sha256 => "sha256",
        Algorithm::Sha384 => "sha384",
        Algorithm::Sha512 => "sha512",
        Algorithm::Sha3_256 => "sha3-256",
        Algorithm::Sha3_512 => "sha3-512",
        Algorithm::Sm3 => "sm3",
        Algorithm::Crc32 => return None,
    })
}

/// BSD 格式中的算法标签
fn bsd_tag(tag: &str) -> Option<Algorithm> {
    Some(match tag.to_ascii_uppercase().replace('-', "").as_str() {
        "MD5" => Algorithm::Md5,
        "SHA1" => Algorithm::Sha1,
        "SHA224" => Algorithm::Sha224,
        "SHA256" => Algorithm::Sha256,
        "SHA384" => Algorithm::Sha384,
        "SHA512" => Algorithm::Sha512,
        "SHA3256" => Algorithm::Sha3_256,
        "SHA3512" => Algorithm::Sha3_512,
        "SM3" => Algorithm::Sm3,
        _ => return None,
    })
}

/// 按清单扩展名与摘要长度猜测算法（64 位十六进制默认 SHA-256）
fn guess(path: &Path, hex_len: usize) -> Option<Algorithm> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let by_ext = [
        Algorithm::Md5,
        Algorithm::Sha1,
        Algorithm::Sha224,
        Algorithm::Sha256,
        Algorithm::Sha384,
        Algorithm::Sha512,
        Algorithm::Sha3_256,
        Algorithm::Sha3_512,
        Algorithm::Sm3,
    ]
    .into_iter()
    .find(|a| {
        let name = extension(*a).unwrap_or_default();
        ext == name || ext == format!("{name}sum") || ext == format!("{name}sums")
    });
    by_ext.or(match hex_len {
        32 => Some(Algorithm::Md5),
        40 => Some(Algorithm::Sha1),
        56 => Some(Algorithm::Sha224),
        64 => Some(Algorithm::Sha256),
        96 => Some(Algorithm::Sha384),
        128 => Some(Algorithm::Sha512),
        _ => None,
    })
}

fn escape_name(name: &str) -> (bool, String) {
    if name.contains(['\\', '\n', '\r']) {
        let escaped = name
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        (true, escaped)
    } else {
        (false, name.to_owned())
    }
}

fn unescape_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub line: usize,
    pub name: String,
    pub digest: String,
    pub algorithm: Option<Algorithm>,
}

/// 解析清单文本；无法识别的非空、非注释行记为错误行号
pub fn parse(text: &str) -> (Vec<Line>, Vec<usize>) {
    let mut entries = Vec::new();
    let mut invalid = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (escaped, body) = match line.strip_prefix('\\') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        // BSD：TAG (name) = digest
        if let Some((head, digest)) = body.rsplit_once(") = ")
            && let Some((tag, name)) = head.split_once(" (")
            && let Some(algorithm) = bsd_tag(tag)
            && digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            entries.push(Line {
                line: number,
                name: if escaped {
                    unescape_name(name)
                } else {
                    name.to_owned()
                },
                digest: digest.to_ascii_lowercase(),
                algorithm: Some(algorithm),
            });
            continue;
        }
        // GNU：digest␠␠name 或 digest␠*name
        let parsed = body.split_once(' ').and_then(|(digest, rest)| {
            let name = rest.strip_prefix(['*', ' ']).unwrap_or(rest);
            (!digest.is_empty()
                && digest.bytes().all(|b| b.is_ascii_hexdigit())
                && !name.is_empty())
            .then_some((digest, name))
        });
        match parsed {
            Some((digest, name)) => entries.push(Line {
                line: number,
                name: if escaped {
                    unescape_name(name)
                } else {
                    name.to_owned()
                },
                digest: digest.to_ascii_lowercase(),
                algorithm: None,
            }),
            None => invalid.push(number),
        }
    }
    (entries, invalid)
}

fn io_error(path: &Path, e: std::io::Error) -> PluginError {
    PluginError::new("fs.io")
        .with("path", path.to_string_lossy().into_owned())
        .with("detail", e.to_string())
}

/// 分块读取并计算单个文件的摘要
fn digest_file(
    path: &Path,
    algorithm: Algorithm,
    ctx: &dyn TaskContext,
    done: &AtomicU64,
    total: u64,
) -> PluginResult<String> {
    let mut file = File::open(path).map_err(|e| io_error(path, e))?;
    let mut hasher = MultiHasher::new(&[algorithm]);
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let read = file.read(&mut buffer).map_err(|e| io_error(path, e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        let now = done.fetch_add(read as u64, Ordering::Relaxed) + read as u64;
        ctx.progress(now, total.max(now));
    }
    Ok(hasher
        .finish(false)
        .pop()
        .map(|(_, digest)| digest)
        .unwrap_or_default())
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Failed,
    Missing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub path: String,
    pub status: Status,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub error: Option<PluginError>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub ok: usize,
    pub failed: usize,
    pub missing: usize,
    pub errors: usize,
    pub invalid_lines: Vec<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub algorithm: Algorithm,
    pub entries: Vec<Entry>,
    pub truncated: bool,
    pub summary: Summary,
    pub total_bytes: u64,
    pub elapsed_ms: u64,
    /// 生成清单时写出的文件路径
    pub output: Option<String>,
}

fn finish_entries(mut entries: Vec<Entry>) -> (Vec<Entry>, bool) {
    // 失败、缺失与出错的排在前面，其余保持原顺序
    entries.sort_by_key(|e| e.status == Status::Ok);
    let truncated = entries.len() > MAX_ENTRIES;
    entries.truncate(MAX_ENTRIES);
    (entries, truncated)
}

/// 用多个线程处理 jobs，保持结果与输入顺序一致
fn parallel<T: Sync, R: Send>(
    jobs: &[T],
    ctx: &dyn TaskContext,
    work: impl Fn(&T) -> PluginResult<R> + Sync,
) -> PluginResult<Vec<R>> {
    let threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .clamp(1, 4)
        .min(jobs.len().max(1));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    let failure: Mutex<Option<PluginError>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= jobs.len() || ctx.is_cancelled() {
                        break;
                    }
                    match work(&jobs[index]) {
                        Ok(result) => {
                            results.lock().unwrap_or_else(|e| e.into_inner())[index] = Some(result);
                        }
                        Err(err) => {
                            *failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(err);
                            next.store(jobs.len(), Ordering::Relaxed);
                            break;
                        }
                    }
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
    Ok(results
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .flatten()
        .collect())
}

#[derive(Deserialize)]
pub struct VerifyArgs {
    pub manifest: String,
    /// 不指定时按 BSD 标签、扩展名或摘要长度识别
    pub algorithm: Option<Algorithm>,
}

pub fn verify(args: VerifyArgs, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let start = Instant::now();
    let manifest = PathBuf::from(&args.manifest);
    ctx.stage("hash.manifest_read");
    let size = std::fs::metadata(&manifest)
        .map_err(|e| io_error(&manifest, e))?
        .len();
    if size > MAX_MANIFEST_BYTES {
        return Err(PluginError::new("hash.manifest_too_large").with("mb", 64));
    }
    let bytes = std::fs::read(&manifest).map_err(|e| io_error(&manifest, e))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let (lines, invalid) = parse(text);
    if lines.is_empty() {
        return Err(PluginError::new("hash.manifest_empty"));
    }
    let algorithm = args
        .algorithm
        .or_else(|| lines.iter().find_map(|l| l.algorithm))
        .or_else(|| guess(&manifest, lines[0].digest.len()))
        .ok_or_else(|| PluginError::new("hash.manifest_algorithm"))?;
    let base = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    ctx.log(
        LogLevel::Info,
        "hash.manifest_loaded",
        json!({ "count": lines.len(), "algorithm": algorithm, "invalid": invalid.len() }),
    );

    let targets: Vec<(Line, PathBuf, Option<u64>)> = lines
        .into_iter()
        .map(|line| {
            let path = base.join(&line.name);
            let size = std::fs::metadata(&path)
                .ok()
                .filter(|m| m.is_file())
                .map(|m| m.len());
            (line, path, size)
        })
        .collect();
    let total: u64 = targets.iter().filter_map(|(.., size)| *size).sum();
    let done = AtomicU64::new(0);

    ctx.stage("hash.manifest_verify");
    let entries = parallel(&targets, ctx, |(line, path, size)| {
        let algorithm = line.algorithm.unwrap_or(algorithm);
        let entry = |status, actual: Option<String>, error| Entry {
            path: line.name.clone(),
            status,
            expected: Some(line.digest.clone()),
            actual,
            error,
        };
        if size.is_none() {
            ctx.log(
                LogLevel::Warn,
                "hash.manifest_missing",
                json!({ "file": line.name }),
            );
            return Ok(entry(Status::Missing, None, None));
        }
        match digest_file(path, algorithm, ctx, &done, total) {
            Ok(actual) if actual == line.digest => Ok(entry(Status::Ok, Some(actual), None)),
            Ok(actual) => {
                ctx.log(
                    LogLevel::Warn,
                    "hash.manifest_mismatch",
                    json!({ "file": line.name }),
                );
                Ok(entry(Status::Failed, Some(actual), None))
            }
            Err(err) if err.code == "task.cancelled" => Err(err),
            Err(err) => {
                ctx.log(
                    LogLevel::Warn,
                    "hash.file_failed",
                    json!({ "file": line.name, "code": err.code }),
                );
                Ok(entry(Status::Error, None, Some(err)))
            }
        }
    })?;

    let mut summary = Summary {
        invalid_lines: invalid,
        ..Summary::default()
    };
    for entry in &entries {
        match entry.status {
            Status::Ok => summary.ok += 1,
            Status::Failed => summary.failed += 1,
            Status::Missing => summary.missing += 1,
            Status::Error => summary.errors += 1,
        }
    }
    ctx.log(
        LogLevel::Info,
        "hash.manifest_done",
        json!({ "ok": summary.ok, "failed": summary.failed, "missing": summary.missing }),
    );
    let (entries, truncated) = finish_entries(entries);
    Ok(Report {
        algorithm,
        entries,
        truncated,
        summary,
        total_bytes: done.into_inner(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        output: None,
    })
}

#[derive(Deserialize)]
pub struct CreateArgs {
    pub dir: String,
    pub algorithm: Algorithm,
    #[serde(default = "yes")]
    pub recursive: bool,
    /// 清单保存位置；位于目录内时相对路径以清单所在目录为准
    pub output: String,
}

fn yes() -> bool {
    true
}

/// 从目录 from 到文件 to 的相对路径（用 / 分隔，必要时含 ..）；不同盘符时返回绝对路径
pub fn relative(from: &Path, to: &Path) -> String {
    let from_parts: Vec<_> = from.components().collect();
    let to_parts: Vec<_> = to.components().collect();
    let common = from_parts
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    if common == 0 {
        return to.to_string_lossy().into_owned();
    }
    let mut parts: Vec<String> = vec!["..".to_owned(); from_parts.len() - common];
    parts.extend(
        to_parts[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

pub fn create(args: CreateArgs, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let start = Instant::now();
    if extension(args.algorithm).is_none() {
        return Err(PluginError::new("hash.manifest_algorithm"));
    }
    let dir = dunce::canonicalize(&args.dir).map_err(|e| io_error(Path::new(&args.dir), e))?;
    let output = PathBuf::from(&args.output);
    let output_dir = output
        .parent()
        .map(|p| dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()))
        .unwrap_or_else(|| dir.clone());
    let output_path = output_dir.join(output.file_name().unwrap_or_default());
    // 路径相对清单所在目录，与 sha256sum -c 及本工具的校验方式一致

    ctx.stage("hash.scan");
    let mut files: Vec<(PathBuf, String, u64)> = Vec::new();
    let walker = walkdir::WalkDir::new(&dir)
        .follow_links(false)
        .max_depth(if args.recursive { usize::MAX } else { 1 })
        .sort_by_file_name();
    for item in walker {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let Ok(item) = item else { continue };
        if !item.file_type().is_file() || item.path() == output_path {
            continue;
        }
        let name = relative(&output_dir, item.path());
        let size = item.metadata().map(|m| m.len()).unwrap_or(0);
        files.push((item.into_path(), name, size));
    }
    if files.is_empty() {
        return Err(PluginError::new("hash.manifest_no_files"));
    }
    let total: u64 = files.iter().map(|(.., size)| size).sum();
    ctx.log(
        LogLevel::Info,
        "hash.scanned",
        json!({ "count": files.len(), "bytes": total }),
    );

    ctx.stage("hash.compute");
    let done = AtomicU64::new(0);
    let digests = parallel(&files, ctx, |(path, name, _)| {
        match digest_file(path, args.algorithm, ctx, &done, total) {
            Ok(digest) => Ok(Ok(digest)),
            Err(err) if err.code == "task.cancelled" => Err(err),
            Err(err) => {
                ctx.log(
                    LogLevel::Warn,
                    "hash.file_failed",
                    json!({ "file": name, "code": err.code }),
                );
                Ok(Err(err))
            }
        }
    })?;

    let mut text = String::new();
    let mut entries = Vec::with_capacity(files.len());
    let mut summary = Summary::default();
    for ((_, name, _), digest) in files.iter().zip(digests) {
        match digest {
            Ok(digest) => {
                let (escaped, shown) = escape_name(name);
                if escaped {
                    text.push('\\');
                }
                text.push_str(&format!("{digest}  {shown}\n"));
                summary.ok += 1;
                entries.push(Entry {
                    path: name.clone(),
                    status: Status::Ok,
                    expected: None,
                    actual: Some(digest),
                    error: None,
                });
            }
            Err(err) => {
                summary.errors += 1;
                entries.push(Entry {
                    path: name.clone(),
                    status: Status::Error,
                    expected: None,
                    actual: None,
                    error: Some(err),
                });
            }
        }
    }

    ctx.stage("hash.manifest_write");
    File::create(&output_path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| io_error(&output_path, e))?;
    ctx.log(
        LogLevel::Info,
        "hash.manifest_written",
        json!({ "path": output_path.to_string_lossy(), "count": summary.ok }),
    );
    let (entries, truncated) = finish_entries(entries);
    Ok(Report {
        algorithm: args.algorithm,
        entries,
        truncated,
        summary,
        total_bytes: done.into_inner(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        output: Some(output_path.to_string_lossy().into_owned()),
    })
}

#[cfg(test)]
#[path = "manifest_test.rs"]
mod tests;
