//! 公众号/知乎等平台粘贴用导出：把渲染后的 HTML 加上**行内样式**（平台编辑器
//! 通常剥掉 <style> 块），主题色与字号可调。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub source: String,
    /// 主题主色（默认微信灰绿）
    #[serde(default)]
    pub accent: Option<String>,
    /// 正文字号（px，14–20，默认 16）
    #[serde(default)]
    pub font_size: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// 全部样式内联的 HTML 片段（可直接粘贴进平台编辑器）
    pub html: String,
}

fn validate_color(color: &str) -> bool {
    color.starts_with('#') && color.len() <= 9 && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn inline_body(html: &str, accent: &str, font_size: u32) -> String {
    let styles: &[(&str, String)] = &[
        ("h1", format!("font-size:{}px;font-weight:700;margin:24px 0 16px;color:#1f2937;", font_size + 8)),
        ("h2", format!("font-size:{}px;font-weight:700;margin:20px 0 12px;color:#1f2937;border-left:4px solid {accent};padding-left:8px;", font_size + 4)),
        ("h3", format!("font-size:{}px;font-weight:700;margin:16px 0 8px;color:#1f2937;", font_size + 2)),
        ("p", format!("margin:0 0 12px;line-height:1.8;font-size:{font_size}px;color:#374151;")),
        ("ul", "margin:0 0 12px;padding-left:20px;".to_owned()),
        ("ol", "margin:0 0 12px;padding-left:20px;".to_owned()),
        ("li", format!("margin:0 0 6px;line-height:1.8;font-size:{font_size}px;color:#374151;")),
        ("blockquote", format!("margin:0 0 12px;padding:8px 12px;border-left:4px solid {accent};background:#f9fafb;color:#6b7280;font-size:{font_size}px;")),
        ("code", "background:#f3f4f6;padding:2px 4px;border-radius:3px;font-size:0.9em;".to_owned()),
        ("pre", "background:#1f2937;color:#f9fafb;padding:12px;border-radius:6px;overflow-x:auto;font-size:0.9em;".to_owned()),
        ("strong", format!("color:{accent};")),
        ("a", format!("color:{accent};text-decoration:none;border-bottom:1px solid {accent};")),
        ("img", "max-width:100%;border-radius:4px;".to_owned()),
        ("table", "border-collapse:collapse;margin:0 0 12px;width:100%;".to_owned()),
        ("th", "border:1px solid #d1d5db;padding:6px 10px;background:#f9fafb;font-weight:700;".to_owned()),
        ("td", "border:1px solid #d1d5db;padding:6px 10px;".to_owned()),
        ("hr", "border:none;border-top:1px solid #e5e7eb;margin:20px 0;".to_owned()),
    ];
    let mut out = html.to_owned();
    // 对 `<tag>` 与 `<tag attr…>` 两种开标签注入 style（保留已有属性）
    for (tag, style) in styles {
        let open_plain = format!("<{tag}>");
        let injected_plain = format!("<{tag} style=\"{style}\">");
        out = out.replace(&open_plain, &injected_plain);
        // 带属性的：`<tag ` → `<tag style="…" `（只处理尚未带 style 的）
        let open_attr = format!("<{tag} ");
        let injected_attr = format!("<{tag} style=\"{style}\" ");
        if !out.contains(&injected_attr) {
            out = out.replace(&open_attr, &injected_attr);
        }
    }
    out
}

pub fn styled_html(args: Args) -> PluginResult<Output> {
    let accent = args.accent.clone().unwrap_or_else(|| "#07c160".into());
    if !validate_color(&accent) {
        return Err(tf_plugin_api::PluginError::new("styled.bad_color").with("accent", accent));
    }
    let font_size = args.font_size.unwrap_or(16).clamp(14, 20);
    let rendered = crate::render::render(crate::render::Args {
        source: args.source,
    })?;
    Ok(Output {
        html: inline_body(&rendered.html, &accent, font_size),
    })
}
