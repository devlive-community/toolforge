//! 坐标轴刻度：线性取「整齐」的步长（1、2、5 × 10ⁿ），对数按数量级，时间按本地时区对齐到常见间隔。

use jiff::{Timestamp, ToSpan, Zoned};
use serde::Serialize;

use crate::infer::local_zone;

/// 目标刻度数
const TARGET: f64 = 6.0;
/// 类目轴最多显示的标签数
const MAX_CATEGORY_LABELS: usize = 24;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// 坐标范围与刻度
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Scale {
    pub min: f64,
    pub max: f64,
    pub ticks: Vec<Tick>,
}

fn nice_step(span: f64, integer: bool) -> f64 {
    let raw = span / TARGET;
    let magnitude = 10f64.powf(raw.log10().floor());
    let norm = raw / magnitude;
    let step = magnitude
        * if norm < 1.5 {
            1.0
        } else if norm < 3.0 {
            2.0
        } else if norm < 7.0 {
            5.0
        } else {
            10.0
        };
    if integer { step.max(1.0).round() } else { step }
}

/// 按步长决定小数位；很大或很小的数用科学计数法
pub fn format_number(value: f64, step: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    let abs = value.abs();
    if !(1e-6..1e15).contains(&abs) || step < 1e-9 {
        return format!("{value:e}");
    }
    let decimals = if step > 0.0 {
        (-step.log10().floor()).max(0.0) as usize
    } else {
        0
    };
    let text = format!("{value:.decimals$}");
    if text.starts_with("-0") && text.trim_start_matches(['-', '0', '.']).is_empty() {
        return "0".into();
    }
    text
}

/// 线性刻度；`zero` 时范围包含 0（柱状图）；`integer` 时步长至少为 1（行号）
pub fn linear(min: f64, max: f64, zero: bool, integer: bool) -> Scale {
    let (mut lo, mut hi) = (min, max);
    if zero {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    if lo == hi {
        let pad = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
        lo -= pad;
        hi += pad;
    }
    let step = nice_step(hi - lo, integer);
    let start = (lo / step).floor() * step;
    let end = (hi / step).ceil() * step;
    let count = ((end - start) / step).round() as usize;
    let ticks = (0..=count)
        .map(|i| {
            let value = start + step * i as f64;
            // 消除浮点误差：0.30000000000000004 → 0.3
            let value = (value / step).round() * step;
            Tick {
                value,
                label: format_number(value, step),
            }
        })
        .collect();
    Scale {
        min: start,
        max: end,
        ticks,
    }
}

/// 对数刻度（min > 0）：10 的整数次幂；跨度不足 3 个数量级时补上 2、5 倍
pub fn log(min: f64, max: f64) -> Scale {
    let lo = min.log10().floor() as i32;
    let mut hi = max.log10().ceil() as i32;
    if hi == lo {
        hi += 1;
    }
    let dense = hi - lo < 3;
    let mut ticks = Vec::new();
    for exp in lo..=hi {
        let base = 10f64.powi(exp);
        let multipliers: &[f64] = if dense && exp < hi {
            &[1.0, 2.0, 5.0]
        } else {
            &[1.0]
        };
        for m in multipliers {
            let value = base * m;
            ticks.push(Tick {
                value,
                label: format_number(value, base.min(1.0)),
            });
        }
    }
    Scale {
        min: 10f64.powi(lo),
        max: 10f64.powi(hi),
        ticks,
    }
}

/// 时间刻度的间隔
#[derive(Debug, Clone, Copy)]
enum Step {
    Millis(i64),
    Days(i64),
    Months(i32),
    Years(i32),
}

const SECOND: i64 = 1000;
const MINUTE: i64 = 60 * SECOND;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

fn time_step(span: f64) -> Step {
    let millis = [
        1,
        2,
        5,
        10,
        20,
        50,
        100,
        200,
        500,
        SECOND,
        2 * SECOND,
        5 * SECOND,
        10 * SECOND,
        15 * SECOND,
        30 * SECOND,
        MINUTE,
        2 * MINUTE,
        5 * MINUTE,
        10 * MINUTE,
        15 * MINUTE,
        30 * MINUTE,
        HOUR,
        2 * HOUR,
        3 * HOUR,
        6 * HOUR,
        12 * HOUR,
    ];
    if let Some(step) = millis.iter().find(|s| span / (**s as f64) <= TARGET + 2.0) {
        return Step::Millis(*step);
    }
    for days in [1, 2, 7, 14] {
        if span / (days * DAY) as f64 <= TARGET + 2.0 {
            return Step::Days(days);
        }
    }
    for months in [1, 2, 3, 6] {
        if span / (months as f64 * 30.4 * DAY as f64) <= TARGET + 2.0 {
            return Step::Months(months);
        }
    }
    let years = span / (365.25 * DAY as f64);
    let step = nice_step(years, true).max(1.0) as i32;
    Step::Years(step)
}

fn zoned(ms: f64) -> Option<Zoned> {
    Timestamp::from_millisecond(ms as i64)
        .ok()
        .map(|t| t.to_zoned(local_zone().clone()))
}

/// 时间刻度：毫秒值；标签按本地时间，跨天时在当天第一个刻度带上日期
pub fn time(min: f64, max: f64) -> Scale {
    let (lo, hi) = if min == max {
        (min - 30.0 * MINUTE as f64, max + 30.0 * MINUTE as f64)
    } else {
        (min, max)
    };
    let step = time_step(hi - lo);
    let Some(start) = zoned(lo) else {
        return linear(lo, hi, false, false);
    };
    let mut values = Vec::new();
    match step {
        Step::Millis(step) => {
            // 按本地时间对齐：先换算到本地「墙上时间」再取整
            let offset = start.offset().seconds() as i64 * SECOND;
            let local = lo as i64 + offset;
            let mut t =
                local.div_euclid(step) * step + if local.rem_euclid(step) == 0 { 0 } else { step };
            while ((t - offset) as f64) <= hi && values.len() < 64 {
                values.push((t - offset) as f64);
                t += step;
            }
        }
        _ => {
            let Ok(mut day) = start.start_of_day() else {
                return linear(lo, hi, false, false);
            };
            if let Step::Months(_) | Step::Years(_) = step {
                let first = day.date().first_of_month();
                let first = if let Step::Years(_) = step {
                    first.first_of_year()
                } else {
                    first
                };
                if let Ok(z) = first.to_zoned(local_zone().clone()) {
                    day = z;
                }
            }
            while (day.timestamp().as_millisecond() as f64) <= hi && values.len() < 64 {
                let ms = day.timestamp().as_millisecond() as f64;
                if ms >= lo {
                    values.push(ms);
                }
                let next = match step {
                    Step::Days(d) => day.checked_add(d.days()),
                    Step::Months(m) => day.checked_add(m.months()),
                    Step::Years(y) => day.checked_add(y.years()),
                    Step::Millis(_) => unreachable!(),
                };
                match next {
                    Ok(n) => day = n,
                    Err(_) => break,
                }
            }
        }
    }
    let ticks = label_times(&values, step);
    Scale {
        min: lo,
        max: hi,
        ticks,
    }
}

fn label_times(values: &[f64], step: Step) -> Vec<Tick> {
    let mut previous_date = None;
    values
        .iter()
        .filter_map(|&value| {
            let z = zoned(value)?;
            let date = z.date();
            let new_day = previous_date != Some(date);
            previous_date = Some(date);
            let label = match step {
                Step::Years(_) => z.strftime("%Y").to_string(),
                Step::Months(_) => z.strftime("%Y-%m").to_string(),
                Step::Days(_) => z.strftime("%m-%d").to_string(),
                Step::Millis(ms) if ms < SECOND => z.strftime("%H:%M:%S%.3f").to_string(),
                Step::Millis(ms) => {
                    let time = if ms < MINUTE {
                        z.strftime("%H:%M:%S")
                    } else {
                        z.strftime("%H:%M")
                    }
                    .to_string();
                    if new_day {
                        format!("{} {time}", z.strftime("%m-%d"))
                    } else {
                        time
                    }
                }
            };
            Some(Tick { value, label })
        })
        .collect()
}

/// 时间值作类目时的简短标签：同一天只显示时间，都是零点只显示日期，秒都为 0 时省略秒
pub fn time_labels(values: &[Option<f64>]) -> Vec<String> {
    let zoned: Vec<Option<Zoned>> = values.iter().map(|v| v.and_then(zoned)).collect();
    let present: Vec<&Zoned> = zoned.iter().flatten().collect();
    let same_day = present.windows(2).all(|w| w[0].date() == w[1].date());
    let same_year = present.windows(2).all(|w| w[0].year() == w[1].year());
    let midnight = present
        .iter()
        .all(|z| z.time() == jiff::civil::Time::midnight());
    let whole_minutes = present
        .iter()
        .all(|z| z.second() == 0 && z.subsec_nanosecond() == 0);
    let clock = if whole_minutes { "%H:%M" } else { "%H:%M:%S" };
    let pattern = match (midnight, same_day, same_year) {
        (true, _, true) => "%m-%d".to_owned(),
        (true, _, false) => "%Y-%m-%d".to_owned(),
        (false, true, _) => clock.to_owned(),
        (false, false, true) => format!("%m-%d {clock}"),
        (false, false, false) => format!("%Y-%m-%d {clock}"),
    };
    zoned
        .iter()
        .map(|z| {
            z.as_ref()
                .map_or_else(String::new, |z| z.strftime(&pattern).to_string())
        })
        .collect()
}

/// 类目轴：标签过多时按间隔抽取
pub fn categories(labels: &[String]) -> Scale {
    let every = labels.len().div_ceil(MAX_CATEGORY_LABELS).max(1);
    let ticks = labels
        .iter()
        .enumerate()
        .filter(|(i, _)| i % every == 0)
        .map(|(i, label)| Tick {
            value: i as f64,
            label: label.clone(),
        })
        .collect();
    Scale {
        min: -0.5,
        max: labels.len() as f64 - 0.5,
        ticks,
    }
}

#[cfg(test)]
#[path = "axis_test.rs"]
mod tests;
