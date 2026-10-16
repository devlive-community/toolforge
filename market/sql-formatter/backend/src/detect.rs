//! 剪贴板识别：以常见 SQL 关键字开头的语句

use tf_plugin_api::Detection;

const STARTS: &[&str] = &[
    "select", "insert", "update", "delete", "create", "alter", "drop", "with", "merge", "truncate",
    "replace",
];
const FOLLOWS: &[&str] = &[
    "from", "into", "set", "table", "values", "where", "index", "view", "as", "join", "database",
];

pub fn detect(text: &str) -> Option<Detection> {
    let mut words = text
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase);
    let first = words.next()?;
    if !STARTS.contains(&first.as_str()) {
        return None;
    }
    words
        .take(200)
        .any(|w| FOLLOWS.contains(&w.as_str()))
        .then(|| Detection::new(85, "statement"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
