//! 剪贴板识别：域名（排除常见的文件名）

use tf_plugin_api::Detection;

/// 形如 readme.md 的文件名不是域名
const FILE_EXTENSIONS: &[&str] = &[
    "md", "txt", "json", "html", "htm", "js", "ts", "tsx", "jsx", "css", "png", "jpg", "jpeg",
    "gif", "svg", "csv", "rs", "go", "py", "java", "kt", "zip", "gz", "pdf", "exe", "log", "xml",
    "yaml", "yml", "toml", "sh", "lock", "c", "h", "cpp", "rb", "php", "sql", "doc", "docx", "xls",
    "xlsx", "ppt", "pptx", "mp3", "mp4", "mov", "wav", "tar", "dmg", "app", "ini", "conf", "cfg",
];

pub fn detect(text: &str) -> Option<Detection> {
    let name = text.strip_suffix('.').unwrap_or(text).to_ascii_lowercase();
    if name.len() > 253 || name.contains(char::is_whitespace) {
        return None;
    }
    let labels: Vec<&str> = name.split('.').collect();
    let tld = *labels.last()?;
    let valid = labels.len() >= 2
        && labels.iter().all(|l| {
            (1..=63).contains(&l.len())
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && tld.len() >= 2
        && tld.bytes().all(|b| b.is_ascii_alphabetic())
        && !FILE_EXTENSIONS.contains(&tld);
    valid.then(|| Detection::new(60, "domain"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
