//! 文件名与文本内容的编码识别：UTF-8 优先，其次 GB18030（兼容 GBK），
//! 用于 Windows 中文系统打包、没有标记 UTF-8 的压缩包。

use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};

/// 压缩包中的文件名；`fallback` 是解压库给出的写法（ZIP 为 CP437 或 Unicode 扩展字段）
pub fn name(raw: &[u8], fallback: Option<&str>) -> String {
    if let Ok(text) = std::str::from_utf8(raw) {
        return text.to_owned();
    }
    // 字符数与字节数不同，说明解压库用的是 Unicode 路径扩展字段，而不是逐字节的 CP437
    if let Some(name) = fallback
        && name.chars().count() != raw.len()
    {
        return name.to_owned();
    }
    let (text, had_errors) = GB18030.decode_without_bom_handling(raw);
    if !had_errors {
        return text.into_owned();
    }
    fallback.map_or_else(|| String::from_utf8_lossy(raw).into_owned(), str::to_owned)
}

/// 预览用的文本：UTF-8（可带 BOM）、带 BOM 的 UTF-16、GB18030；含 NUL 或都解不通时视为二进制。
/// `truncated` 为 true 时末尾可能截断在多字节字符中间。
pub fn content(bytes: &[u8], truncated: bool) -> Option<String> {
    if let Some(rest) = bytes.strip_prefix(b"\xff\xfe") {
        return Some(UTF_16LE.decode_without_bom_handling(rest).0.into_owned());
    }
    if let Some(rest) = bytes.strip_prefix(b"\xfe\xff") {
        return Some(UTF_16BE.decode_without_bom_handling(rest).0.into_owned());
    }
    if bytes.contains(&0) {
        return None;
    }
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => return Some(text.to_owned()),
        Err(err) if err.error_len().is_none() => {
            return Some(String::from_utf8_lossy(&bytes[..err.valid_up_to()]).into_owned());
        }
        Err(_) => {}
    }
    // 截断处可能落在 GB18030 双字节字符中间，去掉最后一个字节再试
    let candidates: &[&[u8]] = if truncated && !bytes.is_empty() {
        &[bytes, &bytes[..bytes.len() - 1]]
    } else {
        &[bytes]
    };
    candidates.iter().find_map(|b| {
        let (text, had_errors) = GB18030.decode_without_bom_handling(b);
        (!had_errors).then(|| text.into_owned())
    })
}

#[cfg(test)]
#[path = "text_test.rs"]
mod tests;
