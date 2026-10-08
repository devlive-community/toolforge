//! 按偏移读取文件，并格式化为十六进制行。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

pub const ROW: u64 = 16;
/// 单次读取的上限
pub const MAX_READ: u64 = 1024 * 1024;

pub fn open(path: &str) -> PluginResult<(File, u64)> {
    let not_found = || PluginError::new("fs.not_found").with("path", path);
    let meta = std::fs::metadata(Path::new(path)).map_err(|_| not_found())?;
    if meta.is_dir() {
        return Err(PluginError::new("hex.not_a_file"));
    }
    let file = File::open(path).map_err(|e| {
        PluginError::new("hex.read_failed")
            .with("path", path)
            .with("detail", e.to_string())
    })?;
    Ok((file, meta.len()))
}

/// 读取 [offset, offset + length)，超出文件末尾的部分截掉
pub fn read_at(path: &str, offset: u64, length: u64) -> PluginResult<Vec<u8>> {
    let (mut file, size) = open(path)?;
    if offset >= size {
        return Ok(Vec::new());
    }
    let length = length.min(size - offset).min(MAX_READ);
    let mut buf = vec![0; length as usize];
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(&mut buf))
        .map_err(|e| {
            PluginError::new("hex.read_failed")
                .with("path", path)
                .with("detail", e.to_string())
        })?;
    Ok(buf)
}

/// 偏移量显示宽度：至少 8 位，大文件按需加宽
pub fn offset_width(size: u64) -> usize {
    format!("{:x}", size.max(1) - 1).len().max(8)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Row {
    pub offset: u64,
    pub label: String,
    pub hex: Vec<String>,
    pub ascii: Vec<String>,
}

fn printable(byte: u8) -> String {
    if (0x20..0x7f).contains(&byte) {
        (byte as char).to_string()
    } else {
        "·".to_owned()
    }
}

pub fn rows(bytes: &[u8], start: u64, width: usize) -> Vec<Row> {
    bytes
        .chunks(ROW as usize)
        .enumerate()
        .map(|(i, chunk)| {
            let offset = start + i as u64 * ROW;
            Row {
                offset,
                label: format!("{offset:0width$X}"),
                hex: chunk.iter().map(|b| format!("{b:02X}")).collect(),
                ascii: chunk.iter().map(|&b| printable(b)).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "file_test.rs"]
mod tests;
