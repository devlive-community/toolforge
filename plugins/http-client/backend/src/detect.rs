//! 剪贴板识别：单个 http(s) 地址

use tf_plugin_api::Detection;

pub fn detect(text: &str) -> Option<Detection> {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    if host.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    Some(Detection::new(70, "url").with("host", host))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
