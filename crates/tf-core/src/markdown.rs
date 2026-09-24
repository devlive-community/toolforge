//! Markdown → 结构化块。前端按块用组件渲染，不执行任何 HTML，也无需前端解析库。

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph {
        spans: Vec<Span>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<Block>>,
    },
    Quote {
        blocks: Vec<Block>,
    },
    Code {
        text: String,
    },
    Rule,
}

/// 同一样式的一段行内文本
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub text: String,
    #[serde(skip_serializing_if = "is_false")]
    pub strong: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub em: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub strike: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub code: bool,
    /// 仅保留 http(s) 链接，其他协议按普通文本处理
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !value
}

enum Frame {
    Blocks(Vec<Block>),
    Quote(Vec<Block>),
    Item(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<Block>>,
    },
}

enum InlineKind {
    Paragraph,
    Heading(u8),
}

#[derive(Default)]
struct Style {
    strong: u32,
    em: u32,
    strike: u32,
    links: Vec<Option<String>>,
}

struct Builder {
    stack: Vec<Frame>,
    inline: Option<(InlineKind, Vec<Span>)>,
    code: Option<String>,
    style: Style,
}

fn safe_href(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")).then(|| url.to_owned())
}

/// 把纯文本中的裸 URL 拆成链接段（GitHub 的 release 说明常用裸链接）
fn autolink(span: Span) -> Vec<Span> {
    if span.code || span.href.is_some() {
        return vec![span];
    }
    let mut out = Vec::new();
    let mut rest = span.text.as_str();
    while let Some(start) = ["https://", "http://"]
        .iter()
        .filter_map(|p| rest.find(p))
        .min()
    {
        let tail = &rest[start..];
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';', ':', ')', '!', '?']);
        if start > 0 {
            out.push(Span {
                text: rest[..start].to_owned(),
                ..span.clone()
            });
        }
        out.push(Span {
            text: url.to_owned(),
            href: Some(url.to_owned()),
            ..span.clone()
        });
        rest = &rest[start + url.len()..];
    }
    if !rest.is_empty() {
        out.push(Span {
            text: rest.to_owned(),
            ..span
        });
    }
    out
}

impl Builder {
    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Frame::Blocks(blocks) | Frame::Quote(blocks) | Frame::Item(blocks)) => {
                blocks.push(block)
            }
            // 列表的直接子节点只会是列表项；兜底放进最后一项
            Some(Frame::List { items, .. }) => match items.last_mut() {
                Some(item) => item.push(block),
                None => items.push(vec![block]),
            },
            None => {}
        }
    }

    fn flush_inline(&mut self) {
        let Some((kind, spans)) = self.inline.take() else {
            return;
        };
        let spans: Vec<Span> = spans.into_iter().flat_map(autolink).collect();
        if spans.iter().all(|s| s.text.trim().is_empty()) {
            return;
        }
        self.push_block(match kind {
            InlineKind::Paragraph => Block::Paragraph { spans },
            InlineKind::Heading(level) => Block::Heading { level, spans },
        });
    }

    fn open_inline(&mut self, kind: InlineKind) {
        self.flush_inline();
        self.inline = Some((kind, Vec::new()));
    }

    fn push_text(&mut self, text: &str, code: bool) {
        if let Some(buffer) = self.code.as_mut() {
            buffer.push_str(text);
            return;
        }
        let span = Span {
            text: text.to_owned(),
            strong: self.style.strong > 0,
            em: self.style.em > 0,
            strike: self.style.strike > 0,
            code,
            href: self.style.links.last().cloned().flatten(),
        };
        // 紧凑列表项中的文本没有段落包裹，这里隐式开启一个段落
        let (_, spans) = self
            .inline
            .get_or_insert_with(|| (InlineKind::Paragraph, Vec::new()));
        match spans.last_mut() {
            Some(last)
                if !code
                    && !last.code
                    && (last.strong, last.em, last.strike, &last.href)
                        == (span.strong, span.em, span.strike, &span.href) =>
            {
                last.text.push_str(text)
            }
            _ => spans.push(span),
        }
    }

    fn pop(&mut self) -> Option<Frame> {
        self.flush_inline();
        self.stack.pop()
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.push_text(&text, false),
            Event::Code(text) => self.push_text(&text, true),
            Event::SoftBreak => self.push_text(" ", false),
            Event::HardBreak => self.push_text("\n", false),
            Event::Rule => {
                self.flush_inline();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => self.push_text(if done { "☑ " } else { "☐ " }, false),
            // 原始 HTML 一律丢弃，杜绝注入
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.open_inline(InlineKind::Paragraph),
            Tag::Heading { level, .. } => self.open_inline(InlineKind::Heading(level as u8)),
            Tag::BlockQuote(_) => {
                self.flush_inline();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::List(start) => {
                self.flush_inline();
                self.stack.push(Frame::List {
                    ordered: start.is_some(),
                    start: start.unwrap_or(1),
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush_inline();
                self.stack.push(Frame::Item(Vec::new()));
            }
            Tag::CodeBlock(_) => {
                self.flush_inline();
                self.code = Some(String::new());
            }
            Tag::Strong => self.style.strong += 1,
            Tag::Emphasis => self.style.em += 1,
            Tag::Strikethrough => self.style.strike += 1,
            Tag::Link { dest_url, .. } => self.style.links.push(safe_href(&dest_url)),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.flush_inline(),
            TagEnd::BlockQuote(_) => {
                if let Some(Frame::Quote(blocks)) = self.pop() {
                    self.push_block(Block::Quote { blocks });
                }
            }
            TagEnd::List(_) => {
                if let Some(Frame::List {
                    ordered,
                    start,
                    items,
                }) = self.pop()
                {
                    self.push_block(Block::List {
                        ordered,
                        start,
                        items,
                    });
                }
            }
            TagEnd::Item => {
                if let Some(Frame::Item(blocks)) = self.pop()
                    && let Some(Frame::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(blocks);
                }
            }
            TagEnd::CodeBlock => {
                if let Some(text) = self.code.take() {
                    self.push_block(Block::Code {
                        text: text.trim_end_matches('\n').to_owned(),
                    });
                }
            }
            TagEnd::Strong => self.style.strong = self.style.strong.saturating_sub(1),
            TagEnd::Emphasis => self.style.em = self.style.em.saturating_sub(1),
            TagEnd::Strikethrough => self.style.strike = self.style.strike.saturating_sub(1),
            TagEnd::Link => {
                self.style.links.pop();
            }
            _ => {}
        }
    }
}

pub fn parse(source: &str) -> Vec<Block> {
    let mut builder = Builder {
        stack: vec![Frame::Blocks(Vec::new())],
        inline: None,
        code: None,
        style: Style::default(),
    };
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(source, options) {
        builder.event(event);
    }
    builder.flush_inline();
    match builder.stack.into_iter().next() {
        Some(Frame::Blocks(blocks)) => blocks,
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "markdown_test.rs"]
mod tests;
