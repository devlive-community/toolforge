//! 支持的码制：校验输入、补全或核对校验位，得到编码器需要的内容与条码下方显示的文字。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Ean13,
    Ean8,
    UpcA,
    Code128,
    Code39,
    Code93,
    Itf,
    Codabar,
    DataMatrix,
    Pdf417,
    Aztec,
}

/// 编码前的内容
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    /// 交给编码器的内容（已含校验位）
    pub content: String,
    /// 自动补上的校验位
    pub check_digit: Option<char>,
    /// 提示（i18n 代码），如 Code 39 使用了扩展模式
    pub notes: Vec<String>,
}

/// GS1 校验位：从右往左，奇数位乘 3、偶数位乘 1
pub fn gs1_check_digit(digits: &str) -> char {
    let sum: u32 = digits
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| (b - b'0') as u32 * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    char::from(b'0' + ((10 - sum % 10) % 10) as u8)
}

fn invalid_char(c: char) -> PluginError {
    PluginError::new("barcode.invalid_char").with("char", c.to_string())
}

fn digits_only(text: &str) -> PluginResult<()> {
    match text.chars().find(|c| !c.is_ascii_digit()) {
        Some(c) => Err(invalid_char(c)),
        None => Ok(()),
    }
}

/// 不带校验位时补上，带了就核对
fn gs1(text: &str, without_check: usize) -> PluginResult<Prepared> {
    digits_only(text)?;
    let len = text.len();
    if len == without_check {
        let check = gs1_check_digit(text);
        return Ok(Prepared {
            content: format!("{text}{check}"),
            check_digit: Some(check),
            notes: Vec::new(),
        });
    }
    if len != without_check + 1 {
        return Err(PluginError::new("barcode.length")
            .with(
                "expected",
                format!("{without_check} / {}", without_check + 1),
            )
            .with("actual", len));
    }
    let expected = gs1_check_digit(&text[..without_check]);
    if !text.ends_with(expected) {
        return Err(PluginError::new("barcode.check_digit")
            .with("expected", expected.to_string())
            .with("actual", text[without_check..].to_owned()));
    }
    Ok(Prepared {
        content: text.to_owned(),
        check_digit: None,
        notes: Vec::new(),
    })
}

fn plain(content: String) -> Prepared {
    Prepared {
        content,
        check_digit: None,
        notes: Vec::new(),
    }
}

const CODE39: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
const CODABAR_BODY: &str = "0123456789-$:/.+";
const CODABAR_GUARDS: &str = "ABCD";

pub fn prepare(format: Format, text: &str) -> PluginResult<Prepared> {
    if text.is_empty() {
        return Err(PluginError::new("barcode.empty"));
    }
    match format {
        Format::Ean13 => gs1(text, 12),
        Format::Ean8 => gs1(text, 7),
        Format::UpcA => gs1(text, 11),
        Format::Itf => {
            digits_only(text)?;
            match text.len() {
                // ITF-14：13 位时补 GS1 校验位
                13 => gs1(text, 13),
                n if n % 2 == 1 => Err(PluginError::new("barcode.itf_even").with("actual", n)),
                n if n > 80 => Err(PluginError::new("barcode.too_long").with("max", 80)),
                _ => Ok(plain(text.to_owned())),
            }
        }
        Format::Code39 => {
            if let Some(c) = text.chars().find(|c| !c.is_ascii() || c.is_ascii_control()) {
                return Err(invalid_char(c));
            }
            let mut prepared = plain(text.to_owned());
            if text.chars().any(|c| !CODE39.contains(c)) {
                // 小写字母等需要扩展模式（Full ASCII），部分扫码枪要开启后才能识别
                prepared.notes.push("code39Extended".into());
            }
            Ok(prepared)
        }
        Format::Code93 | Format::Code128 => match text
            .chars()
            .find(|c| !c.is_ascii() || (c.is_ascii_control() && format == Format::Code128))
        {
            Some(c) => Err(invalid_char(c)),
            None => Ok(plain(text.to_owned())),
        },
        Format::Codabar => {
            let upper = text.to_ascii_uppercase();
            let chars: Vec<char> = upper.chars().collect();
            let guarded = chars.len() >= 2
                && CODABAR_GUARDS.contains(chars[0])
                && CODABAR_GUARDS.contains(chars[chars.len() - 1]);
            let body = if guarded {
                &upper[1..upper.len() - 1]
            } else {
                upper.as_str()
            };
            if let Some(c) = body.chars().find(|c| !CODABAR_BODY.contains(*c)) {
                return Err(invalid_char(c));
            }
            if body.is_empty() {
                return Err(PluginError::new("barcode.empty"));
            }
            Ok(plain(if guarded { upper } else { format!("A{body}A") }))
        }
        Format::DataMatrix | Format::Pdf417 | Format::Aztec => Ok(plain(text.to_owned())),
    }
}

#[cfg(test)]
#[path = "formats_test.rs"]
mod tests;
