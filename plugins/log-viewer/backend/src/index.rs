//! 行索引：记录每个完整行的起始偏移与级别，支持从上次位置继续扫描（文件追加时）。
//! 没有换行结尾的最后一行称为尾行，文件继续增长时它可能变长，因此单独处理。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use tf_plugin_api::{PluginError, PluginResult, cancelled};

use crate::level;

const CHUNK: usize = 1024 * 1024;

#[derive(Debug, Default)]
pub struct Index {
    /// 每个完整行的起始字节偏移
    pub starts: Vec<u64>,
    pub levels: Vec<u8>,
    /// 尾行起点；文件以换行结尾时等于 len
    pub tail_start: u64,
    /// 已扫描到的文件长度
    pub len: u64,
    /// 各级别的完整行数
    pub counts: [u64; 6],
}

pub fn io_error(err: std::io::Error) -> PluginError {
    PluginError::new("fs.io").with("detail", err.to_string())
}

impl Index {
    pub fn complete(&self) -> usize {
        self.starts.len()
    }

    pub fn has_tail(&self) -> bool {
        self.tail_start < self.len
    }

    pub fn total(&self) -> usize {
        self.complete() + usize::from(self.has_tail())
    }

    /// 行的字节范围（不含换行符与行尾的 \r）
    pub fn range(&self, line: usize, file: &mut File) -> PluginResult<(u64, u64)> {
        let (start, end) = if line < self.complete() {
            let next = self
                .starts
                .get(line + 1)
                .copied()
                .unwrap_or(self.tail_start);
            (self.starts[line], next.saturating_sub(1))
        } else if line == self.complete() && self.has_tail() {
            (self.tail_start, self.len)
        } else {
            return Err(PluginError::new("log.line_out_of_range").with("line", line + 1));
        };
        // 仅为去掉 \r 读取最后一个字节
        if end > start {
            let mut last = [0u8];
            file.seek(SeekFrom::Start(end - 1)).map_err(io_error)?;
            file.read_exact(&mut last).map_err(io_error)?;
            if last[0] == b'\r' {
                return Ok((start, end - 1));
            }
        }
        Ok((start, end))
    }

    pub fn level(&self, line: usize) -> u8 {
        self.levels.get(line).copied().unwrap_or(level::NONE)
    }

    /// 从尾行起点扫描到 upto，记录新出现的完整行
    pub fn scan(
        &mut self,
        file: &mut File,
        upto: u64,
        mut progress: impl FnMut(u64),
        is_cancelled: impl Fn() -> bool,
    ) -> PluginResult<()> {
        file.seek(SeekFrom::Start(self.tail_start))
            .map_err(io_error)?;
        let mut reader = file.take(upto.saturating_sub(self.tail_start));
        let mut buffer = vec![0u8; CHUNK];
        let mut position = self.tail_start;
        let mut line_start = self.tail_start;
        let mut head: Vec<u8> = Vec::with_capacity(level::HEAD);
        let mut previous = self.levels.last().copied().unwrap_or(level::NONE);
        loop {
            if is_cancelled() {
                return Err(cancelled());
            }
            let read = reader.read(&mut buffer).map_err(io_error)?;
            if read == 0 {
                break;
            }
            let chunk = &buffer[..read];
            let mut from = 0;
            for newline in memchr::memchr_iter(b'\n', chunk) {
                if head.len() < level::HEAD {
                    let take = (newline - from).min(level::HEAD - head.len());
                    head.extend_from_slice(&chunk[from..from + take]);
                }
                let current = level::classify(&head, previous);
                self.starts.push(line_start);
                self.levels.push(current);
                self.counts[current as usize] += 1;
                previous = current;
                head.clear();
                line_start = position + newline as u64 + 1;
                from = newline + 1;
            }
            if head.len() < level::HEAD {
                let take = (read - from).min(level::HEAD - head.len());
                head.extend_from_slice(&chunk[from..from + take]);
            }
            position += read as u64;
            progress(position);
        }
        self.tail_start = line_start;
        self.len = position;
        Ok(())
    }
}

#[cfg(test)]
#[path = "index_test.rs"]
mod tests;
