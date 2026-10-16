//! 自然语言 / 相对日期解析（中文与英文），全部相对「现在」在所选时区计算。
//!
//! 结构：日期部分 + 时刻部分，两者都可省略其一（英文也接受时刻在前）。
//! - 日期：今天 / 明天 / 后天 / 大后天 / 昨天 / 前天、N 天（周、个月、年）后 / 前、下周 / 下个月 / 明年，
//!   以及 [上 / 下 / 这] 周 X；英文 today / tomorrow / yesterday、in N days、N days ago、next / last / this Monday、
//!   next week / month / year
//! - 时刻：[上午 / 下午 / 晚上…] N 点 [半 / 一刻 / M 分]、HH:MM[:SS]；英文 9am、9:30 pm、noon、midnight
//! - 秒、分钟、小时级的偏移（3 小时后、in 2 hours）是精确时刻，不能再跟时刻
//!
//! 没有给出时刻时沿用当前时刻。有歧义时返回多个候选：
//! 「下周三」既可能是下一个自然周的周三，也可能是接下来最近的周三；不带上午 / 下午的「3 点」可能是 3:00 或 15:00。

use std::sync::OnceLock;

use jiff::civil::{Date, Time};
use jiff::{Span, Zoned};
use regex::{Captures, Regex};

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub at: Zoned,
    /// 解释说明的代码，由界面翻译
    pub notes: Vec<&'static str>,
}

/// 日期部分的一种解释
struct DatePart {
    at: Zoned,
    notes: Vec<&'static str>,
    /// 已经是精确时刻（秒 / 分 / 小时偏移），不再接受时刻
    exact: bool,
}

struct TimePart {
    time: Time,
    note: Option<&'static str>,
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("built-in pattern")
}

macro_rules! pattern {
    ($name:ident, $re:expr) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| re($re))
        }
    };
}

const CN_NUM: &str = r"\d+|[零〇一二两三四五六七八九十百]+";

pattern!(
    cn_offset,
    &format!(
        r"^({CN_NUM}|半)\s*个?\s*(秒钟?|分钟|分|小时|钟头|天|日|周|星期|礼拜|月|年)\s*(之后|以后|后|之前|以前|前)"
    )
);
pattern!(
    cn_day,
    r"^(大后天|后天|明天|明日|今天|今日|昨天|昨日|大前天|前天)"
);
pattern!(
    cn_weekday,
    r"^(上上|下下|上|下|这|本)?\s*个?\s*(周|星期|礼拜)\s*([一二三四五六日天1-7])"
);
pattern!(
    cn_period,
    r"^(上上|下下|上|下|这|本)\s*个?\s*(周|星期|礼拜|月)"
);
pattern!(cn_year, r"^(明年|去年|今年|后年|前年)");
pattern!(
    cn_time,
    &format!(
        r"^(凌晨|清晨|早上|早晨|上午|中午|正午|下午|傍晚|晚上|夜里|夜间)?\s*(?:({CN_NUM})\s*(?:点|时|:|：)\s*(?:(半|一刻|三刻)|({CN_NUM})\s*分?)?\s*(?:({CN_NUM})\s*秒)?|(中午|正午|午夜|半夜))$"
    )
);
pattern!(
    clock,
    r"^(?:at\s+)?(\d{1,2})[:：](\d{2})(?:[:：](\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?$"
);
pattern!(
    en_time,
    r"^(?:at\s+)?(?:(\d{1,2})\s*(am|pm|a\.m\.|p\.m\.|o'clock)?|(noon|midnight))$"
);
pattern!(
    en_offset,
    r"^(?:in\s+(\d+|an?|one|two|three|half an?)\s+(second|minute|hour|day|week|month|year)s?|(\d+|an?|one|two|three)\s+(second|minute|hour|day|week|month|year)s?\s+(ago|later|from now|before|after))"
);
pattern!(
    en_day,
    r"^(the day after tomorrow|the day before yesterday|day after tomorrow|day before yesterday|tomorrow|today|tonight|yesterday|now)\b"
);
pattern!(
    en_weekday,
    r"^(?:(next|last|this|coming|previous)\s+)?(monday|mon|tuesday|tues|tue|wednesday|wed|thursday|thurs|thur|thu|friday|fri|saturday|sat|sunday|sun)\b"
);
pattern!(en_period, r"^(next|last|this)\s+(week|month|year)\b");

/// 中文数字（不超过 999）或阿拉伯数字
fn number(text: &str) -> Option<i64> {
    let text = text.trim();
    if let Ok(n) = text.parse::<i64>() {
        return Some(n);
    }
    let digit = |c: char| match c {
        '零' | '〇' => Some(0),
        '一' => Some(1),
        '二' | '两' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        _ => None,
    };
    let mut total = 0i64;
    let mut current = 0i64;
    let mut seen = false;
    for c in text.chars() {
        match c {
            '十' => {
                total += if current == 0 { 10 } else { current * 10 };
                current = 0;
            }
            '百' => {
                total += if current == 0 { 100 } else { current * 100 };
                current = 0;
            }
            c => current = current * 10 + digit(c)?,
        }
        seen = true;
    }
    seen.then_some(total + current)
}

fn english_number(text: &str) -> Option<i64> {
    match text {
        "a" | "an" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        n => n.parse().ok(),
    }
}

fn cn_weekday_index(c: &str) -> i8 {
    match c {
        "一" | "1" => 1,
        "二" | "2" => 2,
        "三" | "3" => 3,
        "四" | "4" => 4,
        "五" | "5" => 5,
        "六" | "6" => 6,
        _ => 7,
    }
}

fn en_weekday_index(name: &str) -> i8 {
    match &name[..3] {
        "mon" => 1,
        "tue" => 2,
        "wed" => 3,
        "thu" => 4,
        "fri" => 5,
        "sat" => 6,
        _ => 7,
    }
}

fn shift_days(now: &Zoned, days: i64) -> Option<Zoned> {
    now.checked_add(Span::new().try_days(days).ok()?).ok()
}

fn on_date(now: &Zoned, date: Date) -> Option<Zoned> {
    date.to_datetime(now.time())
        .to_zoned(now.time_zone().clone())
        .ok()
}

fn part(at: Option<Zoned>, notes: Vec<&'static str>) -> Vec<DatePart> {
    at.map(|at| DatePart {
        at,
        notes,
        exact: false,
    })
    .into_iter()
    .collect()
}

/// 星期的几种解释：relation 为 -2 / -1 / 0 / 1 / 2（上上 / 上 / 本 / 下 / 下下），None 表示只说了星期几
fn weekday_parts(now: &Zoned, relation: Option<i64>, target: i8) -> Vec<DatePart> {
    let today = now.date();
    let current = today.weekday().to_monday_one_offset() as i64;
    let target = target as i64;
    let week_start = today
        .checked_sub(Span::new().days(current - 1))
        .unwrap_or(today);
    let calendar = |weeks: i64| {
        week_start
            .checked_add(Span::new().days(weeks * 7 + target - 1))
            .ok()
    };
    let ahead = (target - current).rem_euclid(7);
    let behind = (current - target).rem_euclid(7);
    let upcoming_inclusive = today.checked_add(Span::new().days(ahead)).ok();
    let upcoming = today
        .checked_add(Span::new().days(if ahead == 0 { 7 } else { ahead }))
        .ok();
    let previous = today
        .checked_sub(Span::new().days(if behind == 0 { 7 } else { behind }))
        .ok();
    let options: Vec<(Option<Date>, &'static str)> = match relation {
        None | Some(0) => vec![(calendar(0), "thisWeek"), (upcoming_inclusive, "upcoming")],
        Some(1) => vec![(calendar(1), "nextWeek"), (upcoming, "upcoming")],
        Some(-1) => vec![(calendar(-1), "lastWeek"), (previous, "previous")],
        Some(n) => vec![(
            calendar(n),
            if n > 0 {
                "weekAfterNext"
            } else {
                "weekBeforeLast"
            },
        )],
    };
    let mut out: Vec<DatePart> = Vec::new();
    for (date, note) in options {
        let Some(date) = date else { continue };
        if out.iter().any(|p| p.at.date() == date) {
            continue;
        }
        out.extend(part(on_date(now, date), vec![note]));
    }
    // 只有一种解释时不需要说明
    if out.len() == 1 && relation.is_none_or(|r| r == 0) {
        out[0].notes.clear();
    }
    out
}

/// 偏移：unit 为 秒 / 分 / 时 / 天 / 周 / 月 / 年；half 表示「半」
fn offset(now: &Zoned, amount: i64, unit: &str, half: bool, sign: i64) -> Vec<DatePart> {
    let amount = amount * sign;
    let (span, exact) = match unit {
        "second" => (Span::new().try_seconds(amount).ok(), true),
        "minute" => (
            Span::new()
                .try_minutes(if half { 0 } else { amount })
                .ok()
                .map(|s| if half { s.seconds(30 * sign) } else { s }),
            true,
        ),
        "hour" => (
            if half {
                Span::new().try_minutes(30 * sign).ok()
            } else {
                Span::new().try_hours(amount).ok()
            },
            true,
        ),
        "day" => (Span::new().try_days(amount).ok(), false),
        "week" => (Span::new().try_weeks(amount).ok(), false),
        "month" => (Span::new().try_months(amount).ok(), false),
        _ => (Span::new().try_years(amount).ok(), false),
    };
    span.and_then(|span| now.checked_add(span).ok())
        .map(|at| DatePart {
            at,
            notes: Vec::new(),
            exact,
        })
        .into_iter()
        .collect()
}

fn cn_unit(unit: &str) -> &'static str {
    match unit {
        u if u.starts_with('秒') => "second",
        "分钟" | "分" => "minute",
        "小时" | "钟头" => "hour",
        "天" | "日" => "day",
        "周" | "星期" | "礼拜" => "week",
        "月" => "month",
        _ => "year",
    }
}

fn relation(prefix: Option<&str>) -> Option<i64> {
    match prefix? {
        "上上" => Some(-2),
        "上" | "last" | "previous" => Some(-1),
        "这" | "本" | "this" => Some(0),
        "下" | "next" | "coming" => Some(1),
        "下下" => Some(2),
        _ => None,
    }
}

/// 从开头解析日期部分，返回 (消耗的字节数, 各种解释)
fn date_part(text: &str, now: &Zoned) -> Option<(usize, Vec<DatePart>)> {
    let len = |caps: &Captures| caps.get(0).map_or(0, |m| m.end());
    if let Some(caps) = cn_offset().captures(text) {
        let half = &caps[1] == "半";
        let amount = if half { 0 } else { number(&caps[1])? };
        let sign = if caps[3].contains('前') { -1 } else { 1 };
        // 「3 月后」按 3 个月处理
        return Some((
            len(&caps),
            offset(now, amount, cn_unit(&caps[2]), half, sign),
        ));
    }
    if let Some(caps) = cn_day().captures(text) {
        let days = match &caps[1] {
            "大后天" => 3,
            "后天" => 2,
            "明天" | "明日" => 1,
            "今天" | "今日" => 0,
            "昨天" | "昨日" => -1,
            "前天" => -2,
            _ => -3,
        };
        return Some((len(&caps), part(shift_days(now, days), Vec::new())));
    }
    if let Some(caps) = cn_weekday().captures(text)
        // 「下周 3 点」里的数字是时刻，不是星期
        && !text[len(&caps)..]
            .trim_start()
            .starts_with(['点', '时', ':', '：'])
    {
        let parts = weekday_parts(
            now,
            relation(caps.get(1).map(|m| m.as_str())),
            cn_weekday_index(&caps[3]),
        );
        return Some((len(&caps), parts));
    }
    if let Some(caps) = cn_period().captures(text) {
        let n = relation(Some(&caps[1])).unwrap_or(0);
        let unit = if &caps[2] == "月" { "month" } else { "week" };
        return Some((len(&caps), offset(now, n.abs(), unit, false, n.signum())));
    }
    if let Some(caps) = cn_year().captures(text) {
        let n = match &caps[1] {
            "后年" => 2,
            "明年" => 1,
            "今年" => 0,
            "去年" => -1,
            _ => -2,
        };
        return Some((len(&caps), offset(now, n, "year", false, 1)));
    }
    if let Some(caps) = en_offset().captures(text) {
        let (amount, unit, sign) = match caps.get(1) {
            Some(amount) => (amount.as_str(), &caps[2], 1),
            None => {
                let sign = if matches!(&caps[5], "ago" | "before") {
                    -1
                } else {
                    1
                };
                (caps.get(3)?.as_str(), &caps[4], sign)
            }
        };
        let half = amount.starts_with("half");
        let amount = if half { 0 } else { english_number(amount)? };
        return Some((len(&caps), offset(now, amount, unit, half, sign)));
    }
    if let Some(caps) = en_day().captures(text) {
        let (days, exact) = match &caps[1] {
            "the day after tomorrow" | "day after tomorrow" => (2, false),
            "tomorrow" => (1, false),
            "today" | "tonight" => (0, false),
            "now" => (0, true),
            "yesterday" => (-1, false),
            _ => (-2, false),
        };
        let mut parts = part(shift_days(now, days), Vec::new());
        for p in &mut parts {
            p.exact = exact;
        }
        return Some((len(&caps), parts));
    }
    if let Some(caps) = en_weekday().captures(text) {
        let parts = weekday_parts(
            now,
            relation(caps.get(1).map(|m| m.as_str())),
            en_weekday_index(&caps[2]),
        );
        return Some((len(&caps), parts));
    }
    if let Some(caps) = en_period().captures(text) {
        let n = relation(Some(&caps[1])).unwrap_or(0);
        return Some((
            len(&caps),
            offset(now, n.abs(), &caps[2], false, n.signum()),
        ));
    }
    None
}

fn time(hour: i64, minute: i64, second: i64) -> Option<Time> {
    Time::new(
        i8::try_from(hour).ok()?,
        i8::try_from(minute).ok()?,
        i8::try_from(second).ok()?,
        0,
    )
    .ok()
}

/// 时刻部分；整串都必须是时刻
fn time_part(text: &str) -> Option<Vec<TimePart>> {
    let text = text.trim();
    if let Some(caps) = clock().captures(text) {
        let mut hour: i64 = caps[1].parse().ok()?;
        let minute: i64 = caps[2].parse().ok()?;
        let second: i64 = caps.get(3).map_or(Some(0), |s| s.as_str().parse().ok())?;
        if let Some(period) = caps.get(4) {
            hour = twelve_hour(hour, period.as_str().starts_with('p'))?;
        }
        return Some(vec![TimePart {
            time: time(hour, minute, second)?,
            note: None,
        }]);
    }
    if let Some(caps) = en_time().captures(text) {
        if let Some(word) = caps.get(3) {
            let hour = if word.as_str() == "noon" { 12 } else { 0 };
            return Some(vec![TimePart {
                time: time(hour, 0, 0)?,
                note: None,
            }]);
        }
        let hour: i64 = caps[1].parse().ok()?;
        return match caps.get(2).map(|m| m.as_str()) {
            Some(p) if p.starts_with('a') || p.starts_with('p') => Some(vec![TimePart {
                time: time(twelve_hour(hour, p.starts_with('p'))?, 0, 0)?,
                note: None,
            }]),
            _ => ambiguous_hour(hour, 0, 0),
        };
    }
    if let Some(caps) = cn_time().captures(text) {
        if let Some(word) = caps.get(6) {
            let hour = if matches!(word.as_str(), "中午" | "正午") {
                12
            } else {
                0
            };
            return Some(vec![TimePart {
                time: time(hour, 0, 0)?,
                note: None,
            }]);
        }
        let mut hour = number(caps.get(2)?.as_str())?;
        let minute = match caps.get(3).map(|m| m.as_str()) {
            Some("半") => 30,
            Some("一刻") => 15,
            Some("三刻") => 45,
            _ => caps.get(4).map_or(Some(0), |m| number(m.as_str()))?,
        };
        let second = caps.get(5).map_or(Some(0), |m| number(m.as_str()))?;
        return match caps.get(1).map(|m| m.as_str()) {
            Some("下午" | "傍晚" | "晚上" | "夜里" | "夜间") => {
                if hour < 12 {
                    hour += 12;
                }
                Some(vec![TimePart {
                    time: time(hour, minute, second)?,
                    note: None,
                }])
            }
            Some("中午" | "正午") => {
                if hour < 6 {
                    hour += 12;
                }
                Some(vec![TimePart {
                    time: time(hour, minute, second)?,
                    note: None,
                }])
            }
            Some(_) => {
                // 凌晨 12 点即 0 点
                if hour == 12 {
                    hour = 0;
                }
                Some(vec![TimePart {
                    time: time(hour, minute, second)?,
                    note: None,
                }])
            }
            None => ambiguous_hour(hour, minute, second),
        };
    }
    None
}

fn twelve_hour(hour: i64, pm: bool) -> Option<i64> {
    if !(1..=12).contains(&hour) {
        return None;
    }
    Some(match (hour, pm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, true) => h + 12,
        (h, false) => h,
    })
}

/// 没有上午 / 下午时，1–11 点有两种解释
fn ambiguous_hour(hour: i64, minute: i64, second: i64) -> Option<Vec<TimePart>> {
    let written = time(hour, minute, second)?;
    if (1..=11).contains(&hour) {
        Some(vec![
            TimePart {
                time: written,
                note: Some("am"),
            },
            TimePart {
                time: time(hour + 12, minute, second)?,
                note: Some("pm"),
            },
        ])
    } else {
        Some(vec![TimePart {
            time: written,
            note: None,
        }])
    }
}

/// 常见缩写展开：今晚 → 今天晚上
fn normalize(input: &str) -> String {
    let lower = input.trim().to_lowercase();
    let mut text = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    for (from, to) in [
        ("今晚", "今天晚上"),
        ("明晚", "明天晚上"),
        ("昨晚", "昨天晚上"),
        ("今早", "今天早上"),
        ("明早", "明天早上"),
        ("现在", "now"),
    ] {
        text = text.replace(from, to);
    }
    text.trim_end_matches(['。', '.', '!', '！']).to_owned()
}

fn combine(dates: Vec<DatePart>, times: Option<Vec<TimePart>>) -> Option<Vec<Candidate>> {
    let mut out = Vec::new();
    for date in dates {
        match &times {
            None => out.push(Candidate {
                at: date.at,
                notes: date.notes,
            }),
            Some(times) => {
                if date.exact {
                    return None;
                }
                for t in times {
                    let Ok(at) = date
                        .at
                        .date()
                        .to_datetime(t.time)
                        .to_zoned(date.at.time_zone().clone())
                    else {
                        continue;
                    };
                    let mut notes = date.notes.clone();
                    notes.extend(t.note);
                    out.push(Candidate { at, notes });
                }
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

pub fn parse(input: &str, now: &Zoned) -> Option<Vec<Candidate>> {
    let text = normalize(input);
    if text.is_empty() {
        return None;
    }
    // 日期在前：明天上午 9 点、tomorrow at 9am
    if let Some((consumed, dates)) = date_part(&text, now) {
        let rest = text[consumed..]
            .trim()
            .trim_start_matches(['的', ',', '，']);
        let rest = rest.trim();
        let times = if rest.is_empty() {
            None
        } else {
            Some(time_part(rest)?)
        };
        return combine(dates, times);
    }
    // 只有时刻：今天的这个时刻（纯数字不当作时刻）
    if !text.chars().all(|c| c.is_ascii_digit())
        && let Some(times) = time_part(&text)
    {
        return combine(part(Some(now.clone()), Vec::new()), Some(times));
    }
    // 英文时刻在前：9am tomorrow、9:30 pm tomorrow；依次尝试每个空格处的切分
    text.match_indices(' ').find_map(|(split, _)| {
        let (head, tail) = text.split_at(split);
        let times = time_part(head)?;
        let tail = tail.trim();
        let (consumed, dates) = date_part(tail, now)?;
        (consumed == tail.len()).then_some(())?;
        combine(dates, Some(times))
    })
}

#[cfg(test)]
#[path = "natural_test.rs"]
mod tests;
