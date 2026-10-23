//! 识别并解析输入：Markdown 表格、HTML 表格、JSON 数组、TSV、CSV（逗号 / 分号 / 竖线）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::table::{Align, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Markdown,
    Html,
    Json,
    Tsv,
    Csv,
}

fn is_separator_row(line: &str) -> bool {
    let cells = split_markdown_row(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim();
            !c.is_empty() && c.trim_matches(':').chars().all(|ch| ch == '-') && c.contains('-')
        })
}

/// 按竖线拆分 Markdown 表格行，支持 \| 转义
fn split_markdown_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = if line.ends_with('|') && !line.ends_with("\\|") {
        &line[..line.len() - 1]
    } else {
        line
    };
    let mut cells = vec![String::new()];
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cells.last_mut().unwrap().push('|');
                chars.next();
            }
            '|' => cells.push(String::new()),
            c => cells.last_mut().unwrap().push(c),
        }
    }
    cells.into_iter().map(|c| c.trim().to_owned()).collect()
}

fn parse_align(cell: &str) -> Align {
    let c = cell.trim();
    match (c.starts_with(':'), c.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        _ => Align::None,
    }
}

fn markdown(text: &str) -> Option<Table> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let sep = lines.iter().position(|l| is_separator_row(l))?;
    if sep == 0 || !lines.iter().all(|l| l.contains('|')) {
        return None;
    }
    let headers = split_markdown_row(lines[sep - 1]);
    let aligns = split_markdown_row(lines[sep])
        .iter()
        .map(|c| parse_align(c))
        .collect();
    let mut rows: Vec<Vec<String>> = lines[..sep - 1]
        .iter()
        .map(|l| split_markdown_row(l))
        .collect();
    rows.extend(lines[sep + 1..].iter().map(|l| split_markdown_row(l)));
    Some(Table {
        headers,
        rows,
        aligns,
    })
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 简单的 HTML 表格读取：按 tr / th / td 切分，单元格内的标签去掉，<br> 变成空格
fn html(text: &str) -> Option<Table> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<table")?;
    let end = lower[start..]
        .find("</table>")
        .map_or(text.len(), |e| start + e);
    let (body, lower_body) = (&text[start..end], &lower[start..end]);
    let mut table = Table::default();
    let mut pos = 0;
    while let Some(tr) = lower_body[pos..].find("<tr") {
        let row_start = pos + tr;
        let row_end = lower_body[row_start..]
            .find("</tr>")
            .map_or(body.len(), |e| row_start + e);
        let (row, lower_row) = (&body[row_start..row_end], &lower_body[row_start..row_end]);
        let mut cells = Vec::new();
        let mut header = true;
        let mut c = 0;
        while let Some(found) = lower_row[c..].find(['<']) {
            let at = c + found;
            let is_th =
                lower_row[at..].starts_with("<th") && !lower_row[at..].starts_with("<thead");
            let is_td = lower_row[at..].starts_with("<td");
            if !(is_th || is_td) {
                c = at + 1;
                continue;
            }
            header &= is_th;
            let open_end = lower_row[at..].find('>').map_or(row.len(), |e| at + e + 1);
            let close_tag = if is_th { "</th>" } else { "</td>" };
            let close = lower_row[open_end..]
                .find(close_tag)
                .map_or(row.len(), |e| open_end + e);
            let inner = &row[open_end..close];
            let mut plain = String::new();
            let mut in_tag = false;
            let mut tag = String::new();
            for ch in inner.chars() {
                match (in_tag, ch) {
                    (false, '<') => {
                        in_tag = true;
                        tag.clear();
                    }
                    (true, '>') => {
                        in_tag = false;
                        if tag
                            .trim_start_matches('/')
                            .to_ascii_lowercase()
                            .starts_with("br")
                        {
                            plain.push(' ');
                        }
                    }
                    (true, ch) => tag.push(ch),
                    (false, ch) => plain.push(ch),
                }
            }
            let collapsed = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            cells.push(decode_entities(&collapsed));
            c = close;
        }
        if !cells.is_empty() {
            if header && table.headers.is_empty() && table.rows.is_empty() {
                table.headers = cells;
            } else {
                table.rows.push(cells);
            }
        }
        pos = row_end.max(row_start + 3);
    }
    (!table.headers.is_empty() || !table.rows.is_empty()).then_some(table)
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// JSON：对象数组（键作为表头）或二维数组
fn json(text: &str) -> Option<PluginResult<Table>> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return Some(Err(PluginError::new("table.invalid_json")
                .with("line", e.line())
                .with("column", e.column())));
        }
    };
    let Value::Array(items) = value else {
        return None;
    };
    if items.iter().all(Value::is_object) && !items.is_empty() {
        let mut headers: Vec<String> = Vec::new();
        for item in &items {
            for key in item.as_object().unwrap().keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
        let rows = items
            .iter()
            .map(|item| {
                headers
                    .iter()
                    .map(|h| item.get(h).map(cell_text).unwrap_or_default())
                    .collect()
            })
            .collect();
        return Some(Ok(Table {
            headers,
            rows,
            aligns: Vec::new(),
        }));
    }
    if items.iter().all(Value::is_array) {
        let rows = items
            .iter()
            .map(|row| row.as_array().unwrap().iter().map(cell_text).collect())
            .collect();
        return Some(Ok(Table {
            headers: Vec::new(),
            rows,
            aligns: Vec::new(),
        }));
    }
    Some(Err(PluginError::new("table.unsupported_json")))
}

fn delimited(text: &str, delimiter: u8) -> PluginResult<Table> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record
            .map_err(|e| PluginError::new("table.invalid_csv").with("detail", e.to_string()))?;
        rows.push(record.iter().map(str::to_owned).collect());
    }
    Ok(Table {
        headers: Vec::new(),
        rows,
        aligns: Vec::new(),
    })
}

/// 猜测分隔符：在前几行中出现次数最稳定且最多的那个
fn guess_delimiter(text: &str) -> u8 {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(20)
        .collect();
    b",;|"
        .iter()
        .copied()
        .map(|d| {
            let counts: Vec<usize> = lines
                .iter()
                .map(|l| l.bytes().filter(|b| *b == d).count())
                .collect();
            let min = counts.iter().copied().min().unwrap_or(0);
            (d, min)
        })
        .max_by_key(|(_, min)| *min)
        .filter(|(_, min)| *min > 0)
        .map_or(b',', |(d, _)| d)
}

pub fn detect(text: &str) -> Source {
    let trimmed = text.trim_start();
    if trimmed.starts_with('[') {
        return Source::Json;
    }
    if text.to_ascii_lowercase().contains("<table") {
        return Source::Html;
    }
    if markdown(text).is_some() {
        return Source::Markdown;
    }
    if text.lines().take(20).any(|l| l.contains('\t')) {
        return Source::Tsv;
    }
    Source::Csv
}

pub fn parse(text: &str, source: Source) -> PluginResult<Table> {
    if text.trim().is_empty() {
        return Err(PluginError::new("table.empty"));
    }
    let not_found = || {
        PluginError::new("table.not_found").with("format", serde_json::to_value(source).unwrap())
    };
    match source {
        Source::Markdown => markdown(text).ok_or_else(not_found),
        Source::Html => html(text).ok_or_else(not_found),
        Source::Json => json(text).unwrap_or_else(|| Err(not_found())),
        Source::Tsv => delimited(text, b'\t'),
        Source::Csv => delimited(text, guess_delimiter(text)),
    }
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
