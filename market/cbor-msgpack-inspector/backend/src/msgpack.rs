//! MessagePack 解析（官方 spec）：str/bin 区分、fixext/ext 类型与原始载荷、
//! timestamp（ext type -1）展开为秒与纳秒、uint/int 全范围、-0 与非有限浮点。
//! 0xc1 保留码、错误长度与非法 UTF-8 判为不合法并带偏移报告。

use super::model::{Ctx, ERROR, Entry, Node, Stop, WARNING, signature};

fn need(ctx: &mut Ctx, offset: usize, count: usize) -> Result<(), Stop> {
    if offset + count > ctx.bytes.len() {
        ctx.diag(ERROR, "bin.truncated", offset as u64);
        return Err(Stop);
    }
    Ok(())
}

fn bounded(ctx: &mut Ctx, length: u64, offset: u64) -> Result<usize, Stop> {
    if length > ctx.bytes.len() as u64 {
        ctx.diag(ERROR, "bin.length", offset);
        return Err(Stop);
    }
    Ok(length as u64 as usize)
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

fn note_duplicate(ctx: &mut Ctx, signatures: &[String], key: &Node, offset: u64) {
    let sig = signature(key);
    if signatures.iter().any(|seen| *seen == sig) {
        ctx.push(
            WARNING,
            "bin.duplicate_key",
            Some(offset),
            serde_json::Map::from_iter([(
                "kind".to_owned(),
                serde_json::Value::from(key.kind.clone()),
            )]),
        );
    }
}

/// 解析单个 MessagePack 项，返回节点与结束偏移
pub fn parse(ctx: &mut Ctx, depth: usize, offset: usize) -> Result<(Node, usize), Stop> {
    ctx.depth(depth, offset as u64)?;
    need(ctx, offset, 1)?;
    let byte = ctx.bytes[offset];
    let one = offset + 1;
    match byte {
        0x00..=0x7f => Ok((
            finish_node(ctx, "uint", offset as u64, 1, Some(byte.to_string()))?,
            one,
        )),
        0xe0..=0xff => {
            // 负 fixint：符号扩展
            let value = byte as i8;
            Ok((
                finish_node(ctx, "int", offset as u64, 1, Some(value.to_string()))?,
                one,
            ))
        }
        0x80..=0x8f => parse_map(ctx, depth, offset, (byte & 0x0f) as u64),
        0x90..=0x9f => parse_array(ctx, depth, offset, one, (byte & 0x0f) as u64),
        0xa0..=0xbf => parse_str(ctx, offset, (byte & 0x1f) as u64),
        0xc0 => Ok((finish_node(ctx, "null", offset as u64, 1, None)?, one)),
        0xc1 => {
            ctx.diag(ERROR, "mp.reserved", offset as u64);
            Err(Stop)
        }
        0xc2 => Ok((
            finish_node(ctx, "bool", offset as u64, 1, Some("false".to_owned()))?,
            one,
        )),
        0xc3 => Ok((
            finish_node(ctx, "bool", offset as u64, 1, Some("true".to_owned()))?,
            one,
        )),
        0xc4..=0xc6 => {
            let width = 1usize << (byte - 0xc4);
            need(ctx, one, width)?;
            let length = read_uint(ctx, one, width);
            let length = bounded(ctx, length, offset as u64)?;
            let start = one + width;
            need(ctx, start, length)?;
            let data = ctx.bytes[start..start + length].to_vec();
            let mut node = finish_node(
                ctx,
                "bytes",
                offset as u64,
                (start + length - offset) as u64,
                None,
            )?;
            node.value = Some(super::model::hex(&data));
            Ok((node, start + length))
        }
        0xc7..=0xc9 | 0xd4..=0xd8 => parse_ext(ctx, depth, offset),
        0xca => {
            need(ctx, one, 4)?;
            let mut bits: u64 = 0;
            for index in 0..4 {
                bits = (bits << 8) | ctx.bytes[one + index] as u64;
            }
            let value = f64::from(f32::from_bits(bits as u32));
            Ok((
                finish_node(ctx, "float", offset as u64, 5, Some(format!("{value:?}")))?,
                one + 4,
            ))
        }
        0xcb => {
            need(ctx, one, 8)?;
            let mut bits: u64 = 0;
            for index in 0..8 {
                bits = (bits << 8) | ctx.bytes[one + index] as u64;
            }
            let value = f64::from_bits(bits);
            Ok((
                finish_node(ctx, "float", offset as u64, 9, Some(format!("{value:?}")))?,
                one + 8,
            ))
        }
        0xcc..=0xcf => {
            let width = 1usize << (byte - 0xcc);
            need(ctx, one, width)?;
            let mut value: u64 = 0;
            for index in 0..width {
                value = (value << 8) | ctx.bytes[one + index] as u64;
            }
            Ok((
                finish_node(
                    ctx,
                    "uint",
                    offset as u64,
                    (1 + width) as u64,
                    Some(value.to_string()),
                )?,
                one + width,
            ))
        }
        0xd0..=0xd3 => {
            let width = 1usize << (byte - 0xd0);
            need(ctx, one, width)?;
            let mut raw: u64 = 0;
            for index in 0..width {
                raw = (raw << 8) | ctx.bytes[one + index] as u64;
            }
            // 符号扩展
            let value = match width {
                1 => i8::from_be_bytes([raw as u8]).to_string(),
                2 => i16::from_be_bytes([raw as u8, (raw >> 8) as u8]).to_string(),
                4 => i32::from_be_bytes([
                    raw as u8,
                    (raw >> 8) as u8,
                    (raw >> 16) as u8,
                    (raw >> 24) as u8,
                ])
                .to_string(),
                _ => (raw as i64).to_string(),
            };
            Ok((
                finish_node(ctx, "int", offset as u64, (1 + width) as u64, Some(value))?,
                one + width,
            ))
        }
        0xd9..=0xdb => {
            let width = match byte {
                0xd9 => 1,
                0xda => 2,
                _ => 4,
            };
            need(ctx, one, width)?;
            let length = read_uint(ctx, one, width);
            let length = bounded(ctx, length, offset as u64)?;
            let start = one + width;
            need(ctx, start, length)?;
            parse_str_at(ctx, offset, start, length)
        }
        0xdc..=0xdd => {
            let width = if byte == 0xdc { 2 } else { 4 };
            need(ctx, one, width)?;
            let count = read_uint(ctx, one, width);
            let count = bounded(ctx, count, offset as u64)?;
            parse_array(ctx, depth, offset, one + width, count as u64)
        }
        0xde..=0xdf => {
            let width = if byte == 0xde { 2 } else { 4 };
            need(ctx, one, width)?;
            let count = read_uint(ctx, one, width);
            let count = bounded(ctx, count, offset as u64)?;
            // map 的 count 是条目数，不是字节数
            parse_map_counted(ctx, depth, offset, one + width, count as u64)
        }
    }
}

fn parse_str(ctx: &mut Ctx, offset: usize, length: u64) -> Result<(Node, usize), Stop> {
    let length = bounded(ctx, length, offset as u64)?;
    parse_str_at(ctx, offset, offset + 1, length)
}

fn parse_str_at(
    ctx: &mut Ctx,
    offset: usize,
    start: usize,
    length: usize,
) -> Result<(Node, usize), Stop> {
    need(ctx, start, length)?;
    let data = &ctx.bytes[start..start + length];
    let text = match std::str::from_utf8(data) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            ctx.diag(ERROR, "mp.utf8", offset as u64);
            return Err(Stop);
        }
    };
    let node = finish_node(
        ctx,
        "str",
        offset as u64,
        (start + length - offset) as u64,
        Some(text),
    )?;
    Ok((node, start + length))
}

fn parse_array(
    ctx: &mut Ctx,
    depth: usize,
    offset: usize,
    start: usize,
    count: u64,
) -> Result<(Node, usize), Stop> {
    let count = bounded(ctx, count, offset as u64)?;
    let mut children = Vec::new();
    let mut at = start;
    for _ in 0..count {
        let (child, next) = parse(ctx, depth + 1, at)?;
        children.push(child);
        at = next;
    }
    let node = finish_node(ctx, "array", offset as u64, (at - offset) as u64, None)?;
    Ok((with_children(node, children), at))
}

fn parse_map(
    ctx: &mut Ctx,
    depth: usize,
    offset: usize,
    count: u64,
) -> Result<(Node, usize), Stop> {
    parse_map_counted(ctx, depth, offset, offset + 1, count)
}

fn parse_map_counted(
    ctx: &mut Ctx,
    depth: usize,
    offset: usize,
    start: usize,
    count: u64,
) -> Result<(Node, usize), Stop> {
    let count = bounded(ctx, count, offset as u64)?;
    let mut entries = Vec::new();
    let mut signatures: Vec<String> = Vec::new();
    let mut at = start;
    for _ in 0..count {
        let (key, after_key) = parse(ctx, depth + 1, at)?;
        let (value, after_value) = parse(ctx, depth + 1, after_key)?;
        note_duplicate(ctx, &signatures, &key, at as u64);
        signatures.push(signature(&key));
        entries.push(Entry { key, value });
        at = after_value;
    }
    let node = finish_node(ctx, "map", offset as u64, (at - offset) as u64, None)?;
    Ok((with_entries(node, entries), at))
}

/// fixext / ext8/16/32：类型号 + 原始载荷；timestamp（-1）展开秒/纳秒
fn parse_ext(ctx: &mut Ctx, depth: usize, offset: usize) -> Result<(Node, usize), Stop> {
    let _ = depth;
    let byte = ctx.bytes[offset];
    let one = offset + 1;
    let payload_length: u64 = match byte {
        0xd4 => 1,
        0xd5 => 2,
        0xd6 => 4,
        0xd7 => 8,
        0xd8 => 16,
        0xc7 => {
            need(ctx, one, 1)?;
            ctx.bytes[one] as u64
        }
        0xc8 => {
            need(ctx, one, 2)?;
            u16::from_be_bytes([ctx.bytes[one], ctx.bytes[one + 1]]) as u64
        }
        _ => {
            need(ctx, one, 4)?;
            u32::from_be_bytes([
                ctx.bytes[one],
                ctx.bytes[one + 1],
                ctx.bytes[one + 2],
                ctx.bytes[one + 3],
            ]) as u64
        }
    };
    let type_width = match byte {
        0xd4..=0xd8 | 0xc7 => 1,
        0xc8 => 2,
        _ => 4,
    };
    let length_width = match byte {
        0xc7 | 0xc8 | 0xc9 => 1usize << (byte - 0xc7),
        _ => 0,
    };
    let type_at = one + length_width;
    need(ctx, type_at, type_width)?;
    let ext_type = ctx.bytes[type_at] as i8;
    let payload_at = type_at + type_width;
    let payload_length = bounded(ctx, payload_length, offset as u64)?;
    need(ctx, payload_at, payload_length)?;
    let end = payload_at + payload_length;
    let payload = ctx.bytes[payload_at..end].to_vec();

    // 唯一明确支持的扩展：timestamp（type -1）
    if ext_type == -1 {
        let value = match (byte, payload_length) {
            (0xd6, 4) => {
                let seconds =
                    u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as i64;
                format!("seconds={seconds}, nanoseconds=0")
            }
            (0xd7, 8) => {
                let raw = u64::from_be_bytes([
                    payload[0], payload[1], payload[2], payload[3], payload[4], payload[5],
                    payload[6], payload[7],
                ]);
                let nanoseconds = (raw >> 34) as u64;
                let seconds = (raw & 0x3_ffff_ffff) as i64;
                format!("seconds={seconds}, nanoseconds={nanoseconds}")
            }
            (_, 12) if byte == 0xc7 || byte == 0xc8 || byte == 0xc9 => {
                let nanoseconds =
                    u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let seconds = i64::from_be_bytes([
                    payload[4],
                    payload[5],
                    payload[6],
                    payload[7],
                    payload[8],
                    payload[9],
                    payload[10],
                    payload[11],
                ]);
                format!("seconds={seconds}, nanoseconds={nanoseconds}")
            }
            _ => {
                ctx.diag(ERROR, "mp.timestamp_length", offset as u64);
                return Err(Stop);
            }
        };
        let mut node = finish_node(
            ctx,
            "timestamp",
            offset as u64,
            (end - offset) as u64,
            Some(value),
        )?;
        node.number = Some(ext_type as i64);
        return Ok((node, end));
    }
    let mut node = finish_node(
        ctx,
        "ext",
        offset as u64,
        (end - offset) as u64,
        Some(super::model::hex(&payload)),
    )?;
    node.number = Some(ext_type as i64);
    Ok((node, end))
}

fn read_uint(ctx: &Ctx, offset: usize, width: usize) -> u64 {
    let mut value = 0u64;
    for index in 0..width {
        value = (value << 8) | ctx.bytes[offset + index] as u64;
    }
    value
}

fn with_children(mut node: Node, children: Vec<Node>) -> Node {
    node.children = Some(children);
    node
}

fn with_entries(mut node: Node, entries: Vec<Entry>) -> Node {
    node.entries = Some(entries);
    node
}

/// 顶层解析 + 尾随数据检查
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
#[path = "msgpack_test.rs"]
mod tests;
