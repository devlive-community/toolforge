//! 氛围效果模拟器后端：粒子预设参数表与白噪音合成参数。
//! 粒子动画由前端 canvas 渲染，雨声/篝火由前端 Web Audio 按此参数合成。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct AmbientEffects {
    manifest: Manifest,
}

impl Default for AmbientEffects {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for AmbientEffects {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "preset" => to_value(preset(parse_args(args)?)?),
            "noise" => to_value(noise(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(AmbientEffects);

const PRESETS: &[(&str, &str)] = &[
    ("snow", "Snow"),
    ("rain", "Rain"),
    ("fireflies", "Fireflies"),
    ("bubbles", "Bubbles"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetArgs {
    /// snow / rain / fireflies / bubbles
    pub key: String,
    /// 密度 0–100（默认 50）
    #[serde(default)]
    pub density: Option<f64>,
    /// 性能模式（低配机器降粒子数）
    #[serde(default)]
    pub low_power: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetOutput {
    pub key: String,
    pub label: &'static str,
    pub count: usize,
    /// 速度倍率（0.2–3）
    pub speed: f64,
    /// 风力（-1 左 到 1 右）
    pub wind: f64,
    /// 粒子尺寸范围（像素）
    pub size_min: f64,
    pub size_max: f64,
    /// 粒子主色（前端可再调）
    pub color: &'static str,
    /// 形状 key：circle / flake / streak / glow
    pub shape: &'static str,
    /// 背景建议色（透明背景时叠加用）
    pub background: &'static str,
}

pub fn preset(args: PresetArgs) -> PluginResult<PresetOutput> {
    let density = args.density.unwrap_or(50.0);
    if !(0.0..=100.0).contains(&density) {
        return Err(PluginError::new("ambient.bad_density").with("density", density));
    }
    let found = PRESETS
        .iter()
        .find(|(k, _)| *k == args.key)
        .ok_or_else(|| PluginError::new("ambient.unknown_preset").with("key", args.key.clone()))?;
    let factor = density / 100.0;
    let low = if args.low_power { 0.4 } else { 1.0 };
    let (count, speed, size_min, size_max, color, shape, background) = match found.0 {
        "snow" => (
            (300.0 * factor * low).round() as usize,
            0.6,
            2.0,
            6.0,
            "#ffffff",
            "flake",
            "#1e293b",
        ),
        "rain" => (
            (500.0 * factor * low).round() as usize,
            1.8,
            1.0,
            2.0,
            "#a5b4fc",
            "streak",
            "#0f172a",
        ),
        "fireflies" => (
            (80.0 * factor * low).round() as usize,
            0.4,
            2.0,
            4.0,
            "#fde047",
            "glow",
            "#052e16",
        ),
        _ => (
            (120.0 * factor * low).round() as usize,
            0.5,
            4.0,
            10.0,
            "#bae6fd",
            "circle",
            "#0c4a6e",
        ),
    };
    Ok(PresetOutput {
        key: found.0.to_owned(),
        label: found.1,
        count,
        speed,
        wind: 0.0,
        size_min,
        size_max,
        color,
        shape,
        background,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseArgs {
    /// rain / campfire
    pub kind: String,
    /// 音量 0–100
    #[serde(default)]
    pub volume: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseOutput {
    pub kind: String,
    /// 合成方式：filtered_noise / crackle
    pub method: &'static str,
    /// 滤波器类型与频率（rain 用 lowpass，campfire 用 lowpass + 随机爆点）
    pub filter_type: &'static str,
    pub filter_hz: u32,
    pub gain: f64,
    /// campfire 爆点间隔范围（毫秒）
    pub crackle_ms: Option<(u32, u32)>,
}

pub fn noise(args: NoiseArgs) -> PluginResult<NoiseOutput> {
    let volume = args.volume.unwrap_or(50.0);
    if !(0.0..=100.0).contains(&volume) {
        return Err(PluginError::new("ambient.bad_volume").with("volume", volume));
    }
    let gain = volume / 100.0 * 0.4;
    match args.kind.as_str() {
        "rain" => Ok(NoiseOutput {
            kind: args.kind,
            method: "filtered_noise",
            filter_type: "lowpass",
            filter_hz: 1400,
            gain,
            crackle_ms: None,
        }),
        "campfire" => Ok(NoiseOutput {
            kind: args.kind,
            method: "crackle",
            filter_type: "lowpass",
            filter_hz: 900,
            gain,
            crackle_ms: Some((80, 600)),
        }),
        other => Err(PluginError::new("ambient.unknown_noise").with("kind", other)),
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
