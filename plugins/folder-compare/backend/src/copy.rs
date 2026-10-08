//! 把选中的条目从一侧复制到另一侧：文件夹递归复制，文件先写临时文件再替换，并保留修改时间。

use std::path::{Component, Path, PathBuf};

use tf_plugin_api::{PluginError, PluginResult};

use crate::scan::{Ignore, relative};

/// 相对路径只能向下，不能跳出根目录
pub fn resolve(root: &Path, rel: &str) -> PluginResult<PathBuf> {
    let rel_path = Path::new(rel);
    let safe = !rel.is_empty()
        && rel_path
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    if !safe {
        return Err(PluginError::new("fc.invalid_path").with("path", rel));
    }
    Ok(rel
        .split('/')
        .fold(root.to_path_buf(), |p, part| p.join(part)))
}

fn failed(code: &str, path: &Path, e: std::io::Error) -> PluginError {
    PluginError::new(code)
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", e.to_string())
}

fn copy_file(src: &Path, dst: &Path) -> PluginResult<u64> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| failed("fc.write_failed", parent, e))?;
    }
    let name = dst
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dst.with_file_name(format!(".{name}.tf-part"));
    let bytes = std::fs::copy(src, &tmp).map_err(|e| failed("fc.write_failed", dst, e))?;
    // 保留修改时间，快速比较时两侧才会相同
    if let Ok(modified) = std::fs::metadata(src).and_then(|m| m.modified()) {
        let _ = std::fs::File::options()
            .write(true)
            .open(&tmp)
            .and_then(|f| f.set_modified(modified));
    }
    std::fs::rename(&tmp, dst).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        failed("fc.write_failed", dst, e)
    })?;
    Ok(bytes)
}

#[derive(Debug)]
pub struct Copied {
    pub files: usize,
    pub bytes: u64,
}

/// 复制一个条目；`on_file` 在每个文件完成后调用
pub fn copy(
    src_root: &Path,
    dst_root: &Path,
    rel: &str,
    ignore: &Ignore,
    on_file: &mut dyn FnMut(&str),
    cancelled: &dyn Fn() -> bool,
) -> PluginResult<Copied> {
    let src = resolve(src_root, rel)?;
    let dst = resolve(dst_root, rel)?;
    let meta = std::fs::symlink_metadata(&src).map_err(|_| {
        PluginError::new("fs.not_found").with("path", src.to_string_lossy().as_ref())
    })?;
    let mut copied = Copied { files: 0, bytes: 0 };
    if !meta.is_dir() {
        if dst.is_dir() {
            return Err(PluginError::new("fc.kind_mismatch").with("path", rel));
        }
        copied.bytes = copy_file(&src, &dst)?;
        copied.files = 1;
        on_file(rel);
        return Ok(copied);
    }
    if dst.is_file() {
        return Err(PluginError::new("fc.kind_mismatch").with("path", rel));
    }
    std::fs::create_dir_all(&dst).map_err(|e| failed("fc.write_failed", &dst, e))?;
    let walker = walkdir::WalkDir::new(&src)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !ignore.matches(&relative(src_root, e.path())));
    for entry in walker.flatten() {
        if cancelled() {
            return Err(tf_plugin_api::cancelled());
        }
        if entry.file_type().is_symlink() {
            continue;
        }
        let child_rel = relative(src_root, entry.path());
        let target = resolve(dst_root, &child_rel)?;
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| failed("fc.write_failed", &target, e))?;
        } else {
            if target.is_dir() {
                return Err(PluginError::new("fc.kind_mismatch").with("path", child_rel));
            }
            copied.bytes += copy_file(entry.path(), &target)?;
            copied.files += 1;
            on_file(&child_rel);
        }
    }
    Ok(copied)
}

#[cfg(test)]
#[path = "copy_test.rs"]
mod tests;
