use num_bigint::{BigInt, Sign};
use num_traits::{One, Signed, Zero};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 输入数字的最大位数，避免超大数的平方级格式化开销
const MAX_DIGITS: usize = 4096;
const WIDTHS: [u32; 5] = [8, 16, 32, 64, 128];
const BIT_GRID: u32 = 64;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    input: String,
    /// 输入进制 2-36；0 表示按前缀自动识别（默认十进制）
    #[serde(default)]
    from: u32,
    /// 额外展示的自定义进制
    #[serde(default = "default_custom")]
    custom: u32,
    #[serde(default)]
    uppercase: bool,
    /// 按位分组（二进制/十六进制每 4 位、八进制每 3 位、十进制每 3 位）
    #[serde(default)]
    group: bool,
    /// 翻转 64 位视图中的某一位（0 为最低位）
    #[serde(default)]
    toggle: Option<u32>,
}

fn default_custom() -> u32 {
    36
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Width {
    pub bits: u32,
    pub hex: String,
    pub binary: String,
    pub unsigned: String,
    pub signed: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// 实际使用的输入进制
    pub base: u32,
    /// 值在输入进制下的规范写法（不分组、无前缀），翻转位后用于回填输入框
    pub source: String,
    pub negative: bool,
    pub binary: String,
    pub octal: String,
    pub decimal: String,
    pub hex: String,
    pub custom: String,
    pub custom_base: u32,
    pub bit_length: u64,
    pub byte_length: u64,
    /// 能容纳该值的各位宽下的补码表示
    pub widths: Vec<Width>,
    /// 64 位补码的二进制位（最高位在前），值超出 64 位范围时为空
    pub bits: Option<String>,
}

fn invalid_base(base: u32) -> PluginError {
    PluginError::new("base.invalid_base").with("base", base)
}

/// 解析输入：支持正负号、0x/0o/0b 前缀，忽略空格、下划线与逗号分隔符
pub fn parse(input: &str, from: u32) -> PluginResult<(BigInt, u32)> {
    if from != 0 && !(2..=36).contains(&from) {
        return Err(invalid_base(from));
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PluginError::new("base.empty"));
    }
    let (negative, rest) = match trimmed.as_bytes()[0] {
        b'-' => (true, &trimmed[1..]),
        b'+' => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let rest = rest.trim_start();
    let lower = rest.get(..2).map(str::to_ascii_lowercase);
    let prefixed = match lower.as_deref() {
        Some("0x") => Some(16),
        Some("0o") => Some(8),
        Some("0b") => Some(2),
        _ => None,
    };
    let (base, digits) = match (prefixed, from) {
        (Some(p), 0) => (p, &rest[2..]),
        (Some(p), f) if p == f => (p, &rest[2..]),
        (_, 0) => (10, rest),
        (_, f) => (f, rest),
    };

    let mut clean = String::with_capacity(digits.len());
    for (index, ch) in digits.char_indices() {
        if ch.is_whitespace() || ch == '_' || ch == ',' {
            continue;
        }
        if ch.to_digit(base).is_none() {
            return Err(PluginError::new("base.invalid_digit")
                .with("char", ch.to_string())
                .with("position", trimmed.len() - digits.len() + index + 1)
                .with("base", base));
        }
        clean.push(ch);
    }
    if clean.is_empty() {
        return Err(PluginError::new("base.empty"));
    }
    if clean.len() > MAX_DIGITS {
        return Err(PluginError::new("base.too_long").with("limit", MAX_DIGITS));
    }
    let magnitude = BigInt::parse_bytes(clean.as_bytes(), base)
        .ok_or_else(|| PluginError::new("base.empty"))?;
    Ok((if negative { -magnitude } else { magnitude }, base))
}

/// 从右往左每 `size` 个字符插入一个分隔符
pub fn group(digits: &str, size: usize, separator: char) -> String {
    let chars: Vec<char> = digits.chars().collect();
    let mut out = String::with_capacity(chars.len() + chars.len() / size);
    for (index, ch) in chars.iter().enumerate() {
        if index > 0 && (chars.len() - index).is_multiple_of(size) {
            out.push(separator);
        }
        out.push(*ch);
    }
    out
}

struct Style {
    uppercase: bool,
    group: bool,
}

impl Style {
    fn format(&self, value: &BigInt, base: u32) -> String {
        let digits = value.magnitude().to_str_radix(base);
        let digits = if self.uppercase {
            digits.to_ascii_uppercase()
        } else {
            digits
        };
        let digits = if self.group {
            match base {
                2 | 16 => group(&digits, 4, ' '),
                8 => group(&digits, 3, ' '),
                10 => group(&digits, 3, ','),
                _ => digits,
            }
        } else {
            digits
        };
        if value.is_negative() {
            format!("-{digits}")
        } else {
            digits
        }
    }

    /// 定宽补码：固定位数补零
    fn padded(&self, value: &BigInt, base: u32, bits: u32) -> String {
        let width = match base {
            2 => bits as usize,
            _ => bits.div_ceil(4) as usize,
        };
        let digits = format!("{:0>width$}", value.magnitude().to_str_radix(base));
        let digits = if self.uppercase {
            digits.to_ascii_uppercase()
        } else {
            digits
        };
        if self.group {
            group(&digits, 4, ' ')
        } else {
            digits
        }
    }
}

/// 值在 `bits` 位宽下的补码（无符号形式）；超出 [-2^(n-1), 2^n) 时返回 None
pub fn twos_complement(value: &BigInt, bits: u32) -> Option<BigInt> {
    let modulus = BigInt::one() << bits;
    let half = BigInt::one() << (bits - 1);
    if value.is_negative() {
        (-value <= half).then(|| &modulus + value)
    } else {
        (value < &modulus).then(|| value.clone())
    }
}

fn signed_of(unsigned: &BigInt, bits: u32) -> BigInt {
    if unsigned >= &(BigInt::one() << (bits - 1)) {
        unsigned - (BigInt::one() << bits)
    } else {
        unsigned.clone()
    }
}

/// 翻转 64 位补码表示中的一位；输入为负数时结果按有符号解释
pub fn toggle_bit(value: &BigInt, bit: u32) -> PluginResult<BigInt> {
    if bit >= BIT_GRID {
        return Err(PluginError::new("base.invalid_bit").with("bit", bit));
    }
    let unsigned =
        twos_complement(value, BIT_GRID).ok_or_else(|| PluginError::new("base.out_of_range"))?;
    let flipped = unsigned ^ (BigInt::one() << bit);
    Ok(if value.is_negative() {
        signed_of(&flipped, BIT_GRID)
    } else {
        flipped
    })
}

pub fn convert(args: Args) -> PluginResult<Output> {
    if !(2..=36).contains(&args.custom) {
        return Err(invalid_base(args.custom));
    }
    let (mut value, base) = parse(&args.input, args.from)?;
    if let Some(bit) = args.toggle {
        value = toggle_bit(&value, bit)?;
    }
    let style = Style {
        uppercase: args.uppercase,
        group: args.group,
    };
    let plain = Style {
        uppercase: args.uppercase,
        group: false,
    };

    let bit_length = value.magnitude().bits();
    let widths = WIDTHS
        .iter()
        .filter_map(|&bits| {
            let unsigned = twos_complement(&value, bits)?;
            Some(Width {
                bits,
                hex: style.padded(&unsigned, 16, bits),
                binary: style.padded(&unsigned, 2, bits),
                signed: signed_of(&unsigned, bits).to_string(),
                unsigned: unsigned.to_string(),
            })
        })
        .collect();
    let bits = twos_complement(&value, BIT_GRID)
        .map(|unsigned| format!("{:0>64}", unsigned.magnitude().to_str_radix(2)));

    Ok(Output {
        base,
        source: plain.format(&value, base),
        negative: value.sign() == Sign::Minus,
        binary: style.format(&value, 2),
        octal: style.format(&value, 8),
        decimal: style.format(&value, 10),
        hex: style.format(&value, 16),
        custom: style.format(&value, args.custom),
        custom_base: args.custom,
        bit_length,
        byte_length: if value.is_zero() {
            1
        } else {
            bit_length.div_ceil(8)
        },
        widths,
        bits,
    })
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
