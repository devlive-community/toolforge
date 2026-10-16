//! Source Map v3 文档：解析 JSON、校验字段、解码 mappings，支持常规映射与
//! 内嵌 sections 的 index map。
//!
//! 位置语义（ECMA-426）：mappings 内部行、列均为 0 基；生成的列按 UTF-16
//! code unit 计数。对外展示时由调用方统一 +1 转成 1 基。

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::PluginError;

use crate::vlq;

/// 粘贴文本上限
pub const MAX_TEXT: usize = 32 * 1024 * 1024;
/// 打开文件的大小上限
pub const MAX_FILE: u64 = 128 * 1024 * 1024;
/// 解码后的段数上限
pub const MAX_SEGMENTS: usize = 4_000_000;
pub const MAX_SOURCES: usize = 200_000;
pub const MAX_NAMES: usize = 1_000_000;

/// 非致命问题的说明（逐条 / 逐段），code 由前端 i18n，params 提供插值
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Diag {
    pub code: String,
    #[serde(default)]
    pub params: Value,
}

impl Diag {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            params: json!({}),
        }
    }

    /// 追加一个插值参数
    pub fn param(mut self, key: &str, value: impl Into<Value>) -> Self {
        if let Value::Object(params) = &mut self.params {
            params.insert(key.to_owned(), value.into());
        }
        self
    }
}

/// 反序列化视图：字段全部宽容处理（缺失 / null 在构建阶段判断）
#[derive(Deserialize)]
pub struct RawMap {
    #[serde(default)]
    pub version: Option<Value>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default, rename = "sourceRoot")]
    pub source_root: Option<String>,
    #[serde(default)]
    pub sources: Option<Vec<Option<String>>>,
    #[serde(default, rename = "sourcesContent")]
    pub sources_content: Option<Vec<Option<String>>>,
    #[serde(default)]
    pub names: Option<Vec<String>>,
    #[serde(default)]
    pub mappings: Option<String>,
    #[serde(default)]
    pub sections: Option<Vec<RawSection>>,
}

#[derive(Deserialize)]
pub struct RawSection {
    pub offset: RawOffset,
    #[serde(default)]
    pub map: Option<RawMap>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Deserialize)]
pub struct RawOffset {
    #[serde(default)]
    pub line: i64,
    #[serde(default)]
    pub column: i64,
}

/// 一条映射：生成位置（行由所在分组决定）与原始位置、可选名称索引
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub gen_col: u32,
    pub source: Option<u32>,
    pub src_line: u32,
    pub src_col: u32,
    pub name: Option<u32>,
}

/// 一份常规（非 index）映射解码后的完整数据
#[derive(Debug)]
pub struct RegularMap {
    pub sources: Vec<Option<String>>,
    pub sources_content: Vec<Option<String>>,
    pub names: Vec<String>,
    /// 按生成行分组；空行是空 Vec
    pub lines: Vec<Vec<Segment>>,
    pub segments: usize,
    pub mapped_lines: usize,
    /// 每个来源命中的段数（与 sources 同序）
    pub per_source: Vec<usize>,
}

#[derive(Debug)]
pub struct SectionData {
    pub offset_line: u32,
    pub offset_column: u32,
    pub map: RegularMap,
    pub url: Option<String>,
}

#[derive(Debug)]
pub enum Kind {
    Regular(RegularMap),
    Index(Vec<SectionData>),
}

#[derive(Debug)]
pub struct SourceMapData {
    pub file: Option<String>,
    pub source_root: Option<String>,
    pub kind: Kind,
    pub segments: usize,
    pub diagnostics: Vec<Diag>,
}

/// 数量限制；测试用更小的值
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_segments: usize,
    pub max_sources: usize,
    pub max_names: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_segments: MAX_SEGMENTS,
            max_sources: MAX_SOURCES,
            max_names: MAX_NAMES,
        }
    }
}

fn check_version(raw: &RawMap, required: bool) -> Result<(), PluginError> {
    let ok = matches!(
        &raw.version,
        Some(Value::Number(n)) if n.as_i64() == Some(3)
    ) || matches!(&raw.version, Some(Value::String(s)) if s == "3");
    match (ok, raw.version.is_some() || required) {
        (true, _) | (false, false) => Ok(()),
        (false, true) => Err(PluginError::new("sourcemap.unsupported_version")),
    }
}

fn offset(section: &RawSection, index: usize) -> Result<(u32, u32), PluginError> {
    let (line, column) = (section.offset.line, section.offset.column);
    if line < 0 || column < 0 || line > u32::MAX as i64 || column > u32::MAX as i64 {
        return Err(PluginError::new("sourcemap.invalid_offset").with("index", index));
    }
    Ok((line as u32, column as u32))
}

/// 解析粘贴文本（UTF-8）
pub fn parse_text(text: &str) -> Result<SourceMapData, PluginError> {
    parse_text_with(text, Limits::default())
}

/// 用自定义数量限制解析（测试用小限制）
pub fn parse_text_with(text: &str, limits: Limits) -> Result<SourceMapData, PluginError> {
    let raw: RawMap = serde_json::from_str(text)
        .map_err(|e| PluginError::new("sourcemap.invalid_json").with("detail", e.to_string()))?;
    build_top(raw, &limits)
}

/// 解析文件字节（load 任务读入后调用）
pub fn parse_slice(bytes: &[u8]) -> Result<SourceMapData, PluginError> {
    let raw: RawMap = serde_json::from_slice(bytes)
        .map_err(|e| PluginError::new("sourcemap.invalid_json").with("detail", e.to_string()))?;
    build_top(raw, &Limits::default())
}

fn build_top(mut raw: RawMap, limits: &Limits) -> Result<SourceMapData, PluginError> {
    check_version(&raw, true)?;
    let file = raw.file.take();
    let source_root = raw.source_root.take();
    let mut diagnostics = Vec::new();
    let (kind, segments) = match raw.sections.take() {
        Some(sections) => {
            let mut out = Vec::new();
            for (index, section) in sections.into_iter().enumerate() {
                let (offset_line, offset_column) = offset(&section, index)?;
                match section.map {
                    Some(inner) => {
                        check_version(&inner, false)?;
                        out.push(SectionData {
                            offset_line,
                            offset_column,
                            map: build_regular(inner, limits)?,
                            url: section.url,
                        });
                    }
                    None => {
                        // 外部 sections URL 不自动加载，只留下诊断
                        match section.url {
                            Some(url) => diagnostics
                                .push(Diag::new("sourcemap.section_url_skipped").param("url", url)),
                            None => diagnostics
                                .push(Diag::new("sourcemap.section_empty").param("index", index)),
                        }
                    }
                }
            }
            out.sort_by_key(|s| (s.offset_line, s.offset_column));
            let segments = out.iter().map(|s| s.map.segments).sum();
            (Kind::Index(out), segments)
        }
        None => {
            let map = build_regular(raw, limits)?;
            let segments = map.segments;
            (Kind::Regular(map), segments)
        }
    };
    Ok(SourceMapData {
        file,
        source_root,
        kind,
        segments,
        diagnostics,
    })
}

fn build_regular(raw: RawMap, limits: &Limits) -> Result<RegularMap, PluginError> {
    if raw.sections.is_some() {
        return Err(PluginError::new("sourcemap.nested_sections"));
    }
    let sources = raw.sources.unwrap_or_default();
    if sources.len() > limits.max_sources {
        return Err(PluginError::new("sourcemap.too_many_sources").with("max", limits.max_sources));
    }
    let names = raw.names.unwrap_or_default();
    if names.len() > limits.max_names {
        return Err(PluginError::new("sourcemap.too_many_names").with("max", limits.max_names));
    }
    let mappings = raw
        .mappings
        .ok_or_else(|| PluginError::new("sourcemap.mappings_missing"))?;
    let sources_content = raw.sources_content.unwrap_or_default();
    let lines = decode_mappings(&mappings, sources.len(), names.len(), limits)?;
    let mut per_source = vec![0usize; sources.len()];
    let mut segments = 0usize;
    let mut mapped_lines = 0usize;
    for segs in &lines {
        if segs.iter().any(|s| s.source.is_some()) {
            mapped_lines += 1;
        }
        for segment in segs {
            segments += 1;
            if let Some(source) = segment.source {
                per_source[source as usize] += 1;
            }
        }
    }
    Ok(RegularMap {
        sources,
        sources_content,
        names,
        lines,
        segments,
        mapped_lines,
        per_source,
    })
}

fn decode_mappings(
    mappings: &str,
    sources_len: usize,
    names_len: usize,
    limits: &Limits,
) -> Result<Vec<Vec<Segment>>, PluginError> {
    let invalid =
        |detail: Value| PluginError::new("sourcemap.invalid_mappings").with("detail", detail);
    let mut lines = Vec::new();
    let mut src_idx: i64 = 0;
    let mut src_line: i64 = 0;
    let mut src_col: i64 = 0;
    let mut name_idx: i64 = 0;
    let mut total = 0usize;
    for group in mappings.split(';') {
        let mut segments = Vec::new();
        let mut gen_col: i64 = 0;
        for part in group.split(',') {
            if part.is_empty() {
                continue;
            }
            let bytes = part.as_bytes();
            let mut fields = [0i64; 5];
            let mut count = 0usize;
            let mut at = 0usize;
            while at < bytes.len() {
                if count == 5 {
                    return Err(invalid(json!({ "fields": count })));
                }
                let (value, used) = vlq::decode(bytes, at)
                    .map_err(|e| invalid(json!({ "column": at, "kind": format!("{e:?}") })))?;
                fields[count] = value;
                count += 1;
                at += used;
            }
            let unsigned = |value: i64, field: &str| {
                if value < 0 {
                    Err(invalid(json!({ "field": field, "value": value })))
                } else {
                    Ok(value as u32)
                }
            };
            gen_col += fields[0];
            let gen_col = unsigned(gen_col, "genCol")?;
            let segment = match count {
                1 => Segment {
                    gen_col,
                    source: None,
                    src_line: 0,
                    src_col: 0,
                    name: None,
                },
                4 | 5 => {
                    src_idx += fields[1];
                    src_line += fields[2];
                    src_col += fields[3];
                    let src_idx = unsigned(src_idx, "source")?;
                    let src_line = unsigned(src_line, "srcLine")?;
                    let src_col = unsigned(src_col, "srcColumn")?;
                    if src_idx as usize >= sources_len {
                        return Err(PluginError::new("sourcemap.index_out_of_range")
                            .with("field", "sources")
                            .with("index", src_idx));
                    }
                    let name = if count == 5 {
                        name_idx += fields[4];
                        let name_idx = unsigned(name_idx, "name")?;
                        if name_idx as usize >= names_len {
                            return Err(PluginError::new("sourcemap.index_out_of_range")
                                .with("field", "names")
                                .with("index", name_idx));
                        }
                        Some(name_idx)
                    } else {
                        None
                    };
                    Segment {
                        gen_col,
                        source: Some(src_idx),
                        src_line,
                        src_col,
                        name,
                    }
                }
                _ => return Err(invalid(json!({ "fields": count }))),
            };
            total += 1;
            if total > limits.max_segments {
                return Err(PluginError::new("sourcemap.too_many_segments")
                    .with("max", limits.max_segments));
            }
            segments.push(segment);
        }
        lines.push(segments);
    }
    Ok(lines)
}

/// 命中一条映射时的原始位置
pub struct Hit<'a> {
    pub source: Option<&'a str>,
    /// 来源在 sources 数组里的下标
    pub source_index: Option<u32>,
    pub src_line: u32,
    pub src_col: u32,
    pub name: Option<&'a str>,
    pub section: Option<(u32, u32)>,
    /// 命中来源的 sourcesContent（可能缺失）
    pub content: Option<&'a str>,
}

impl RegularMap {
    /// 在生成行上找覆盖 `column` 的段：最后一个 gen_col ≤ column 的段；
    /// 第一段之前的位置没有映射。
    pub fn resolve(&self, line: u32, column: u32) -> Option<&Segment> {
        let segments = self.lines.get(line as usize)?;
        let point = segments.partition_point(|s| s.gen_col <= column);
        let index = point.checked_sub(1)?;
        Some(&segments[index])
    }

    pub fn source_name(&self, index: u32) -> Option<&str> {
        self.sources.get(index as usize)?.as_deref()
    }

    pub fn source_content(&self, index: u32) -> Option<&str> {
        self.sources_content.get(index as usize)?.as_deref()
    }

    pub fn name(&self, index: u32) -> Option<&str> {
        self.names.get(index as usize).map(String::as_str)
    }
}

impl SourceMapData {
    /// 来源总数（index map 时为各 section 之和）
    pub fn sources_len(&self) -> usize {
        match &self.kind {
            Kind::Regular(map) => map.sources.len(),
            Kind::Index(list) => list.iter().map(|s| s.map.sources.len()).sum(),
        }
    }

    /// 把生成位置（0 基行、0 基列）还原为原始位置
    pub fn resolve(&self, line: u32, column: u32) -> Option<Hit<'_>> {
        match &self.kind {
            Kind::Regular(map) => map.resolve(line, column).and_then(|s| hit(map, s, None)),
            Kind::Index(sections) => {
                let index = sections
                    .partition_point(|s| (s.offset_line, s.offset_column) <= (line, column))
                    .checked_sub(1)?;
                let section = &sections[index];
                // 位置不能到达下一个 section 的起点
                if let Some(next) = sections.get(index + 1)
                    && (line, column) >= (next.offset_line, next.offset_column)
                {
                    return None;
                }
                let rel_line = line - section.offset_line;
                let rel_column = if rel_line == 0 {
                    column.checked_sub(section.offset_column)?
                } else {
                    column
                };
                section.map.resolve(rel_line, rel_column).and_then(|s| {
                    hit(
                        &section.map,
                        s,
                        Some((section.offset_line, section.offset_column)),
                    )
                })
            }
        }
    }
}

/// 来源概览
#[derive(serde::Serialize)]
pub struct SourceInfo {
    pub name: Option<String>,
    pub has_content: bool,
    pub segments: usize,
}

/// section 概览
#[derive(serde::Serialize)]
pub struct SectionInfo {
    pub index: usize,
    pub offset_line: u32,
    pub offset_column: u32,
    pub has_map: bool,
    pub url: Option<String>,
    pub sources: usize,
    pub segments: usize,
}

/// 解析后的整体概览（含引用句柄，供后续调用使用）
#[derive(serde::Serialize)]
pub struct Overview {
    pub handle: u64,
    /// regular | index
    pub kind: &'static str,
    pub file: Option<String>,
    pub source_root: Option<String>,
    pub sources: Vec<SourceInfo>,
    pub names: usize,
    pub segments: usize,
    pub mapped_lines: usize,
    pub sections: Option<Vec<SectionInfo>>,
    pub diagnostics: Vec<Diag>,
}

pub fn overview(data: &SourceMapData, handle: u64) -> Overview {
    let (kind, sources, mapped_lines, sections, names) = match &data.kind {
        Kind::Regular(map) => (
            "regular",
            map.sources
                .iter()
                .enumerate()
                .map(|(index, name)| SourceInfo {
                    name: name.clone(),
                    has_content: map
                        .sources_content
                        .get(index)
                        .and_then(|content| content.as_ref())
                        .is_some(),
                    segments: map.per_source[index],
                })
                .collect(),
            map.mapped_lines,
            None,
            map.names.len(),
        ),
        Kind::Index(list) => (
            "index",
            Vec::new(),
            list.iter().map(|s| s.map.mapped_lines).sum(),
            Some(
                list.iter()
                    .enumerate()
                    .map(|(index, section)| SectionInfo {
                        index,
                        offset_line: section.offset_line,
                        offset_column: section.offset_column,
                        has_map: true,
                        url: section.url.clone(),
                        sources: section.map.sources.len(),
                        segments: section.map.segments,
                    })
                    .collect(),
            ),
            list.iter().map(|s| s.map.names.len()).sum(),
        ),
    };
    Overview {
        handle,
        kind,
        file: data.file.clone(),
        source_root: data.source_root.clone(),
        sources,
        names,
        segments: data.segments,
        mapped_lines,
        sections,
        diagnostics: data.diagnostics.clone(),
    }
}

fn hit<'a>(map: &'a RegularMap, segment: &Segment, section: Option<(u32, u32)>) -> Option<Hit<'a>> {
    Some(Hit {
        source: segment.source.and_then(|i| map.source_name(i)),
        source_index: segment.source,
        src_line: segment.src_line,
        src_col: segment.src_col,
        name: segment.name.and_then(|i| map.name(i)),
        section,
        content: segment.source.and_then(|i| map.source_content(i)),
    })
}

#[cfg(test)]
#[path = "sourcemap_test.rs"]
mod tests;
