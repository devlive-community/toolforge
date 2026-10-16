//! 雪花 ID：按 epoch 与位布局生成 64 位整数 ID，并按常见预设反解时间、机器与序列号。
//!
//! 布局（高位到低位）：1 位符号（恒 0）| 时间 | 数据中心 | 机器 | 序列号。

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// 起始时间（Unix 毫秒）
    pub epoch_ms: i64,
    /// 时间单位（毫秒），1 为毫秒、1000 为秒
    pub unit_ms: i64,
    pub datacenter_bits: u32,
    pub worker_bits: u32,
    pub sequence_bits: u32,
}

impl Layout {
    pub fn time_bits(&self) -> u32 {
        63 - self.datacenter_bits - self.worker_bits - self.sequence_bits
    }

    fn validate(&self) -> PluginResult<()> {
        let fixed = self.datacenter_bits + self.worker_bits + self.sequence_bits;
        if self.sequence_bits == 0 || fixed > 63 || 63 - fixed < 20 {
            return Err(PluginError::new("uuid.snowflake.invalid_layout"));
        }
        if self.unit_ms <= 0 || self.epoch_ms < 0 {
            return Err(PluginError::new("uuid.snowflake.invalid_layout"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    /// Twitter 原版，MyBatis-Plus、Hutool 等沿用同一 epoch 与位布局
    Twitter,
    /// 美团 Leaf：10 位 workerId
    Leaf,
    /// 百度 UidGenerator：秒级时间 29 位、workerId 21 位、序列号 13 位
    Baidu,
    Discord,
    Custom,
}

pub const PRESETS: [Preset; 4] = [
    Preset::Twitter,
    Preset::Leaf,
    Preset::Baidu,
    Preset::Discord,
];

impl Preset {
    pub fn layout(self) -> Option<Layout> {
        let ms = |epoch_ms, datacenter_bits, worker_bits| Layout {
            epoch_ms,
            unit_ms: 1,
            datacenter_bits,
            worker_bits,
            sequence_bits: 12,
        };
        Some(match self {
            Preset::Twitter => ms(1_288_834_974_657, 5, 5),
            Preset::Leaf => ms(1_288_834_974_657, 0, 10),
            // 官方 README 推荐配置：29 位秒级时间（约 17 年）、21 位 workerId、13 位序列号，
            // epoch 2016-09-20 00:00:00 +08:00（代码默认的 28 位时间已在 2024 年用尽）
            Preset::Baidu => Layout {
                epoch_ms: 1_474_300_800_000,
                unit_ms: 1000,
                datacenter_bits: 0,
                worker_bits: 21,
                sequence_bits: 13,
            },
            Preset::Discord => ms(1_420_070_400_000, 5, 5),
            Preset::Custom => return None,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    pub preset: Preset,
    pub layout: Option<Layout>,
    #[serde(default)]
    pub datacenter_id: u64,
    #[serde(default)]
    pub worker_id: u64,
}

fn resolve(preset: Preset, custom: Option<Layout>) -> PluginResult<Layout> {
    let layout = preset
        .layout()
        .or(custom)
        .ok_or_else(|| PluginError::new("uuid.snowflake.invalid_layout"))?;
    layout.validate()?;
    Ok(layout)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 上一次生成的布局、时间刻度与序列号；同一布局在多次调用间保持严格递增
static LAST: Mutex<Option<(Layout, i64, u64)>> = Mutex::new(None);

pub fn generate(args: &GenerateArgs, count: usize) -> PluginResult<Vec<String>> {
    generate_at(args, count, now_ms())
}

pub fn generate_at(args: &GenerateArgs, count: usize, now_ms: i64) -> PluginResult<Vec<String>> {
    let layout = resolve(args.preset, args.layout)?;
    let check = |value: u64, bits: u32, code: &'static str| {
        let max = (1u64 << bits) - 1;
        if value > max {
            Err(PluginError::new(code).with("max", max))
        } else {
            Ok(())
        }
    };
    check(
        args.datacenter_id,
        layout.datacenter_bits,
        "uuid.snowflake.invalid_datacenter",
    )?;
    check(
        args.worker_id,
        layout.worker_bits,
        "uuid.snowflake.invalid_worker",
    )?;

    let tick = (now_ms - layout.epoch_ms).div_euclid(layout.unit_ms);
    if tick < 0 {
        return Err(PluginError::new("uuid.snowflake.before_epoch"));
    }
    let max_seq = (1u64 << layout.sequence_bits) - 1;
    let max_tick = (1i64 << layout.time_bits()) - 1;

    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    // 时间未前进（或时钟回拨）时沿用上次刻度并继续累加序列号，序列号用尽则借用下一刻度
    let (mut tick, mut seq) = match *last {
        Some((prev, prev_tick, prev_seq)) if prev == layout && prev_tick >= tick => {
            if prev_seq >= max_seq {
                (prev_tick + 1, 0)
            } else {
                (prev_tick, prev_seq + 1)
            }
        }
        _ => (tick, 0),
    };

    let mut ids = Vec::with_capacity(count);
    for index in 0..count {
        if index > 0 {
            if seq >= max_seq {
                tick += 1;
                seq = 0;
            } else {
                seq += 1;
            }
        }
        if tick > max_tick {
            return Err(PluginError::new("uuid.snowflake.overflow"));
        }
        let id = ((tick as u64) << (63 - layout.time_bits()))
            | (args.datacenter_id << (layout.worker_bits + layout.sequence_bits))
            | (args.worker_id << layout.sequence_bits)
            | seq;
        ids.push(id.to_string());
    }
    *last = Some((layout, tick, seq));
    Ok(ids)
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Decoded {
    pub preset: Preset,
    pub unix_ms: i64,
    pub timestamp: Option<String>,
    pub datacenter_id: Option<u64>,
    pub worker_id: u64,
    pub sequence: u64,
    /// 时间落在 2000 年之后、当前时间一天以内，更可能是这种布局生成的
    pub plausible: bool,
}

pub fn decode(id: u64, layout: Layout) -> (i64, Option<u64>, u64, u64) {
    let mask = |bits: u32| (1u64 << bits) - 1;
    let sequence = id & mask(layout.sequence_bits);
    let worker = (id >> layout.sequence_bits) & mask(layout.worker_bits);
    let datacenter =
        (id >> (layout.sequence_bits + layout.worker_bits)) & mask(layout.datacenter_bits);
    let tick = (id >> (63 - layout.time_bits())) as i64;
    let unix_ms = layout.epoch_ms + tick * layout.unit_ms;
    let datacenter = (layout.datacenter_bits > 0).then_some(datacenter);
    (unix_ms, datacenter, worker, sequence)
}

/// 解析纯数字输入：按所有预设反解。非 63 位正整数时返回 None
pub fn inspect(input: &str) -> Option<Vec<Decoded>> {
    if input.is_empty() || input.len() > 19 || !input.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let id: u64 = input.parse().ok()?;
    if id == 0 || id > i64::MAX as u64 {
        return None;
    }
    let lower = 946_684_800_000; // 2000-01-01
    let upper = now_ms() + 86_400_000;
    Some(
        PRESETS
            .iter()
            .filter_map(|&preset| {
                let layout = preset.layout()?;
                let (unix_ms, datacenter_id, worker_id, sequence) = decode(id, layout);
                Some(Decoded {
                    preset,
                    unix_ms,
                    timestamp: jiff::Timestamp::from_millisecond(unix_ms)
                        .ok()
                        .map(|t| t.to_string()),
                    datacenter_id,
                    worker_id,
                    sequence,
                    plausible: (lower..=upper).contains(&unix_ms),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
#[path = "snowflake_test.rs"]
mod tests;
