//! Meta 标签生成器后端：从表单数据生成整套 head 代码 + 长度告警。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct MetaTags {
    manifest: Manifest,
}

impl Default for MetaTags {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for MetaTags {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(MetaTags);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub keywords: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    /// Open Graph / Twitter 图片 URL
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// og:type：website / article / product
    #[serde(default)]
    pub og_type: Option<String>,
    /// 站点名（og:site_name）
    #[serde(default)]
    pub site_name: Option<String>,
    /// twitter:card：summary / summary_large_image
    #[serde(default)]
    pub twitter_card: Option<String>,
    /// 语言（hreflang 与 og:locale）
    #[serde(default)]
    pub locale: Option<String>,
    /// JSON-LD 预设：none / website / article
    #[serde(default)]
    pub json_ld: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub field: &'static str,
    /// 规则键 warnings.<key>
    pub rule: &'static str,
    pub actual: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// 完整 <head> 片段
    pub html: String,
    pub warnings: Vec<Warning>,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn generate(args: Args) -> PluginResult<Output> {
    let title = args.title.trim().to_owned();
    if title.is_empty() {
        return Err(PluginError::new("meta.empty_title"));
    }
    if title.len() > 500 {
        return Err(PluginError::new("meta.title_too_long"));
    }
    let mut warnings: Vec<Warning> = Vec::new();
    // 长度告警（按字符计）
    let title_chars = title.chars().count();
    if title_chars > 60 {
        warnings.push(Warning {
            field: "title",
            rule: "title_too_long",
            actual: title_chars,
        });
    }
    let description = args.description.unwrap_or_default();
    let desc_chars = description.chars().count();
    if desc_chars > 160 {
        warnings.push(Warning {
            field: "description",
            rule: "description_too_long",
            actual: desc_chars,
        });
    }
    let og_type = args.og_type.unwrap_or_else(|| "website".into());
    if !matches!(og_type.as_str(), "website" | "article" | "product") {
        return Err(PluginError::new("meta.bad_og_type").with("type", og_type));
    }
    let twitter_card = args
        .twitter_card
        .unwrap_or_else(|| "summary_large_image".into());
    if !matches!(twitter_card.as_str(), "summary" | "summary_large_image") {
        return Err(PluginError::new("meta.bad_card").with("card", twitter_card));
    }
    let json_ld_kind = args.json_ld.unwrap_or_else(|| "none".into());
    if !matches!(json_ld_kind.as_str(), "none" | "website" | "article") {
        return Err(PluginError::new("meta.bad_jsonld").with("kind", json_ld_kind));
    }
    let mut html = String::with_capacity(2048);
    html.push_str(&format!("<title>{}</title>\n", escape(&title)));
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    if !description.is_empty() {
        html.push_str(&format!(
            "<meta name=\"description\" content=\"{}\">\n",
            escape(&description)
        ));
    }
    if let Some(keywords) = args
        .keywords
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        html.push_str(&format!(
            "<meta name=\"keywords\" content=\"{}\">\n",
            escape(keywords)
        ));
    }
    if let Some(author) = args
        .author
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
    {
        html.push_str(&format!(
            "<meta name=\"author\" content=\"{}\">\n",
            escape(author)
        ));
    }
    if let Some(url) = args.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        html.push_str(&format!(
            "<link rel=\"canonical\" href=\"{}\">\n",
            escape(url)
        ));
    }
    let locale = args.locale.clone().unwrap_or_else(|| "en_US".into());
    // Open Graph
    html.push_str(&format!(
        "<meta property=\"og:title\" content=\"{}\">\n",
        escape(&title)
    ));
    if !description.is_empty() {
        html.push_str(&format!(
            "<meta property=\"og:description\" content=\"{}\">\n",
            escape(&description)
        ));
    }
    if let Some(image) = args
        .image
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty())
    {
        html.push_str(&format!(
            "<meta property=\"og:image\" content=\"{}\">\n",
            escape(image)
        ));
    }
    html.push_str(&format!(
        "<meta property=\"og:type\" content=\"{}\">\n",
        escape(&og_type)
    ));
    if let Some(url) = args.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        html.push_str(&format!(
            "<meta property=\"og:url\" content=\"{}\">\n",
            escape(url)
        ));
    }
    if let Some(site_name) = args
        .site_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        html.push_str(&format!(
            "<meta property=\"og:site_name\" content=\"{}\">\n",
            escape(site_name)
        ));
    }
    html.push_str(&format!(
        "<meta property=\"og:locale\" content=\"{}\">\n",
        escape(&locale)
    ));
    // Twitter Card
    html.push_str(&format!(
        "<meta name=\"twitter:card\" content=\"{}\">\n",
        escape(&twitter_card)
    ));
    html.push_str(&format!(
        "<meta name=\"twitter:title\" content=\"{}\">\n",
        escape(&title)
    ));
    if !description.is_empty() {
        html.push_str(&format!(
            "<meta name=\"twitter:description\" content=\"{}\">\n",
            escape(&description)
        ));
    }
    if let Some(image) = args
        .image
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty())
    {
        html.push_str(&format!(
            "<meta name=\"twitter:image\" content=\"{}\">\n",
            escape(image)
        ));
    }
    // hreflang
    html.push_str(&format!(
        "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">\n",
        escape(&locale),
        args.url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(escape)
            .unwrap_or_else(|| "https://example.com".into())
    ));
    // JSON-LD
    match json_ld_kind.as_str() {
        "website" => {
            html.push_str(&format!(
                "<script type=\"application/ld+json\">\n{{\"@context\":\"https://schema.org\",\"@type\":\"WebSite\",\"name\":\"{}\"{}\n</script>\n",
                escape(&title),
                args.url
                    .as_deref()
                    .map(str::trim)
                    .filter(|u| !u.is_empty())
                    .map(|u| format!(",\"url\":\"{}\"", escape(u)))
                    .unwrap_or_default()
            ));
        }
        "article" => {
            html.push_str(&format!(
                "<script type=\"application/ld+json\">\n{{\"@context\":\"https://schema.org\",\"@type\":\"Article\",\"headline\":\"{}\"{}\n</script>\n",
                escape(&title),
                args.url
                    .as_deref()
                    .map(str::trim)
                    .filter(|u| !u.is_empty())
                    .map(|u| format!(",\"url\":\"{}\"", escape(u)))
                    .unwrap_or_default()
            ));
        }
        _ => {}
    }
    Ok(Output { html, warnings })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
