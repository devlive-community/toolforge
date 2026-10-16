//! 把报告中的源文件路径映射到本地源码根下的文件。
//!
//! 规则：先按「CI 路径前缀 → 本地目录」映射（取最长的匹配前缀）；映射后仍为相对路径的，拼到源码根下；
//! 绝对路径必须位于源码根内。路径只做字面规范化（. 与 ..），越出源码根的一律拒绝。
//! Windows 路径（盘符、反斜杠）与 POSIX 路径都按 / 统一比较，盘符不区分大小写。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub path: String,
    pub root: String,
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    /// 本地文件路径（使用源码根的分隔符风格）
    pub path: String,
    /// 命中的映射前缀
    pub mapping: Option<String>,
}

/// 盘符或 UNC 前缀 + 路径段
#[derive(Debug, Clone, PartialEq)]
struct Parsed {
    /// "/"、"c:/" 或 "//server/share/"；相对路径为空
    anchor: String,
    segments: Vec<String>,
}

fn parse(path: &str) -> Parsed {
    let unified = path.trim().replace('\\', "/");
    let (anchor, rest) = if let Some(rest) = unified.strip_prefix("//") {
        // UNC：//server/share/...
        let mut parts = rest.splitn(3, '/');
        let server = parts.next().unwrap_or_default();
        let share = parts.next().unwrap_or_default();
        (
            format!("//{server}/{share}/").to_lowercase(),
            parts.next().unwrap_or_default().to_owned(),
        )
    } else if unified.len() >= 2
        && unified.as_bytes()[1] == b':'
        && unified.as_bytes()[0].is_ascii_alphabetic()
    {
        (
            format!("{}:/", unified[..1].to_ascii_lowercase()),
            unified[2..].trim_start_matches('/').to_owned(),
        )
    } else if let Some(rest) = unified.strip_prefix('/') {
        ("/".to_owned(), rest.to_owned())
    } else {
        (String::new(), unified)
    };
    Parsed {
        anchor,
        segments: rest
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .map(str::to_owned)
            .collect(),
    }
}

/// 规范化 ..；越过开头时返回 None
fn collapse(segments: &[String]) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for segment in segments {
        if segment == ".." {
            out.pop()?;
        } else {
            out.push(segment.clone());
        }
    }
    Some(out)
}

fn same_segments(a: &[String], b: &[String], case_insensitive: bool) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            if case_insensitive {
                x.eq_ignore_ascii_case(y)
            } else {
                x == y
            }
        })
}

fn starts_with(path: &Parsed, prefix: &Parsed) -> bool {
    path.anchor == prefix.anchor
        && path.segments.len() >= prefix.segments.len()
        && same_segments(
            &path.segments[..prefix.segments.len()],
            &prefix.segments,
            !path.anchor.is_empty() && path.anchor != "/",
        )
}

pub fn resolve(args: &Args) -> PluginResult<Resolved> {
    let root = parse(&args.root);
    if root.anchor.is_empty() {
        return Err(PluginError::new("lcov.root_not_absolute"));
    }
    let root_segments =
        collapse(&root.segments).ok_or_else(|| PluginError::new("lcov.root_not_absolute"))?;
    let root = Parsed {
        anchor: root.anchor,
        segments: root_segments,
    };

    let source = parse(&args.path);
    // 最长前缀优先
    let mut mapped: Option<(Parsed, String)> = None;
    let mut best = 0usize;
    for mapping in &args.mappings {
        let from = parse(&mapping.from);
        if from.segments.is_empty() && from.anchor.is_empty() {
            continue;
        }
        if starts_with(&source, &from) && from.segments.len() + 1 > best {
            best = from.segments.len() + 1;
            let to = parse(&mapping.to);
            let mut segments = to.segments.clone();
            segments.extend_from_slice(&source.segments[from.segments.len()..]);
            mapped = Some((
                Parsed {
                    anchor: to.anchor,
                    segments,
                },
                mapping.from.clone(),
            ));
        }
    }
    let (target, mapping) = match mapped {
        Some((parsed, from)) => (parsed, Some(from)),
        None => (source, None),
    };
    let absolute = if target.anchor.is_empty() {
        let mut segments = root.segments.clone();
        segments.extend(target.segments);
        Parsed {
            anchor: root.anchor.clone(),
            segments,
        }
    } else {
        target
    };
    let segments = collapse(&absolute.segments)
        .ok_or_else(|| PluginError::new("lcov.outside_root").with("path", args.path.as_str()))?;
    let normalized = Parsed {
        anchor: absolute.anchor,
        segments,
    };
    if !starts_with(&normalized, &root) {
        return Err(PluginError::new("lcov.outside_root").with("path", args.path.as_str()));
    }

    // 按源码根的写法输出：保留其盘符大小写与分隔符
    let windows = args.root.contains('\\');
    let separator = if windows { "\\" } else { "/" };
    let root_text = args.root.trim().trim_end_matches(['/', '\\']);
    let relative = &normalized.segments[root.segments.len()..];
    let path = if relative.is_empty() {
        root_text.to_owned()
    } else {
        format!("{root_text}{separator}{}", relative.join(separator))
    };
    Ok(Resolved { path, mapping })
}

#[cfg(test)]
#[path = "paths_test.rs"]
mod tests;
