//! 星空小向导后端：亮星 J2000 坐标 → 观测时地方地平坐标（方位/高度），
//! 附少量常见星座连线。数据为公开天文常识（Hipparcos 时代亮星，坐标历元 J2000），
//! 只覆盖肉眼亮星（V ≤ 1.6）与辨认用连线，不冒充完整星表。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::f64::consts::FRAC_PI_2;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct StargazingGuide {
    manifest: Manifest,
}

impl Default for StargazingGuide {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for StargazingGuide {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "sky" => to_value(sky(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(StargazingGuide);

/// (键, 中文名, 西名, 星座键, 视星等, 赤经小时, 赤纬度)
const STARS: &[(&str, &str, &str, &str, f64, f64, f64)] = &[
    ("sirius", "天狼星", "Sirius", "cma", -1.46, 6.7525, -16.716),
    (
        "canopus",
        "老人星",
        "Canopus",
        "car",
        -0.74,
        6.3992,
        -52.696,
    ),
    (
        "arcturus",
        "大角星",
        "Arcturus",
        "boo",
        -0.05,
        14.2610,
        19.182,
    ),
    ("vega", "织女星", "Vega", "lyr", 0.03, 18.6156, 38.784),
    ("capella", "五车二", "Capella", "aur", 0.08, 5.2782, 45.998),
    ("rigel", "参宿七", "Rigel", "ori", 0.13, 5.2423, -8.202),
    ("procyon", "南河三", "Procyon", "cmi", 0.34, 7.6550, 5.225),
    (
        "betelgeuse",
        "参宿四",
        "Betelgeuse",
        "ori",
        0.50,
        5.9195,
        7.407,
    ),
    (
        "achernar",
        "水委一",
        "Achernar",
        "eri",
        0.46,
        1.6286,
        -57.237,
    ),
    ("altair", "牛郎星", "Altair", "aql", 0.76, 19.8464, 8.868),
    (
        "aldebaran",
        "毕宿五",
        "Aldebaran",
        "tau",
        0.86,
        4.5987,
        16.509,
    ),
    (
        "antares",
        "心宿二",
        "Antares",
        "sco",
        1.09,
        16.4901,
        -26.432,
    ),
    ("spica", "角宿一", "Spica", "vir", 0.97, 13.4199, -11.161),
    ("pollux", "北河三", "Pollux", "gem", 1.14, 7.7553, 28.026),
    ("deneb", "天津四", "Deneb", "cyg", 1.25, 20.6905, 45.280),
    (
        "regulus",
        "轩辕十四",
        "Regulus",
        "leo",
        1.35,
        10.1395,
        11.967,
    ),
    ("polaris", "北极星", "Polaris", "umi", 1.98, 2.5303, 89.264),
];

/// 星座连线：星座键、显示名、星键对列表
type Constellation = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// 星座连线（星键对，辨认辅助；两端必须都在亮星表内）
const CONSTELLATIONS: &[Constellation] = &[(
    "ori",
    "猎户座",
    &[
        ("betelgeuse", "rigel"),
        ("betelgeuse", "procyon"),
        ("procyon", "rigel"),
    ],
)];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 纬度（度，北正南负）
    pub latitude: f64,
    /// 经度（度，东正西负）
    pub longitude: f64,
    /// UTC 时间戳（毫秒）
    pub timestamp_ms: i64,
    /// 最低高度角（度，低于此不返回；默认 -0.5 略过地平）
    #[serde(default)]
    pub min_altitude: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarPosition {
    pub key: &'static str,
    pub name_zh: &'static str,
    pub name_en: &'static str,
    pub constellation: &'static str,
    pub magnitude: f64,
    /// 方位角（度，北 0 东 90）
    pub azimuth: f64,
    /// 高度角（度，地平 0）
    pub altitude: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyOutput {
    pub latitude: f64,
    pub longitude: f64,
    pub local_sidereal_time_hours: f64,
    pub stars: Vec<StarPosition>,
    /// 连线（星键对；两端都在地平线上方才返回）
    pub lines: Vec<(String, String)>,
}

/// Unix 毫秒 → 格林尼治平恒星时（小时）
fn gmst_hours(timestamp_ms: i64) -> f64 {
    let days = timestamp_ms as f64 / 86_400_000.0;
    // Meeus 12.4 近似（足够肉眼辨认用途）
    let jd = days + 2_440_587.5;
    let t = (jd - 2_451_545.0) / 36_525.0;
    let gmst_deg = 280.460_618_37 + 360.985_647_366_29 * (jd - 2_451_545.0) + 0.000_387_933 * t * t
        - t * t * t / 387_100.0;
    gmst_deg.rem_euclid(360.0) / 15.0
}

/// 赤道坐标 → 地平坐标
fn horizontal(ra_hours: f64, dec_deg: f64, lat_deg: f64, lst_hours: f64) -> (f64, f64) {
    let ha = (lst_hours - ra_hours).rem_euclid(24.0) * 15.0_f64.to_radians();
    let lat = lat_deg.to_radians();
    let dec = dec_deg.to_radians();
    let sin_alt = dec.sin() * lat.sin() + dec.cos() * lat.cos() * ha.cos();
    let alt = sin_alt.clamp(-1.0, 1.0).asin();
    // 方位角（北 0，向东为正）
    let cos_az = (dec.sin() - alt.sin() * lat.sin()) / (alt.cos() * lat.cos());
    let mut az = cos_az.clamp(-1.0, 1.0).acos();
    if ha.sin() > 0.0 {
        az = std::f64::consts::TAU - az;
    }
    (az.to_degrees(), alt.to_degrees())
}

pub fn sky(args: Args) -> PluginResult<SkyOutput> {
    if !(-90.0..=90.0).contains(&args.latitude) || !(-180.0..=180.0).contains(&args.longitude) {
        return Err(PluginError::new("sky.bad_location")
            .with("latitude", args.latitude)
            .with("longitude", args.longitude));
    }
    if !(-631_152_000_000i64..=253_402_300_799_000i64).contains(&args.timestamp_ms) {
        return Err(PluginError::new("sky.bad_time").with("timestampMs", args.timestamp_ms));
    }
    let min_alt = args.min_altitude.unwrap_or(-0.5);
    let lst = (gmst_hours(args.timestamp_ms) + args.longitude / 15.0).rem_euclid(24.0);
    let mut stars = Vec::new();
    let mut above: Vec<&str> = Vec::new();
    for (key, name_zh, name_en, constellation, magnitude, ra, dec) in STARS {
        let (az, alt) = horizontal(*ra, *dec, args.latitude, lst);
        if alt >= min_alt {
            above.push(key);
            stars.push(StarPosition {
                key,
                name_zh,
                name_en,
                constellation,
                magnitude: *magnitude,
                azimuth: (az * 100.0).round() / 100.0,
                altitude: (alt * 100.0).round() / 100.0,
            });
        }
    }
    let mut lines = Vec::new();
    for (constellation, _, pairs) in CONSTELLATIONS {
        let _ = constellation;
        for (a, b) in *pairs {
            if above.contains(a) && above.contains(b) {
                lines.push(((*a).to_owned(), (*b).to_owned()));
            }
        }
    }
    // 北极星特殊校验：北半球高度 ≈ 纬度（用于自检说明，不参与业务）
    Ok(SkyOutput {
        latitude: args.latitude,
        longitude: args.longitude,
        local_sidereal_time_hours: (lst * 100.0).round() / 100.0,
        stars,
        lines,
    })
}

const _: () = {
    // 保持 FRAC_PI_2 引用（供后续地平线绘制扩展）
    fn _touch() -> f64 {
        FRAC_PI_2
    }
};

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
