//! 解析 WebAssembly core module 的结构：section 布局与体积、类型、导入导出、函数、内存与表、
//! custom section（含 name / producers）。只读分析，不实例化、不执行。
//!
//! 解析遇到损坏的字节时停在出错位置：已读到的部分照常返回，错误带字节偏移。

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::Serialize;
use wasmparser::{
    CompositeInnerType, Encoding, ExternalKind, FuncType, KnownCustom, MemoryType, Name, Parser,
    Payload, TableType, TagType, TypeRef, ValType,
};

/// 内存页大小默认 64 KiB
pub const PAGE_SIZE: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// 错误码，前端翻译；具体原因保留在 message
    pub code: String,
    pub message: String,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: u8,
    /// type / import / … / custom / unknown
    pub kind: String,
    /// custom section 的名称
    pub name: Option<String>,
    /// section 起始偏移（含 id 与长度字段）
    pub offset: u64,
    /// section 总字节数（含 id 与长度字段）
    pub size: u64,
    /// 条目数（标准 section）
    pub items: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sizes {
    pub total: u64,
    /// 文件头 8 字节
    pub header: u64,
    pub code: u64,
    pub data: u64,
    pub custom: u64,
    /// 除 code / data / custom 外的标准 section
    pub other: u64,
    /// 不含 custom section 的模块体积
    pub without_custom: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub types: u32,
    pub imported_functions: u32,
    pub imported_tables: u32,
    pub imported_memories: u32,
    pub imported_globals: u32,
    pub imported_tags: u32,
    pub functions: u32,
    pub tables: u32,
    pub memories: u32,
    pub globals: u32,
    pub tags: u32,
    pub exports: u32,
    pub elements: u32,
    pub data: u32,
    pub start: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub index: u32,
    /// 导入时的 `模块.名称`
    pub import: Option<String>,
    pub min: u64,
    pub max: Option<u64>,
    /// 内存：每页字节数；表：元素类型
    pub page_size: Option<u64>,
    pub element: Option<String>,
    pub is64: bool,
    pub shared: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Producer {
    pub field: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    pub module: String,
    pub name: String,
    pub kind: &'static str,
    /// 在该类索引空间中的序号
    pub index: u32,
    pub desc: String,
    pub offset: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportRow {
    pub name: String,
    pub kind: &'static str,
    pub index: u32,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRow {
    /// 函数索引空间中的序号（导入函数在前）
    pub index: u32,
    pub name: Option<String>,
    pub export: Option<String>,
    pub signature: String,
    /// 函数体字节数（含局部变量声明）
    pub size: u64,
    pub offset: u64,
    pub locals: u32,
}

/// 解析结果
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub version: Option<u16>,
    pub sections: Vec<Section>,
    pub sizes: Sizes,
    pub counts: Counts,
    pub memories: Vec<Limits>,
    pub tables: Vec<Limits>,
    pub producers: Vec<Producer>,
    #[serde(skip)]
    pub types: Vec<String>,
    #[serde(skip)]
    pub imports: Vec<ImportRow>,
    #[serde(skip)]
    pub exports: Vec<ExportRow>,
    #[serde(skip)]
    pub functions: Vec<FunctionRow>,
    /// 解析在此处停止（损坏的模块）
    pub parse_error: Option<Problem>,
}

fn section_kind(id: u8) -> &'static str {
    match id {
        0 => "custom",
        1 => "type",
        2 => "import",
        3 => "function",
        4 => "table",
        5 => "memory",
        6 => "global",
        7 => "export",
        8 => "start",
        9 => "element",
        10 => "code",
        11 => "data",
        12 => "datacount",
        13 => "tag",
        _ => "unknown",
    }
}

fn val_list(types: &[ValType]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// 函数签名，如 `(param i32 i64) (result i32)`；无参数无返回值为 `()`
pub fn signature(ty: &FuncType) -> String {
    let mut out = String::new();
    if !ty.params().is_empty() {
        let _ = write!(out, "(param {})", val_list(ty.params()));
    }
    if !ty.results().is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "(result {})", val_list(ty.results()));
    }
    if out.is_empty() {
        out.push_str("()");
    }
    out
}

fn limits_text(min: u64, max: Option<u64>) -> String {
    match max {
        Some(max) => format!("{min}..{max}"),
        None => format!("{min}.."),
    }
}

fn memory_desc(m: &MemoryType) -> String {
    let mut out = limits_text(m.initial, m.maximum);
    out.push_str(if m.memory64 { " i64" } else { "" });
    if m.shared {
        out.push_str(" shared");
    }
    if let Some(log2) = m.page_size_log2 {
        let _ = write!(out, " page={}", 1u64 << log2);
    }
    out
}

fn table_desc(t: &TableType) -> String {
    format!(
        "{}{} {}",
        limits_text(t.initial, t.maximum),
        if t.table64 { " i64" } else { "" },
        t.element_type
    )
}

fn memory_limits(index: u32, import: Option<String>, m: &MemoryType) -> Limits {
    Limits {
        index,
        import,
        min: m.initial,
        max: m.maximum,
        page_size: Some(m.page_size_log2.map_or(PAGE_SIZE, |log2| 1u64 << log2)),
        element: None,
        is64: m.memory64,
        shared: m.shared,
    }
}

fn table_limits(index: u32, import: Option<String>, t: &TableType) -> Limits {
    Limits {
        index,
        import,
        min: t.initial,
        max: t.maximum,
        page_size: None,
        element: Some(t.element_type.to_string()),
        is64: t.table64,
        shared: t.shared,
    }
}

fn kind_name(kind: ExternalKind) -> &'static str {
    match kind {
        ExternalKind::Func | ExternalKind::FuncExact => "func",
        ExternalKind::Table => "table",
        ExternalKind::Memory => "memory",
        ExternalKind::Global => "global",
        ExternalKind::Tag => "tag",
    }
}

fn problem(code: &str, err: &wasmparser::BinaryReaderError) -> Problem {
    Problem {
        code: code.to_owned(),
        message: err.message().to_owned(),
        offset: Some(err.offset()),
    }
}

/// 解析状态：按索引空间累积
#[derive(Default)]
struct Builder {
    module: Module,
    /// 函数索引 → 类型索引
    func_types: Vec<u32>,
    global_descs: Vec<String>,
    names: HashMap<u32, String>,
    defined_seen: u32,
}

impl Builder {
    fn type_desc(&self, index: u32) -> String {
        self.module
            .types
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| format!("(type {index})"))
    }

    fn func_desc(&self, func: u32) -> String {
        self.func_types
            .get(func as usize)
            .map_or_else(|| format!("(func {func})"), |t| self.type_desc(*t))
    }

    fn tag_desc(&self, tag: &TagType) -> String {
        self.type_desc(tag.func_type_idx)
    }

    fn payload(&mut self, payload: Payload<'_>) -> Result<(), wasmparser::BinaryReaderError> {
        let m = &mut self.module;
        match payload {
            Payload::TypeSection(reader) => {
                for group in reader {
                    for sub in group?.into_types() {
                        let desc = match &sub.composite_type.inner {
                            CompositeInnerType::Func(f) => signature(f),
                            CompositeInnerType::Array(_) => "(array)".into(),
                            CompositeInnerType::Struct(s) => {
                                format!("(struct {} fields)", s.fields.len())
                            }
                            CompositeInnerType::Cont(_) => "(cont)".into(),
                        };
                        self.module.types.push(desc);
                    }
                }
                self.module.counts.types = self.module.types.len() as u32;
            }
            Payload::ImportSection(reader) => {
                for item in reader.into_imports_with_offsets() {
                    let (offset, import) = item?;
                    let place = Some(format!("{}.{}", import.module, import.name));
                    let c = &mut self.module.counts;
                    let (kind, index, desc) = match import.ty {
                        TypeRef::Func(t) | TypeRef::FuncExact(t) => {
                            self.func_types.push(t);
                            c.imported_functions += 1;
                            ("func", c.imported_functions - 1, self.type_desc(t))
                        }
                        TypeRef::Table(t) => {
                            c.imported_tables += 1;
                            let index = c.imported_tables - 1;
                            self.module.tables.push(table_limits(index, place, &t));
                            ("table", index, table_desc(&t))
                        }
                        TypeRef::Memory(mem) => {
                            c.imported_memories += 1;
                            let index = c.imported_memories - 1;
                            self.module.memories.push(memory_limits(index, place, &mem));
                            ("memory", index, memory_desc(&mem))
                        }
                        TypeRef::Global(g) => {
                            c.imported_globals += 1;
                            let desc = format!(
                                "{}{}",
                                if g.mutable { "mut " } else { "" },
                                g.content_type
                            );
                            self.global_descs.push(desc.clone());
                            ("global", c.imported_globals - 1, desc)
                        }
                        TypeRef::Tag(tag) => {
                            c.imported_tags += 1;
                            ("tag", c.imported_tags - 1, self.tag_desc(&tag))
                        }
                    };
                    self.module.imports.push(ImportRow {
                        module: import.module.to_owned(),
                        name: import.name.to_owned(),
                        kind,
                        index,
                        desc,
                        offset,
                    });
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    self.func_types.push(ty?);
                    m.counts.functions += 1;
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    let table = table?;
                    let index = m.counts.imported_tables + m.counts.tables;
                    m.tables.push(table_limits(index, None, &table.ty));
                    m.counts.tables += 1;
                }
            }
            Payload::MemorySection(reader) => {
                for mem in reader {
                    let mem = mem?;
                    let index = m.counts.imported_memories + m.counts.memories;
                    m.memories.push(memory_limits(index, None, &mem));
                    m.counts.memories += 1;
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let g = global?.ty;
                    self.global_descs.push(format!(
                        "{}{}",
                        if g.mutable { "mut " } else { "" },
                        g.content_type
                    ));
                    self.module.counts.globals += 1;
                }
            }
            Payload::TagSection(reader) => {
                for tag in reader {
                    tag?;
                    m.counts.tags += 1;
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export?;
                    let kind = kind_name(export.kind);
                    let desc = match kind {
                        "func" => self.func_desc(export.index),
                        "global" => self
                            .global_descs
                            .get(export.index as usize)
                            .cloned()
                            .unwrap_or_default(),
                        "memory" => self
                            .module
                            .memories
                            .iter()
                            .find(|l| l.index == export.index)
                            .map(|l| limits_text(l.min, l.max))
                            .unwrap_or_default(),
                        "table" => self
                            .module
                            .tables
                            .iter()
                            .find(|l| l.index == export.index)
                            .map(|l| {
                                format!(
                                    "{} {}",
                                    limits_text(l.min, l.max),
                                    l.element.as_deref().unwrap_or("")
                                )
                            })
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    self.module.exports.push(ExportRow {
                        name: export.name.to_owned(),
                        kind,
                        index: export.index,
                        desc,
                    });
                    self.module.counts.exports += 1;
                }
            }
            Payload::StartSection { func, .. } => m.counts.start = Some(func),
            Payload::ElementSection(reader) => m.counts.elements = reader.count(),
            Payload::DataSection(reader) => m.counts.data = reader.count(),
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                let mut locals = 0u32;
                for local in body.get_locals_reader()? {
                    locals = locals.saturating_add(local?.0);
                }
                let index = m.counts.imported_functions + self.defined_seen;
                self.defined_seen += 1;
                let signature = self.func_desc(index);
                self.module.functions.push(FunctionRow {
                    index,
                    name: None,
                    export: None,
                    signature,
                    size: range.end - range.start,
                    offset: range.start,
                    locals,
                });
            }
            Payload::CustomSection(reader) => match reader.as_known() {
                KnownCustom::Name(names) => {
                    for name in names {
                        // name section 损坏不影响模块本身，跳过即可
                        if let Ok(Name::Function(map)) = name {
                            for naming in map.into_iter().flatten() {
                                self.names.insert(naming.index, naming.name.to_owned());
                            }
                        }
                    }
                }
                KnownCustom::Producers(reader) => {
                    for field in reader.into_iter().flatten() {
                        let values = field
                            .values
                            .into_iter()
                            .flatten()
                            .map(|v| {
                                if v.version.is_empty() {
                                    v.name.to_owned()
                                } else {
                                    format!("{} {}", v.name, v.version)
                                }
                            })
                            .collect();
                        self.module.producers.push(Producer {
                            field: field.name.to_owned(),
                            values,
                        });
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn finish(mut self) -> Module {
        let mut exported: HashMap<u32, String> = HashMap::new();
        for e in self.module.exports.iter().filter(|e| e.kind == "func") {
            exported.entry(e.index).or_insert_with(|| e.name.clone());
        }
        for f in &mut self.module.functions {
            f.name = self.names.remove(&f.index);
            f.export = exported.get(&f.index).cloned();
        }
        self.module
    }
}

/// 解析模块；`cancelled` 在每个 payload 之间检查
pub fn analyze(bytes: &[u8], cancelled: &dyn Fn() -> bool) -> Result<Module, Problem> {
    if bytes.len() < 8 || &bytes[..4] != b"\0asm" {
        return Err(Problem {
            code: "wasm.bad_magic".into(),
            message: "missing \\0asm magic".into(),
            offset: Some(0),
        });
    }
    let mut b = Builder::default();
    b.module.sizes.total = bytes.len() as u64;
    b.module.sizes.header = 8;
    // section 紧密相连：每个 section 从上一个 section 的末尾开始
    let mut section_start = 8u64;
    for payload in Parser::new(0).parse_all(bytes) {
        if cancelled() {
            return Err(Problem {
                code: "task.cancelled".into(),
                message: String::new(),
                offset: None,
            });
        }
        let payload = match payload {
            Ok(payload) => payload,
            Err(err) => {
                b.module.parse_error = Some(problem("wasm.parse", &err));
                break;
            }
        };
        if let Payload::Version { num, encoding, .. } = &payload {
            if *encoding == Encoding::Component {
                return Err(Problem {
                    code: "wasm.component".into(),
                    message: String::new(),
                    offset: Some(4),
                });
            }
            b.module.version = Some(*num);
        }
        if let Some((id, range)) = payload.as_section() {
            let size = range.end - section_start;
            let (kind, name, items) = match &payload {
                Payload::CustomSection(c) => ("custom", Some(c.name().to_owned()), None),
                Payload::UnknownSection { id, .. } => (section_kind(*id), None, None),
                Payload::CodeSectionStart { count, .. } => ("code", None, Some(*count)),
                Payload::DataCountSection { count, .. } => ("datacount", None, Some(*count)),
                Payload::StartSection { .. } => ("start", None, None),
                _ => (section_kind(id), None, section_items(&payload)),
            };
            let sizes = &mut b.module.sizes;
            match kind {
                "custom" => sizes.custom += size,
                "code" => sizes.code += size,
                "data" => sizes.data += size,
                _ => sizes.other += size,
            }
            b.module.sections.push(Section {
                id,
                kind: kind.to_owned(),
                name,
                offset: section_start,
                size,
                items,
            });
            section_start = range.end;
        }
        if let Err(err) = b.payload(payload) {
            b.module.parse_error = Some(problem("wasm.parse", &err));
            break;
        }
    }
    let sizes = &mut b.module.sizes;
    sizes.without_custom = sizes.total - sizes.custom;
    Ok(b.finish())
}

fn section_items(payload: &Payload<'_>) -> Option<u32> {
    Some(match payload {
        Payload::TypeSection(r) => r.count(),
        Payload::ImportSection(r) => r.count(),
        Payload::FunctionSection(r) => r.count(),
        Payload::TableSection(r) => r.count(),
        Payload::MemorySection(r) => r.count(),
        Payload::GlobalSection(r) => r.count(),
        Payload::TagSection(r) => r.count(),
        Payload::ExportSection(r) => r.count(),
        Payload::ElementSection(r) => r.count(),
        Payload::DataSection(r) => r.count(),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
