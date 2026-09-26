//! 剪贴板识别：host:port 形式的地址（如 localhost:3000、example.com:443、10.0.0.5:22）

use tf_plugin_api::Detection;

pub fn detect(text: &str) -> Option<Detection> {
    if text.contains(char::is_whitespace) || text.contains("://") {
        return None;
    }
    let (host, port) = match text.strip_prefix('[') {
        // [::1]:8080
        Some(rest) => {
            let (host, port) = rest.split_once("]:")?;
            (host, port)
        }
        None => text.rsplit_once(':')?,
    };
    let port: u16 = port.parse().ok().filter(|p| *p > 0)?;
    // 纯数字的「主机」更可能是时间（如 12:30）
    let valid_host = !host.is_empty()
        && !host.bytes().all(|b| b.is_ascii_digit())
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b));
    valid_host.then(|| {
        Detection::new(60, "endpoint")
            .with("host", host)
            .with("port", port)
    })
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
