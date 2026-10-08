//! 按页面列表生成新 PDF：合并、提取、删除、重排与旋转都是它的特例。
//! 页面连同其引用的对象一起复制，页面树重建，未引用的对象被清除。

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::source::{self, INHERITABLE, Metadata, Source};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct PageRef {
    /// sources 中的序号
    pub source: usize,
    /// 页码，从 1 开始
    pub page: u32,
    /// 额外旋转（90 的倍数）
    #[serde(default)]
    pub rotate: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// 写入文档信息；为空时沿用第一个来源的信息
    #[serde(default)]
    pub metadata: Option<Metadata>,
    /// 压缩未压缩的内容流
    #[serde(default = "yes")]
    pub compress: bool,
}

fn yes() -> bool {
    true
}

impl Default for Options {
    fn default() -> Self {
        Self {
            metadata: None,
            compress: true,
        }
    }
}

/// 把来源文档的对象搬进输出文档，返回 页码 → 新的页面对象 id
fn import(out: &mut Document, mut doc: Document) -> BTreeMap<u32, ObjectId> {
    doc.renumber_objects_with(out.max_id + 1);
    let pages = doc.get_pages();
    // 页面脱离原页面树前，把继承的属性写到页面自身
    for page in pages.values() {
        let mut resolved = Vec::new();
        for key in INHERITABLE {
            if let Ok(dict) = doc.get_dictionary(*page)
                && !dict.has(key)
                && let Some(value) = source::inherited(&doc, *page, key)
            {
                resolved.push((key, value));
            }
        }
        if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(*page) {
            for (key, value) in resolved {
                dict.set(key, value);
            }
        }
    }
    out.max_id = out.max_id.max(doc.max_id);
    out.objects.extend(doc.objects);
    pages
}

fn info_object(metadata: &Metadata) -> Dictionary {
    let mut info = Dictionary::new();
    for (key, value) in [
        ("Title", &metadata.title),
        ("Author", &metadata.author),
        ("Subject", &metadata.subject),
        ("Keywords", &metadata.keywords),
        ("Creator", &metadata.creator),
    ] {
        if !value.trim().is_empty() {
            info.set(key, lopdf::text_string(value.trim()));
        }
    }
    let producer = if metadata.producer.trim().is_empty() {
        "ToolForge"
    } else {
        metadata.producer.trim()
    };
    info.set("Producer", lopdf::text_string(producer));
    info
}

/// 生成新文档（尚未写盘）
pub fn compose(sources: &[Source], pages: &[PageRef], options: &Options) -> PluginResult<Document> {
    if pages.is_empty() {
        return Err(PluginError::new("pdf.no_pages_selected"));
    }
    let mut out = Document::with_version("1.7");
    let mut imported: HashMap<usize, BTreeMap<u32, ObjectId>> = HashMap::new();
    let mut first_metadata = None;
    let mut version = String::from("1.4");
    for reference in pages {
        if imported.contains_key(&reference.source) {
            continue;
        }
        let source = sources.get(reference.source).ok_or_else(|| {
            PluginError::new("pdf.invalid_source").with("source", reference.source)
        })?;
        let doc = source::load(source)?;
        if first_metadata.is_none() {
            first_metadata = Some(source::metadata(&doc));
        }
        if doc.version > version {
            version = doc.version.clone();
        }
        let map = import(&mut out, doc);
        imported.insert(reference.source, map);
    }
    out.version = version;

    let pages_id = out.new_object_id();
    let mut kids = Vec::with_capacity(pages.len());
    let mut used = std::collections::HashSet::new();
    for reference in pages {
        let map = &imported[&reference.source];
        let original = *map.get(&reference.page).ok_or_else(|| {
            PluginError::new("pdf.page_out_of_range")
                .with("page", reference.page)
                .with("count", map.len())
        })?;
        // 同一页出现多次时复制页面字典（内容流共享），页面树中的节点必须唯一
        let id = if used.insert(original) {
            original
        } else {
            let copy = out
                .get_object(original)
                .map_err(|e| PluginError::new("pdf.invalid").with("detail", e.to_string()))?
                .clone();
            out.add_object(copy)
        };
        let dict = out
            .get_object_mut(id)
            .and_then(Object::as_dict_mut)
            .map_err(|e| PluginError::new("pdf.invalid").with("detail", e.to_string()))?;
        let base = dict.get(b"Rotate").and_then(Object::as_i64).unwrap_or(0);
        let rotate = (base + reference.rotate).rem_euclid(360);
        if rotate % 90 != 0 {
            return Err(PluginError::new("pdf.invalid_rotation"));
        }
        if rotate == 0 {
            dict.remove(b"Rotate");
        } else {
            dict.set("Rotate", rotate);
        }
        dict.set("Parent", pages_id);
        kids.push(Object::Reference(id));
    }
    let count = kids.len() as i64;
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", count);
    out.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", pages_id);
    let catalog_id = out.add_object(catalog);
    let metadata = options
        .metadata
        .clone()
        .or(first_metadata)
        .unwrap_or_default();
    let info_id = out.add_object(info_object(&metadata));
    out.trailer = Dictionary::new();
    out.trailer.set("Root", catalog_id);
    out.trailer.set("Info", info_id);
    out.prune_objects();
    if options.compress {
        out.compress();
    }
    Ok(out)
}

/// 写入临时文件后改名，失败时不留下半个文件
pub fn save(mut doc: Document, output: &Path) -> PluginResult<u64> {
    let temp = output.with_file_name(format!(
        ".{}.part",
        output
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let result = doc
        .save(&temp)
        .map(|_| ())
        .map_err(|e| PluginError::new("pdf.write_failed").with("detail", e.to_string()));
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    std::fs::rename(&temp, output)
        .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
    Ok(std::fs::metadata(output).map(|m| m.len()).unwrap_or(0))
}

/// 解析页码范围：`1-3, 5, 8-`（8- 表示到最后一页）
pub fn parse_ranges(text: &str, count: u32) -> PluginResult<Vec<(u32, u32)>> {
    let mut ranges = Vec::new();
    for part in text
        .split([',', '，', ';', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let invalid = || PluginError::new("pdf.invalid_range").with("range", part);
        let (start, end) = match part.split_once('-') {
            Some((a, b)) => {
                let start = if a.trim().is_empty() {
                    1
                } else {
                    a.trim().parse().map_err(|_| invalid())?
                };
                let end = if b.trim().is_empty() {
                    count
                } else {
                    b.trim().parse().map_err(|_| invalid())?
                };
                (start, end)
            }
            None => {
                let page = part.parse().map_err(|_| invalid())?;
                (page, page)
            }
        };
        if start == 0 || start > end || end > count {
            return Err(invalid().with("count", count));
        }
        ranges.push((start, end));
    }
    if ranges.is_empty() {
        return Err(PluginError::new("pdf.no_pages_selected"));
    }
    Ok(ranges)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum Split {
    /// 每个范围一个文件
    Ranges,
    /// 每 n 页一个文件
    #[serde(rename_all = "camelCase")]
    Every { size: u32 },
    /// 每页一个文件
    Single,
}

/// 计算拆分后的每一份（起止页码）
pub fn split_parts(split: Split, ranges: &str, count: u32) -> PluginResult<Vec<(u32, u32)>> {
    Ok(match split {
        Split::Ranges => parse_ranges(ranges, count)?,
        Split::Single => (1..=count).map(|p| (p, p)).collect(),
        Split::Every { size } => {
            if size == 0 {
                return Err(PluginError::new("pdf.invalid_range").with("range", "0"));
            }
            (0..count.div_ceil(size))
                .map(|i| (i * size + 1, ((i + 1) * size).min(count)))
                .collect()
        }
    })
}

/// 拆分输出的文件名：报告_p1-3.pdf，不覆盖已有文件
pub fn part_path(dir: &Path, stem: &str, (start, end): (u32, u32)) -> PathBuf {
    let label = if start == end {
        format!("p{start}")
    } else {
        format!("p{start}-{end}")
    };
    let mut candidate = dir.join(format!("{stem}_{label}.pdf"));
    let mut n = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}_{label} ({n}).pdf"));
        n += 1;
    }
    candidate
}

#[cfg(test)]
#[path = "compose_test.rs"]
mod tests;
