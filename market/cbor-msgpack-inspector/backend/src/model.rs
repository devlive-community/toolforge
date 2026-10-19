//! 共享模型：类型树节点、诊断、解析上下文与上限。
//! 上限（字节 / 节点 / 深度 / 诊断数）触达后标记 incomplete，报告绝不冒充完整通过。

use serde::Serialize;
use serde_json::Value;

/// 输入字节上限
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// 节点数量上限
pub const MAX_NODES: u64 = 200_000;
/// 递归深度上限
pub const MAX_DEPTH: usize = 64;
/// 诊断数量上限
pub const MAX_DIAGNOSTICS: usize = 1000;
/// 节点原始字节预览上限
pub const HEX_PREVIEW: usize = 256;

pub const ERROR: &str = "error";
pub const WARNING: &str = "warning";
pub const HINT: &str = "hint";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Diag {
    pub level: &'static str,
    pub code: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, Value>,
    /// 字节偏移；运输编码错误等无法定位时为 null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// uint | int | float | bool | null | undefined | simple | str | bytes |
    /// bigint | array | map | tag | ext | timestamp
    pub kind: String,
    pub offset: u64,
    pub length: u64,
    /// 标量显示值（大整数为十进制、bytes 为 hex、浮点保留 -0/非有限）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// 原始字节预览（≤256 字节）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex_truncated: Option<bool>,
    /// 数组元素
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
    /// Map 条目：键是任意类型的节点，不强转字符串
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<Entry>>,
    /// tag 号 / ext 类型号 / simple 值号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<i64>,
    /// 不定长（CBOR）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indefinite: Option<bool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub key: Node,
    pub value: Node,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub nodes: u64,
    pub errors: u64,
    pub warnings: u64,
    pub hints: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Trailing {
    pub offset: u64,
    pub length: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// cbor | msgpack
    pub format: &'static str,
    /// passed | issues | incomplete
    pub overall: &'static str,
    pub incomplete: bool,
    pub stats: Stats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Node>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing: Option<Trailing>,
    pub diagnostics: Vec<Diag>,
}

/// 达到结构上限后的停止信号：报告保留已解析部分并标记 incomplete
#[derive(Debug)]
pub struct Stop;

/// 计数节点并返回其原始字节预览
pub struct Ctx<'a> {
    pub bytes: &'a [u8],
    pub nodes: u64,
    pub diags: Vec<Diag>,
    pub incomplete: bool,
}

impl<'a> Ctx<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            nodes: 0,
            diags: Vec::new(),
            incomplete: false,
        }
    }

    pub fn push(
        &mut self,
        level: &'static str,
        code: &str,
        offset: Option<u64>,
        params: serde_json::Map<String, Value>,
    ) {
        if self.diags.len() >= MAX_DIAGNOSTICS {
            self.incomplete = true;
            return;
        }
        self.diags.push(Diag {
            level,
            code: code.to_owned(),
            params,
            offset,
        });
    }

    pub fn diag(&mut self, level: &'static str, code: &str, offset: u64) {
        self.push(level, code, Some(offset), serde_json::Map::new());
    }

    pub fn diagp(
        &mut self,
        level: &'static str,
        code: &str,
        offset: u64,
        key: &str,
        value: impl Into<Value>,
    ) {
        self.push(
            level,
            code,
            Some(offset),
            serde_json::Map::from_iter([(key.to_owned(), value.into())]),
        );
    }

    /// 节点计数 + 原始字节预览
    pub fn begin(
        &mut self,
        offset: u64,
        length: u64,
    ) -> Result<(Option<String>, Option<bool>), Stop> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            self.incomplete = true;
            return Err(Stop);
        }
        let end = offset as usize + length as usize;
        let Some(slice) = self.bytes.get(offset as usize..end.min(self.bytes.len())) else {
            return Ok((None, None));
        };
        if slice.len() <= HEX_PREVIEW {
            Ok((Some(hex(slice)), Some(false)))
        } else {
            Ok((Some(hex(&slice[..HEX_PREVIEW])), Some(true)))
        }
    }

    pub fn depth(&mut self, depth: usize, offset: u64) -> Result<(), Stop> {
        if depth > MAX_DEPTH {
            self.incomplete = true;
            self.diag(ERROR, "bin.depth", offset);
            return Err(Stop);
        }
        Ok(())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 直接向报告追加诊断（运输错误等解析前的问题）
pub fn push_report_diag(
    report: &mut Report,
    level: &'static str,
    code: &str,
    params: serde_json::Map<String, Value>,
    offset: Option<u64>,
) {
    report.diagnostics.push(Diag {
        level,
        code: code.to_owned(),
        params,
        offset,
    });
    match level {
        ERROR => report.stats.errors += 1,
        WARNING => report.stats.warnings += 1,
        _ => report.stats.hints += 1,
    }
}

/// 大整数（CBOR tag 2/3）：按 256 进制转十进制，不引入大数依赖。
/// tag 3 表示 -1 - n：先对 n 加 1 再取负。
pub fn bigint_to_decimal(bytes: &[u8], negative: bool) -> String {
    // 小端不重要：逐字节乘 256 加当前值，用十进制向量模拟
    let mut digits: Vec<u32> = vec![0]; // 十进制低位在前
    for byte in bytes {
        let mut carry = *byte as u32;
        for digit in digits.iter_mut() {
            let value = *digit * 256 + carry;
            *digit = value % 10;
            carry = value / 10;
        }
        while carry > 0 {
            digits.push(carry % 10);
            carry /= 10;
        }
    }
    if negative {
        // +1：tag 3 是 -1 - n
        let mut carry = 1;
        for digit in digits.iter_mut() {
            let value = *digit + carry;
            *digit = value % 10;
            carry = value / 10;
        }
        while carry > 0 {
            digits.push(carry % 10);
            carry /= 10;
        }
    }
    while digits.len() > 1 && *digits.last().unwrap() == 0 {
        digits.pop();
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    for digit in digits.iter().rev() {
        out.push_str(&digit.to_string());
    }
    out
}

/// 键签名：用于重复键检测。只取语义部分（类型/值/子节点），
/// 不含 offset/length/hex，否则同一键在不同位置永远不相等。
pub fn signature(node: &Node) -> String {
    let mut out = format!("{}:{:?}", node.kind, node.value);
    if let Some(number) = node.number {
        out.push_str(&format!("#{number}"));
    }
    if let Some(children) = &node.children {
        out.push('[');
        for child in children {
            out.push_str(&signature(child));
            out.push(',');
        }
        out.push(']');
    }
    if let Some(entries) = &node.entries {
        out.push('{');
        for entry in entries {
            out.push_str(&signature(&entry.key));
            out.push(':');
            out.push_str(&signature(&entry.value));
            out.push(',');
        }
        out.push('}');
    }
    out
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
