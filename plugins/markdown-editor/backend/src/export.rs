//! 导出为独立 HTML 文件：内联样式，跟随系统浅色 / 深色主题。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::render::{Args, render};

const STYLE: &str = include_str!("export.css");

#[derive(Deserialize)]
pub struct ExportArgs {
    source: String,
    /// 文档标题；为空时使用第一个一级标题
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Exported {
    pub html: String,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn export_html(args: ExportArgs) -> PluginResult<Exported> {
    let rendered = render(Args {
        source: args.source,
    })?;
    let title = args
        .title
        .filter(|t| !t.trim().is_empty())
        .or(rendered.title)
        .unwrap_or_default();
    let html = format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"generator\" content=\"ToolForge\">\n<title>{}</title>\n\
         <style>\n{STYLE}</style>\n</head>\n<body>\n<article class=\"markdown-body\">\n{}</article>\n</body>\n</html>\n",
        escape(title.trim()),
        rendered.html
    );
    Ok(Exported { html })
}

#[cfg(test)]
#[path = "export_test.rs"]
mod tests;
