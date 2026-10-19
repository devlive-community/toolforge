//! 跨时区会议时段规划：按各地当日实际偏移（含 DST）计算共同可用窗口。
//! 可用性只来自用户填写的每周规则；时段一律以 UTC 分钟为最小单位做区间运算，
//! 会议时长按真实经过时间计算；春季跳时的时刻跳过并提示，秋季重复时刻列出候选。

use jiff::civil::{Date, Time};
use jiff::tz::{AmbiguousOffset, TimeZone};
use jiff::{Span, Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 参与人数上限
pub const MAX_PARTICIPANTS: usize = 32;
/// 每人每周时段数上限
pub const MAX_SLOTS: usize = 14;
/// 搜索天数上限
pub const MAX_SEARCH_DAYS: i64 = 60;
/// 会议时长范围（分钟）
pub const MIN_DURATION: i64 = 15;
pub const MAX_DURATION: i64 = 24 * 60;
/// 提示条目上限（gaps/ambiguities 各自）
const MAX_NOTICES: usize = 64;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

fn out_of_range() -> PluginError {
    PluginError::new("time.out_of_range")
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    /// ISO 星期：0=周一 … 6=周日
    pub day: i64,
    /// 本地开始 "HH:MM"
    pub start: String,
    /// 本地结束 "HH:MM"，小于 start 表示跨午夜
    pub end: String,
    /// 秋季重复时刻的消歧选择：本地日期 → 端点使用的 UTC 偏移秒
    #[serde(default)]
    pub choices: Vec<Choice>,
}

#[derive(Deserialize, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub date: String,
    pub offset_seconds: i64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    #[serde(default)]
    pub label: String,
    /// IANA 名称；local 表示本机时区
    pub timezone: String,
    #[serde(default)]
    pub slots: Vec<Slot>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArgs {
    pub participants: Vec<Participant>,
    /// 结果展示所用基准时区（窗口按它的日期分天）
    pub base_timezone: String,
    /// 搜索起始日期（基准时区）"YYYY-MM-DD"
    pub start_date: String,
    /// 会议时长（真实分钟）
    pub duration_minutes: i64,
    /// 搜索天数
    pub search_days: i64,
}

// ---- 输出 ----

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanRow {
    pub label: String,
    pub timezone: String,
    /// 本地开始日期 "YYYY-MM-DD"
    pub date: String,
    pub weekday: i64,
    /// 本地日内分钟（0..=1439）
    pub start_minute: i64,
    /// 结束的本地日期（跨午夜时晚于 date）
    pub end_date: String,
    pub end_minute: i64,
    pub cross_day: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanWindow {
    /// 基准时区日内分钟；end_minute 可为 1440（即 24:00）
    pub start_minute: i64,
    pub end_minute: i64,
    /// 真实经过分钟（DST 切换日与 end−start 不同）
    pub duration_minutes: i64,
    pub utc_start: String,
    pub utc_end: String,
    pub rows: Vec<PlanRow>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanDay {
    /// 基准时区日期 "YYYY-MM-DD"
    pub date: String,
    pub weekday: i64,
    pub windows: Vec<PlanWindow>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GapNotice {
    pub label: String,
    pub timezone: String,
    /// 该时刻在春令时切换中被跳过
    pub date: String,
    pub time: String,
    /// 定位来源：参与者与每周时段
    pub participant_index: i64,
    pub slot_index: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AmbiguityNotice {
    pub label: String,
    pub timezone: String,
    pub date: String,
    pub time: String,
    /// 两个候选的 UTC 偏移秒与 ISO 显示
    pub earlier_offset_seconds: i64,
    pub earlier_offset: String,
    pub later_offset_seconds: i64,
    pub later_offset: String,
    /// 定位来源：参与者与每周时段
    pub participant_index: i64,
    pub slot_index: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOutput {
    pub base_timezone: String,
    pub days: Vec<PlanDay>,
    pub gaps: Vec<GapNotice>,
    pub ambiguities: Vec<AmbiguityNotice>,
    /// 复制用整段文本
    pub text: String,
}

// ---- 基础解析 ----

pub fn parse_hhmm(text: &str, code: &str) -> PluginResult<(i64, i64)> {
    let text = text.trim();
    let Some((h, m)) = text.split_once(':') else {
        return Err(errp(code, "value", text));
    };
    if h.len() != 2
        || m.len() != 2
        || !h.chars().all(|c| c.is_ascii_digit())
        || !m.chars().all(|c| c.is_ascii_digit())
    {
        return Err(errp(code, "value", text));
    }
    let hour: i64 = h.parse().map_err(|_| errp(code, "value", text))?;
    let minute: i64 = m.parse().map_err(|_| errp(code, "value", text))?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return Err(errp(code, "value", text));
    }
    Ok((hour, minute))
}

fn parse_date(text: &str, code: &str) -> PluginResult<Date> {
    text.parse::<Date>().map_err(|_| errp(code, "value", text))
}

fn time_of(hour: i64, minute: i64) -> Time {
    Time::new(hour as i8, minute as i8, 0, 0).expect("validated HH:MM")
}

fn add_days(date: Date, days: i64) -> PluginResult<Date> {
    date.checked_add(Span::new().days(days as i32))
        .map_err(|_| out_of_range())
}

/// 偏移秒 → "+08:30" / "-04:00"
fn offset_text(seconds: i64) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.abs();
    format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

fn minutes_of_zoned(zoned: &Zoned) -> i64 {
    let time = zoned.time();
    i64::from(time.hour()) * 60 + i64::from(time.minute())
}

fn hhmm(minute: i64) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn weekday_text(weekday: i64) -> &'static str {
    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][weekday.clamp(0, 6) as usize]
}

/// 提示条目来源定位
struct NoticeContext {
    label: String,
    timezone: String,
    participant_index: usize,
    slot_index: usize,
}

/// 本地日期时间 → UTC 时刻。歧义时按 choices/默认 earlier 消歧；空洞返回 None 并记录提示。
fn to_utc(
    tz: &TimeZone,
    dt: jiff::civil::DateTime,
    slot: &Slot,
    ctx: &NoticeContext,
    gaps: &mut Vec<GapNotice>,
    ambiguities: &mut Vec<AmbiguityNotice>,
) -> PluginResult<Option<Timestamp>> {
    let amb = tz.to_ambiguous_zoned(dt);
    let date_text = dt.date().to_string();
    let time = dt.time();
    match amb.offset() {
        AmbiguousOffset::Unambiguous { .. } => amb
            .compatible()
            .map(|z| Some(z.timestamp()))
            .map_err(|_| out_of_range()),
        AmbiguousOffset::Gap { .. } => {
            if gaps.len() < MAX_NOTICES {
                gaps.push(GapNotice {
                    label: ctx.label.clone(),
                    timezone: ctx.timezone.clone(),
                    date: date_text.to_owned(),
                    time: time.to_string(),
                    participant_index: ctx.participant_index as i64,
                    slot_index: ctx.slot_index as i64,
                });
            }
            Ok(None)
        }
        AmbiguousOffset::Fold {
            before: earlier,
            after: later,
        } => {
            let chosen = slot
                .choices
                .iter()
                .find(|c| c.date == date_text)
                .map(|c| c.offset_seconds);
            let use_later = chosen == Some(i64::from(later.seconds()));
            let stamp = if use_later {
                amb.later().map_err(|_| out_of_range())?
            } else {
                amb.earlier().map_err(|_| out_of_range())?
            };
            if ambiguities.len() < MAX_NOTICES {
                ambiguities.push(AmbiguityNotice {
                    label: ctx.label.clone(),
                    timezone: ctx.timezone.clone(),
                    date: date_text.to_owned(),
                    time: time.to_string(),
                    earlier_offset_seconds: i64::from(earlier.seconds()),
                    earlier_offset: offset_text(i64::from(earlier.seconds())),
                    later_offset_seconds: i64::from(later.seconds()),
                    later_offset: offset_text(i64::from(later.seconds())),
                    participant_index: ctx.participant_index as i64,
                    slot_index: ctx.slot_index as i64,
                });
            }
            Ok(Some(stamp.timestamp()))
        }
    }
}

/// UTC 分钟区间
type Range = (i64, i64);

/// 生成一个参与者在整个搜索范围内（±2 天缓冲）的可用区间（已合并、升序）
fn participant_ranges(
    participant: &Participant,
    participant_index: usize,
    start: Date,
    days: i64,
    gaps: &mut Vec<GapNotice>,
    ambiguities: &mut Vec<AmbiguityNotice>,
) -> PluginResult<Vec<Range>> {
    let tz = super::zone::resolve(&participant.timezone)?;
    let first = add_days(start, -2)?;
    let last = add_days(start, days + 2)?;
    let mut raw: Vec<Range> = Vec::new();
    let mut date = first;
    let mut guard = 0;
    while date <= last {
        let weekday = i64::from(date.weekday().to_monday_zero_offset());
        for (slot_index, slot) in participant.slots.iter().enumerate() {
            if slot.day != weekday {
                continue;
            }
            let ctx = NoticeContext {
                label: if participant.label.is_empty() {
                    participant.timezone.clone()
                } else {
                    participant.label.clone()
                },
                timezone: participant.timezone.clone(),
                participant_index,
                slot_index,
            };
            let (sh, sm) = parse_hhmm(&slot.start, "meet.bad_time")?;
            let (eh, em) = parse_hhmm(&slot.end, "meet.bad_time")?;
            if (eh, em) == (sh, sm) {
                return Err(errp("meet.zero_slot", "start", slot.start.as_str()));
            }
            let cross_day = (eh, em) < (sh, sm);
            let end_date = if cross_day { add_days(date, 1)? } else { date };
            let Some(s) = to_utc(
                &tz,
                date.to_datetime(time_of(sh, sm)),
                slot,
                &ctx,
                gaps,
                ambiguities,
            )?
            else {
                continue;
            };
            let Some(e) = to_utc(
                &tz,
                end_date.to_datetime(time_of(eh, em)),
                slot,
                &ctx,
                gaps,
                ambiguities,
            )?
            else {
                continue;
            };
            raw.push((s.as_second() / 60, e.as_second() / 60));
        }
        date = add_days(date, 1)?;
        guard += 1;
        if guard > days + 8 {
            break;
        }
    }
    raw.sort();
    // 合并重叠/相接区间
    let mut merged: Vec<Range> = Vec::new();
    for (s, e) in raw {
        if e <= s {
            continue;
        }
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    Ok(merged)
}

fn intersect(a: &[Range], b: &[Range]) -> Vec<Range> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let s = a[i].0.max(b[j].0);
        let e = a[i].1.min(b[j].1);
        if s < e {
            out.push((s, e));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

fn to_zoned_utc(minutes: i64) -> PluginResult<Zoned> {
    Ok(Timestamp::from_second(minutes * 60)
        .map_err(|_| out_of_range())?
        .to_zoned(TimeZone::UTC))
}

/// 生成复制文本：按天分节，每窗口列出基准、各地与 UTC
fn build_text(base_name: &str, days: &[PlanDay]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for day in days {
        if day.windows.is_empty() {
            continue;
        }
        lines.push(format!("{} {}", day.date, weekday_text(day.weekday)));
        for window in &day.windows {
            lines.push(format!(
                "  {}–{} ({base_name}, {} min)",
                hhmm(window.start_minute),
                if window.end_minute >= 1440 {
                    "24:00".to_owned()
                } else {
                    hhmm(window.end_minute)
                },
                window.duration_minutes
            ));
            for row in &window.rows {
                lines.push(format!(
                    "    {} ({}): {} {}–{}{}",
                    row.label,
                    row.timezone,
                    row.date,
                    hhmm(row.start_minute),
                    if row.end_minute >= 1440 {
                        "24:00".to_owned()
                    } else {
                        hhmm(row.end_minute)
                    },
                    if row.cross_day { "+1d" } else { "" }
                ));
            }
            lines.push(format!("    UTC: {}–{}", window.utc_start, window.utc_end));
        }
    }
    lines.join("\n")
}

pub fn plan(args: PlanArgs) -> PluginResult<PlanOutput> {
    if args.participants.is_empty() || args.participants.len() > MAX_PARTICIPANTS {
        return Err(errp("meet.participants_range", "max", MAX_PARTICIPANTS));
    }
    if args.participants.iter().any(|p| p.slots.len() > MAX_SLOTS) {
        return Err(errp("meet.too_many_slots", "max", MAX_SLOTS));
    }
    if args
        .participants
        .iter()
        .any(|p| p.slots.iter().any(|s| !(0..=6).contains(&s.day)))
    {
        return Err(errp("meet.bad_weekday", "value", "day"));
    }
    let choice_total: usize = args
        .participants
        .iter()
        .map(|p| p.slots.iter().map(|s| s.choices.len()).sum::<usize>())
        .sum();
    if choice_total > MAX_NOTICES {
        return Err(errp("meet.too_many_slots", "max", MAX_SLOTS));
    }
    if !(MIN_DURATION..=MAX_DURATION).contains(&args.duration_minutes) {
        return Err(errp("meet.duration_range", "max", MAX_DURATION));
    }
    if !(1..=MAX_SEARCH_DAYS).contains(&args.search_days) {
        return Err(errp("meet.search_range", "max", MAX_SEARCH_DAYS));
    }
    let start = parse_date(&args.start_date, "meet.bad_date")?;
    let base_tz = super::zone::resolve(&args.base_timezone)?;

    // 搜索总窗口（基准时区语义），UTC 分钟
    let window_end_date = add_days(start, args.search_days)?;
    let midnight = time_of(0, 0);
    let t0 = base_tz
        .to_zoned(start.to_datetime(midnight))
        .map_err(|_| out_of_range())?
        .timestamp()
        .as_second()
        / 60;
    let t1 = base_tz
        .to_zoned(window_end_date.to_datetime(midnight))
        .map_err(|_| out_of_range())?
        .timestamp()
        .as_second()
        / 60;

    let mut gaps = Vec::new();
    let mut ambiguities = Vec::new();
    let mut current: Vec<Range> = vec![(t0, t1)];
    for (index, participant) in args.participants.iter().enumerate() {
        let ranges = participant_ranges(
            participant,
            index,
            start,
            args.search_days,
            &mut gaps,
            &mut ambiguities,
        )?;
        current = intersect(&current, &ranges);
        if current.is_empty() {
            break;
        }
    }

    // 按基准日分组输出
    let mut days: Vec<PlanDay> = Vec::new();
    for offset in 0..args.search_days {
        let day_start = add_days(start, offset)?;
        let w0 = base_tz
            .to_zoned(day_start.to_datetime(midnight))
            .map_err(|_| out_of_range())?
            .timestamp()
            .as_second()
            / 60;
        let w1 = base_tz
            .to_zoned(add_days(day_start, 1)?.to_datetime(midnight))
            .map_err(|_| out_of_range())?
            .timestamp()
            .as_second()
            / 60;
        let mut windows: Vec<PlanWindow> = Vec::new();
        for &(s, e) in &current {
            if e <= w0 || s >= w1 {
                continue;
            }
            let clip_start = s.max(w0);
            let clip_end = e.min(w1);
            // 只推荐完整容纳会议时长的区间
            if clip_end - clip_start < args.duration_minutes {
                continue;
            }
            let base_start = Timestamp::from_second(clip_start * 60)
                .map_err(|_| out_of_range())?
                .to_zoned(base_tz.clone());
            let base_end = Timestamp::from_second(clip_end * 60)
                .map_err(|_| out_of_range())?
                .to_zoned(base_tz.clone());
            let utc_start = to_zoned_utc(clip_start)?
                .strftime("%Y-%m-%d %H:%M")
                .to_string();
            let utc_end = to_zoned_utc(clip_end)?
                .strftime("%Y-%m-%d %H:%M")
                .to_string();
            let mut rows: Vec<PlanRow> = Vec::with_capacity(args.participants.len());
            for participant in &args.participants {
                let tz = super::zone::resolve(&participant.timezone)?;
                let local_start = Timestamp::from_second(clip_start * 60)
                    .map_err(|_| out_of_range())?
                    .to_zoned(tz.clone());
                let local_end = Timestamp::from_second(clip_end * 60)
                    .map_err(|_| out_of_range())?
                    .to_zoned(tz);
                rows.push(PlanRow {
                    label: if participant.label.is_empty() {
                        participant.timezone.clone()
                    } else {
                        participant.label.clone()
                    },
                    timezone: participant.timezone.clone(),
                    date: local_start.date().to_string(),
                    weekday: i64::from(local_start.date().weekday().to_monday_zero_offset()),
                    start_minute: minutes_of_zoned(&local_start),
                    end_date: local_end.date().to_string(),
                    end_minute: if clip_end == w1 {
                        1440
                    } else {
                        minutes_of_zoned(&local_end)
                    },
                    cross_day: local_end.date() != local_start.date(),
                });
            }
            windows.push(PlanWindow {
                start_minute: minutes_of_zoned(&base_start),
                end_minute: if clip_end == w1 {
                    1440
                } else {
                    minutes_of_zoned(&base_end)
                },
                duration_minutes: clip_end - clip_start,
                utc_start,
                utc_end,
                rows,
            });
        }
        days.push(PlanDay {
            date: day_start.to_string(),
            weekday: i64::from(day_start.weekday().to_monday_zero_offset()),
            windows,
        });
    }

    let base_name = if args.base_timezone.is_empty() || args.base_timezone == "local" {
        base_tz.iana_name().unwrap_or("local").to_owned()
    } else {
        args.base_timezone.clone()
    };
    let text = build_text(&base_name, &days);

    Ok(PlanOutput {
        base_timezone: base_name,
        days,
        gaps,
        ambiguities,
        text,
    })
}

#[cfg(test)]
#[path = "meeting_test.rs"]
mod tests;
