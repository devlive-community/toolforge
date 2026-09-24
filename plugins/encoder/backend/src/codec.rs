use std::time::Instant;

use data_encoding::{
    BASE32, BASE32_NOPAD, BASE64, BASE64_NOPAD, BASE64URL, BASE64URL_NOPAD, Encoding, HEXLOWER,
    HEXLOWER_PERMISSIVE, HEXUPPER,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::unicode;

const MAX_INPUT: usize = 20 * 1024 * 1024;
const MIME_WIDTH: usize = 76;

/// encodeURIComponent：除 A-Z a-z 0-9 - _ . ! ~ * ' ( ) 外全部编码
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// encodeURI：额外保留 URL 保留字符与 %
const FULL_URL: &AsciiSet = &COMPONENT
    .remove(b';')
    .remove(b',')
    .remove(b'/')
    .remove(b'?')
    .remove(b':')
    .remove(b'@')
    .remove(b'&')
    .remove(b'=')
    .remove(b'+')
    .remove(b'$')
    .remove(b'#')
    .remove(b'[')
    .remove(b']')
    .remove(b'%');

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Codec {
    Base64,
    Base64url,
    Base32,
    Hex,
    UrlComponent,
    Url,
    Form,
    Html,
    Unicode,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Encode,
    Decode,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HtmlMode {
    /// 只转义 & < > " '
    #[default]
    Basic,
    /// 另把非 ASCII 字符转为十进制实体
    Decimal,
    /// 另把非 ASCII 字符转为十六进制实体
    Hex,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// Base64 / Base32 是否补 =
    #[serde(default = "yes")]
    pub padding: bool,
    /// Base64 每 76 字符换行（MIME）
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub uppercase: bool,
    #[serde(default)]
    pub html_mode: HtmlMode,
    #[serde(default)]
    pub unicode_style: unicode::Style,
    /// Unicode 转义是否连 ASCII 一起转义
    #[serde(default)]
    pub escape_ascii: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
pub struct Args {
    pub input: String,
    pub codec: Codec,
    pub direction: Direction,
    #[serde(default)]
    pub options: Options,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    /// 解码结果不是合法 UTF-8 时为 false，此时 output 为十六进制
    pub text: bool,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub elapsed_ms: f64,
}

fn invalid(codec: Codec, detail: impl ToString) -> PluginError {
    PluginError::new("encode.invalid_input")
        .with("codec", format!("{codec:?}").to_lowercase())
        .with("detail", detail.to_string())
}

fn wrapped(base: &Encoding, width: usize) -> Encoding {
    let mut spec = base.specification();
    spec.wrap.width = width;
    spec.wrap.separator = "\n".into();
    spec.encoding().expect("valid base64 spec")
}

/// 解码时忽略空白，并兼容缺少补位的输入
fn decode_with(
    padded: &Encoding,
    nopad: &Encoding,
    codec: Codec,
    input: &str,
) -> PluginResult<Vec<u8>> {
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let trimmed = compact.trim_end_matches('=');
    padded
        .decode(compact.as_bytes())
        .or_else(|_| nopad.decode(trimmed.as_bytes()))
        .map_err(|e| invalid(codec, e))
}

fn encode_bytes(codec: Codec, bytes: &[u8], options: &Options) -> String {
    // data-encoding 的内置编码是 const，需要绑定到局部变量再使用
    let encoding: Encoding = match (codec, options.padding) {
        (Codec::Base64, true) => BASE64,
        (Codec::Base64, false) => BASE64_NOPAD,
        (Codec::Base64url, true) => BASE64URL,
        (Codec::Base64url, false) => BASE64URL_NOPAD,
        (Codec::Base32, true) => BASE32,
        (Codec::Base32, false) => BASE32_NOPAD,
        (Codec::Hex, _) if options.uppercase => HEXUPPER,
        (Codec::Hex, _) => HEXLOWER,
        _ => unreachable!("text codecs are handled separately"),
    };
    if codec == Codec::Base64 && options.wrap {
        wrapped(&encoding, MIME_WIDTH).encode(bytes)
    } else {
        encoding.encode(bytes)
    }
}

fn html_encode(input: &str, mode: HtmlMode) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if !c.is_ascii() && mode == HtmlMode::Decimal => {
                out.push_str(&format!("&#{};", c as u32))
            }
            c if !c.is_ascii() && mode == HtmlMode::Hex => {
                out.push_str(&format!("&#x{:X};", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

/// 解码得到的字节：UTF-8 返回文本，否则返回空格分隔的十六进制
fn bytes_output(bytes: Vec<u8>) -> (String, bool) {
    match String::from_utf8(bytes) {
        Ok(text) => (text, true),
        Err(err) => {
            let hex = err
                .as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            (hex, false)
        }
    }
}

pub fn transform(args: Args) -> PluginResult<Output> {
    if args.input.len() > MAX_INPUT {
        return Err(PluginError::new("encode.too_large").with("limit", "20 MB"));
    }
    let start = Instant::now();
    let input = args.input.as_str();
    let options = &args.options;

    let (output, text) = match (args.codec, args.direction) {
        (Codec::Base64 | Codec::Base64url | Codec::Base32 | Codec::Hex, Direction::Encode) => {
            (encode_bytes(args.codec, input.as_bytes(), options), true)
        }
        (Codec::Base64, Direction::Decode) => {
            bytes_output(decode_with(&BASE64, &BASE64_NOPAD, args.codec, input)?)
        }
        (Codec::Base64url, Direction::Decode) => bytes_output(decode_with(
            &BASE64URL,
            &BASE64URL_NOPAD,
            args.codec,
            input,
        )?),
        (Codec::Base32, Direction::Decode) => bytes_output(decode_with(
            &BASE32,
            &BASE32_NOPAD,
            args.codec,
            &input.to_ascii_uppercase(),
        )?),
        (Codec::Hex, Direction::Decode) => {
            let compact: String = input
                .split_whitespace()
                .flat_map(|part| part.split([':', '-', ',']))
                .map(|part| part.trim_start_matches("0x").trim_start_matches("0X"))
                .collect();
            bytes_output(
                HEXLOWER_PERMISSIVE
                    .decode(compact.as_bytes())
                    .map_err(|e| invalid(args.codec, e))?,
            )
        }
        (Codec::UrlComponent, Direction::Encode) => {
            (utf8_percent_encode(input, COMPONENT).to_string(), true)
        }
        (Codec::Url, Direction::Encode) => (utf8_percent_encode(input, FULL_URL).to_string(), true),
        (Codec::Form, Direction::Encode) => (
            form_urlencoded::byte_serialize(input.as_bytes()).collect(),
            true,
        ),
        (Codec::UrlComponent | Codec::Url, Direction::Decode) => {
            bytes_output(percent_decode_str(input).collect())
        }
        (Codec::Form, Direction::Decode) => {
            bytes_output(percent_decode_str(&input.replace('+', " ")).collect())
        }
        (Codec::Html, Direction::Encode) => (html_encode(input, options.html_mode), true),
        (Codec::Html, Direction::Decode) => {
            (html_escape::decode_html_entities(input).into_owned(), true)
        }
        (Codec::Unicode, Direction::Encode) => (
            unicode::escape(input, options.unicode_style, options.escape_ascii),
            true,
        ),
        (Codec::Unicode, Direction::Decode) => (unicode::unescape(input)?, true),
    };

    Ok(Output {
        input_bytes: input.len(),
        output_bytes: output.len(),
        output,
        text,
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

/// 供文件编码复用
pub fn encode_base64(bytes: &[u8], wrap: bool) -> String {
    let options = Options {
        padding: true,
        wrap,
        ..Options::default()
    };
    encode_bytes(Codec::Base64, bytes, &options)
}

#[cfg(test)]
#[path = "codec_test.rs"]
mod tests;
