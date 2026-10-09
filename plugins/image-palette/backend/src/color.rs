//! 颜色空间换算（sRGB ⇄ OKLab）与各种写法、对比度。

use serde::Serialize;

fn to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> u8 {
    let c = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// OKLab 坐标 [L, a, b]
pub type Lab = [f32; 3];

pub fn rgb_to_oklab([r, g, b]: [u8; 3]) -> Lab {
    let (r, g, b) = (to_linear(r), to_linear(g), to_linear(b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

pub fn oklab_to_rgb([l, a, b]: Lab) -> [u8; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        from_linear(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        from_linear(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_),
        from_linear(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
    ]
}

pub fn distance(a: &Lab, b: &Lab) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// WCAG 相对亮度
fn luminance([r, g, b]: [u8; 3]) -> f32 {
    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

fn contrast(a: f32, b: f32) -> f32 {
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

fn round(value: f32, places: i32) -> f32 {
    let k = 10f32.powi(places);
    (value * k).round() / k
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Formats {
    pub hex: String,
    pub rgb: String,
    pub hsl: String,
    pub oklch: String,
    /// 与白色、黑色文字的对比度
    pub contrast_white: f32,
    pub contrast_black: f32,
    /// 在这个颜色上放文字时更清楚的是白字还是黑字
    pub text: &'static str,
}

pub fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn hsl([r, g, b]: [u8; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l * 100.0);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s * 100.0, l * 100.0)
}

pub fn formats(rgb: [u8; 3]) -> Formats {
    let (h, s, l) = hsl(rgb);
    let [lightness, a, b] = rgb_to_oklab(rgb);
    let chroma = (a * a + b * b).sqrt();
    let hue = if chroma < 1e-4 {
        0.0
    } else {
        b.atan2(a).to_degrees().rem_euclid(360.0)
    };
    let lum = luminance(rgb);
    let (white, black) = (contrast(lum, 1.0), contrast(lum, 0.0));
    Formats {
        hex: hex(rgb),
        rgb: format!("rgb({} {} {})", rgb[0], rgb[1], rgb[2]),
        hsl: format!("hsl({} {}% {}%)", h.round(), s.round(), l.round()),
        oklch: format!(
            "oklch({}% {} {})",
            round(lightness * 100.0, 1),
            round(chroma, 3),
            round(hue, 1)
        ),
        contrast_white: round(white, 2),
        contrast_black: round(black, 2),
        text: if white >= black { "light" } else { "dark" },
    }
}

#[cfg(test)]
#[path = "color_test.rs"]
mod tests;
