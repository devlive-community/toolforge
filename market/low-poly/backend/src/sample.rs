//! 三角形取色：包围盒 + 重心坐标逐像素平均；透明像素按底色合成。

use image::RgbaImage;

/// 采样每个三角形的平均色；亮度 -100..=100 直接调整输出色
pub fn colors(
    image: &RgbaImage,
    points: &[[f64; 2]],
    triangles: &[[usize; 3]],
    brightness: i32,
) -> Vec<String> {
    triangles
        .iter()
        .map(|tri| {
            let a = points[tri[0]];
            let b = points[tri[1]];
            let c = points[tri[2]];
            let hex = average(image, a, b, c);
            apply_brightness(&hex, brightness)
        })
        .collect()
}

fn average(image: &RgbaImage, a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> [u8; 3] {
    let (w, h) = (image.width(), image.height());
    let min_x = (a[0].min(b[0]).min(c[0]).floor() as i64).clamp(0, i64::from(w) - 1);
    let max_x = (a[0].max(b[0]).max(c[0]).ceil() as i64).clamp(0, i64::from(w) - 1);
    let min_y = (a[1].min(b[1]).min(c[1]).floor() as i64).clamp(0, i64::from(h) - 1);
    let max_y = (a[1].max(b[1]).max(c[1]).ceil() as i64).clamp(0, i64::from(h) - 1);
    let area2 = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let mut sum = [0u64; 3];
    let mut count = 0u64;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = f64::from(x as u32) + 0.5;
            let py = f64::from(y as u32) + 0.5;
            if area2 == 0.0 {
                continue;
            }
            // 重心坐标（允许微小的边界浮点误差）
            let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area2;
            let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area2;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                continue;
            }
            let [r, g, bl, alpha] = image.get_pixel(x as u32, y as u32).0;
            // 透明像素按底色合成，避免透明裂缝影响取色
            let blend = |fg: u8, bg: u8| {
                ((u16::from(fg) * u16::from(alpha) + u16::from(bg) * u16::from(255 - alpha)) / 255)
                    as u8
            };
            sum[0] += u64::from(blend(r, 255));
            sum[1] += u64::from(blend(g, 255));
            sum[2] += u64::from(blend(bl, 255));
            count += 1;
        }
    }
    if count == 0 {
        // 退化三角：退回任一顶点最近像素
        let x = (a[0] as u32).min(w - 1);
        let y = (a[1] as u32).min(h - 1);
        let [r, g, b, _] = image.get_pixel(x, y).0;
        return [r, g, b];
    }
    [
        (sum[0] / count) as u8,
        (sum[1] / count) as u8,
        (sum[2] / count) as u8,
    ]
}

fn apply_brightness(color: &[u8; 3], brightness: i32) -> String {
    let shift = |v: u8| {
        let next = i32::from(v) + brightness * 255 / 100;
        next.clamp(0, 255) as u8
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        shift(color[0]),
        shift(color[1]),
        shift(color[2])
    )
}

/// 解析 #RRGGBB → [r, g, b]（渲染与采样共用）
pub fn parse_color(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

#[cfg(test)]
#[path = "sample_test.rs"]
mod tests;
