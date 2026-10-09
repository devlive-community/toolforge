//! 十进制数：用数字字符串精确表示，避免浮点误差；解析阿拉伯数字输入并提供分组、科学计数法等格式。

use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Number {
    pub negative: bool,
    /// 整数部分，不含前导 0；为 0 时是 "0"
    pub int: String,
    /// 小数部分，不含末尾 0
    pub frac: String,
}

/// 整数部分最多的位数
pub const MAX_DIGITS: usize = 48;

impl Number {
    pub fn new(negative: bool, int: &str, frac: &str) -> PluginResult<Self> {
        let int = int.trim_start_matches('0');
        let frac = frac.trim_end_matches('0');
        if int.len() > MAX_DIGITS || frac.len() > MAX_DIGITS {
            return Err(PluginError::new("num.too_long"));
        }
        let int = if int.is_empty() { "0" } else { int };
        Ok(Self {
            negative: negative && (int != "0" || !frac.is_empty()),
            int: int.to_owned(),
            frac: frac.to_owned(),
        })
    }

    pub fn is_zero(&self) -> bool {
        self.int == "0" && self.frac.is_empty()
    }

    pub fn is_integer(&self) -> bool {
        self.frac.is_empty()
    }

    /// 四舍五入到 places 位小数
    pub fn round(&self, places: usize) -> Number {
        if self.frac.len() <= places {
            return self.clone();
        }
        let mut digits: Vec<u8> = self
            .int
            .bytes()
            .chain(self.frac.bytes().take(places))
            .map(|b| b - b'0')
            .collect();
        if self.frac.as_bytes()[places] >= b'5' {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
        let text: String = digits.iter().map(|d| (b'0' + d) as char).collect();
        let split = text.len() - places;
        Number::new(self.negative, &text[..split], &text[split..]).unwrap_or_else(|_| self.clone())
    }

    /// 整数部分（不超过 u128 时）
    pub fn int_value(&self) -> Option<u128> {
        self.int.parse().ok()
    }

    pub fn to_f64(&self) -> f64 {
        let text = format!(
            "{}{}.{}",
            if self.negative { "-" } else { "" },
            self.int,
            if self.frac.is_empty() {
                "0"
            } else {
                &self.frac
            }
        );
        text.parse().unwrap_or(f64::NAN)
    }

    pub fn plain(&self) -> String {
        self.grouped(0, ',')
    }

    /// 整数部分分组：group 为 0 表示不分组，3 为千分位；印度分组见 indian
    pub fn grouped(&self, group: usize, sep: char) -> String {
        let int = if group == 0 {
            self.int.clone()
        } else {
            let bytes: Vec<char> = self.int.chars().collect();
            let mut out = String::new();
            for (i, c) in bytes.iter().enumerate() {
                if i > 0 && (bytes.len() - i).is_multiple_of(group) {
                    out.push(sep);
                }
                out.push(*c);
            }
            out
        };
        self.with_sign_and_frac(int)
    }

    /// 印度分组：最后三位一组，之前两位一组（12,34,567）
    pub fn indian(&self) -> String {
        let int = &self.int;
        if int.len() <= 3 {
            return self.with_sign_and_frac(int.clone());
        }
        let (head, tail) = int.split_at(int.len() - 3);
        let chars: Vec<char> = head.chars().collect();
        let mut out = String::new();
        for (i, c) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i).is_multiple_of(2) {
                out.push(',');
            }
            out.push(*c);
        }
        self.with_sign_and_frac(format!("{out},{tail}"))
    }

    fn with_sign_and_frac(&self, int: String) -> String {
        let sign = if self.negative { "-" } else { "" };
        if self.frac.is_empty() {
            format!("{sign}{int}")
        } else {
            format!("{sign}{int}.{}", self.frac)
        }
    }

    /// 科学计数法：保留全部有效数字，如 1.23456e+6
    pub fn scientific(&self) -> String {
        if self.is_zero() {
            return "0".to_owned();
        }
        let digits = format!("{}{}", self.int, self.frac);
        let lead = digits.bytes().position(|b| b != b'0').unwrap_or(0);
        let exponent = self.int.len() as i64 - 1 - lead as i64;
        let significant = digits[lead..].trim_end_matches('0');
        let mantissa = if significant.len() > 1 {
            format!("{}.{}", &significant[..1], &significant[1..])
        } else {
            significant.to_owned()
        };
        format!(
            "{}{mantissa}e{}{exponent}",
            if self.negative { "-" } else { "" },
            if exponent >= 0 { "+" } else { "" }
        )
    }
}

/// 阿拉伯数字：允许千分位逗号、空格、下划线，科学计数法，以及 万 / 亿 / k / m 等后缀
pub fn parse_arabic(input: &str) -> Option<PluginResult<Number>> {
    let text: String = input
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '，' | '_' | ' ' | '\u{a0}'))
        .collect();
    let (negative, text) = match text.strip_prefix(['-', '−']) {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(&text)),
    };
    // 中文或英文的数量级后缀
    let suffixes: [(&str, i64); 8] = [
        ("万亿", 12),
        ("亿", 8),
        ("万", 4),
        ("千", 3),
        ("k", 3),
        ("K", 3),
        ("m", 6),
        ("M", 6),
    ];
    let (body, mut shift) = suffixes
        .iter()
        .find_map(|(s, n)| text.strip_suffix(s).map(|b| (b, *n)))
        .unwrap_or((text, 0));
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    if let Some(e) = exponent {
        shift += e.parse::<i64>().ok()?;
    }
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if (int.is_empty() && frac.is_empty())
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if shift.unsigned_abs() > MAX_DIGITS as u64 * 2 {
        return Some(Err(PluginError::new("num.too_long")));
    }
    // 移动小数点
    let digits = format!("{int}{frac}");
    let point = int.len() as i64 + shift;
    let (int, frac) = if point <= 0 {
        (
            String::new(),
            format!("{}{digits}", "0".repeat((-point) as usize)),
        )
    } else if point as usize >= digits.len() {
        (
            format!("{digits}{}", "0".repeat(point as usize - digits.len())),
            String::new(),
        )
    } else {
        (
            digits[..point as usize].to_owned(),
            digits[point as usize..].to_owned(),
        )
    };
    Some(Number::new(negative, &int, &frac))
}

#[cfg(test)]
#[path = "number_test.rs"]
mod tests;
