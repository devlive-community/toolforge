//! 其他编码：Base58（比特币字母表）、Base85（RFC 1924 / git 字母表）、Ascii85（Adobe）与摩斯电码。

use tf_plugin_api::{PluginError, PluginResult};

const BASE58: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE85: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

fn invalid(codec: &str, detail: impl ToString) -> PluginError {
    PluginError::new("encode.invalid_input")
        .with("codec", codec)
        .with("detail", detail.to_string())
}

pub fn base58_encode(bytes: &[u8]) -> String {
    let zeros = bytes.iter().take_while(|b| **b == 0).count();
    // 以 58 为基数的大数，低位在前
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 138 / 100 + 1);
    for &byte in &bytes[zeros..] {
        let mut carry = byte as u32;
        for digit in digits.iter_mut() {
            carry += (*digit as u32) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::with_capacity(zeros + digits.len());
    out.extend(std::iter::repeat_n('1', zeros));
    out.extend(digits.iter().rev().map(|d| BASE58[*d as usize] as char));
    out
}

pub fn base58_decode(input: &str) -> PluginResult<Vec<u8>> {
    let input = input.trim();
    let zeros = input.bytes().take_while(|b| *b == b'1').count();
    let mut bytes: Vec<u8> = Vec::with_capacity(input.len());
    for (index, c) in input.bytes().enumerate().skip(zeros) {
        let value = BASE58
            .iter()
            .position(|b| *b == c)
            .ok_or_else(|| invalid("base58", format!("invalid character at {}", index + 1)))?;
        let mut carry = value as u32;
        for byte in bytes.iter_mut() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let mut out = vec![0u8; zeros];
    out.extend(bytes.iter().rev());
    Ok(out)
}

/// 4 字节一组转为 5 个字符；末组不足 4 字节时补 0 后只输出 n+1 个字符
fn encode85(bytes: &[u8], alphabet: &[u8], out: &mut String, zero_shortcut: bool) {
    for chunk in bytes.chunks(4) {
        let mut group = [0u8; 4];
        group[..chunk.len()].copy_from_slice(chunk);
        let value = u32::from_be_bytes(group);
        if zero_shortcut && value == 0 && chunk.len() == 4 {
            out.push('z');
            continue;
        }
        let mut digits = [0u8; 5];
        let mut v = value;
        for digit in digits.iter_mut().rev() {
            *digit = (v % 85) as u8;
            v /= 85;
        }
        for digit in &digits[..chunk.len() + 1] {
            out.push(alphabet[*digit as usize] as char);
        }
    }
}

fn decode85(
    codec: &str,
    digits: impl Iterator<Item = PluginResult<u8>>,
    zero_shortcut: bool,
) -> PluginResult<Vec<u8>> {
    let mut out = Vec::new();
    let mut group: Vec<u8> = Vec::with_capacity(5);
    let flush = |group: &[u8], out: &mut Vec<u8>| -> PluginResult<()> {
        if group.len() == 1 {
            return Err(invalid(codec, "dangling final character"));
        }
        let mut padded = [84u8; 5];
        padded[..group.len()].copy_from_slice(group);
        let value = padded
            .iter()
            .try_fold(0u32, |acc, d| acc.checked_mul(85)?.checked_add(*d as u32))
            .ok_or_else(|| invalid(codec, "group overflows 32 bits"))?;
        out.extend_from_slice(&value.to_be_bytes()[..group.len() - 1]);
        Ok(())
    };
    for digit in digits {
        let digit = digit?;
        if zero_shortcut && digit == u8::MAX {
            if !group.is_empty() {
                return Err(invalid(codec, "'z' inside a group"));
            }
            out.extend_from_slice(&[0; 4]);
            continue;
        }
        group.push(digit);
        if group.len() == 5 {
            flush(&group, &mut out)?;
            group.clear();
        }
    }
    if !group.is_empty() {
        flush(&group, &mut out)?;
    }
    Ok(out)
}

pub fn base85_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 5 / 4 + 5);
    encode85(bytes, BASE85, &mut out, false);
    out
}

pub fn base85_decode(input: &str) -> PluginResult<Vec<u8>> {
    let digits = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .enumerate()
        .map(|(index, c)| {
            BASE85
                .iter()
                .position(|b| *b as char == c)
                .map(|p| p as u8)
                .ok_or_else(|| {
                    invalid(
                        "base85",
                        format!("invalid character '{c}' at {}", index + 1),
                    )
                })
        });
    decode85("base85", digits, false)
}

pub fn ascii85_encode(bytes: &[u8]) -> String {
    let alphabet: Vec<u8> = (b'!'..=b'u').collect();
    let mut out = String::from("<~");
    encode85(bytes, &alphabet, &mut out, true);
    out.push_str("~>");
    out
}

pub fn ascii85_decode(input: &str) -> PluginResult<Vec<u8>> {
    let trimmed = input.trim();
    let body = trimmed.strip_prefix("<~").unwrap_or(trimmed);
    let body = body.strip_suffix("~>").unwrap_or(body);
    let digits =
        body.chars()
            .filter(|c| !c.is_whitespace())
            .enumerate()
            .map(|(index, c)| match c {
                'z' => Ok(u8::MAX),
                '!'..='u' => Ok(c as u8 - b'!'),
                _ => Err(invalid(
                    "ascii85",
                    format!("invalid character '{c}' at {}", index + 1),
                )),
            });
    decode85("ascii85", digits, true)
}

/// ITU-R M.1677 摩斯电码
const MORSE: &[(char, &str)] = &[
    ('A', ".-"),
    ('B', "-..."),
    ('C', "-.-."),
    ('D', "-.."),
    ('E', "."),
    ('F', "..-."),
    ('G', "--."),
    ('H', "...."),
    ('I', ".."),
    ('J', ".---"),
    ('K', "-.-"),
    ('L', ".-.."),
    ('M', "--"),
    ('N', "-."),
    ('O', "---"),
    ('P', ".--."),
    ('Q', "--.-"),
    ('R', ".-."),
    ('S', "..."),
    ('T', "-"),
    ('U', "..-"),
    ('V', "...-"),
    ('W', ".--"),
    ('X', "-..-"),
    ('Y', "-.--"),
    ('Z', "--.."),
    ('0', "-----"),
    ('1', ".----"),
    ('2', "..---"),
    ('3', "...--"),
    ('4', "....-"),
    ('5', "....."),
    ('6', "-...."),
    ('7', "--..."),
    ('8', "---.."),
    ('9', "----."),
    ('.', ".-.-.-"),
    (',', "--..--"),
    ('?', "..--.."),
    ('\'', ".----."),
    ('!', "-.-.--"),
    ('/', "-..-."),
    ('(', "-.--."),
    (')', "-.--.-"),
    ('&', ".-..."),
    (':', "---..."),
    (';', "-.-.-."),
    ('=', "-...-"),
    ('+', ".-.-."),
    ('-', "-....-"),
    ('_', "..--.-"),
    ('"', ".-..-."),
    ('$', "...-..-"),
    ('@', ".--.-."),
];

fn morse_of(c: char) -> Option<&'static str> {
    let upper = c.to_ascii_uppercase();
    MORSE.iter().find(|(k, _)| *k == upper).map(|(_, v)| *v)
}

/// 编码：字母间空格，单词间 " / "；utf8 为 true 时无对应码的字符先转为 UTF-8 十六进制再编码
pub fn morse_encode(input: &str, utf8: bool) -> PluginResult<String> {
    let mut words = Vec::new();
    for word in input.split_whitespace() {
        let mut letters = Vec::new();
        for c in word.chars() {
            match morse_of(c) {
                Some(code) => letters.push(code.to_owned()),
                None if utf8 => {
                    let mut buf = [0u8; 4];
                    for byte in c.encode_utf8(&mut buf).bytes() {
                        for hex in format!("{byte:02X}").chars() {
                            letters.push(morse_of(hex).expect("hex digit").to_owned());
                        }
                    }
                }
                None => {
                    return Err(
                        PluginError::new("encode.morse_unsupported").with("char", c.to_string())
                    );
                }
            }
        }
        words.push(letters.join(" "));
    }
    Ok(words.join(" / "))
}

/// 解码：接受 . - 以及 · •（点）和 _ −（划）；单词以 / 或 | 或多个空格分隔
pub fn morse_decode(input: &str) -> PluginResult<String> {
    let normalized: String = input
        .chars()
        .map(|c| match c {
            '·' | '•' => '.',
            '_' | '−' | '–' => '-',
            '|' => '/',
            c => c,
        })
        .collect();
    let mut words = Vec::new();
    for word in normalized.split('/').flat_map(|w| w.split("   ")) {
        let mut text = String::new();
        for code in word.split_whitespace() {
            let c = MORSE
                .iter()
                .find(|(_, v)| *v == code)
                .map(|(k, _)| *k)
                .ok_or_else(|| invalid("morse", format!("unknown code {code}")))?;
            text.push(c);
        }
        if !text.is_empty() {
            words.push(text);
        }
    }
    Ok(words.join(" "))
}

#[cfg(test)]
#[path = "extra_test.rs"]
mod tests;
