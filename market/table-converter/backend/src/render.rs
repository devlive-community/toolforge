//! 把表格输出为各种格式。

use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

use crate::table::{Align, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Markdown,
    Csv,
    Tsv,
    Html,
    Json,
    Ascii,
    Sql,
    Latex,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// Markdown 不补空格对齐
    #[serde(default)]
    pub compact: bool,
    /// SQL INSERT 的表名
    #[serde(default)]
    pub table_name: Option<String>,
}

/// 显示宽度：中日韩字符按两个字符宽计算
fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn pad(text: &str, to: usize, align: Align) -> String {
    let gap = to.saturating_sub(width(text));
    match align {
        Align::Right => format!("{}{text}", " ".repeat(gap)),
        Align::Center => {
            let left = gap / 2;
            format!("{}{text}{}", " ".repeat(left), " ".repeat(gap - left))
        }
        _ => format!("{text}{}", " ".repeat(gap)),
    }
}

/// 所有行（表头在前）
fn all_rows(table: &Table) -> Vec<&Vec<String>> {
    std::iter::once(&table.headers)
        .filter(|h| !h.is_empty())
        .chain(table.rows.iter())
        .collect()
}

fn column_widths(rows: &[Vec<String>], min: usize) -> Vec<usize> {
    let n = rows.iter().map(Vec::len).max().unwrap_or(0);
    (0..n)
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(0, |s| width(s)))
                .max()
                .unwrap_or(0)
                .max(min)
        })
        .collect()
}

fn markdown(table: &Table, compact: bool) -> String {
    let escape = |s: &String| s.replace('|', "\\|").replace('\n', "<br>");
    // Markdown 表格必须有表头；没有时用空表头
    let headers: Vec<String> = if table.headers.is_empty() {
        vec![String::new(); table.columns()]
    } else {
        table.headers.iter().map(escape).collect()
    };
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|r| r.iter().map(escape).collect())
        .collect();
    let mut all = vec![headers.clone()];
    all.extend(rows.iter().cloned());
    let widths = if compact {
        vec![0; headers.len()]
    } else {
        column_widths(&all, 3)
    };
    let align = |c: usize| table.aligns.get(c).copied().unwrap_or_default();
    let line = |cells: &[String]| {
        let parts: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(c, s)| {
                if compact {
                    s.clone()
                } else {
                    pad(s, widths[c], align(c))
                }
            })
            .collect();
        format!("| {} |", parts.join(" | "))
    };
    let separator: Vec<String> = (0..headers.len())
        .map(|c| {
            let w = if compact { 3 } else { widths[c].max(3) };
            match align(c) {
                Align::None => "-".repeat(w),
                Align::Left => format!(":{}", "-".repeat(w - 1)),
                Align::Right => format!("{}:", "-".repeat(w - 1)),
                Align::Center => format!(":{}:", "-".repeat(w.saturating_sub(2).max(1))),
            }
        })
        .collect();
    let mut out = vec![line(&headers), format!("| {} |", separator.join(" | "))];
    out.extend(rows.iter().map(|r| line(r)));
    out.join("\n")
}

fn delimited(table: &Table, delimiter: u8) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::new());
    for row in all_rows(table) {
        let _ = writer.write_record(row);
    }
    String::from_utf8(writer.into_inner().unwrap_or_default())
        .unwrap_or_default()
        .trim_end()
        .to_owned()
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn html(table: &Table) -> String {
    let style = |c: usize| match table.aligns.get(c).copied().unwrap_or_default() {
        Align::None => String::new(),
        Align::Left => " style=\"text-align: left\"".into(),
        Align::Center => " style=\"text-align: center\"".into(),
        Align::Right => " style=\"text-align: right\"".into(),
    };
    let mut out = vec!["<table>".to_owned()];
    if !table.headers.is_empty() {
        out.push("  <thead>\n    <tr>".into());
        for (c, h) in table.headers.iter().enumerate() {
            out.push(format!("      <th{}>{}</th>", style(c), escape_html(h)));
        }
        out.push("    </tr>\n  </thead>".into());
    }
    out.push("  <tbody>".into());
    for row in &table.rows {
        out.push("    <tr>".into());
        for (c, cell) in row.iter().enumerate() {
            out.push(format!("      <td{}>{}</td>", style(c), escape_html(cell)));
        }
        out.push("    </tr>".into());
    }
    out.push("  </tbody>\n</table>".into());
    out.join("\n")
}

fn json(table: &Table) -> String {
    let value = if table.headers.is_empty() {
        serde_json::to_value(&table.rows).unwrap_or_default()
    } else {
        serde_json::Value::Array(
            table
                .rows
                .iter()
                .map(|row| {
                    let mut object = serde_json::Map::new();
                    for (h, cell) in table.headers.iter().zip(row) {
                        object.insert(h.clone(), serde_json::Value::String(cell.clone()));
                    }
                    serde_json::Value::Object(object)
                })
                .collect(),
        )
    };
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// 带边框的纯文本表格
fn ascii(table: &Table) -> String {
    let rows: Vec<Vec<String>> = all_rows(table).into_iter().cloned().collect();
    let widths = column_widths(&rows, 0);
    let rule = |l: &str, m: &str, r: &str| {
        let parts: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        format!("{l}{}{r}", parts.join(m))
    };
    let line = |cells: &[String]| {
        let parts: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(c, w)| {
                let align = table.aligns.get(c).copied().unwrap_or_default();
                format!(
                    " {} ",
                    pad(cells.get(c).map_or("", String::as_str), *w, align)
                )
            })
            .collect();
        format!("│{}│", parts.join("│"))
    };
    let mut out = vec![rule("┌", "┬", "┐")];
    for (i, row) in rows.iter().enumerate() {
        out.push(line(row));
        if i == 0 && !table.headers.is_empty() && rows.len() > 1 {
            out.push(rule("├", "┼", "┤"));
        }
    }
    out.push(rule("└", "┴", "┘"));
    out.join("\n")
}

/// 只有数字和一个小数点（可带负号）才按数字写入；有前导 0 的（如编号 007、电话）按文本处理
fn is_number(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    if digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.") {
        return false;
    }
    digits.parse::<f64>().is_ok()
}

fn sql_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn sql(table: &Table, name: &str) -> String {
    let columns: Vec<String> = if table.headers.is_empty() {
        (1..=table.columns())
            .map(|i| format!("column{i}"))
            .collect()
    } else {
        table.headers.clone()
    };
    let column_list = columns
        .iter()
        .map(|c| sql_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
    let values: Vec<String> = table
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .map(|cell| {
                    if cell.is_empty() {
                        "NULL".to_owned()
                    } else if is_number(cell) {
                        cell.clone()
                    } else {
                        format!("'{}'", cell.replace('\'', "''"))
                    }
                })
                .collect();
            format!("  ({})", cells.join(", "))
        })
        .collect();
    format!(
        "INSERT INTO {} ({column_list}) VALUES\n{};",
        sql_identifier(name),
        values.join(",\n")
    )
}

fn escape_latex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            c => out.push(c),
        }
    }
    out
}

fn latex(table: &Table) -> String {
    let spec: String = (0..table.columns())
        .map(|c| match table.aligns.get(c).copied().unwrap_or_default() {
            Align::Center => 'c',
            Align::Right => 'r',
            _ => 'l',
        })
        .collect();
    let line = |row: &[String]| {
        format!(
            "  {} \\\\",
            row.iter()
                .map(|c| escape_latex(c))
                .collect::<Vec<_>>()
                .join(" & ")
        )
    };
    let mut out = vec![
        format!("\\begin{{tabular}}{{{spec}}}"),
        "  \\hline".to_owned(),
    ];
    if !table.headers.is_empty() {
        out.push(line(&table.headers));
        out.push("  \\hline".into());
    }
    out.extend(table.rows.iter().map(|r| line(r)));
    out.push("  \\hline".into());
    out.push("\\end{tabular}".into());
    out.join("\n")
}

pub fn render(table: &Table, target: Target, options: &Options) -> String {
    match target {
        Target::Markdown => markdown(table, options.compact),
        Target::Csv => delimited(table, b','),
        Target::Tsv => delimited(table, b'\t'),
        Target::Html => html(table),
        Target::Json => json(table),
        Target::Ascii => ascii(table),
        Target::Sql => sql(
            table,
            options
                .table_name
                .as_deref()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or("my_table"),
        ),
        Target::Latex => latex(table),
    }
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
