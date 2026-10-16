//! Source Map v3 的 Base64 VLQ 解码。
//!
//! mappings 是一个用 `;` 分行、`,` 分段、每段 1/4/5 个变长整数的字符串：
//! 第一列是生成列（相对本行上一段），其余四列（来源、源行、源列、名称索引）
//! 相对全局上一段累计。数值为有符号 VLQ：低 1 位是符号，其余是 5 位一组的
//! 小端数字，最高位（0x20）表示还有后续字节。

fn digit(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlqError {
    /// 出现字母表之外的字符
    InvalidChar(u8),
    /// 连续性位为 1 但字符已耗尽
    Truncated,
    /// 位移超过 i64 安全范围
    Overflow,
}

/// 解码一个 VLQ 数；返回（值，消耗的字符数）
pub fn decode(bytes: &[u8], mut at: usize) -> Result<(i64, usize), VlqError> {
    let start = at;
    let mut raw: u64 = 0;
    let mut shift = 0u32;
    loop {
        let Some(byte) = bytes.get(at) else {
            return Err(VlqError::Truncated);
        };
        let part = digit(*byte).ok_or(VlqError::InvalidChar(*byte))? as u64;
        if shift >= 63 {
            return Err(VlqError::Overflow);
        }
        raw |= (part & 0x1F) << shift;
        at += 1;
        if part & 0x20 == 0 {
            break;
        }
        shift += 5;
    }
    let negative = raw & 1 == 1;
    let value = (raw >> 1) as i64;
    Ok((if negative { -value } else { value }, at - start))
}

#[cfg(test)]
#[path = "vlq_test.rs"]
mod tests;
