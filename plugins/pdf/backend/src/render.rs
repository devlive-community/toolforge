//! 页面缩略图：用纯 Rust 的 hayro 渲染为 PNG。

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{RenderCache, RenderSettings};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::source::Source;

/// 缩略图宽度上限（像素）
const MAX_WIDTH: u32 = 1200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub page: u32,
    pub data_uri: Option<String>,
}

pub fn thumbnails(source: &Source, pages: &[u32], width: u32) -> PluginResult<Vec<Thumbnail>> {
    let data = std::fs::read(&source.path)
        .map_err(|_| PluginError::new("fs.not_found").with("path", source.path.as_str()))?;
    let password = source.password.as_deref().unwrap_or("");
    let pdf = Pdf::new_with_password(data, password).map_err(|_| {
        if password.is_empty() {
            PluginError::new("pdf.password_required")
        } else {
            PluginError::new("pdf.wrong_password")
        }
    })?;
    let all = pdf.pages();
    let cache = RenderCache::new();
    let settings = InterpreterSettings::default();
    let width = width.clamp(32, MAX_WIDTH) as f32;
    Ok(pages
        .iter()
        .map(|&number| {
            // 单页渲染失败（不支持的特性等）时返回空缩略图，不影响其他页面
            let data_uri = number
                .checked_sub(1)
                .and_then(|index| all.get(index as usize))
                .and_then(|page| {
                    let (page_w, _) = page.render_dimensions();
                    let scale = width / page_w.max(1.0);
                    let pixmap = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        hayro::render(
                            page,
                            &cache,
                            &settings,
                            &RenderSettings {
                                x_scale: scale,
                                y_scale: scale,
                                bg_color: hayro::vello_cpu::color::palette::css::WHITE,
                                ..Default::default()
                            },
                        )
                    }))
                    .ok()?;
                    let png = pixmap.into_png().ok()?;
                    Some(format!(
                        "data:image/png;base64,{}",
                        data_encoding::BASE64.encode(&png)
                    ))
                });
            Thumbnail {
                page: number,
                data_uri,
            }
        })
        .collect())
}
