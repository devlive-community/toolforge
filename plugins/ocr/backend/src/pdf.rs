//! PDF 输入：用纯 Rust 的 hayro 逐页渲染为位图，交给识别管线处理。
//! 加密文件暂不支持。

use std::io::Cursor;
use std::panic::AssertUnwindSafe;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{RenderCache, RenderSettings};
use image::RgbImage;
use tf_plugin_api::{PluginError, PluginResult};

use crate::ocr::MAX_PIXELS;

/// 渲染倍率：页面点数（72 dpi）× 倍率 = 像素，144 dpi 与 180 dpi 之间，兼顾小字与内存
const RENDER_SCALE: f32 = 2.5;

pub struct PdfDoc {
    pdf: Pdf,
    settings: InterpreterSettings,
    /// 渲染倍率，按最大页面的像素预算收缩
    scale: f32,
}

fn failed(detail: impl std::fmt::Display) -> PluginError {
    PluginError::new("ocr.pdf_failed").with("detail", detail.to_string())
}

impl PdfDoc {
    pub fn open(path: &str) -> PluginResult<Self> {
        let data = std::fs::read(path).map_err(failed)?;
        let pdf = Pdf::new(data).map_err(|e| failed(format!("{e:?}")))?;
        // 超大幅面（工程图纸等）按像素上限收缩倍率，避免一次渲染占满内存
        let scale = pdf
            .pages()
            .iter()
            .map(|page| {
                let (width, height) = page.render_dimensions();
                width as f64 * height as f64
            })
            .fold(RENDER_SCALE, |scale, points| {
                if points > 0.0 {
                    scale.min((MAX_PIXELS as f32 / points as f32).sqrt())
                } else {
                    scale
                }
            })
            .max(0.1);
        Ok(Self {
            pdf,
            settings: InterpreterSettings::default(),
            scale,
        })
    }

    pub fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    /// 渲染一页（下标从 0 开始）；渲染可能 panic（不支持的内容），由调用方决定跳过
    pub fn render_page(&self, index: usize) -> PluginResult<RgbImage> {
        let Some(page) = self.pdf.pages().get(index) else {
            return Err(PluginError::new("ocr.pdf_page").with("page", index as u64));
        };
        // 渲染缓存的生存期绑定 PDF 数据的借用，只能在本次调用内创建；
        // 对扫描件没有可复用的字体缓存，代价可以忽略
        let cache = RenderCache::new();
        let render = || {
            hayro::render(
                page,
                &cache,
                &self.settings,
                &RenderSettings {
                    x_scale: self.scale,
                    y_scale: self.scale,
                    bg_color: hayro::vello_cpu::color::palette::css::WHITE,
                    ..Default::default()
                },
            )
        };
        let pixmap = std::panic::catch_unwind(AssertUnwindSafe(render))
            .map_err(|_| failed("the page could not be rendered"))?;
        let png = pixmap.into_png().map_err(failed)?;
        let image = image::ImageReader::new(Cursor::new(png))
            .with_guessed_format()
            .map_err(failed)?
            .decode()
            .map_err(failed)?;
        Ok(image.to_rgb8())
    }
}

#[cfg(test)]
#[path = "pdf_test.rs"]
mod tests;
