//! robots.txt 与 sitemap.xml 生成与校验后端。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct RobotsSitemap {
    manifest: Manifest,
}

impl Default for RobotsSitemap {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for RobotsSitemap {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "robots" => to_value(robots(parse_args(args)?)?),
            "sitemap" => to_value(sitemap(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(RobotsSitemap);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotsArgs {
    /// 规则组：UA + 规则（Allow/Disallow）+ 可选 Crawl-delay
    pub groups: Vec<Group>,
    /// Sitemap: 行的 URL 列表
    #[serde(default)]
    pub sitemaps: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub user_agent: String,
    /// "allow:/path" 或 "disallow:/path" 前缀的规则
    pub rules: Vec<String>,
    #[serde(default)]
    pub crawl_delay: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotsOutput {
    pub text: String,
    pub groups: usize,
}

pub fn robots(args: RobotsArgs) -> PluginResult<RobotsOutput> {
    if args.groups.is_empty() {
        return Err(PluginError::new("robots.no_groups"));
    }
    let mut text = String::new();
    for group in &args.groups {
        let ua = group.user_agent.trim();
        if ua.is_empty() {
            return Err(PluginError::new("robots.empty_ua"));
        }
        text.push_str(&format!("User-agent: {ua}\n"));
        for rule in &group.rules {
            let (kind, path) = rule
                .split_once(':')
                .ok_or_else(|| PluginError::new("robots.bad_rule").with("rule", rule.trim()))?;
            let kind = kind.trim().to_lowercase();
            if kind != "allow" && kind != "disallow" {
                return Err(PluginError::new("robots.bad_rule").with("rule", rule.trim()));
            }
            text.push_str(&format!(
                "{}: {}\n",
                kind[0..1].to_uppercase() + &kind[1..],
                path.trim()
            ));
        }
        if let Some(delay) = group.crawl_delay {
            text.push_str(&format!("Crawl-delay: {delay}\n"));
        }
        text.push('\n');
    }
    for sitemap in &args.sitemaps {
        text.push_str(&format!("Sitemap: {}\n", sitemap.trim()));
    }
    Ok(RobotsOutput {
        text,
        groups: args.groups.len(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapArgs {
    /// URL 列表
    pub urls: Vec<String>,
    /// lastmod（ISO 日期，可空）
    #[serde(default)]
    pub lastmod: Option<String>,
    /// always/hourly/daily/weekly/monthly/yearly/never
    #[serde(default)]
    pub changefreq: Option<String>,
    /// 0.0–1.0
    #[serde(default)]
    pub priority: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapOutput {
    /// 主 sitemap XML（URL ≤ 50000 时为单个文件）
    pub xml: String,
    /// URL 超过 50000 时的分片数
    pub shards: usize,
    /// 校验警告（URL 非法等）
    pub warnings: Vec<String>,
}

const SITEMAP_URL_LIMIT: usize = 50_000;

pub fn sitemap(args: SitemapArgs) -> PluginResult<SitemapOutput> {
    if args.urls.is_empty() {
        return Err(PluginError::new("sitemap.no_urls"));
    }
    let mut warnings = Vec::new();
    let mut valid_urls = Vec::new();
    for url in &args.urls {
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            warnings.push(format!("not_absolute:{url}"));
            continue;
        }
        if url.contains(' ') || url.contains('<') || url.contains('>') {
            warnings.push(format!("invalid_chars:{url}"));
            continue;
        }
        valid_urls.push(url.to_owned());
    }
    if valid_urls.is_empty() {
        return Err(PluginError::new("sitemap.no_valid_urls"));
    }
    if let Some(freq) = &args.changefreq
        && !matches!(
            freq.as_str(),
            "always" | "hourly" | "daily" | "weekly" | "monthly" | "yearly" | "never"
        )
    {
        return Err(PluginError::new("sitemap.bad_changefreq").with("value", freq.as_str()));
    }
    if let Some(priority) = args.priority
        && !(0.0..=1.0).contains(&priority)
    {
        return Err(PluginError::new("sitemap.bad_priority").with("value", priority));
    }
    let shards = valid_urls.len().div_ceil(SITEMAP_URL_LIMIT);
    // 首版输出首个分片（≤50000 条）
    let first: Vec<&String> = valid_urls.iter().take(SITEMAP_URL_LIMIT).collect();
    let mut xml = String::with_capacity(valid_urls.len() * 120);
    xml.push_str(r#"<?xml version='1.0' encoding='UTF-8'?>"#);
    xml.push_str("\n<urlset xmlns='http://www.sitemaps.org/schemas/sitemap/0.9'>\n");
    for url in first {
        xml.push_str("  <url>\n    <loc>");
        // XML 转义
        let escaped = url
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        xml.push_str(&escaped);
        xml.push_str("</loc>\n");
        if let Some(lastmod) = &args.lastmod {
            xml.push_str(&format!("    <lastmod>{}</lastmod>\n", lastmod.trim()));
        }
        if let Some(freq) = &args.changefreq {
            xml.push_str(&format!("    <changefreq>{freq}</changefreq>\n"));
        }
        if let Some(priority) = args.priority {
            xml.push_str(&format!("    <priority>{priority}</priority>\n"));
        }
        xml.push_str("  </url>\n");
    }
    xml.push_str("</urlset>");
    Ok(SitemapOutput {
        xml,
        shards,
        warnings,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
