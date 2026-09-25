//! 剪贴板识别：从电子表格复制的制表符分隔内容，或结构整齐的 CSV

use tf_plugin_api::Detection;

use crate::load::detect_delimiter;

pub fn detect(text: &str) -> Option<Detection> {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(200)
        .collect();
    if lines.len() < 2 {
        return None;
    }
    let delimiter = detect_delimiter(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let counts: Vec<usize> = reader
        .byte_records()
        .take(200)
        .map_while(Result::ok)
        .map(|r| r.len())
        .collect();
    let columns = *counts.first()?;
    if columns < 2 || counts.iter().any(|&c| c != columns) {
        return None;
    }
    let (score, label) = match delimiter {
        b'\t' => (72, "tsv"),
        _ if counts.len() >= 3 && columns >= 3 => (55, "csv"),
        _ => return None,
    };
    Some(
        Detection::new(score, label)
            .with("rows", counts.len())
            .with("columns", columns),
    )
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
