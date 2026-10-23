//! 剪贴板识别：IP 地址与 CIDR 网段

use tf_plugin_api::Detection;

use crate::ip::parse_net;

pub fn detect(text: &str) -> Option<Detection> {
    let plausible = text.len() <= 64
        && (text.contains('.') || text.contains(':'))
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b".:/".contains(&b));
    if !plausible || parse_net(text).is_err() {
        return None;
    }
    let label = if text.contains('/') { "cidr" } else { "ip" };
    Some(Detection::new(90, label).with("input", text))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
