//! 导出渲染：PNG 逐像素填充 + 粗描边；SVG 真多边形矢量输出。

use image::ImageEncoder;
use image::RgbaImage;
use tf_plugin_api::{PluginError, PluginResult};

use crate::sample;

/// PNG：按输出尺寸逐三角形填充，再叠描边
#[allow(clippy::too_many_arguments)]
pub fn render_png(
    work: &RgbaImage,
    points: &[[f64; 2]],
    triangles: &[[usize; 3]],
    colors: &[String],
    scale: f64,
    stroke_width: u32,
    stroke: [u8; 3],
    background: [u8; 3],
) -> PluginResult<Vec<u8>> {
    let out_w = ((f64::from(work.width()) * scale).round() as u32).max(1);
    let out_h = ((f64::from(work.height()) * scale).round() as u32).max(1);
    let mut canvas = RgbaImage::from_pixel(
        out_w,
        out_h,
        image::Rgba([background[0], background[1], background[2], 255]),
    );
    for (tri, color) in triangles.iter().zip(colors) {
        let rgb = sample::parse_color(color).unwrap_or([128, 128, 128]);
        fill_triangle(
            &mut canvas,
            points[tri[0]],
            points[tri[1]],
            points[tri[2]],
            scale,
            rgb,
        );
    }
    if stroke_width > 0 {
        for tri in triangles {
            draw_line(
                &mut canvas,
                points[tri[0]],
                points[tri[1]],
                scale,
                stroke_width,
                stroke,
            );
            draw_line(
                &mut canvas,
                points[tri[1]],
                points[tri[2]],
                scale,
                stroke_width,
                stroke,
            );
            draw_line(
                &mut canvas,
                points[tri[2]],
                points[tri[0]],
                scale,
                stroke_width,
                stroke,
            );
        }
    }
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut bytes,
        image::codecs::png::CompressionType::Default,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(
        canvas.as_raw(),
        out_w,
        out_h,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| errp("lowpoly.encode_failed", "detail", e.to_string()))?;
    Ok(bytes)
}

/// SVG：真正的 polygon 元素（矢量），不是位图内嵌
#[allow(clippy::too_many_arguments)]
pub fn render_svg(
    _work: &RgbaImage,
    points: &[[f64; 2]],
    triangles: &[[usize; 3]],
    colors: &[String],
    scale: f64,
    stroke_width: u32,
    stroke_color: String,
    background: String,
) -> String {
    let w = format!(
        "{:.2}",
        points.iter().fold(0.0f64, |m, p| m.max(p[0])) * scale
    );
    let h = format!(
        "{:.2}",
        points.iter().fold(0.0f64, |m, p| m.max(p[1])) * scale
    );
    let mut svg = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<rect width="{w}" height="{h}" fill="#{bg}"/>
"##,
        w = w,
        h = h,
        bg = background.trim_start_matches('#'),
    );
    let stroke = if stroke_width > 0 {
        format!(
            r##" stroke="#{}" stroke-width="{}" stroke-linejoin="round""##,
            stroke_color.trim_start_matches('#'),
            stroke_width
        )
    } else {
        String::new()
    };
    for (tri, color) in triangles.iter().zip(colors) {
        let pts = tri
            .iter()
            .map(|v| format!("{:.2},{:.2}", points[*v][0] * scale, points[*v][1] * scale))
            .collect::<Vec<_>>()
            .join(" ");
        svg.push_str(&format!(
            r#"<polygon points="{pts}" fill="{color}"{stroke}/>"#,
        ));
        svg.push('\n');
    }
    svg.push_str("</svg>\n");
    svg
}

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 重心坐标遍历填充（无 AA：低多边形风格就是硬边）
fn fill_triangle(
    canvas: &mut RgbaImage,
    a: [f64; 2],
    b: [f64; 2],
    c: [f64; 2],
    scale: f64,
    rgb: [u8; 3],
) {
    let (w, h) = (canvas.width(), canvas.height());
    let ax = a[0] * scale;
    let ay = a[1] * scale;
    let bx = b[0] * scale;
    let by = b[1] * scale;
    let cx = c[0] * scale;
    let cy = c[1] * scale;
    let min_x = (ax.min(bx).min(cx).floor() as i64).clamp(0, i64::from(w) - 1);
    let max_x = (ax.max(bx).max(cx).ceil() as i64).clamp(0, i64::from(w) - 1);
    let min_y = (ay.min(by).min(cy).floor() as i64).clamp(0, i64::from(h) - 1);
    let max_y = (ay.max(by).max(cy).ceil() as i64).clamp(0, i64::from(h) - 1);
    let area2 = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
    if area2 == 0.0 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = f64::from(x as u32) + 0.5;
            let py = f64::from(y as u32) + 0.5;
            let w0 = ((bx - px) * (cy - py) - (by - py) * (cx - px)) / area2;
            let w1 = ((cx - px) * (ay - py) - (cy - py) * (ax - px)) / area2;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                continue;
            }
            canvas.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([rgb[0], rgb[1], rgb[2], 255]),
            );
        }
    }
}

/// 粗线：沿边按半宽步进画方点（简单够用，边数有限）
fn draw_line(
    canvas: &mut RgbaImage,
    a: [f64; 2],
    b: [f64; 2],
    scale: f64,
    width: u32,
    stroke: [u8; 3],
) {
    let (w, h) = (canvas.width(), canvas.height());
    let ax = a[0] * scale;
    let ay = a[1] * scale;
    let bx = b[0] * scale;
    let by = b[1] * scale;
    let dx = bx - ax;
    let dy = by - ay;
    let length = (dx * dx + dy * dy).sqrt();
    let steps = (length.ceil() as usize).max(1);
    let half = f64::from(width) / 2.0;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let px = ax + dx * t;
        let py = ay + dy * t;
        let x0 = ((px - half).ceil() as i64).max(0);
        let x1 = ((px + half).floor() as i64).min(i64::from(w) - 1);
        let y0 = ((py - half).ceil() as i64).max(0);
        let y1 = ((py + half).floor() as i64).min(i64::from(h) - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                canvas.put_pixel(
                    x as u32,
                    y as u32,
                    image::Rgba([stroke[0], stroke[1], stroke[2], 255]),
                );
            }
        }
    }
}
