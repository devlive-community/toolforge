//! 分形渲染：逐像素迭代 + 平滑配色，输出 PNG。

use image::RgbaImage;
use serde::Serialize;

use crate::fractal::{self, Kind};
use crate::palette;

/// 像素预算（预览与导出共用上限）
pub const MAX_PIXELS: u64 = 2_500_000;
/// 迭代上限
pub const MAX_ITER_LIMIT: u32 = 5000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderStats {
    pub width: u32,
    pub height: u32,
    pub max_iter: u32,
    /// 未逃逸像素占比（0-1）：仅说明「本次未逃逸」，不宣称数学归属
    pub interior_ratio: f64,
    /// 视角宽度已低于双精度可用下限
    pub precision_limited: bool,
    pub elapsed_ms: u64,
}

pub struct RenderInput<'a> {
    pub kind: Kind,
    pub center_x: f64,
    pub center_y: f64,
    /// 复平面水平跨度（垂直按画布比例换算，不拉伸）
    pub span_x: f64,
    pub julia_x: f64,
    pub julia_y: f64,
    pub width: u32,
    pub height: u32,
    pub max_iter: u32,
    pub palette: &'a str,
    pub brightness: palette::Brightness,
    /// 逐行回调（任务进度）
    pub progress: Option<&'a dyn Fn(u32)>,
    /// 取消探测
    pub is_cancelled: Option<&'a dyn Fn() -> bool>,
}

pub struct RenderOutput {
    pub image: RgbaImage,
    pub stats: RenderStats,
}

/// 渲染一帧；span 低于精度极限时按极限渲染并标注
pub fn render(input: &RenderInput<'_>) -> crate::PluginResult<RenderOutput> {
    let width = input.width.max(1);
    let height = input.height.max(1);
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(crate::errp(
            "fractal.pixels_too_large",
            "limit",
            crate::MAX_PIXELS_LABEL,
        ));
    }
    let span_x = if input.span_x <= 0.0 {
        return Err(crate::errp("fractal.bad_span", "value", input.span_x));
    } else {
        input.span_x.min(8.0)
    };
    let precision_limited = span_x < fractal::PRECISION_LIMIT;
    let effective_span = span_x.max(fractal::PRECISION_LIMIT);
    let span_y = effective_span * f64::from(height) / f64::from(width);
    let start = std::time::Instant::now();
    let mut canvas = RgbaImage::new(width, height);
    let mut interior = 0u64;
    for row in 0..height {
        if let Some(cancelled) = input.is_cancelled
            && cancelled()
        {
            return Err(tf_plugin_api::cancelled());
        }
        // 像素中心 → 复平面（y 向上）
        let py =
            input.center_y + span_y / 2.0 - (f64::from(row) + 0.5) / f64::from(height) * span_y;
        for col in 0..width {
            let px = input.center_x - effective_span / 2.0
                + (f64::from(col) + 0.5) / f64::from(width) * effective_span;
            let smooth = fractal::escape(
                input.kind,
                px,
                py,
                input.julia_x,
                input.julia_y,
                input.max_iter,
            );
            let color = match smooth {
                None => {
                    interior += 1;
                    [0, 0, 0]
                }
                Some(value) => {
                    // 归一化到调色板：迭代数 / 上限，取对数尺度让低迭代区域也有层次
                    let position = (value / f64::from(input.max_iter) * 3.0).min(1.0);
                    palette::sample(input.palette, position, input.brightness)
                }
            };
            canvas.put_pixel(col, row, image::Rgba([color[0], color[1], color[2], 255]));
        }
        if let Some(progress) = input.progress {
            progress(row + 1);
        }
    }
    Ok(RenderOutput {
        image: canvas,
        stats: RenderStats {
            width,
            height,
            max_iter: input.max_iter,
            interior_ratio: interior as f64 / (u64::from(width) * u64::from(height)) as f64,
            precision_limited,
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
    })
}
