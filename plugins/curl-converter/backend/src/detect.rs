//! 剪贴板识别：curl 命令

use tf_plugin_api::Detection;

pub fn detect(text: &str) -> Option<Detection> {
    let command = text.strip_prefix("$ ").unwrap_or(text);
    let first = command.split_whitespace().next()?;
    let name = first.rsplit(['/', '\\']).next().unwrap_or(first);
    (name.eq_ignore_ascii_case("curl") || name.eq_ignore_ascii_case("curl.exe"))
        .then(|| Detection::new(99, "command"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
