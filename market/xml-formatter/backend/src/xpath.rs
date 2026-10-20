//! 有界的 XPath 1.0 查询：原文解析、命名空间、类型结果与可取消求值。
use std::cell::Cell;
use std::time::{Duration, Instant};

use quick_xml::{Reader, events::Event};
use serde::{Deserialize, Serialize};
use sxd_document::parser;
use sxd_xpath::{Context, Factory, Value, nodeset::Node};
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

pub const MAX_INPUT: usize = 512 * 1024;
pub const MAX_RESULT: usize = 4 * 1024 * 1024;
pub const MAX_COPY: usize = 256 * 1024;
pub const MAX_NODES: usize = 8_000;
pub const PAGE_SIZE: usize = 20;
const MAX_DEPTH: usize = 64;
const MAX_EXPRESSION: usize = 2_048;
const MAX_PREVIEW: usize = 2_048;
const XML_URI: &str = "http://www.w3.org/XML/1998/namespace";
const FUNCTIONS: &[&str] = &[
    "last",
    "position",
    "count",
    "local-name",
    "namespace-uri",
    "name",
    "string",
    "concat",
    "starts-with",
    "contains",
    "substring-before",
    "substring-after",
    "substring",
    "string-length",
    "normalize-space",
    "translate",
    "boolean",
    "not",
    "true",
    "false",
    "number",
    "sum",
    "floor",
    "ceiling",
    "round",
    "text",
    "node",
    "comment",
    "processing-instruction",
];
const AXES: &[&str] = &[
    "child",
    "descendant",
    "parent",
    "ancestor",
    "following-sibling",
    "preceding-sibling",
    "following",
    "preceding",
    "attribute",
    "namespace",
    "self",
    "descendant-or-self",
    "ancestor-or-self",
];

#[derive(Clone, Deserialize, Serialize)]
pub struct Namespace {
    pub prefix: String,
    pub uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub input: String,
    pub expression: String,
    #[serde(default)]
    pub namespaces: Vec<Namespace>,
    #[serde(default)]
    pub revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub index: usize,
    pub kind: &'static str,
    pub path: String,
    pub value: String,
    /// 重新序列化的节点片段，并非原文摘录或源字符范围。
    pub fragment: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub handle: u64,
    pub revision: u64,
    pub expression: String,
    pub namespaces: Vec<Namespace>,
    pub kind: &'static str,
    pub scalar: Option<String>,
    pub items: Vec<Item>,
    pub elapsed_ms: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub index: usize,
    pub kind: &'static str,
    pub path: String,
    pub value: String,
    pub fragment: Option<String>,
    pub value_truncated: bool,
    pub fragment_truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub handle: u64,
    pub revision: u64,
    pub kind: &'static str,
    pub scalar: Option<String>,
    pub scalar_truncated: bool,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub items: Vec<Preview>,
    pub elapsed_ms: u128,
}

pub struct Limits {
    pub work: usize,
    pub elapsed: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            work: 2_000_000,
            elapsed: Duration::from_secs(3),
        }
    }
}

fn limit(name: &str) -> PluginError {
    PluginError::new("xpath.limit").with("limit", name)
}
fn detail(code: &str, value: impl ToString) -> PluginError {
    PluginError::new(code).with("detail", preview(&value.to_string()).0)
}
fn preview(value: &str) -> (String, bool) {
    let end = value.floor_char_boundary(MAX_PREVIEW.min(value.len()));
    (value[..end].to_owned(), end < value.len())
}

impl Report {
    pub fn page(&self, offset: usize, limit: usize) -> PluginResult<Page> {
        if limit == 0 || limit > PAGE_SIZE || offset > self.items.len() {
            return Err(PluginError::new("xpath.page_invalid"));
        }
        let items = self
            .items
            .iter()
            .skip(offset)
            .take(limit)
            .map(|item| {
                let (value, value_truncated) = preview(&item.value);
                let (fragment, fragment_truncated) = item
                    .fragment
                    .as_ref()
                    .map(|s| {
                        let (text, truncated) = preview(s);
                        (Some(text), truncated)
                    })
                    .unwrap_or((None, false));
                Preview {
                    index: item.index,
                    kind: item.kind,
                    path: item.path.clone(),
                    value,
                    fragment,
                    value_truncated,
                    fragment_truncated,
                }
            })
            .collect();
        let (scalar, scalar_truncated) = self
            .scalar
            .as_ref()
            .map(|s| {
                let (text, truncated) = preview(s);
                (Some(text), truncated)
            })
            .unwrap_or((None, false));
        Ok(Page {
            handle: self.handle,
            revision: self.revision,
            kind: self.kind,
            scalar,
            scalar_truncated,
            total: self.items.len(),
            offset,
            limit,
            items,
            elapsed_ms: self.elapsed_ms,
        })
    }

    pub fn copy(&self, index: Option<usize>, fragment: bool) -> PluginResult<&str> {
        let text = if let Some(index) = index {
            let item = self
                .items
                .get(index)
                .ok_or_else(|| PluginError::new("xpath.page_invalid"))?;
            if fragment {
                item.fragment
                    .as_deref()
                    .ok_or_else(|| PluginError::new("xpath.no_fragment"))?
            } else {
                &item.value
            }
        } else {
            self.scalar
                .as_deref()
                .ok_or_else(|| PluginError::new("xpath.page_invalid"))?
        };
        if text.len() > MAX_COPY {
            return Err(limit("copy"));
        }
        Ok(text)
    }
}

fn guard_xml(input: &str, ctx: &dyn TaskContext) -> PluginResult<()> {
    if input.trim().is_empty() {
        return Err(PluginError::new("xml.empty"));
    }
    if input.len() > MAX_INPUT {
        return Err(limit("input"));
    }
    let mut reader = Reader::from_str(input);
    let mut depth = 0usize;
    let mut nodes = 0usize;
    loop {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        match reader.read_event().map_err(|e| detail("xpath.xml", e))? {
            Event::DocType(_) => return Err(PluginError::new("xpath.dtd")),
            Event::Start(e) => {
                depth += 1;
                nodes += 1 + e.attributes().count();
            }
            Event::Empty(e) => nodes += 1 + e.attributes().count(),
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::Eof => break,
            _ => nodes += 1,
        }
        if depth > MAX_DEPTH {
            return Err(limit("depth"));
        }
        if nodes > MAX_NODES {
            return Err(limit("nodes"));
        }
    }
    Ok(())
}

fn name_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}
fn name_char(c: char) -> bool {
    name_start(c) || c.is_ascii_digit() || matches!(c, '-' | '.')
}

/// 只检查能力与资源限制；完整表达式语法仍由查询引擎解析。
fn guard_expression(expression: &str, namespaces: &[Namespace]) -> PluginResult<()> {
    if expression.len() > MAX_EXPRESSION {
        return Err(limit("expression"));
    }
    if namespaces.len() > 32 {
        return Err(limit("namespaces"));
    }
    let mut prefixes = std::collections::HashSet::new();
    for ns in namespaces {
        if ns.prefix.is_empty()
            || !ns.prefix.chars().next().is_some_and(name_start)
            || !ns.prefix.chars().all(name_char)
            || ns.uri.is_empty()
            || ns.uri.len() > 2_048
            || ns.prefix == "xmlns"
            || (ns.prefix == "xml" && ns.uri != XML_URI)
            || ns.uri == "http://www.w3.org/2000/xmlns/"
            || (ns.uri == XML_URI && ns.prefix != "xml")
            || !prefixes.insert(ns.prefix.as_str())
        {
            return Err(
                PluginError::new("xpath.namespace_invalid").with("prefix", ns.prefix.as_str())
            );
        }
    }
    let chars: Vec<char> = expression.chars().collect();
    let mut i = 0;
    let mut depth = 0usize;
    let mut tokens = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' || ch == '"' {
            i += 1;
            while i < chars.len() && chars[i] != ch {
                i += 1;
            }
            if i == chars.len() {
                return Err(detail("xpath.syntax", "unterminated literal"));
            }
        } else if ch == '$' {
            return Err(PluginError::new("xpath.unsupported").with("name", "variables"));
        } else if ch == '(' || ch == '[' {
            depth += 1;
            if depth > 32 {
                return Err(limit("expression_depth"));
            }
        } else if ch == ')' || ch == ']' {
            depth = depth.saturating_sub(1);
        } else if name_start(ch) {
            let begin = i;
            while i < chars.len() && name_char(chars[i]) {
                i += 1;
            }
            let first: String = chars[begin..i].iter().collect();
            if chars.get(i) == Some(&':') && chars.get(i + 1) == Some(&':') {
                if !AXES.contains(&first.as_str()) {
                    return Err(PluginError::new("xpath.unsupported").with("name", first.as_str()));
                }
                i += 2;
            } else if chars.get(i) == Some(&':') {
                if first != "xml" && !prefixes.contains(first.as_str()) {
                    return Err(
                        PluginError::new("xpath.unbound_prefix").with("prefix", first.as_str())
                    );
                }
                i += 1;
                while i < chars.len() && name_char(chars[i]) {
                    i += 1;
                }
            }
            tokens += 1;
            if tokens > 256 {
                return Err(limit("expression"));
            }
            continue;
        }
        i += 1;
    }
    for name in Factory::new()
        .function_names(expression)
        .map_err(|error| detail("xpath.syntax", error))?
    {
        if !FUNCTIONS.contains(&name.as_str()) {
            return Err(PluginError::new("xpath.unsupported").with("name", name));
        }
    }
    Ok(())
}

fn escape(text: &str, attribute: bool) -> String {
    let mut text = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if attribute {
        text = text
            .replace('"', "&quot;")
            .replace('\n', "&#10;")
            .replace('\r', "&#13;")
            .replace('\t', "&#9;");
    }
    text
}
fn serialize(node: Node<'_>, out: &mut String) {
    match node {
        Node::Root(_) => {
            for child in node.children() {
                serialize(child, out);
            }
        }
        Node::Element(element) => {
            let name = node
                .prefixed_name()
                .unwrap_or_else(|| element.name().local_part().to_owned());
            out.push('<');
            out.push_str(&name);
            if element.name().namespace_uri().is_none() {
                out.push_str(" xmlns=\"\"");
            }
            let mut namespaces = element.namespaces_in_scope();
            namespaces.sort_by(|a, b| a.prefix().cmp(b.prefix()));
            for ns in namespaces {
                if ns.prefix() == "xml"
                    || (ns.prefix().is_empty() && element.name().namespace_uri().is_none())
                {
                    continue;
                }
                out.push_str(" xmlns");
                if !ns.prefix().is_empty() {
                    out.push(':');
                    out.push_str(ns.prefix());
                }
                out.push_str("=\"");
                out.push_str(&escape(ns.uri(), true));
                out.push('"');
            }
            for attribute in element.attributes() {
                let attr: Node<'_> = attribute.into();
                out.push(' ');
                out.push_str(
                    &attr
                        .prefixed_name()
                        .unwrap_or_else(|| attribute.name().local_part().to_owned()),
                );
                out.push_str("=\"");
                out.push_str(&escape(attribute.value(), true));
                out.push('"');
            }
            if element.children().is_empty() {
                out.push_str("/>");
            } else {
                out.push('>');
                for child in node.children() {
                    serialize(child, out);
                }
                out.push_str("</");
                out.push_str(&name);
                out.push('>');
            }
        }
        Node::Text(text) => out.push_str(&escape(text.text(), false)),
        Node::Comment(comment) => {
            out.push_str("<!--");
            out.push_str(comment.text());
            out.push_str("-->");
        }
        Node::ProcessingInstruction(pi) => {
            out.push_str("<?");
            out.push_str(pi.target());
            if let Some(value) = pi.value() {
                out.push(' ');
                out.push_str(value);
            }
            out.push_str("?>");
        }
        _ => {}
    }
}
fn node_path(node: Node<'_>) -> String {
    if matches!(node, Node::Root(_)) {
        return "/".to_owned();
    }
    let parent = node.parent();
    let prefix = parent.map(node_path).unwrap_or_default();
    let separator = if prefix == "/" { "" } else { "/" };
    let step = match node {
        Node::Element(e) => {
            let siblings = parent.map(|p| p.children()).unwrap_or_default();
            let position = siblings
                .iter()
                .filter(|n| matches!(n, Node::Element(x) if x.name()==e.name()))
                .position(|n| *n == node)
                .unwrap_or(0)
                + 1;
            format!(
                "Q{{{}}}{}[{}]",
                e.name().namespace_uri().unwrap_or(""),
                e.name().local_part(),
                position
            )
        }
        Node::Attribute(a) => format!(
            "@Q{{{}}}{}",
            a.name().namespace_uri().unwrap_or(""),
            a.name().local_part()
        ),
        Node::Namespace(ns) => format!("namespace::{}", ns.prefix()),
        _ => {
            let label = match node {
                Node::Text(_) => "text()",
                Node::Comment(_) => "comment()",
                _ => "processing-instruction()",
            };
            let siblings = parent.map(|p| p.children()).unwrap_or_default();
            let position = siblings
                .iter()
                .filter(|n| std::mem::discriminant(*n) == std::mem::discriminant(&node))
                .position(|n| *n == node)
                .unwrap_or(0)
                + 1;
            format!("{label}[{position}]")
        }
    };
    format!("{prefix}{separator}{step}")
}
fn kind(node: Node<'_>) -> &'static str {
    match node {
        Node::Root(_) => "root",
        Node::Element(_) => "element",
        Node::Attribute(_) => "attribute",
        Node::Text(_) => "text",
        Node::Comment(_) => "comment",
        Node::ProcessingInstruction(_) => "processingInstruction",
        Node::Namespace(_) => "namespace",
    }
}

pub fn query(args: Args, ctx: &dyn TaskContext, limits: Limits) -> PluginResult<Report> {
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    guard_xml(&args.input, ctx)?;
    guard_expression(&args.expression, &args.namespaces)?;
    let start = Instant::now();
    let remaining = Cell::new(limits.work);
    let stopped = Cell::new(false);
    let cancelled_state = Cell::new(false);
    let package = parser::parse(&args.input).map_err(|e| detail("xpath.xml", e))?;
    let document = package.as_document();
    let mut context = Context::new();
    context.set_node_limit(MAX_NODES);
    context.set_namespace("xml", XML_URI);
    for ns in &args.namespaces {
        context.set_namespace(&ns.prefix, &ns.uri);
    }
    context.set_evaluation_check(|work| {
        if stopped.get() {
            return false;
        }
        if remaining.get() < work || start.elapsed() > limits.elapsed {
            stopped.set(true);
            return false;
        }
        if ctx.is_cancelled() {
            cancelled_state.set(true);
            stopped.set(true);
            return false;
        }
        remaining.set(remaining.get() - work);
        true
    });
    ctx.stage("xpath.evaluating");
    let compiled = Factory::new()
        .build(&args.expression)
        .map_err(|e| detail("xpath.syntax", e))?
        .ok_or_else(|| PluginError::new("xpath.expression_empty"))?;
    let result = compiled.evaluate(&context, document.root());
    if cancelled_state.get() || ctx.is_cancelled() {
        return Err(cancelled());
    }
    if stopped.get() {
        return Err(limit("work"));
    }
    let value = result.map_err(|e| detail("xpath.evaluate", e))?;
    let mut report = Report {
        handle: 0,
        revision: args.revision,
        expression: args.expression,
        namespaces: args.namespaces,
        kind: "nodeset",
        scalar: None,
        items: Vec::new(),
        elapsed_ms: 0,
    };
    match value {
        Value::Nodeset(nodes) => {
            if nodes.size() > MAX_NODES {
                return Err(limit("results"));
            }
            let mut bytes = 0usize;
            for (index, node) in nodes.document_order().into_iter().enumerate() {
                if ctx.is_cancelled() {
                    return Err(cancelled());
                }
                if start.elapsed() > limits.elapsed {
                    return Err(limit("work"));
                }
                let value = node.string_value();
                let path = node_path(node);
                let fragment = if matches!(
                    node,
                    Node::Root(_)
                        | Node::Element(_)
                        | Node::Comment(_)
                        | Node::ProcessingInstruction(_)
                ) {
                    let mut text = String::new();
                    serialize(node, &mut text);
                    Some(text)
                } else {
                    None
                };
                bytes = bytes.saturating_add(
                    value.len() + path.len() + fragment.as_ref().map_or(0, String::len),
                );
                if bytes > MAX_RESULT {
                    return Err(limit("results"));
                }
                report.items.push(Item {
                    index,
                    kind: kind(node),
                    path,
                    value,
                    fragment,
                });
            }
        }
        Value::String(text) => {
            report.kind = "string";
            report.scalar = Some(text);
        }
        Value::Boolean(value) => {
            report.kind = "boolean";
            report.scalar = Some(value.to_string());
        }
        Value::Number(value) => {
            report.kind = "number";
            report.scalar = Some(if value.is_nan() {
                "NaN".into()
            } else if value == f64::INFINITY {
                "Infinity".into()
            } else if value == f64::NEG_INFINITY {
                "-Infinity".into()
            } else if value == 0.0 && value.is_sign_negative() {
                "-0".into()
            } else {
                value.to_string()
            });
        }
    }
    if report.scalar.as_ref().is_some_and(|s| s.len() > MAX_RESULT) {
        return Err(limit("results"));
    }
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    report.elapsed_ms = start.elapsed().as_millis();
    Ok(report)
}

#[cfg(test)]
#[path = "xpath_test.rs"]
mod tests;
