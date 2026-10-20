//! 注册表值类型：原始字节与解读值分离；整数走字符串解析不经浮点。

use serde::Serialize;

/// 支持与保留的值类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RegValue {
    /// REG_SZ：UTF-16LE 文本
    Sz { text: String },
    /// REG_EXPAND_SZ：环境变量引用只显示原文，不套用本机环境
    ExpandSz { text: String },
    /// REG_MULTI_SZ：多字符串保留边界
    MultiSz { items: Vec<String> },
    /// REG_DWORD：32 位无符号（十进制 + 原始十六进制）
    Dword { value: u32 },
    /// REG_QWORD：64 位无符号
    Qword { value: u64 },
    /// REG_BINARY：原始字节
    Binary { bytes: Vec<u8> },
    /// 未知类型：保留类型号与原始数据
    Unknown { type_id: u32, bytes: Vec<u8> },
}

impl RegValue {
    /// 类型名（前端徽章）
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Sz { .. } => "REG_SZ",
            Self::ExpandSz { .. } => "REG_EXPAND_SZ",
            Self::MultiSz { .. } => "REG_MULTI_SZ",
            Self::Dword { .. } => "REG_DWORD",
            Self::Qword { .. } => "REG_QWORD",
            Self::Binary { .. } => "REG_BINARY",
            Self::Unknown { .. } => "REG_UNKNOWN",
        }
    }

    /// 数据等价比较（类型与内容都必须一致；"同字节不同类型"视为变化）
    pub fn same(&self, other: &RegValue) -> bool {
        match (self, other) {
            (Self::Sz { text: a }, Self::Sz { text: b }) => a == b,
            (Self::ExpandSz { text: a }, Self::ExpandSz { text: b }) => a == b,
            (Self::MultiSz { items: a }, Self::MultiSz { items: b }) => a == b,
            (Self::Dword { value: a }, Self::Dword { value: b }) => a == b,
            (Self::Qword { value: a }, Self::Qword { value: b }) => a == b,
            (Self::Binary { bytes: a }, Self::Binary { bytes: b }) => a == b,
            (
                Self::Unknown {
                    type_id: a,
                    bytes: x,
                },
                Self::Unknown {
                    type_id: b,
                    bytes: y,
                },
            ) => a == b && x == y,
            _ => false,
        }
    }

    /// 从 hex 负载与类型号构造；损坏长度/终止符收集诊断
    pub fn from_hex(type_id: u32, bytes: Vec<u8>) -> (RegValue, Vec<String>) {
        let mut issues = Vec::new();
        let value = match type_id {
            1 => {
                let text = decode_utf16(&bytes, false, &mut issues);
                RegValue::Sz { text }
            }
            2 => {
                let text = decode_utf16(&bytes, false, &mut issues);
                RegValue::ExpandSz { text }
            }
            7 => {
                let items = decode_multi_sz(&bytes, &mut issues);
                RegValue::MultiSz { items }
            }
            4 => match bytes.as_slice().try_into() {
                Ok(b) => RegValue::Dword {
                    value: u32::from_le_bytes(b),
                },
                Err(_) => {
                    issues.push("reg.bad_dword_length".to_owned());
                    RegValue::Unknown { type_id, bytes }
                }
            },
            11 => match bytes.as_slice().try_into() {
                Ok(b) => RegValue::Qword {
                    value: u64::from_le_bytes(b),
                },
                Err(_) => {
                    issues.push("reg.bad_qword_length".to_owned());
                    RegValue::Unknown { type_id, bytes }
                }
            },
            3 => RegValue::Binary { bytes },
            other => {
                if other == 0 {
                    issues.push("reg.unknown_type_zero".to_owned());
                }
                RegValue::Unknown {
                    type_id: other,
                    bytes,
                }
            }
        };
        (value, issues)
    }

    /// 供对比与导出的解读摘要
    pub fn display(&self) -> String {
        match self {
            Self::Sz { text } | Self::ExpandSz { text } => text.clone(),
            Self::MultiSz { items } => items.join(" ⏎ "),
            Self::Dword { value } => format!("0x{value:08x} ({value})"),
            Self::Qword { value } => format!("0x{value:016x} ({value})"),
            Self::Binary { bytes } => hex_dump(bytes),
            Self::Unknown { type_id, bytes } => format!("type {} · {}", type_id, hex_dump(bytes)),
        }
    }
}

/// 解码 UTF-16LE 文本；无终止符记诊断
fn decode_utf16(bytes: &[u8], _double_terminate: bool, issues: &mut Vec<String>) -> String {
    if bytes.len() % 2 != 0 {
        issues.push("reg.odd_utf16_length".to_owned());
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    if bytes.len() >= 2 && !bytes.ends_with(&[0, 0]) {
        issues.push("reg.missing_terminator".to_owned());
    }
    String::from_utf16_lossy(&units)
}

/// REG_MULTI_SZ：字符串以 \0 分隔，列表以双 \0 结束
fn decode_multi_sz(bytes: &[u8], issues: &mut Vec<String>) -> Vec<String> {
    if bytes.len() % 2 != 0 {
        issues.push("reg.odd_utf16_length".to_owned());
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let mut items = Vec::new();
    let mut current: Vec<u16> = Vec::new();
    let mut index = 0;
    while index < units.len() {
        if units[index] == 0 {
            if index + 1 < units.len() && units[index + 1] == 0 {
                // 列表终止
                items.push(String::from_utf16_lossy(&current));
                current.clear();
                break;
            }
            items.push(String::from_utf16_lossy(&current));
            current.clear();
        } else {
            current.push(units[index]);
        }
        index += 1;
    }
    if !current.is_empty() {
        issues.push("reg.missing_terminator".to_owned());
        items.push(String::from_utf16_lossy(&current));
    }
    items
}

/// 紧凑十六进制（分节空格，长数据每 32 字节断行由前端处理）
fn hex_dump(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".to_owned();
    }
    let out = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    if bytes.len() > 256 {
        // 大二进制截断展示（原始值对比仍用全量字节）
        format!("{}… ({} bytes)", &out[..512], bytes.len())
    } else {
        out
    }
}

#[cfg(test)]
#[path = "value_test.rs"]
mod tests;
