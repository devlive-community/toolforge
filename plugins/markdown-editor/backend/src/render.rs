//! Markdown 渲染：GFM 扩展（表格、任务列表、脚注、删除线、提示块），
//! 原始 HTML 按文本显示，不安全的链接协议被移除；同时生成目录与统计信息。

use std::collections::HashMap;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

pub const MAX_SOURCE: usize = 5 * 1024 * 1024;

#[derive(Deserialize)]
pub struct Args {
    pub source: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// 预览中的锚点
    pub id: String,
    /// 所在行（从 1 开始），用于在编辑器中跳转
    pub line: usize,
}

#[derive(Debug, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub chars: usize,
    /// 英文等按单词计，中日韩文字按字计
    pub words: usize,
    pub lines: usize,
    pub reading_minutes: u32,
    pub links: usize,
    pub images: usize,
    pub code_blocks: usize,
    pub tasks: usize,
    pub tasks_done: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub html: String,
    pub outline: Vec<Heading>,
    pub stats: Stats,
    /// 第一个一级标题
    pub title: Option<String>,
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_GFM
}

/// 链接是否可以保留：允许 http(s)、mailto、页内锚点与相对路径；图片额外允许位图 data URI
pub fn safe_url(url: &str, image: bool) -> bool {
    // 浏览器解析 URL 时会忽略其中的空白与控制字符
    let cleaned: String = url
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .flat_map(char::to_lowercase)
        .collect();
    match cleaned.find(':') {
        None => true,
        Some(i) if cleaned[..i].contains(['/', '?', '#']) => true,
        Some(i) => {
            matches!(&cleaned[..i], "http" | "https" | "mailto")
                || (image
                    && cleaned.starts_with("data:image/")
                    && !cleaned.starts_with("data:image/svg"))
        }
    }
}

/// 类似 GitHub 的标题锚点：小写，空白转为连字符，去掉标点
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        | 0xAC00..=0xD7AF | 0x20000..=0x2FA1F)
}

/// 返回（单词数，中日韩字数）
pub fn count_words(text: &str) -> (usize, usize) {
    let (mut words, mut cjk, mut in_word) = (0, 0, false);
    for c in text.chars() {
        if is_cjk(c) {
            cjk += 1;
            in_word = false;
        } else if c.is_alphanumeric() || (in_word && (c == '\'' || c == '’')) {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    (words, cjk)
}

fn reading_minutes(words: usize, cjk: usize) -> u32 {
    if words + cjk == 0 {
        return 0;
    }
    ((words as f64 / 230.0 + cjk as f64 / 400.0).ceil() as u32).max(1)
}

pub fn render(args: Args) -> PluginResult<Rendered> {
    let source = args.source.as_str();
    if source.len() > MAX_SOURCE {
        return Err(PluginError::new("md.too_large").with("limit", "5 MB"));
    }
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_of = |offset: usize| line_starts.partition_point(|&start| start <= offset);

    let mut events: Vec<(Event, std::ops::Range<usize>)> = Parser::new_ext(source, options())
        .into_offset_iter()
        .collect();
    let mut stats = Stats::default();
    let mut outline = Vec::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut plain = String::new();
    let mut in_metadata = false;

    // 原始 HTML 不执行，按文本显示（先转换，标题文字才完整）
    for (event, _) in events.iter_mut() {
        if let Event::Html(raw) | Event::InlineHtml(raw) = event {
            *event = Event::Text(std::mem::replace(raw, CowStr::Borrowed("")));
        }
    }

    for index in 0..events.len() {
        match &mut events[index].0 {
            Event::Start(Tag::Heading { .. }) => {
                let text: String = events[index + 1..]
                    .iter()
                    .take_while(|(e, _)| !matches!(e, Event::End(TagEnd::Heading(_))))
                    .filter_map(|(e, _)| match e {
                        Event::Text(t) | Event::Code(t) => Some(t.as_ref()),
                        _ => None,
                    })
                    .collect();
                let line = line_of(events[index].1.start);
                let Event::Start(Tag::Heading { level, id, .. }) = &mut events[index].0 else {
                    unreachable!()
                };
                let base = match id.as_deref() {
                    Some(explicit) if !explicit.is_empty() => explicit.to_owned(),
                    _ => Some(slug(&text))
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "section".to_owned()),
                };
                let seen = used.entry(base.clone()).or_insert(0);
                let unique = if *seen == 0 {
                    base
                } else {
                    format!("{base}-{seen}")
                };
                *seen += 1;
                *id = Some(CowStr::from(unique.clone()));
                outline.push(Heading {
                    level: *level as u8,
                    text: text.trim().to_owned(),
                    id: unique,
                    line,
                });
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                stats.links += 1;
                if !safe_url(dest_url, false) {
                    *dest_url = CowStr::Borrowed("");
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                stats.images += 1;
                if !safe_url(dest_url, true) {
                    *dest_url = CowStr::Borrowed("");
                }
            }
            Event::Start(Tag::CodeBlock(_)) => stats.code_blocks += 1,
            Event::Start(Tag::MetadataBlock(_)) => in_metadata = true,
            Event::End(TagEnd::MetadataBlock(_)) => in_metadata = false,
            Event::TaskListMarker(done) => {
                stats.tasks += 1;
                stats.tasks_done += usize::from(*done);
            }
            Event::Text(t) | Event::Code(t) if !in_metadata => plain.push_str(t),
            // 块与换行之间补空白，避免相邻段落的词连在一起
            Event::End(_) | Event::SoftBreak | Event::HardBreak => plain.push(' '),
            _ => {}
        }
    }

    let (words, cjk) = count_words(&plain);
    stats.words = words + cjk;
    stats.reading_minutes = reading_minutes(words, cjk);
    stats.chars = source.chars().count();
    stats.lines = if source.is_empty() {
        0
    } else {
        line_starts.len()
    };

    let mut html = String::with_capacity(source.len() * 3 / 2);
    html::push_html(&mut html, events.into_iter().map(|(event, _)| event));
    Ok(Rendered {
        html,
        title: outline
            .iter()
            .find(|h| h.level == 1 && !h.text.is_empty())
            .map(|h| h.text.clone()),
        outline,
        stats,
    })
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
