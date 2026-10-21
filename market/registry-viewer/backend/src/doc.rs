//! .reg 文本解析：键路径、默认值、具名值、转义、hex 续行、注释与操作声明。
//! 只支持 "Windows Registry Editor Version 5.00"；REGEDIT4 明确不支持。

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::value::RegValue;

/// 键数量与值数量上限（报告体积保护）
pub const MAX_KEYS: usize = 5000;
pub const MAX_VALUES: usize = 50_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueEntry {
    /// 原拼写值名；默认值为空字符串且 is_default = true
    pub name: String,
    pub is_default: bool,
    pub value: RegValue,
    /// 源文件行号（1 起，与源文定位互相对应）
    pub line: u32,
    /// 该值名在本键内重复赋值（保留原顺序并标为操作声明）
    pub duplicate: bool,
    /// 解析诊断（损坏长度、终止符缺失等）
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyNode {
    /// 完整键路径（保留原拼写，不含方括号）
    pub path: String,
    /// 路径段
    pub segments: Vec<String>,
    pub values: Vec<ValueEntry>,
    /// 源文件行号
    pub line: u32,
    /// [-HKEY...] 删键操作声明
    pub delete: bool,
    /// 同路径键在本文件中重复出现
    pub duplicate: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegDoc {
    pub keys: Vec<KeyNode>,
    /// 遇到的非致命诊断（如值截断）
    pub issues: Vec<String>,
}

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 解码文件字节：UTF-16LE BOM、UTF-8 BOM 或无 BOM UTF-8
pub fn decode_text(bytes: &[u8]) -> PluginResult<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return Ok(String::from_utf16_lossy(&units));
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    String::from_utf8(bytes.to_vec())
        .map_err(|_| errp("reg.encoding", "hint", "utf-16le-bom-or-utf-8"))
}

/// 解析 .reg 文本
pub fn parse(text: &str) -> PluginResult<RegDoc> {
    let mut lines = text.lines();
    let header = lines.next().map(str::trim).unwrap_or_default();
    if header != "Windows Registry Editor Version 5.00" {
        if header.starts_with("REGEDIT4") {
            return Err(errp("reg.unsupported_version", "found", "REGEDIT4"));
        }
        return Err(errp("reg.bad_header", "found", header));
    }

    let mut doc = RegDoc {
        keys: Vec::new(),
        issues: Vec::new(),
    };
    let mut line_number = 1u32; // 首行是版本头
    // 逻辑行缓冲：值行可能以 \ 续行
    let mut pending: Option<(String, u32)> = None;

    for raw in lines {
        line_number += 1;
        let line = raw.trim_end_matches(['\r']);
        // 续行拼接（剥掉行尾反斜杠）
        if let Some((mut logical, start)) = pending.take() {
            logical.pop();
            logical.push_str(line.trim_start());
            if logical.ends_with('\\') {
                pending = Some((logical, start));
                continue;
            }
            apply_line(&mut doc, &logical, start, false);
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.ends_with('\\') {
            pending = Some((trimmed.to_owned(), line_number));
            continue;
        }
        apply_line(&mut doc, trimmed, line_number, true);
    }
    // 尾部悬空续行
    if let Some((logical, start)) = pending.take() {
        apply_line(&mut doc, logical.trim_end_matches('\\'), start, false);
        doc.issues.push("reg.trailing_continuation".to_owned());
    }
    if doc.keys.len() > MAX_KEYS {
        return Err(errp("reg.too_many_keys", "limit", MAX_KEYS as u64));
    }
    Ok(doc)
}

/// 处理一条逻辑行：键头或值
fn apply_line(doc: &mut RegDoc, line: &str, line_number: u32, trim: bool) {
    let line = if trim { line.trim() } else { line };
    if line.is_empty() || line.starts_with(';') {
        return;
    }
    if let Some((path, delete)) = parse_key_header(line) {
        if doc.keys.len() >= MAX_KEYS {
            doc.issues.push("reg.keys_truncated".to_owned());
            return;
        }
        let duplicate = doc.keys.iter().any(|k| k.path == path);
        let segments: Vec<String> = path.split('\\').map(str::to_owned).collect();
        doc.keys.push(KeyNode {
            path,
            segments,
            values: Vec::new(),
            line: line_number,
            delete,
            duplicate,
        });
        return;
    }
    if doc.keys.iter().map(|k| k.values.len()).sum::<usize>() >= MAX_VALUES {
        doc.issues.push("reg.values_truncated".to_owned());
        return;
    }
    // 值行：归属最后一个键
    let Some(key) = doc.keys.last_mut() else {
        doc.issues.push("reg.value_outside_key".to_owned());
        return;
    };
    if key.delete {
        doc.issues.push("reg.value_in_delete_key".to_owned());
        return;
    }
    match parse_value_line(line) {
        Some((name, is_default, value, issues)) => {
            let duplicate = key
                .values
                .iter()
                .any(|v| v.name.eq_ignore_ascii_case(&name) && v.is_default == is_default);
            key.values.push(ValueEntry {
                name,
                is_default,
                value,
                line: line_number,
                duplicate,
                issues,
            });
        }
        None => doc.issues.push("reg.bad_value_line".to_owned()),
    }
}

/// 键头：[HKEY_...] 或 [-HKEY...]
fn parse_key_header(line: &str) -> Option<(String, bool)> {
    let delete = line.starts_with("[-");
    let body = if delete {
        line.strip_prefix("[-")?.strip_suffix(']')?
    } else {
        line.strip_prefix('[')?.strip_suffix(']')?
    };
    if body.is_empty() {
        return None;
    }
    Some((body.to_owned(), delete))
}

/// 值行：`"name"=data` / `@=data` / `"name"=-`
fn parse_value_line(line: &str) -> Option<(String, bool, RegValue, Vec<String>)> {
    let (name, is_default, rest) = if let Some(rest) = line.strip_prefix("@=") {
        (String::new(), true, rest)
    } else {
        let quoted = line.strip_prefix('"')?;
        let (name, rest) = scan_quoted(quoted)?;
        let rest = rest.strip_prefix('=')?;
        (name, false, rest)
    };
    let rest = rest.trim();
    // 删值声明
    if rest == "-" {
        return Some((
            name,
            is_default,
            RegValue::Unknown {
                type_id: u32::MAX,
                bytes: Vec::new(),
            },
            vec!["reg.delete_value".to_owned()],
        ));
    }
    if let Some(hex) = rest.strip_prefix("hex") {
        // hex:（BINARY）/ hex(2):（EXPAND_SZ）/ hex(7):（MULTI_SZ）/ hex(b):（QWORD）/ hex(N):
        let (type_id, payload) = if let Some(rest) = hex.strip_prefix(':') {
            (3u32, rest)
        } else {
            let inner = hex.strip_prefix('(')?;
            let close = inner.find(')')?;
            let type_id = u32::from_str_radix(&inner[..close], 16).ok()?;
            let rest = inner[close + 1..].strip_prefix(':')?;
            (type_id, rest)
        };
        let bytes = parse_hex_payload(payload).ok()?;
        let (value, issues) = RegValue::from_hex(type_id, bytes);
        return Some((name, is_default, value, issues));
    }
    if let Some(text) = rest.strip_prefix("dword:") {
        let value = u32::from_str_radix(text.trim(), 16).ok()?;
        return Some((name, is_default, RegValue::Dword { value }, vec![]));
    }
    // REG_SZ 与转义
    if let Some(quoted) = rest.strip_prefix('"') {
        let (text, _) = scan_quoted(quoted)?;
        return Some((name, is_default, RegValue::Sz { text }, vec![]));
    }
    None
}

/// 扫描带转义的引号串，返回内容与其后剩余文本
fn scan_quoted(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                escaped => out.push(escaped),
            },
            '"' => return Some((out, chars.as_str())),
            other => out.push(other),
        }
    }
    None
}

/// hex 负载：逗号分隔 + 行内注释；续行已由上层拼接
fn parse_hex_payload(text: &str) -> PluginResult<Vec<u8>> {
    let mut bytes = Vec::new();
    for token in text.split(',') {
        let token = token.trim();
        if token.is_empty() || token.starts_with(';') {
            continue;
        }
        // 每段最多两位十六进制
        let value = u8::from_str_radix(token.get(..2).unwrap_or(token), 16)
            .or_else(|_| u8::from_str_radix(token, 16))
            .map_err(|_| errp("reg.bad_hex", "token", token))?;
        bytes.push(value);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "doc_test.rs"]
mod tests;
