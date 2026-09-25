use croner::Cron;
use croner::errors::CronError;
use croner::parser::{CronParser, Seconds};
use jiff::Zoned;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

const MAX_RUNS: usize = 50;

/// 常见别名展开为五段式
const ALIASES: &[(&str, &str)] = &[
    ("@yearly", "0 0 1 1 *"),
    ("@annually", "0 0 1 1 *"),
    ("@monthly", "0 0 1 * *"),
    ("@weekly", "0 0 * * 0"),
    ("@daily", "0 0 * * *"),
    ("@midnight", "0 0 * * *"),
    ("@hourly", "0 * * * *"),
];

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

fn default_count() -> usize {
    10
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    expression: String,
    /// IANA 时区；为空时使用系统时区
    #[serde(default)]
    timezone: String,
    #[serde(default = "default_count")]
    count: usize,
    /// 同时满足「日」与「星期」（默认按 POSIX：任一满足即可）
    #[serde(default)]
    dom_and_dow: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    Second,
    Minute,
    Hour,
    DayOfMonth,
    Month,
    DayOfWeek,
    Year,
}

/// 字段中逗号分隔的一段，前端据此组合出可翻译的说明
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Part {
    /// `*` 或 `?`
    Any,
    /// 单个值
    Value { value: u32 },
    /// `a-b`
    Range { from: u32, to: u32 },
    /// `*/n`、`a/n`、`a-b/n`
    Step {
        from: Option<u32>,
        to: Option<u32>,
        step: u32,
    },
    /// 日：`L`
    LastDay,
    /// 日：`LW`
    LastWeekday,
    /// 日：`15W`
    NearestWeekday { day: u32 },
    /// 星期：`5L`
    LastOfMonth { weekday: u32 },
    /// 星期：`1#2`
    Nth { weekday: u32, nth: u32 },
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    pub field: Field,
    pub raw: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// 所选时区下的本地时间
    pub local: String,
    pub iso: String,
    /// 0 = 周日
    pub weekday: u8,
    /// 距现在的秒数（负数表示过去）
    pub in_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// 展开别名后的表达式
    pub normalized: String,
    pub timezone: String,
    pub has_seconds: bool,
    pub fields: Vec<FieldInfo>,
    pub next: Vec<Run>,
    pub previous: Option<Run>,
}

fn invalid(detail: impl Into<String>) -> PluginError {
    PluginError::new("cron.invalid").with("detail", detail.into())
}

/// 替换月份与星期名称（大小写不敏感）为数字
fn named(token: &str, field: Field) -> String {
    let upper = token.to_ascii_uppercase();
    let names: &[&str] = match field {
        Field::Month => &MONTHS,
        Field::DayOfWeek => &WEEKDAYS,
        _ => return upper,
    };
    let offset = if field == Field::Month { 1 } else { 0 };
    let mut out = upper;
    for (i, name) in names.iter().enumerate() {
        out = out.replace(name, &(i + offset).to_string());
    }
    out
}

fn number(text: &str, raw: &str) -> PluginResult<u32> {
    text.parse::<u32>()
        .map_err(|_| invalid(format!("unexpected “{text}” in “{raw}”")))
}

fn parse_part(token: &str, field: Field, raw: &str) -> PluginResult<Part> {
    let token = named(token, field);
    let token = token.as_str();
    match (field, token) {
        (_, "*" | "?") => return Ok(Part::Any),
        (Field::DayOfMonth, "L") => return Ok(Part::LastDay),
        (Field::DayOfMonth, "LW") => return Ok(Part::LastWeekday),
        _ => {}
    }
    if field == Field::DayOfMonth
        && let Some(day) = token.strip_suffix('W')
    {
        return Ok(Part::NearestWeekday {
            day: number(day, raw)?,
        });
    }
    if field == Field::DayOfWeek {
        if let Some(day) = token.strip_suffix('L') {
            return Ok(Part::LastOfMonth {
                weekday: number(day, raw)? % 7,
            });
        }
        if let Some((day, nth)) = token.split_once('#') {
            let weekday = number(day, raw)? % 7;
            if nth == "L" {
                return Ok(Part::LastOfMonth { weekday });
            }
            return Ok(Part::Nth {
                weekday,
                nth: number(nth, raw)?,
            });
        }
    }
    let (range, step) = match token.split_once('/') {
        Some((range, step)) => (range, Some(number(step, raw)?)),
        None => (token, None),
    };
    let (from, to) = match range {
        "*" | "" => (None, None),
        _ => match range.split_once('-') {
            Some((a, b)) => (Some(number(a, raw)?), Some(number(b, raw)?)),
            None => (Some(number(range, raw)?), None),
        },
    };
    Ok(match (from, to, step) {
        (_, _, Some(step)) => Part::Step { from, to, step },
        (Some(from), Some(to), None) => Part::Range { from, to },
        (Some(value), None, None) => Part::Value { value },
        _ => Part::Any,
    })
}

/// 解析各字段用于展示；有效性以 croner 为准
pub fn explain(expression: &str) -> PluginResult<(String, Vec<FieldInfo>)> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return Err(PluginError::new("cron.empty"));
    }
    let normalized = ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(trimmed))
        .map(|(_, expr)| expr.to_string())
        .unwrap_or_else(|| trimmed.split_whitespace().collect::<Vec<_>>().join(" "));
    let tokens: Vec<&str> = normalized.split(' ').collect();
    let layout: &[Field] = match tokens.len() {
        5 => &[
            Field::Minute,
            Field::Hour,
            Field::DayOfMonth,
            Field::Month,
            Field::DayOfWeek,
        ],
        6 => &[
            Field::Second,
            Field::Minute,
            Field::Hour,
            Field::DayOfMonth,
            Field::Month,
            Field::DayOfWeek,
        ],
        7 => &[
            Field::Second,
            Field::Minute,
            Field::Hour,
            Field::DayOfMonth,
            Field::Month,
            Field::DayOfWeek,
            Field::Year,
        ],
        count => {
            return Err(PluginError::new("cron.field_count").with("count", count));
        }
    };
    let fields = tokens
        .iter()
        .zip(layout)
        .map(|(raw, field)| {
            let parts = raw
                .split(',')
                .map(|part| parse_part(part, *field, raw))
                .collect::<PluginResult<Vec<_>>>()?;
            Ok(FieldInfo {
                field: *field,
                raw: raw.to_string(),
                parts,
            })
        })
        .collect::<PluginResult<Vec<_>>>()?;
    Ok((normalized, fields))
}

fn map_error(err: CronError) -> PluginError {
    match err {
        CronError::EmptyPattern => PluginError::new("cron.empty"),
        CronError::TimeSearchLimitExceeded => PluginError::new("cron.never"),
        other => invalid(other.to_string()),
    }
}

fn timezone(name: &str) -> PluginResult<TimeZone> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(TimeZone::system());
    }
    TimeZone::get(name)
        .map_err(|_| PluginError::new("cron.unknown_timezone").with("timezone", name))
}

fn run(time: &Zoned, now: &Zoned) -> Run {
    Run {
        local: time.strftime("%Y-%m-%d %H:%M:%S").to_string(),
        iso: time.timestamp().to_string(),
        weekday: time.weekday().to_sunday_zero_offset() as u8,
        in_seconds: time.timestamp().as_second() - now.timestamp().as_second(),
    }
}

fn parser(dom_and_dow: bool) -> CronParser {
    CronParser::builder()
        .seconds(Seconds::Optional)
        .dom_and_dow(dom_and_dow)
        // 兼容 Quartz / Spring 常见的 `0/15` 写法
        .sloppy_ranges(true)
        .build()
}

/// 结构与取值范围都合法
pub fn is_valid(expression: &str) -> bool {
    explain(expression).is_ok_and(|(normalized, _)| parser(false).parse(&normalized).is_ok())
}

pub fn evaluate(args: Args) -> PluginResult<Report> {
    let (normalized, fields) = explain(&args.expression)?;
    let cron: Cron = parser(args.dom_and_dow)
        .parse(&normalized)
        .map_err(map_error)?;
    let tz = timezone(&args.timezone)?;
    let now = Zoned::now().with_time_zone(tz.clone());

    let mut next = Vec::new();
    for time in cron
        .iter_after(now.clone())
        .take(args.count.clamp(1, MAX_RUNS))
    {
        next.push(run(&time, &now));
    }
    if next.is_empty() {
        return Err(PluginError::new("cron.never"));
    }
    let previous = cron.iter_before(now.clone()).next().map(|t| run(&t, &now));

    Ok(Report {
        normalized,
        timezone: tz.iana_name().unwrap_or("UTC").to_owned(),
        has_seconds: fields.first().is_some_and(|f| f.field == Field::Second),
        fields,
        next,
        previous,
    })
}

/// 可选的 IANA 时区列表
pub fn timezones() -> Vec<String> {
    let mut names: Vec<String> = jiff::tz::db()
        .available()
        .map(|name| name.to_string())
        .filter(|name| name.contains('/') || name == "UTC")
        .collect();
    names.sort();
    names
}

#[cfg(test)]
#[path = "cron_test.rs"]
mod tests;
