//! CBOR 解码（RFC 8949）：全部 major type、不定长与字符串分块、tag（2/3 大整数、
//! 其余保留类型与原值）、simple value、半/单/双精度浮点（保留 -0 与非有限）。
//! 保留附加信息、裸 break、错误长度与非法 UTF-8 都按规范判为不合法并带偏移报告。

use super::model::{Ctx, ERROR, Entry, Node, Stop, WARNING, bigint_to_decimal, signature};
use serde_json::Value;

const MAJOR_UINT: u8 = 0;
const MAJOR_NEGINT: u8 = 1;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_ARRAY: u8 = 4;
const MAJOR_MAP: u8 = 5;
const MAJOR_TAG: u8 = 6;
const MAJOR_SIMPLE: u8 = 7;

fn need(ctx: &mut Ctx, offset: usize, count: usize) -> Result<(), Stop> {
    if offset + count > ctx.bytes.len() {
        ctx.diag(ERROR, "bin.truncated", offset as u64);
        return Err(Stop);
    }
    Ok(())
}

/// 长度/数量守卫：wasm32 的 usize 是 32 位，先把 u64 与输入上限比较
fn bounded(ctx: &mut Ctx, length: u64, offset: u64) -> Result<usize, Stop> {
    if length > ctx.bytes.len() as u64 {
        ctx.diag(ERROR, "bin.length", offset);
        return Err(Stop);
    }
    Ok(length as usize)
}

/// 解析初始字节：返回 (major, 附加信息, 参数字节数, 是否 AI=31)
fn head(ctx: &mut Ctx, offset: usize) -> Result<(u8, u8, usize, bool), Stop> {
    need(ctx, offset, 1)?;
    let byte = ctx.bytes[offset];
    let major = byte >> 5;
    let info = byte & 0x1f;
    let (args, break_or_indefinite) = match info {
        0..=23 => (0usize, false),
        24 => (1, false),
        25 => (2, false),
        26 => (4, false),
        27 => (8, false),
        31 => (0, true),
        // 28-30 保留
        _ => {
            ctx.diag(ERROR, "cbor.reserved", offset as u64);
            return Err(Stop);
        }
    };
    need(ctx, offset + 1, args)?;
    Ok((major, info, args, break_or_indefinite))
}

fn read_uint(ctx: &Ctx, offset: usize, args: usize) -> u64 {
    let mut value = 0u64;
    for index in 0..args {
        value = (value << 8) | ctx.bytes[offset + 1 + index] as u64;
    }
    value
}

fn finish_node(
    ctx: &mut Ctx,
    kind: &str,
    offset: u64,
    length: u64,
    value: Option<String>,
) -> Result<Node, Stop> {
    let (hex, hex_truncated) = ctx.begin(offset, length)?;
    Ok(Node {
        kind: kind.to_owned(),
        offset,
        length,
        value,
        hex,
        hex_truncated,
        children: None,
        entries: None,
        number: None,
        indefinite: None,
    })
}

fn mark_indefinite(mut node: Node) -> Node {
    node.indefinite = Some(true);
    node
}

fn note_duplicate(ctx: &mut Ctx, signatures: &[String], key: &Node, offset: u64) {
    let sig = signature(key);
    if signatures.iter().any(|seen| *seen == sig) {
        ctx.push(
            WARNING,
            "bin.duplicate_key",
            Some(offset),
            serde_json::Map::from_iter([("kind".to_owned(), Value::from(key.kind.clone()))]),
        );
    }
}

/// 读一个字节串或文本串（major 2/3，定长或不定长分块）；tag 2/3 与文本共用
fn read_chunks(
    ctx: &mut Ctx,
    depth: usize,
    offset: usize,
    major: u8,
    kind: &str,
) -> Result<(Vec<u8>, Node, usize), Stop> {
    let (_item_major, info, args, indefinite) = head(ctx, offset)?;
    // AI ≤ 23 时参数就是附加信息本身
    let argument = if args == 0 {
        info as u64
    } else {
        read_uint(ctx, offset, args)
    };
    let body = offset + 1 + args;
    let (data, end, is_indefinite) = if indefinite {
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let mut at = body;
        loop {
            ctx.depth(depth, at as u64)?;
            let (chunk_major, chunk_info, chunk_args, _) = head(ctx, at)?;
            if chunk_major == MAJOR_SIMPLE && chunk_info == 31 {
                let end = at + 1;
                let data = parts.concat();
                let node = finish_node(ctx, kind, offset as u64, (end - offset) as u64, None)?;
                return Ok((data, mark_indefinite(node), end));
            }
            if chunk_major != major {
                ctx.diag(ERROR, "cbor.chunk_type", at as u64);
                return Err(Stop);
            }
            let chunk_argument = if chunk_args == 0 {
                chunk_info as u64
            } else {
                read_uint(ctx, at, chunk_args)
            };
            let length = bounded(ctx, chunk_argument, at as u64)?;
            let start = at + 1 + chunk_args;
            need(ctx, start, length)?;
            parts.push(ctx.bytes[start..start + length].to_vec());
            at = start + length;
        }
    } else {
        let length = bounded(ctx, argument, offset as u64)?;
        need(ctx, body, length)?;
        (
            ctx.bytes[body..body + length].to_vec(),
            body + length,
            false,
        )
    };
    let node = finish_node(ctx, kind, offset as u64, (end - offset) as u64, None)?;
    Ok((
        data,
        if is_indefinite {
            mark_indefinite(node)
        } else {
            node
        },
        end,
    ))
}

/// 解析单个 CBOR 项，返回节点与其结束偏移
pub fn parse(ctx: &mut Ctx, depth: usize, offset: usize) -> Result<(Node, usize), Stop> {
    ctx.depth(depth, offset as u64)?;
    let (major, info, args, indefinite) = head(ctx, offset)?;
    // AI ≤ 23 时参数就是附加信息本身
    let argument = if args == 0 {
        info as u64
    } else {
        read_uint(ctx, offset, args)
    };
    let body = offset + 1 + args;
    match major {
        MAJOR_UINT => {
            if indefinite {
                ctx.diag(ERROR, "cbor.reserved", offset as u64);
                return Err(Stop);
            }
            let node = finish_node(
                ctx,
                "uint",
                offset as u64,
                (1 + args) as u64,
                Some(argument.to_string()),
            )?;
            Ok((node, body))
        }
        MAJOR_NEGINT => {
            if indefinite {
                ctx.diag(ERROR, "cbor.reserved", offset as u64);
                return Err(Stop);
            }
            // -1 - n；超出 i64 范围时仍保留精确十进制
            let value = if argument <= i64::MAX as u64 {
                (-1 - argument as i64).to_string()
            } else {
                format!("-{}", argument as u128 + 1)
            };
            let node = finish_node(ctx, "int", offset as u64, (1 + args) as u64, Some(value))?;
            Ok((node, body))
        }
        MAJOR_BYTES => {
            let (data, mut node, end) = read_chunks(ctx, depth, offset, MAJOR_BYTES, "bytes")?;
            node.value = Some(super::model::hex(&data));
            Ok((node, end))
        }
        MAJOR_TEXT => {
            let (data, mut node, end) = read_chunks(ctx, depth, offset, MAJOR_TEXT, "str")?;
            match String::from_utf8(data) {
                Ok(text) => node.value = Some(text),
                Err(_) => {
                    ctx.diag(ERROR, "cbor.utf8", offset as u64);
                    return Err(Stop);
                }
            }
            Ok((node, end))
        }
        MAJOR_ARRAY => {
            if indefinite {
                let mut children = Vec::new();
                let mut at = body;
                loop {
                    ctx.depth(depth + 1, at as u64)?;
                    let (item_major, item_info, _, _) = head(ctx, at)?;
                    if item_major == MAJOR_SIMPLE && item_info == 31 {
                        at += 1;
                        let node =
                            finish_node(ctx, "array", offset as u64, (at - offset) as u64, None)?;
                        return Ok((mark_indefinite(with_children(node, children)), at));
                    }
                    let (child, next) = parse(ctx, depth + 1, at)?;
                    children.push(child);
                    at = next;
                }
            }
            let count = bounded(ctx, argument, offset as u64)?;
            let mut children = Vec::new();
            let mut at = body;
            for _ in 0..count {
                let (child, next) = parse(ctx, depth + 1, at)?;
                children.push(child);
                at = next;
            }
            let node = finish_node(ctx, "array", offset as u64, (at - offset) as u64, None)?;
            Ok((with_children(node, children), at))
        }
        MAJOR_MAP => {
            let mut entries = Vec::new();
            let mut signatures: Vec<String> = Vec::new();
            let end = if indefinite {
                let mut at = body;
                loop {
                    ctx.depth(depth + 1, at as u64)?;
                    let (key_major, key_info, _, _) = head(ctx, at)?;
                    if key_major == MAJOR_SIMPLE && key_info == 31 {
                        at += 1;
                        break at;
                    }
                    let (key, after_key) = parse(ctx, depth + 1, at)?;
                    let (value, after_value) = parse(ctx, depth + 1, after_key)?;
                    note_duplicate(ctx, &signatures, &key, at as u64);
                    signatures.push(signature(&key));
                    entries.push(Entry { key, value });
                    at = after_value;
                }
            } else {
                let count = bounded(ctx, argument, offset as u64)?;
                let mut at = body;
                for _ in 0..count {
                    let (key, after_key) = parse(ctx, depth + 1, at)?;
                    let (value, after_value) = parse(ctx, depth + 1, after_key)?;
                    note_duplicate(ctx, &signatures, &key, at as u64);
                    signatures.push(signature(&key));
                    entries.push(Entry { key, value });
                    at = after_value;
                }
                at
            };
            let node = finish_node(ctx, "map", offset as u64, (end - offset) as u64, None)?;
            Ok((with_entries(node, entries), end))
        }
        MAJOR_TAG => {
            if indefinite {
                ctx.diag(ERROR, "cbor.reserved", offset as u64);
                return Err(Stop);
            }
            let tag = argument as i64;
            let (node, end) = match tag {
                2 | 3 => {
                    let (inner_major, _, _, _) = head(ctx, body)?;
                    if inner_major != MAJOR_BYTES {
                        ctx.diag(ERROR, "cbor.tag_bytes", body as u64);
                        return Err(Stop);
                    }
                    let (data, _inner, end) =
                        read_chunks(ctx, depth + 1, body, MAJOR_BYTES, "bytes")?;
                    let text = bigint_to_decimal(&data, tag == 3);
                    (
                        finish_node(
                            ctx,
                            "bigint",
                            offset as u64,
                            (end - offset) as u64,
                            Some(text),
                        )?,
                        end,
                    )
                }
                _ => {
                    let (inner, end) = parse(ctx, depth + 1, body)?;
                    let mut node =
                        finish_node(ctx, "tag", offset as u64, (end - offset) as u64, None)?;
                    node.number = Some(tag);
                    node.children = Some(vec![inner]);
                    (node, end)
                }
            };
            Ok((node, end))
        }
        MAJOR_SIMPLE => match info {
            20 => Ok((
                finish_node(ctx, "bool", offset as u64, 1, Some("false".to_owned()))?,
                offset + 1,
            )),
            21 => Ok((
                finish_node(ctx, "bool", offset as u64, 1, Some("true".to_owned()))?,
                offset + 1,
            )),
            22 => Ok((
                finish_node(ctx, "null", offset as u64, 1, None)?,
                offset + 1,
            )),
            23 => Ok((
                finish_node(ctx, "undefined", offset as u64, 1, None)?,
                offset + 1,
            )),
            24 => {
                // 1 字节 simple value 只允许 ≥32
                if argument < 32 {
                    ctx.diag(ERROR, "cbor.simple_invalid", offset as u64);
                    return Err(Stop);
                }
                let node =
                    finish_node(ctx, "simple", offset as u64, 2, Some(argument.to_string()))?;
                Ok((node, offset + 2))
            }
            25 | 26 | 27 => {
                let width = 1usize << (info - 24);
                need(ctx, offset + 1, width)?;
                let mut bits: u64 = 0;
                for index in 0..width {
                    bits = (bits << 8) | ctx.bytes[offset + 1 + index] as u64;
                }
                let value = match info {
                    25 => half_to_f64(bits as u16),
                    // 按数值转换：f32 位模式直接 shift 会破坏指数偏置
                    26 => f64::from(f32::from_bits(bits as u32)),
                    _ => f64::from_bits(bits),
                };
                // 保留 -0 与非有限浮点：Rust Debug 显示 -0.0 / NaN / inf / -inf
                let node = finish_node(
                    ctx,
                    "float",
                    offset as u64,
                    (1 + width) as u64,
                    Some(format!("{value:?}")),
                )?;
                Ok((node, offset + 1 + width))
            }
            31 => {
                // 不在定长结构里的裸 break
                ctx.diag(ERROR, "cbor.break", offset as u64);
                Err(Stop)
            }
            _ => {
                ctx.diag(ERROR, "cbor.simple_reserved", offset as u64);
                Err(Stop)
            }
        },
        _ => Err(Stop),
    }
}

fn with_children(mut node: Node, children: Vec<Node>) -> Node {
    node.children = Some(children);
    node
}

fn with_entries(mut node: Node, entries: Vec<Entry>) -> Node {
    node.entries = Some(entries);
    node
}

/// IEEE 754 半精度 → f64
fn half_to_f64(bits: u16) -> f64 {
    let sign = ((bits >> 15) as u64) << 63;
    let exponent = ((bits >> 10) & 0x1f) as u64;
    let fraction = (bits & 0x3ff) as u64;
    match exponent {
        0 if fraction == 0 => f64::from_bits(sign),
        0 => {
            let value = fraction as f64 / 1024.0 * 2f64.powi(-14);
            if sign == 0 { value } else { -value }
        }
        31 => f64::from_bits(sign | 0x7ff0_0000_0000_0000 | (fraction << 42)),
        _ => {
            let bits = sign | ((exponent - 15 + 1023) << 52) | (fraction << 42);
            f64::from_bits(bits)
        }
    }
}

/// 顶层解析 + 尾随数据检查；失败时返回 None 根与诊断
pub fn run(ctx: &mut Ctx) -> (Option<Node>, Option<super::model::Trailing>) {
    match parse(ctx, 0, 0) {
        Ok((node, end)) => {
            let trailing = if end < ctx.bytes.len() {
                Some(super::model::Trailing {
                    offset: end as u64,
                    length: (ctx.bytes.len() - end) as u64,
                })
            } else {
                None
            };
            (Some(node), trailing)
        }
        Err(_) => (None, None),
    }
}

#[cfg(test)]
#[path = "cbor_test.rs"]
mod tests;
