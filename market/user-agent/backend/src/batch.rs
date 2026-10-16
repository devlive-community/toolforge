//! 批量解析：多行 UA、CSV 指定列或访问日志（combined 格式）中提取的 UA。
//! 相同的 UA 合并为一行并计数，同时统计浏览器、系统、设备类型与爬虫分布。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{PluginError, PluginResult};

use crate::parse::{self, Parsed};
use crate::rules::RULES_VERSION;

pub const MAX_INPUT: usize = 20 * 1024 * 1024;
pub const MAX_LINES: usize = 200_000;
/// 结果中最多返回的不同 UA 数（统计仍覆盖全部）
pub const MAX_ROWS: usize = 5_000;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Lines,
    Csv,
    Log,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub input: String,
    #[serde(default)]
    pub mode: Mode,
    /// CSV 中的列：列名或 1 起始的序号；为空时自动选名为 user-agent 的列，否则第 1 列
    #[serde(default)]
    pub column: String,
    #[serde(default = "comma")]
    pub delimiter: char,
}

fn comma() -> char {
    ','
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub ua: String,
    pub count: usize,
    /// 第一次出现的行号
    pub line: usize,
    #[serde(flatten)]
    pub parsed: Parsed,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub total: usize,
    pub unique: usize,
    pub empty: usize,
    pub bots: usize,
    pub browsers: Vec<Bucket>,
    pub systems: Vec<Bucket>,
    pub devices: Vec<Bucket>,
    pub bot_names: Vec<Bucket>,
    pub bot_categories: Vec<Bucket>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub rules_version: &'static str,
    pub rows: Vec<Row>,
    /// 不同 UA 超过 MAX_ROWS 时只返回出现次数最多的部分
    pub truncated: bool,
    pub stats: Stats,
    pub columns: Vec<String>,
    /// 日志模式下无法提取 UA 的行数
    pub skipped: usize,
}

/// (行号, UA)
type Items = Vec<(usize, String)>;

fn from_lines(input: &str) -> Items {
    input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim().to_owned()))
        .collect()
}

/// 行中的双引号字段（支持 \" 转义）
fn quoted_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut field = String::new();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        field.push(next);
                    }
                }
                '"' => {
                    closed = true;
                    break;
                }
                other => field.push(other),
            }
        }
        if !closed {
            break;
        }
        fields.push(field);
    }
    fields
}

/// combined 日志格式中 UA 是第 3 个引号字段（请求行、Referer、UA），其后可能还有自定义字段
fn from_log(input: &str) -> (Items, usize) {
    let mut items = Vec::new();
    let mut skipped = 0;
    for (i, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match quoted_fields(line).into_iter().nth(2) {
            Some(ua) => items.push((i + 1, ua)),
            None => skipped += 1,
        }
    }
    (items, skipped)
}

fn from_csv(args: &Args) -> PluginResult<(Items, Vec<String>)> {
    if !args.delimiter.is_ascii() {
        return Err(PluginError::new("ua.invalid_delimiter"));
    }
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(args.delimiter as u8)
        .flexible(true)
        .from_reader(args.input.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| PluginError::new("ua.csv_invalid").with("detail", e.to_string()))?
        .iter()
        .map(str::to_owned)
        .collect();
    let wanted = args.column.trim();
    let index = if wanted.is_empty() {
        headers
            .iter()
            .position(|h| {
                let h = h.trim().to_ascii_lowercase().replace(['-', '_', ' '], "");
                h == "useragent" || h == "ua"
            })
            .unwrap_or(0)
    } else if let Some(i) = headers.iter().position(|h| h.trim() == wanted) {
        i
    } else {
        match wanted.parse::<usize>() {
            Ok(n) if (1..=headers.len()).contains(&n) => n - 1,
            _ => return Err(PluginError::new("ua.column_not_found").with("column", wanted)),
        }
    };
    let mut items = Vec::new();
    for (i, record) in reader.records().enumerate() {
        let record =
            record.map_err(|e| PluginError::new("ua.csv_invalid").with("detail", e.to_string()))?;
        items.push((
            i + 2,
            record.get(index).unwrap_or_default().trim().to_owned(),
        ));
    }
    Ok((items, headers))
}

fn buckets(counts: HashMap<String, usize>) -> Vec<Bucket> {
    let mut out: Vec<Bucket> = counts
        .into_iter()
        .map(|(name, count)| Bucket { name, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    out
}

pub fn run(args: &Args) -> PluginResult<Output> {
    analyze(args, MAX_ROWS)
}

/// 解析并统计；不同 UA 超过 limit 时只保留出现次数最多的 limit 条
fn analyze(args: &Args, limit: usize) -> PluginResult<Output> {
    if args.input.len() > MAX_INPUT {
        return Err(PluginError::new("ua.too_large").with("mb", MAX_INPUT / 1024 / 1024));
    }
    let (items, columns, skipped) = match args.mode {
        Mode::Lines => (from_lines(&args.input), Vec::new(), 0),
        Mode::Csv => {
            let (items, columns) = from_csv(args)?;
            (items, columns, 0)
        }
        Mode::Log => {
            let (items, skipped) = from_log(&args.input);
            (items, Vec::new(), skipped)
        }
    };
    if items.len() > MAX_LINES {
        return Err(PluginError::new("ua.too_many").with("max", MAX_LINES));
    }

    // 去重计数，保留首次出现顺序
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut unique: Vec<(String, usize, usize)> = Vec::new();
    let mut stats = Stats::default();
    for (line, ua) in items {
        if ua.is_empty() || ua == "-" {
            stats.empty += 1;
            continue;
        }
        stats.total += 1;
        match index.get(&ua) {
            Some(&i) => unique[i].2 += 1,
            None => {
                index.insert(ua.clone(), unique.len());
                unique.push((ua, line, 1));
            }
        }
    }
    stats.unique = unique.len();

    let mut browsers = HashMap::new();
    let mut systems = HashMap::new();
    let mut devices = HashMap::new();
    let mut bot_names = HashMap::new();
    let mut bot_categories = HashMap::new();
    let mut rows: Vec<Row> = unique
        .into_iter()
        .map(|(ua, line, count)| {
            let parsed = parse::parse(&ua);
            *devices.entry(parsed.device.kind.to_owned()).or_default() += count;
            if let Some(bot) = &parsed.bot {
                stats.bots += count;
                *bot_names.entry(bot.name.clone()).or_default() += count;
                *bot_categories.entry(bot.category.to_owned()).or_default() += count;
            } else {
                if let Some(browser) = &parsed.browser {
                    *browsers.entry(browser.name.clone()).or_default() += count;
                }
                if let Some(os) = &parsed.os {
                    *systems.entry(os.name.clone()).or_default() += count;
                }
            }
            Row {
                ua,
                count,
                line,
                parsed,
            }
        })
        .collect();
    stats.browsers = buckets(browsers);
    stats.systems = buckets(systems);
    stats.devices = buckets(devices);
    stats.bot_names = buckets(bot_names);
    stats.bot_categories = buckets(bot_categories);

    let truncated = rows.len() > limit;
    if truncated {
        rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.line.cmp(&b.line)));
        rows.truncate(limit);
    }
    Ok(Output {
        rules_version: RULES_VERSION,
        rows,
        truncated,
        stats,
        columns,
        skipped,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    #[serde(flatten)]
    pub args: Args,
    pub format: ExportFormat,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// 导出全部不同 UA（不受 MAX_ROWS 限制）
pub fn export(args: ExportArgs) -> PluginResult<String> {
    let output = analyze(&args.args, usize::MAX)?;
    Ok(match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&json!({
            "rulesVersion": output.rules_version,
            "stats": output.stats,
            "rows": output.rows,
        }))
        .map_err(|e| PluginError::new("ua.export_failed").with("detail", e.to_string()))?,
        ExportFormat::Csv => {
            let mut text = String::from(
                "count,user_agent,browser,browser_version,engine,engine_version,os,os_version,device_type,vendor,model,bot,bot_category,rules\n",
            );
            for row in &output.rows {
                let p = &row.parsed;
                let name = |part: &Option<parse::Part>| {
                    part.as_ref().map(|p| p.name.clone()).unwrap_or_default()
                };
                let version = |part: &Option<parse::Part>| {
                    part.as_ref()
                        .and_then(|p| p.version.clone())
                        .unwrap_or_default()
                };
                let rules: Vec<&str> = [
                    p.bot.as_ref().map(|b| b.hit.rule),
                    p.browser.as_ref().map(|b| b.hit.rule),
                    p.engine.as_ref().map(|b| b.hit.rule),
                    p.os.as_ref().map(|b| b.hit.rule),
                    p.device.hit.as_ref().map(|b| b.rule),
                ]
                .into_iter()
                .flatten()
                .collect();
                let fields = [
                    row.count.to_string(),
                    row.ua.clone(),
                    name(&p.browser),
                    version(&p.browser),
                    name(&p.engine),
                    version(&p.engine),
                    name(&p.os),
                    version(&p.os),
                    p.device.kind.to_owned(),
                    p.device.vendor.clone().unwrap_or_default(),
                    p.device.model.clone().unwrap_or_default(),
                    p.bot.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
                    p.bot
                        .as_ref()
                        .map(|b| b.category.to_owned())
                        .unwrap_or_default(),
                    rules.join(" "),
                ];
                let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
                text.push_str(&line.join(","));
                text.push('\n');
            }
            text
        }
    })
}

#[cfg(test)]
#[path = "batch_test.rs"]
mod tests;
