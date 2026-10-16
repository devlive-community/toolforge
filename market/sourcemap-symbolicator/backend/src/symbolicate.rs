//! 堆栈帧还原：把生成位置映射回源文件、行列与符号名，附源码上下文与逐帧诊断。
//!
//! 规则：产物与 map 的关联由用户显式给出（默认取 map 的 `file` 字段），不凭
//! 同名猜测；帧的行列来自浏览器堆栈（1 基），map 内部为 0 基，输出统一转成
//! 1 基；生成列按 UTF-16 code unit 计量。缺列不伪造精确位置，缺失映射的帧
//! 保留原文并给出原因。

use serde::Serialize;
use tf_plugin_api::PluginError;

use crate::sourcemap::{Diag, SourceMapData};
use crate::stack::{self, Frame};

/// 上下文行数：目标行上下各取多少行
pub const CONTEXT_LINES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Stats {
    pub resolved: usize,
    pub no_mapping: usize,
    pub unrelated: usize,
    pub unassociated: usize,
    pub no_column: usize,
    pub messages: usize,
}

#[derive(Debug, Serialize)]
pub struct ContextLine {
    /// 1 基源码行号
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Context {
    pub lines: Vec<ContextLine>,
    /// 目标行在 `lines` 里的下标
    pub target: usize,
}

#[derive(Debug, Serialize)]
pub struct SectionPoint {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Serialize)]
pub struct Resolved {
    /// 原始文件名（sources 里的原始串；匿名来源为 null）
    pub source: Option<String>,
    /// 来源在 sources 数组里的下标
    pub source_index: u32,
    /// 1 基源码行
    pub line: u32,
    /// 1 基源码列（UTF-16 code unit）
    pub column: u32,
    pub name: Option<String>,
    /// 命中的 section 偏移（index map 时才有）
    pub section: Option<SectionPoint>,
    pub context: Option<Context>,
}

#[derive(Debug, Serialize)]
pub struct FrameOut {
    pub index: usize,
    pub raw: String,
    /// "frame" 参与还原；"message" 原样保留
    pub kind: &'static str,
    pub function: Option<String>,
    pub url: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// resolved | no_mapping | unrelated | unassociated | no_column | message
    pub status: &'static str,
    pub reason: Option<Diag>,
    pub resolved: Option<Resolved>,
}

#[derive(Debug, Serialize)]
pub struct Symbolication {
    pub asset: Option<String>,
    pub frames: Vec<FrameOut>,
    pub diagnostics: Vec<Diag>,
    pub stats: Stats,
}

fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match trimmed.rsplit_once(['/', '\\']) {
        Some((_, last)) => last,
        None => trimmed,
    }
}

/// 生成指向某列（UTF-16 code unit，0 基）的插入符行；列按 UTF-16 计量，
/// 超出行宽时收敛到行尾
fn caret(text: &str, column_utf16: u32) -> String {
    let width = text.chars().map(char::len_utf16).sum::<usize>();
    let column = (column_utf16 as usize).min(width);
    format!("{}^", " ".repeat(column))
}

fn build_context(content: &str, src_line: u32) -> Context {
    let lines: Vec<&str> = content
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let target = src_line as usize;
    let start = target.saturating_sub(CONTEXT_LINES);
    let end = (target + CONTEXT_LINES + 1).min(lines.len());
    let lines = (start..end)
        .map(|index| ContextLine {
            line: index as u32 + 1,
            text: lines[index].to_owned(),
        })
        .collect();
    Context {
        lines,
        target: target - start,
    }
}

pub fn run(
    data: &SourceMapData,
    stack: Option<&str>,
    asset: Option<&str>,
    position: Option<(u64, u64)>,
) -> Result<Symbolication, PluginError> {
    if stack.is_none() && position.is_none() {
        return Err(PluginError::new("sourcemap.no_input"));
    }
    let asset = asset
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| data.file.as_deref().map(basename).map(str::to_owned));
    let mut diagnostics = Vec::new();
    let mut frames: Vec<FrameOut> = Vec::new();
    let raw_frames: Vec<Frame> = match stack {
        Some(text) => {
            let parsed = stack::parse(text, crate::MAX_FRAMES);
            if parsed.truncated {
                diagnostics
                    .push(Diag::new("sourcemap.too_many_frames").param("max", crate::MAX_FRAMES));
            }
            parsed.frames
        }
        None => vec![Frame {
            raw: format!("{}:{}", position.unwrap().0, position.unwrap().1),
            is_frame: true,
            function: None,
            // 单位置模式由用户显式指定，关联产物直接取默认值
            url: asset.as_deref().map(str::to_owned),
            line: Some(position.unwrap().0.min(u32::MAX as u64) as u32),
            column: Some(position.unwrap().1.min(u32::MAX as u64) as u32),
        }],
    };
    let mut stats = Stats {
        resolved: 0,
        no_mapping: 0,
        unrelated: 0,
        unassociated: 0,
        no_column: 0,
        messages: 0,
    };
    for frame in raw_frames {
        if !frame.is_frame {
            stats.messages += 1;
            frames.push(FrameOut {
                index: frames.len(),
                raw: frame.raw,
                kind: "message",
                function: None,
                url: None,
                line: None,
                column: None,
                status: "message",
                reason: None,
                resolved: None,
            });
            continue;
        }
        let index = frames.len();
        let Some(asset_name) = asset.as_deref() else {
            frames.push(pending(
                &mut stats,
                index,
                &frame,
                "unassociated",
                Some(Diag::new("sourcemap.frame_unassociated")),
            ));
            continue;
        };
        let url = frame.url.as_deref().unwrap_or_default();
        if basename(url) != asset_name {
            frames.push(pending(
                &mut stats,
                index,
                &frame,
                "unrelated",
                Some(Diag::new("sourcemap.frame_unrelated").param("url", basename(url))),
            ));
            continue;
        }
        let (Some(line), Some(column)) = (frame.line, frame.column) else {
            frames.push(pending(
                &mut stats,
                index,
                &frame,
                "no_column",
                Some(Diag::new("sourcemap.frame_no_column")),
            ));
            continue;
        };
        if line == 0 || column == 0 {
            frames.push(pending(
                &mut stats,
                index,
                &frame,
                "no_mapping",
                Some(Diag::new("sourcemap.frame_invalid_position")),
            ));
            continue;
        }
        match data.resolve(line - 1, column - 1) {
            Some(hit) => {
                stats.resolved += 1;
                let context = hit
                    .content
                    .map(|content| build_context(content, hit.src_line));
                frames.push(FrameOut {
                    index,
                    raw: frame.raw.clone(),
                    kind: "frame",
                    function: frame.function.clone(),
                    url: frame.url.clone(),
                    line: frame.line,
                    column: frame.column,
                    status: "resolved",
                    reason: None,
                    resolved: Some(Resolved {
                        source: hit.source.map(str::to_owned),
                        source_index: hit.source_index.unwrap_or(u32::MAX),
                        line: hit.src_line + 1,
                        column: hit.src_col + 1,
                        name: hit.name.map(str::to_owned),
                        section: hit
                            .section
                            .map(|(line, column)| SectionPoint { line, column }),
                        context,
                    }),
                });
            }
            None => {
                frames.push(pending(
                    &mut stats,
                    index,
                    &frame,
                    "no_mapping",
                    Some(Diag::new("sourcemap.frame_no_mapping")),
                ));
            }
        }
    }
    Ok(Symbolication {
        asset,
        frames,
        diagnostics,
        stats,
    })
}

/// 未命中的帧：保留原始信息并附原因
fn pending(
    stats: &mut Stats,
    index: usize,
    frame: &Frame,
    status: &'static str,
    reason: Option<Diag>,
) -> FrameOut {
    match status {
        "no_mapping" => stats.no_mapping += 1,
        "unrelated" => stats.unrelated += 1,
        "unassociated" => stats.unassociated += 1,
        "no_column" => stats.no_column += 1,
        _ => {}
    }
    FrameOut {
        index,
        raw: frame.raw.clone(),
        kind: "frame",
        function: frame.function.clone(),
        url: frame.url.clone(),
        line: frame.line,
        column: frame.column,
        status,
        reason,
        resolved: None,
    }
}

/// Markdown 报告：帧对照、说明与源码上下文（插入符按 UTF-16 列对齐）
pub fn markdown(symbolication: &Symbolication) -> String {
    let mut out = String::new();
    out.push_str("# Source Map Symbolication\n\n");
    out.push_str(&format!(
        "- Asset: {}\n- Frames: {} (resolved {}, no mapping {}, unrelated {}, unassociated {}, without column {}, messages {})\n\n",
        symbolication.asset.as_deref().unwrap_or("-"),
        symbolication.frames.len(),
        symbolication.stats.resolved,
        symbolication.stats.no_mapping,
        symbolication.stats.unrelated,
        symbolication.stats.unassociated,
        symbolication.stats.no_column,
        symbolication.stats.messages,
    ));
    for frame in &symbolication.frames {
        out.push_str(&format!(
            "## Frame {}\n\n```text\n{}\n```\n\n",
            frame.index + 1,
            frame.raw
        ));
        match (frame.status, frame.resolved.as_ref()) {
            ("resolved", Some(resolved)) => {
                let source = resolved.source.as_deref().unwrap_or("(anonymous source)");
                out.push_str(&format!(
                    "- Source: `{}`\n- Position: {}:{}\n",
                    source, resolved.line, resolved.column
                ));
                if let Some(name) = &resolved.name {
                    out.push_str(&format!("- Name: `{name}`\n"));
                }
                if let Some(section) = &resolved.section {
                    out.push_str(&format!("- Section: {}:{}\n", section.line, section.column));
                }
                if let Some(context) = &resolved.context {
                    out.push_str("\n```text\n");
                    for (offset, line) in context.lines.iter().enumerate() {
                        out.push_str(&format!("{:>5} | {}\n", line.line, line.text));
                        if offset == context.target {
                            let column = resolved.column.saturating_sub(1);
                            let text = &line.text;
                            out.push_str(&format!(
                                "      | {}{}\n",
                                caret(text, column),
                                "\u{21d2}"
                            ));
                        }
                    }
                    out.push_str("```\n");
                }
                out.push('\n');
            }
            ("message", _) => {}
            (status, _) => {
                let reason = frame
                    .reason
                    .as_ref()
                    .map(|diag| diag.code.as_str())
                    .unwrap_or("sourcemap.frame_unresolved");
                out.push_str(&format!("- Status: `{status}`\n- Reason: `{reason}`\n\n"));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "symbolicate_test.rs"]
mod tests;
