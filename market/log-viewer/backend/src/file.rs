//! 用户选择的日志文件：经宿主按偏移读取，实现 Read 与 Seek，供索引、筛选与读行使用。
//! 文件可能在打开期间继续增长，所以不缓存长度。

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use tf_plugin_api::{PluginError, PluginResult};
use tf_plugin_guest::net;

/// 宿主单次读取的上限
const MAX_READ: u64 = 4 * 1024 * 1024;

pub struct LogFile {
    path: String,
    pos: u64,
}

/// 文件当前长度；文件夹返回 `log.not_a_file`
pub fn len_of(path: &str) -> PluginResult<u64> {
    net::file_size(path).map_err(|error| match error.code.as_str() {
        "fs.not_a_file" => PluginError::new("log.not_a_file"),
        "fs.not_found" => PluginError::new("fs.not_found").with("path", path),
        _ => error,
    })
}

impl LogFile {
    pub fn open(path: &Path) -> PluginResult<Self> {
        let path = path.to_string_lossy().into_owned();
        len_of(&path)?;
        Ok(Self { path, pos: 0 })
    }

    pub fn len(&self) -> PluginResult<u64> {
        len_of(&self.path)
    }
}

fn io_failure(error: PluginError) -> io::Error {
    let detail = error
        .params
        .get("detail")
        .and_then(|d| d.as_str())
        .unwrap_or(&error.code)
        .to_owned();
    io::Error::other(detail)
}

impl Read for LogFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let want = (buf.len() as u64).min(MAX_READ) as u32;
        let chunk = net::read_file(&self.path, self.pos, want).map_err(io_failure)?;
        buf[..chunk.len()].copy_from_slice(&chunk);
        self.pos += chunk.len() as u64;
        Ok(chunk.len())
    }
}

impl Seek for LogFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let pos = match from {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
            SeekFrom::End(delta) => self.len().map_err(io_failure)?.checked_add_signed(delta),
        };
        self.pos =
            pos.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.pos)
    }
}
