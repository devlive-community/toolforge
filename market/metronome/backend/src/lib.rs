//! 节拍器后端：按 BPM/拍号/分拍/小节数生成节拍时间表（毫秒时刻 + 重音级别），
//! 校验递增练习方案。合成点击音的波形参数也在此给出，前端用 Web Audio 播放。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Metronome {
    manifest: Manifest,
}

impl Default for Metronome {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Metronome {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "schedule" => to_value(schedule(parse_args(args)?)?),
            "practice_plan" => to_value(practice_plan(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Metronome);

const MIN_BPM: f64 = 30.0;
const MAX_BPM: f64 = 240.0;

fn check_bpm(bpm: f64) -> PluginResult<f64> {
    if !bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&bpm) {
        return Err(PluginError::new("metronome.bad_bpm")
            .with("bpm", bpm)
            .with("min", MIN_BPM)
            .with("max", MAX_BPM));
    }
    Ok(bpm)
}

/// 拍号分子（每小节拍数）；6/8 按八分音符 6 拍计速
fn beats_per_bar(signature: &str) -> PluginResult<usize> {
    match signature {
        "2/4" => Ok(2),
        "3/4" => Ok(3),
        "4/4" => Ok(4),
        "6/8" => Ok(6),
        other => Err(PluginError::new("metronome.bad_signature").with("signature", other)),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleArgs {
    pub bpm: f64,
    /// 2/4、3/4、4/4、6/8
    pub signature: String,
    /// 每拍等分数：1–4
    #[serde(default)]
    pub subdivision: Option<usize>,
    /// 生成的小节数（1–64）
    #[serde(default)]
    pub bars: Option<usize>,
    /// 每拍声音级别：0 静音 / 1 弱音 / 2 重音（循环应用）；缺省按小节首拍重音
    #[serde(default)]
    pub accents: Vec<u8>,
    /// 开始前预备小节数（0–4）
    #[serde(default)]
    pub count_in: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Beat {
    /// 相对开始时刻（毫秒）
    pub time_ms: u64,
    /// 小节号（预备拍为负号段，用 0 表示预备；正数从 1 开始）
    pub bar: u64,
    /// 小节内第几拍（1 起）
    pub beat: usize,
    /// 分拍序号（1 起；不分拍恒为 1）
    pub sub: usize,
    /// 声音级别 0/1/2
    pub level: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleOutput {
    pub bpm: f64,
    pub beat_ms: u64,
    pub beats_per_bar: usize,
    pub subdivision: usize,
    pub beats: Vec<Beat>,
    /// 点击音合成参数：重音/普通/分拍 的频率与时长
    pub tones: Tones,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tones {
    pub accent_hz: u32,
    pub normal_hz: u32,
    pub sub_hz: u32,
    pub duration_ms: u32,
}

pub fn schedule(args: ScheduleArgs) -> PluginResult<ScheduleOutput> {
    let bpm = check_bpm(args.bpm)?;
    let beats_per_bar = beats_per_bar(&args.signature)?;
    let subdivision = args.subdivision.unwrap_or(1);
    if !(1..=4).contains(&subdivision) {
        return Err(
            PluginError::new("metronome.bad_subdivision").with("subdivision", subdivision as u64)
        );
    }
    let bars = args.bars.unwrap_or(4);
    if !(1..=64).contains(&bars) {
        return Err(PluginError::new("metronome.bad_bars").with("bars", bars as u64));
    }
    let count_in = args.count_in.unwrap_or(0);
    if count_in > 4 {
        return Err(PluginError::new("metronome.bad_count_in").with("count_in", count_in as u64));
    }
    let beat_ms = (60_000.0 / bpm).round() as u64;
    let sub_ms = beat_ms / subdivision as u64;
    let mut beats = Vec::new();
    let total_bars = count_in as u64 + bars as u64;
    for bar in 0..total_bars {
        for beat in 1..=beats_per_bar {
            for sub in 1..=subdivision {
                let time_ms = ((bar * beats_per_bar as u64 + (beat - 1) as u64) * beat_ms)
                    + (sub - 1) as u64 * sub_ms;
                let level = if bar < count_in as u64 {
                    // 预备小节：小节首拍重音，其余弱音
                    if beat == 1 && sub == 1 { 2 } else { 1 }
                } else if let Some(custom) = args.accents.get(beat - 1) {
                    *custom
                } else if beat == 1 && sub == 1 {
                    2
                } else {
                    1
                };
                // 预备拍显示为小节 0，正曲从 1 开始
                let display_bar = if bar < count_in as u64 {
                    0
                } else {
                    bar - count_in as u64 + 1
                };
                beats.push(Beat {
                    time_ms,
                    bar: display_bar,
                    beat,
                    sub,
                    level,
                });
            }
        }
    }
    Ok(ScheduleOutput {
        bpm,
        beat_ms,
        beats_per_bar,
        subdivision,
        beats,
        tones: Tones {
            accent_hz: 1600,
            normal_hz: 1000,
            sub_hz: 700,
            duration_ms: 40,
        },
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeArgs {
    pub start_bpm: f64,
    pub target_bpm: f64,
    /// 每隔几小节升速
    #[serde(default)]
    pub every_bars: Option<usize>,
    /// 每次升速幅度（BPM）
    #[serde(default)]
    pub step: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub bpm: f64,
    pub bars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeOutput {
    pub start_bpm: f64,
    pub target_bpm: f64,
    pub every_bars: usize,
    pub step: f64,
    pub stages: Vec<Stage>,
}

/// 递增练习方案校验与分段
pub fn practice_plan(args: PracticeArgs) -> PluginResult<PracticeOutput> {
    let start = check_bpm(args.start_bpm)?;
    let target = check_bpm(args.target_bpm)?;
    let every_bars = args.every_bars.unwrap_or(4);
    if !(1..=32).contains(&every_bars) {
        return Err(
            PluginError::new("metronome.bad_every_bars").with("every_bars", every_bars as u64)
        );
    }
    let step = args.step.unwrap_or(4.0);
    if !step.is_finite() || step <= 0.0 || step > 60.0 {
        return Err(PluginError::new("metronome.bad_step").with("step", step));
    }
    if start > target {
        return Err(PluginError::new("metronome.start_above_target")
            .with("start", start)
            .with("target", target));
    }
    let mut stages = Vec::new();
    let mut bpm = start;
    loop {
        stages.push(Stage {
            bpm: (bpm * 10.0).round() / 10.0,
            bars: if bpm >= target {
                every_bars.max(4)
            } else {
                every_bars
            },
        });
        if bpm >= target {
            break;
        }
        bpm = (bpm + step).min(target);
    }
    if stages.len() > 32 {
        return Err(
            PluginError::new("metronome.too_many_stages").with("stages", stages.len() as u64)
        );
    }
    Ok(PracticeOutput {
        start_bpm: start,
        target_bpm: target,
        every_bars,
        step,
        stages,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
