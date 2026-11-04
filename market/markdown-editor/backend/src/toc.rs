//! 目录生成与文内锚点校验：从 render 的 outline 生成缩进 TOC Markdown，
//! 并把文中 `[](#anchor)` 链接与标题 slug 集合比对。

use crate::render::{self};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub source: String,
    /// 目录深度 1–6（默认 3）
    #[serde(default)]
    pub max_depth: Option<u8>,
    /// 目录项是否带链接（默认 true）
    #[serde(default)]
    pub linked: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenAnchor {
    /// 所在行（从 1 开始）
    pub line: usize,
    pub anchor: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// 目录 Markdown（可直接插入文档）
    pub toc: String,
    pub entries: usize,
    /// 文内锚点链接总数
    pub anchor_count: usize,
    pub broken_anchors: Vec<BrokenAnchor>,
}

pub fn toc(args: Args) -> PluginResult<Output> {
    let max_depth = args.max_depth.unwrap_or(3).clamp(1, 6);
    let linked = args.linked.unwrap_or(true);
    if args.source.len() > 5 * 1024 * 1024 {
        return Err(PluginError::new("md.too_large").with("limit", "5 MB"));
    }
    let rendered = render::render(render::Args {
        source: args.source.clone(),
    })?;
    // 目录
    let mut toc = String::new();
    let mut entries = 0usize;
    for heading in &rendered.outline {
        if heading.level > max_depth {
            continue;
        }
        let indent = "  ".repeat((heading.level - 1) as usize);
        if linked {
            toc.push_str(&format!(
                "{}- [{}](#{})\n",
                indent, heading.text, heading.id
            ));
        } else {
            toc.push_str(&format!("{}- {}\n", indent, heading.text));
        }
        entries += 1;
    }
    // 锚点校验：标题 slug 集合
    let valid: std::collections::HashSet<String> =
        rendered.outline.iter().map(|h| h.id.clone()).collect();
    let mut broken = Vec::new();
    let mut anchor_count = 0usize;
    for (index, line) in args.source.lines().enumerate() {
        let mut rest = line;
        while let Some(start) = rest.find("](#") {
            let after = &rest[start + 3..];
            let Some(end) = after.find(')') else { break };
            let anchor = &after[..end];
            anchor_count += 1;
            if !valid.contains(anchor) {
                broken.push(BrokenAnchor {
                    line: index + 1,
                    anchor: anchor.to_owned(),
                });
            }
            rest = &after[end + 1..];
        }
    }
    Ok(Output {
        toc,
        entries,
        anchor_count,
        broken_anchors: broken,
    })
}

#[cfg(test)]
#[path = "toc_test.rs"]
mod tests;
