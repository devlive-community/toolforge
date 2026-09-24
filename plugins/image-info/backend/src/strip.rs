//! 无损移除图片元数据：只删除 JPEG 的 APP1（EXIF / XMP）、APP13（IPTC）与注释段，
//! 以及 PNG 的 eXIf / tEXt / zTXt / iTXt / tIME 块；像素数据原样保留，不重新编码。
//! 颜色配置（JPEG APP2 ICC、PNG iCCP）会保留，避免颜色发生变化。

use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Default, PartialEq)]
pub struct Stripped {
    pub bytes: Vec<u8>,
    /// 移除的段 / 块名称
    pub removed: Vec<String>,
}

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const PNG_DROP: &[&[u8; 4]] = &[b"eXIf", b"tEXt", b"zTXt", b"iTXt", b"tIME"];

fn corrupt() -> PluginError {
    PluginError::new("info.corrupt")
}

/// 记录移除的类型，同类只记一次
fn note(removed: &mut Vec<String>, name: &str) {
    if !removed.iter().any(|r| r == name) {
        removed.push(name.to_owned());
    }
}

pub fn strip(data: &[u8]) -> PluginResult<Stripped> {
    if data.starts_with(&[0xFF, 0xD8]) {
        strip_jpeg(data)
    } else if data.starts_with(PNG_SIGNATURE) {
        strip_png(data)
    } else {
        Err(PluginError::new("info.strip_unsupported"))
    }
}

fn strip_jpeg(data: &[u8]) -> PluginResult<Stripped> {
    let mut out = Vec::with_capacity(data.len());
    let mut removed = Vec::new();
    out.extend_from_slice(&data[..2]);
    let mut pos = 2;
    loop {
        // 段之间可能有填充的 0xFF
        while data.get(pos) == Some(&0xFF) && data.get(pos + 1) == Some(&0xFF) {
            pos += 1;
        }
        let (Some(&0xFF), Some(&marker)) = (data.get(pos), data.get(pos + 1)) else {
            return Err(corrupt());
        };
        // SOS 之后是压缩数据，直到文件结束原样复制
        if marker == 0xDA || marker == 0xD9 {
            out.extend_from_slice(&data[pos..]);
            break;
        }
        // 无长度的标记（RSTn、TEM）
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            out.extend_from_slice(&data[pos..pos + 2]);
            pos += 2;
            continue;
        }
        let len = u16::from_be_bytes([
            *data.get(pos + 2).ok_or_else(corrupt)?,
            *data.get(pos + 3).ok_or_else(corrupt)?,
        ]) as usize;
        let end = pos + 2 + len;
        if len < 2 || end > data.len() {
            return Err(corrupt());
        }
        let payload = &data[pos + 4..end];
        let drop = match marker {
            0xE1 if payload.starts_with(b"Exif\0") => Some("EXIF"),
            0xE1 if payload.starts_with(b"http://ns.adobe.com/xap/") => Some("XMP"),
            0xE1 => Some("APP1"),
            0xED => Some("IPTC"),
            0xFE => Some("Comment"),
            _ => None,
        };
        match drop {
            Some(name) => note(&mut removed, name),
            None => out.extend_from_slice(&data[pos..end]),
        }
        pos = end;
    }
    Ok(Stripped {
        bytes: out,
        removed,
    })
}

fn strip_png(data: &[u8]) -> PluginResult<Stripped> {
    let mut out = Vec::with_capacity(data.len());
    let mut removed = Vec::new();
    out.extend_from_slice(PNG_SIGNATURE);
    let mut pos = PNG_SIGNATURE.len();
    while pos < data.len() {
        let header = data.get(pos..pos + 8).ok_or_else(corrupt)?;
        let len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let kind: [u8; 4] = [header[4], header[5], header[6], header[7]];
        let end = pos + 12 + len;
        if end > data.len() {
            return Err(corrupt());
        }
        if PNG_DROP.contains(&&kind) {
            note(&mut removed, &String::from_utf8_lossy(&kind));
        } else {
            out.extend_from_slice(&data[pos..end]);
        }
        pos = end;
        if &kind == b"IEND" {
            break;
        }
    }
    Ok(Stripped {
        bytes: out,
        removed,
    })
}

#[cfg(test)]
#[path = "strip_test.rs"]
mod tests;
