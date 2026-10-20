//! 渐变调色板：预置 stops 插值 + 亮度调整。

use serde::Deserialize;

pub const SPECTRUM: &str = "spectrum";
pub const EMBER: &str = "ember";
pub const OCEAN: &str = "ocean";
pub const MONO: &str = "mono";

/// 渐变 stops（位置 0-1 + RGB）
type StopList = &'static [(f64, [u8; 3])];
const STOPS: &[(&str, StopList)] = &[
    (
        SPECTRUM,
        &[
            (0.0, [8, 12, 60]),
            (0.16, [28, 90, 189]),
            (0.42, [42, 188, 152]),
            (0.65, [250, 219, 97]),
            (0.85, [233, 87, 55]),
            (1.0, [32, 18, 42]),
        ],
    ),
    (
        EMBER,
        &[
            (0.0, [12, 8, 8]),
            (0.3, [120, 24, 12]),
            (0.55, [230, 92, 20]),
            (0.8, [252, 210, 90]),
            (1.0, [255, 250, 235]),
        ],
    ),
    (
        OCEAN,
        &[
            (0.0, [4, 10, 24]),
            (0.35, [10, 66, 116]),
            (0.6, [32, 145, 190]),
            (0.82, [148, 219, 226]),
            (1.0, [247, 254, 250]),
        ],
    ),
    (
        MONO,
        &[
            (0.0, [10, 10, 10]),
            (0.5, [128, 128, 128]),
            (1.0, [250, 250, 250]),
        ],
    ),
];

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Brightness {
    #[default]
    Normal,
    Bright,
    Dim,
}

/// 位置 0-1 → RGB；线性插值 + 亮度调整；未逃逸集合色由调用方指定
pub fn sample(palette: &str, position: f64, brightness: Brightness) -> [u8; 3] {
    let stops = STOPS
        .iter()
        .find(|(name, _)| *name == palette)
        .map(|(_, stops)| *stops)
        .unwrap_or(STOPS[0].1);
    let position = position.clamp(0.0, 1.0);
    // 找到所在区间线性插值
    let mut color = stops[stops.len() - 1].1;
    for window in stops.windows(2) {
        let (start, start_color) = window[0];
        let (end, end_color) = window[1];
        if position >= start && position <= end {
            let span = (end - start).max(f64::EPSILON);
            let t = (position - start) / span;
            color = [
                lerp(start_color[0], end_color[0], t),
                lerp(start_color[1], end_color[1], t),
                lerp(start_color[2], end_color[2], t),
            ];
            break;
        }
    }
    let factor = match brightness {
        Brightness::Normal => 1.0,
        Brightness::Bright => 1.25,
        Brightness::Dim => 0.7,
    };
    [
        ((color[0] as f64) * factor).round().clamp(0.0, 255.0) as u8,
        ((color[1] as f64) * factor).round().clamp(0.0, 255.0) as u8,
        ((color[2] as f64) * factor).round().clamp(0.0, 255.0) as u8,
    ]
}

fn lerp(a: u8, b: u8, t: f64) -> u8 {
    (f64::from(a) + (f64::from(b) - f64::from(a)) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

pub fn palettes() -> Vec<&'static str> {
    STOPS.iter().map(|(name, _)| *name).collect()
}

#[cfg(test)]
#[path = "palette_test.rs"]
mod tests;
