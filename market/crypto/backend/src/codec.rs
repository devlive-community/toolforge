//! 文本与字节之间的编码：UTF-8、十六进制与 Base64（兼容 URL 安全字母表与缺省填充）。

use data_encoding::{BASE64, BASE64_NOPAD, BASE64URL, BASE64URL_NOPAD, HEXLOWER_PERMISSIVE};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    Utf8,
    Hex,
    Base64,
}

pub fn decode(text: &str, encoding: Encoding, field: &str) -> PluginResult<Vec<u8>> {
    match encoding {
        Encoding::Utf8 => Ok(text.as_bytes().to_vec()),
        Encoding::Hex => {
            let clean: String = text
                .chars()
                .filter(|c| !c.is_whitespace() && *c != ':')
                .collect();
            let clean = clean.strip_prefix("0x").unwrap_or(&clean);
            HEXLOWER_PERMISSIVE
                .decode(clean.as_bytes())
                .map_err(|_| PluginError::new("crypto.invalid_hex").with("field", field))
        }
        Encoding::Base64 => {
            let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let bytes = clean.as_bytes();
            [BASE64, BASE64_NOPAD, BASE64URL, BASE64URL_NOPAD]
                .iter()
                .find_map(|e| e.decode(bytes).ok())
                .ok_or_else(|| PluginError::new("crypto.invalid_base64").with("field", field))
        }
    }
}

pub fn encode(bytes: &[u8], encoding: Encoding) -> PluginResult<String> {
    match encoding {
        Encoding::Utf8 => {
            String::from_utf8(bytes.to_vec()).map_err(|_| PluginError::new("crypto.not_utf8"))
        }
        Encoding::Hex => Ok(data_encoding::HEXLOWER.encode(bytes)),
        Encoding::Base64 => Ok(BASE64.encode(bytes)),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(bytes)
}

#[cfg(test)]
#[path = "codec_test.rs"]
mod tests;
