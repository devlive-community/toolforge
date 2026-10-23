//! 在文件中查找字节序列：分块读取，块之间重叠以免漏掉跨块的匹配。

use std::io::{Read, Seek, SeekFrom};

use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::file;

const CHUNK: usize = 8 * 1024 * 1024;
const MAX_PATTERN: usize = 4096;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Hex,
    Text,
}

/// 解析查找内容：十六进制可用空格或逗号分隔，可带 0x 前缀
pub fn pattern(input: &str, mode: Mode) -> PluginResult<Vec<u8>> {
    let bytes = match mode {
        Mode::Text => input.as_bytes().to_vec(),
        Mode::Hex => {
            let cleaned: String = input
                .split(|c: char| c.is_whitespace() || c == ',')
                .map(|part| part.trim_start_matches("0x").trim_start_matches("0X"))
                .collect();
            if !cleaned.len().is_multiple_of(2) || !cleaned.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(PluginError::new("hex.invalid_pattern"));
            }
            (0..cleaned.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
                .collect()
        }
    };
    if bytes.is_empty() {
        return Err(PluginError::new("hex.empty_pattern"));
    }
    if bytes.len() > MAX_PATTERN {
        return Err(PluginError::new("hex.pattern_too_long").with("max", MAX_PATTERN));
    }
    Ok(bytes)
}

fn lower(bytes: &[u8]) -> Vec<u8> {
    bytes.to_ascii_lowercase()
}

/// 从 `from` 开始向后查找（含 from），找不到返回 None
pub fn forward(
    path: &str,
    needle: &[u8],
    from: u64,
    ignore_case: bool,
    cancelled: &dyn Fn() -> bool,
) -> PluginResult<Option<u64>> {
    let (mut handle, size) = file::open(path)?;
    let needle = if ignore_case {
        lower(needle)
    } else {
        needle.to_vec()
    };
    let finder = memchr::memmem::Finder::new(&needle);
    let mut start = from;
    let mut buf = vec![0; CHUNK + needle.len()];
    while start < size {
        if cancelled() {
            return Ok(None);
        }
        let len = ((size - start) as usize).min(buf.len());
        handle
            .seek(SeekFrom::Start(start))
            .and_then(|_| handle.read_exact(&mut buf[..len]))
            .map_err(|e| {
                PluginError::new("hex.read_failed")
                    .with("path", path)
                    .with("detail", e.to_string())
            })?;
        let hay = if ignore_case {
            lower(&buf[..len])
        } else {
            buf[..len].to_vec()
        };
        if let Some(i) = finder.find(&hay) {
            return Ok(Some(start + i as u64));
        }
        if start + len as u64 >= size {
            break;
        }
        start += (len - (needle.len() - 1)) as u64;
    }
    Ok(None)
}

/// 查找结束位置在 `before` 之前（不含从 before 开始的匹配）的最后一个匹配
pub fn backward(
    path: &str,
    needle: &[u8],
    before: u64,
    ignore_case: bool,
    cancelled: &dyn Fn() -> bool,
) -> PluginResult<Option<u64>> {
    let (mut handle, size) = file::open(path)?;
    let needle = if ignore_case {
        lower(needle)
    } else {
        needle.to_vec()
    };
    let finder = memchr::memmem::FinderRev::new(&needle);
    // 匹配必须完整落在 [0, end) 内
    let mut end = (before.min(size) + needle.len() as u64 - 1).min(size);
    let mut buf = vec![0; CHUNK + needle.len()];
    while end >= needle.len() as u64 {
        if cancelled() {
            return Ok(None);
        }
        let len = (end as usize).min(buf.len());
        let start = end - len as u64;
        handle
            .seek(SeekFrom::Start(start))
            .and_then(|_| handle.read_exact(&mut buf[..len]))
            .map_err(|e| {
                PluginError::new("hex.read_failed")
                    .with("path", path)
                    .with("detail", e.to_string())
            })?;
        let hay = if ignore_case {
            lower(&buf[..len])
        } else {
            buf[..len].to_vec()
        };
        if let Some(i) = finder.rfind(&hay) {
            let at = start + i as u64;
            if at < before {
                return Ok(Some(at));
            }
        }
        if start == 0 {
            break;
        }
        end = start + needle.len() as u64 - 1;
    }
    Ok(None)
}

#[cfg(test)]
#[path = "search_test.rs"]
mod tests;
