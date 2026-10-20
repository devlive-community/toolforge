//! 剪贴板识别：头部可解码且包含 alg 的 JWT

use data_encoding::BASE64URL_NOPAD;
use serde_json::Value;
use tf_plugin_api::Detection;

fn base64url(part: &str) -> bool {
    !part.is_empty()
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn detect(text: &str) -> Option<Detection> {
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 3 || !base64url(parts[0]) || !base64url(parts[1]) {
        return None;
    }
    let header = BASE64URL_NOPAD.decode(parts[0].as_bytes()).ok()?;
    let header: Value = serde_json::from_slice(&header).ok()?;
    let alg = header.get("alg")?.as_str()?;
    Some(Detection::new(98, "token").with("alg", alg))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
