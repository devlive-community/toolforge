//! 解压：只写入安全路径，跳过链接，按冲突策略处理已存在的文件，还原修改时间与权限。

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::format::{self, Flow, Kind, Meta};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Conflict {
    /// 已存在时自动编号（name (1).txt）
    #[default]
    Rename,
    Overwrite,
    Skip,
}

pub struct Request<'a> {
    pub archive: &'a Path,
    pub password: Option<&'a str>,
    pub dest: &'a Path,
    /// 要解压的条目或文件夹路径；为空表示全部
    pub entries: &'a [String],
    pub conflict: Conflict,
    /// 解压到以压缩包命名的新文件夹中
    pub create_folder: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub dest: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    /// 因已存在而跳过
    pub skipped: u64,
    /// 路径不安全而跳过
    pub unsafe_paths: u64,
    /// 链接条目
    pub links: u64,
}

fn io(path: &Path, err: std::io::Error) -> PluginError {
    PluginError::new("fs.io")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", err.to_string())
}

/// 在 dir 中生成不冲突的名字
pub fn unique(path: &Path) -> PathBuf {
    if std::fs::symlink_metadata(path).is_err() {
        return path.to_owned();
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
        .find(|candidate| std::fs::symlink_metadata(candidate).is_err())
        .unwrap_or_else(|| path.to_owned())
}

/// 文件夹没有扩展名，编号加在完整名字后面（photos.v2 → photos.v2 (1)）
fn unique_folder(path: &Path) -> PathBuf {
    if std::fs::symlink_metadata(path).is_err() {
        return path.to_owned();
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    (1..)
        .map(|n| path.with_file_name(format!("{name} ({n})")))
        .find(|candidate| std::fs::symlink_metadata(candidate).is_err())
        .unwrap_or_else(|| path.to_owned())
}

/// 压缩包名去掉所有压缩扩展名（photos.tar.gz → photos）
pub fn folder_name(archive: &Path) -> String {
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "archive".into());
    let lower = name.to_ascii_lowercase();
    for suffix in [
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".txz", ".zip", ".7z",
        ".tar", ".gz", ".bz2", ".xz", ".zst",
    ] {
        if lower.ends_with(suffix) && lower.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_owned();
        }
    }
    name
}

fn selected(entries: &[String], path: &str) -> bool {
    entries.is_empty()
        || entries
            .iter()
            .any(|e| path == e || path.starts_with(&format!("{e}/")))
}

/// 写入目标必须仍在解压目录内（防止已存在的符号链接把文件引到外面）
fn inside(base: &Path, target: &Path) -> bool {
    target
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .is_some_and(|p| p.starts_with(base))
}

fn copy_cancellable(
    reader: &mut dyn Read,
    writer: &mut File,
    ctx: &dyn TaskContext,
) -> PluginResult<u64> {
    let mut buffer = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    loop {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let n = reader.read(&mut buffer).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidData | std::io::ErrorKind::Other
                if e.to_string().to_lowercase().contains("password") =>
            {
                PluginError::new("archive.wrong_password")
            }
            _ => PluginError::new("archive.corrupt").with("detail", e.to_string()),
        })?;
        if n == 0 {
            return Ok(total);
        }
        writer
            .write_all(&buffer[..n])
            .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
        total += n as u64;
    }
}

fn finish_file(path: &Path, meta: &Meta) {
    if let Some(mtime) = meta.mtime
        && let Ok(file) = File::options().write(true).open(path)
    {
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime.max(0) as u64);
        let _ = file.set_modified(time);
    }
    #[cfg(unix)]
    if let Some(mode) = meta.mode {
        use std::os::unix::fs::PermissionsExt;
        // 只保留权限位，并保证自己可读写
        let _ = std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode((mode & 0o777) | 0o600),
        );
    }
}

pub fn extract(request: &Request, ctx: &dyn TaskContext) -> PluginResult<Outcome> {
    std::fs::create_dir_all(request.dest).map_err(|e| io(request.dest, e))?;
    let base = if request.create_folder {
        let folder = unique_folder(&request.dest.join(folder_name(request.archive)));
        std::fs::create_dir_all(&folder).map_err(|e| io(&folder, e))?;
        folder
    } else {
        request.dest.to_owned()
    };
    let base = base.canonicalize().map_err(|e| io(&base, e))?;
    let total = std::fs::metadata(request.archive)
        .map(|m| m.len())
        .unwrap_or(0);
    let read = Arc::new(AtomicU64::new(0));
    let mut outcome = Outcome {
        dest: base.to_string_lossy().into_owned(),
        ..Outcome::default()
    };
    let mut visit = |meta: &Meta, reader: Option<&mut dyn Read>| -> PluginResult<Flow> {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        ctx.progress(read.load(std::sync::atomic::Ordering::Relaxed), total);
        if !selected(request.entries, &meta.path) {
            return Ok(Flow::Continue);
        }
        if !meta.safe {
            outcome.unsafe_paths += 1;
            ctx.log(
                LogLevel::Warn,
                "archive.unsafe_path",
                json!({ "path": meta.path }),
            );
            return Ok(Flow::Continue);
        }
        let target = base.join(&meta.path);
        match meta.kind {
            Kind::Link => {
                outcome.links += 1;
                ctx.log(
                    LogLevel::Warn,
                    "archive.link_skipped",
                    json!({ "path": meta.path }),
                );
            }
            Kind::Dir => {
                std::fs::create_dir_all(&target).map_err(|e| io(&target, e))?;
                outcome.dirs += 1;
            }
            Kind::File => {
                let Some(reader) = reader else {
                    return Ok(Flow::Continue);
                };
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
                }
                if !inside(&base, &target) {
                    outcome.unsafe_paths += 1;
                    ctx.log(
                        LogLevel::Warn,
                        "archive.unsafe_path",
                        json!({ "path": meta.path }),
                    );
                    return Ok(Flow::Continue);
                }
                let exists = std::fs::symlink_metadata(&target).is_ok();
                let target = match (exists, request.conflict) {
                    (true, Conflict::Skip) => {
                        outcome.skipped += 1;
                        return Ok(Flow::Continue);
                    }
                    (true, Conflict::Rename) => unique(&target),
                    (true, Conflict::Overwrite) => {
                        // 先删除，避免通过已存在的符号链接写到别处
                        let _ = std::fs::remove_file(&target);
                        target
                    }
                    (false, _) => target,
                };
                let mut file = File::create(&target).map_err(|e| io(&target, e))?;
                let written = match copy_cancellable(reader, &mut file, ctx) {
                    Ok(n) => n,
                    Err(err) => {
                        drop(file);
                        let _ = std::fs::remove_file(&target);
                        return Err(err);
                    }
                };
                drop(file);
                finish_file(&target, meta);
                outcome.files += 1;
                outcome.bytes += written;
            }
        }
        Ok(Flow::Continue)
    };
    format::walk(request.archive, request.password, true, &read, &mut visit)?;
    ctx.progress(total, total);
    Ok(outcome)
}

#[cfg(test)]
#[path = "extract_test.rs"]
mod tests;
