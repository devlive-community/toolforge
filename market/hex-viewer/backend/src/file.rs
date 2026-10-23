//! 按偏移读取文件，并格式化为十六进制行。

use std::io::{self, Read, Seek, SeekFrom};

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};
use tf_plugin_guest::net;

pub const ROW: u64 = 16;
/// 单次读取的上限
pub const MAX_READ: u64 = 1024 * 1024;

/// 用户选择的文件：经宿主按偏移读取，实现 Read 与 Seek 供搜索与熵计算使用
pub struct Handle {
    path: String,
    pos: u64,
    size: u64,
}

impl Read for Handle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.size || buf.is_empty() {
            return Ok(0);
        }
        let want = (buf.len() as u64).min(self.size - self.pos).min(MAX_READ) as u32;
        let chunk =
            net::read_file(&self.path, self.pos, want).map_err(|e| io::Error::other(detail(&e)))?;
        buf[..chunk.len()].copy_from_slice(&chunk);
        self.pos += chunk.len() as u64;
        Ok(chunk.len())
    }
}

impl Seek for Handle {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let pos = match from {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::End(delta) => self.size.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
        };
        self.pos =
            pos.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.pos)
    }
}

fn detail(error: &PluginError) -> String {
    error
        .params
        .get("detail")
        .and_then(|d| d.as_str())
        .unwrap_or(&error.code)
        .to_owned()
}

pub fn open(path: &str) -> PluginResult<(Handle, u64)> {
    let size = net::file_size(path).map_err(|error| match error.code.as_str() {
        "fs.not_a_file" => PluginError::new("hex.not_a_file"),
        _ => PluginError::new("fs.not_found").with("path", path),
    })?;
    let handle = Handle {
        path: path.to_owned(),
        pos: 0,
        size,
    };
    Ok((handle, size))
}

/// 读取 [offset, offset + length)，超出文件末尾的部分截掉
pub fn read_at(path: &str, offset: u64, length: u64) -> PluginResult<Vec<u8>> {
    let (_, size) = open(path)?;
    if offset >= size {
        return Ok(Vec::new());
    }
    let length = length.min(size - offset).min(MAX_READ);
    net::read_file(path, offset, length as u32).map_err(|error| {
        PluginError::new("hex.read_failed")
            .with("path", path)
            .with("detail", detail(&error))
    })
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
