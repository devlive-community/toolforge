//! Protobuf wire format 解码：varint / zigzag / 长度前缀，供 descriptor 解析使用。
//! 只做结构解码，不解释字段语义；越界、截断、非法标签返回 pbd.* 错误。

use tf_plugin_api::{PluginError, PluginResult};

/// wire type：0=varint 1=64-bit 2=length-delimited 5=32-bit
pub const WT_VARINT: u8 = 0;
pub const WT_64BIT: u8 = 1;
pub const WT_LEN: u8 = 2;
pub const WT_32BIT: u8 = 5;

fn err(code: &str) -> PluginError {
    PluginError::new(code)
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

/// 单个 wire 字段：字段号 + wire type + 载荷
#[derive(Debug, Clone, PartialEq)]
pub struct WireField {
    pub number: u32,
    pub wire_type: u8,
    /// varint 值（wire type 0）
    pub varint: u64,
    /// 长度前缀载荷（wire type 2）
    pub bytes: Vec<u8>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_byte(&mut self) -> PluginResult<u8> {
        let byte = *self
            .data
            .get(self.pos)
            .ok_or_else(|| err("pbd.truncated"))?;
        self.pos += 1;
        Ok(byte)
    }

    pub fn read_varint(&mut self) -> PluginResult<u64> {
        let mut value: u64 = 0;
        let mut shift = 0u32;
        for _ in 0..10 {
            let byte = self.read_byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
        Err(err("pbd.bad_varint"))
    }

    pub fn read_bytes(&mut self, len: usize) -> PluginResult<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| err("pbd.truncated"))?;
        if end > self.data.len() {
            return Err(err("pbd.truncated"));
        }
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    /// 读取下一个字段头与载荷；遇到未知 wire type 返回错误
    pub fn next_field(&mut self) -> PluginResult<Option<WireField>> {
        if self.at_end() {
            return Ok(None);
        }
        let key = self.read_varint()?;
        let number = (key >> 3) as u32;
        if number == 0 {
            return Err(err("pbd.bad_tag"));
        }
        let wire_type = (key & 0x7) as u8;
        let field = match wire_type {
            WT_VARINT => WireField {
                number,
                wire_type,
                varint: self.read_varint()?,
                bytes: Vec::new(),
            },
            WT_LEN => {
                let len = self.read_varint()? as usize;
                WireField {
                    number,
                    wire_type,
                    varint: 0,
                    bytes: self.read_bytes(len)?.to_vec(),
                }
            }
            WT_64BIT => WireField {
                number,
                wire_type,
                varint: 0,
                bytes: self.read_bytes(8)?.to_vec(),
            },
            WT_32BIT => WireField {
                number,
                wire_type,
                varint: 0,
                bytes: self.read_bytes(4)?.to_vec(),
            },
            _ => return Err(err("pbd.bad_wire_type").with("wire_type", i64::from(wire_type))),
        };
        Ok(Some(field))
    }
}

/// zigzag 解码：sint32/sint64 存储映射（测试用；descriptor 字段值解码暂不需要）
#[cfg(test)]
pub fn zigzag_decode(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

#[cfg(test)]
#[path = "wire_test.rs"]
mod tests;
