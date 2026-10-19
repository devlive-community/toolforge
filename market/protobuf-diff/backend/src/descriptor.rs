//! FileDescriptorSet 解码：按 descriptor.proto 的标准字段号把 wire bytes 还原为
//! 结构化描述符。只支持 proto3；proto2/Editions 的文件标记为不支持而非丢弃，
//! 避免缺失文件被误报为“兼容”。依赖完整性、重复符号在此层校验。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::wire::{Reader, WT_LEN, WT_VARINT};

/// 上限：描述符文件 4 MB、文件数 256、符号 2000、嵌套深度 32
pub const MAX_BYTES: usize = 4 << 20;
pub const MAX_FILES: usize = 256;
pub const MAX_SYMBOLS: usize = 2000;
pub const MAX_DEPTH: usize = 32;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 字段标签：1=optional（含 proto3 隐式 presence）2=required（proto2）3=repeated
pub const LABEL_OPTIONAL: i64 = 1;
pub const LABEL_REQUIRED: i64 = 2;
pub const LABEL_REPEATED: i64 = 3;

/// 字段类型编号（descriptor.proto 标准）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldType {
    Double = 1,
    Float = 2,
    Int64 = 3,
    Uint64 = 4,
    Int32 = 5,
    Fixed64 = 6,
    Fixed32 = 7,
    Bool = 8,
    String = 9,
    Group = 10,
    Message = 11,
    Bytes = 12,
    Uint32 = 13,
    Enum = 14,
    Sfixed32 = 15,
    Sfixed64 = 16,
    Sint32 = 17,
    Sint64 = 18,
}

impl FieldType {
    pub fn from_code(code: i64) -> Option<FieldType> {
        Some(match code {
            1 => FieldType::Double,
            2 => FieldType::Float,
            3 => FieldType::Int64,
            4 => FieldType::Uint64,
            5 => FieldType::Int32,
            6 => FieldType::Fixed64,
            7 => FieldType::Fixed32,
            8 => FieldType::Bool,
            9 => FieldType::String,
            10 => FieldType::Group,
            11 => FieldType::Message,
            12 => FieldType::Bytes,
            13 => FieldType::Uint32,
            14 => FieldType::Enum,
            15 => FieldType::Sfixed32,
            16 => FieldType::Sfixed64,
            17 => FieldType::Sint32,
            18 => FieldType::Sint64,
            _ => return None,
        })
    }

    /// wire class：varint 组 / 64-bit 组 / 32-bit 组 / 长度前缀组
    pub fn wire_class(self) -> WireClass {
        match self {
            FieldType::Int32
            | FieldType::Int64
            | FieldType::Uint32
            | FieldType::Uint64
            | FieldType::Bool
            | FieldType::Enum
            | FieldType::Sint32
            | FieldType::Sint64 => WireClass::Varint,
            FieldType::Fixed64 | FieldType::Sfixed64 | FieldType::Double => WireClass::Bits64,
            FieldType::Fixed32 | FieldType::Sfixed32 | FieldType::Float => WireClass::Bits32,
            FieldType::String | FieldType::Bytes | FieldType::Message | FieldType::Group => {
                WireClass::LengthDelimited
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WireClass {
    Varint,
    Bits64,
    Bits32,
    LengthDelimited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDesc {
    pub name: String,
    pub number: i64,
    pub label: i64,
    /// 原始类型码；Group 视为不支持
    pub type_code: i64,
    /// 规范化后的完整类型名（消息/枚举），原始类型为空
    pub type_name: String,
    /// 所属 oneof 的名称（proto3 optional 不算 oneof 成员）
    pub oneof: String,
    pub proto3_optional: bool,
    pub json_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDesc {
    pub name: String,
    /// 含包名的完整名
    pub full_name: String,
    pub fields: Vec<FieldDesc>,
    /// oneof 声明名（供未使用检查）
    pub oneofs: Vec<String>,
    /// reserved 字段号区间 [start, end)，end 为开区间上界（proto 语义 +1）
    pub reserved_ranges: Vec<(i64, i64)>,
    pub reserved_names: Vec<String>,
    pub nested: Vec<MessageDesc>,
    pub enums: Vec<EnumDesc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumDesc {
    pub name: String,
    pub full_name: String,
    /// (名称, 数值)，按描述符顺序
    pub values: Vec<(String, i64)>,
    pub reserved_ranges: Vec<(i64, i64)>,
    pub reserved_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodDesc {
    pub name: String,
    pub input_type: String,
    pub output_type: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDesc {
    pub name: String,
    pub full_name: String,
    pub methods: Vec<MethodDesc>,
}

/// 源码位置：descriptor 逻辑路径（字段号链）与行列
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub path: Vec<i64>,
    /// [start_line, start_col, end_line, end_col]，0 起
    pub span: Vec<i64>,
    pub leading_comments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDesc {
    pub name: String,
    pub package: String,
    pub dependencies: Vec<String>,
    pub syntax: String,
    /// proto2 / editions 等不支持的语法标记
    pub unsupported_syntax: Option<String>,
    pub messages: Vec<MessageDesc>,
    pub enums: Vec<EnumDesc>,
    pub services: Vec<ServiceDesc>,
    /// 按逻辑路径（如 [4,0,2,1] = 第 4 个 message 的第 0 个…）索引的位置信息
    pub locations: Vec<SourceLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileSet {
    pub files: Vec<FileDesc>,
}

impl FileSet {
    /// 全部符号（消息/枚举/服务完整名）→ 所在文件名，供依赖检查与类型引用
    pub fn symbol_table(&self) -> BTreeMap<String, String> {
        let mut table = BTreeMap::new();
        for file in &self.files {
            let prefix = if file.package.is_empty() {
                String::new()
            } else {
                format!("{}.", file.package)
            };
            for message in &file.messages {
                collect_message_symbols(&prefix, message, &file.name, &mut table);
            }
            for enum_desc in &file.enums {
                table.insert(format!("{prefix}{}", enum_desc.name), file.name.clone());
            }
            for service in &file.services {
                table.insert(format!("{prefix}{}", service.name), file.name.clone());
            }
        }
        table
    }

    /// 缺失的依赖文件名
    pub fn missing_dependencies(&self) -> Vec<String> {
        let known: Vec<&str> = self.files.iter().map(|f| f.name.as_str()).collect();
        let mut missing = Vec::new();
        for file in &self.files {
            for dep in &file.dependencies {
                if !known.contains(&dep.as_str()) && !missing.contains(dep) {
                    missing.push(dep.clone());
                }
            }
        }
        missing
    }
}

fn collect_message_symbols(
    prefix: &str,
    message: &MessageDesc,
    file: &str,
    table: &mut BTreeMap<String, String>,
) {
    let full = format!("{prefix}{}", message.name);
    table.insert(full.clone(), file.to_owned());
    let child_prefix = format!("{full}.");
    for nested in &message.nested {
        collect_message_symbols(&child_prefix, nested, file, table);
    }
    for enum_desc in &message.enums {
        table.insert(format!("{child_prefix}{}", enum_desc.name), file.to_owned());
    }
}

/// json_name 默认规则：snake_case → lowerCamelCase（protoc 同款）
pub fn default_json_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

// ---- 解码 ----

struct Parsing {
    symbols: usize,
}

impl Parsing {
    fn budget(&mut self) -> PluginResult<()> {
        self.symbols += 1;
        if self.symbols > MAX_SYMBOLS {
            return Err(PluginError::new("pbd.too_many_symbols").with("max", MAX_SYMBOLS));
        }
        Ok(())
    }
}

/// 解析 FileDescriptorSet（FileDescriptorProto repeated field 1）
pub fn parse_file_set(data: &[u8]) -> PluginResult<FileSet> {
    if data.len() > MAX_BYTES {
        return Err(PluginError::new("pbd.too_large").with("mb", (MAX_BYTES >> 20) as i64));
    }
    let mut set = FileSet::default();
    let mut reader = Reader::new(data);
    let mut state = Parsing { symbols: 0 };
    while let Some(field) = reader.next_field()? {
        if field.number == 1 && field.wire_type == WT_LEN {
            if set.files.len() >= MAX_FILES {
                return Err(PluginError::new("pbd.too_many_files").with("max", MAX_FILES));
            }
            set.files.push(parse_file(&field.bytes, &mut state)?);
        }
        // 未知顶层字段忽略
    }
    Ok(set)
}

/// 解析单个 FileDescriptorProto
fn parse_file(data: &[u8], state: &mut Parsing) -> PluginResult<FileDesc> {
    let mut file = FileDesc {
        name: String::new(),
        package: String::new(),
        dependencies: Vec::new(),
        syntax: "proto2".to_owned(),
        unsupported_syntax: None,
        messages: Vec::new(),
        enums: Vec::new(),
        services: Vec::new(),
        locations: Vec::new(),
    };
    let mut reader = Reader::new(data);
    while let Some(field) = reader.next_field()? {
        match (field.number, field.wire_type) {
            (1, WT_LEN) => file.name = String::from_utf8_lossy(&field.bytes).into_owned(),
            (2, WT_LEN) => file.package = String::from_utf8_lossy(&field.bytes).into_owned(),
            (3, WT_LEN) => file
                .dependencies
                .push(String::from_utf8_lossy(&field.bytes).into_owned()),
            (4, WT_LEN) => {
                file.messages
                    .push(parse_message(&field.bytes, &file.package, 0, state)?)
            }
            (5, WT_LEN) => file
                .enums
                .push(parse_enum(&field.bytes, &file.package, state)?),
            (6, WT_LEN) => file
                .services
                .push(parse_service(&field.bytes, &file.package, state)?),
            (9, WT_LEN) => parse_source_info(&field.bytes, &mut file.locations)?,
            (12, WT_LEN) => {
                file.syntax = String::from_utf8_lossy(&field.bytes).into_owned();
            }
            (14, WT_LEN) => {
                file.syntax = format!("edition:{}", String::from_utf8_lossy(&field.bytes));
            }
            _ => {}
        }
    }
    if file.name.is_empty() {
        return Err(PluginError::new("pbd.file_no_name"));
    }
    // proto2 是 descriptor 的默认 syntax；proto3/editions 显式声明。
    // 明确支持 proto3；proto2 与 editions 标记为不支持。
    match file.syntax.as_str() {
        "proto3" => {}
        other => file.unsupported_syntax = Some(other.to_owned()),
    }
    Ok(file)
}

/// 解析 DescriptorProto；scope 为外层作用域完整名（包或外层消息），用于拼接 nested full_name
fn parse_message(
    data: &[u8],
    scope: &str,
    depth: usize,
    state: &mut Parsing,
) -> PluginResult<MessageDesc> {
    if depth > MAX_DEPTH {
        return Err(PluginError::new("pbd.too_deep").with("max", MAX_DEPTH));
    }
    state.budget()?;
    let mut name = String::new();
    let mut raw_fields: Vec<Vec<u8>> = Vec::new();
    let mut raw_nested: Vec<Vec<u8>> = Vec::new();
    let mut raw_enums: Vec<Vec<u8>> = Vec::new();
    let mut oneofs: Vec<String> = Vec::new();
    let mut reserved_ranges = Vec::new();
    let mut reserved_names = Vec::new();
    let mut reader = Reader::new(data);
    while let Some(field) = reader.next_field()? {
        match (field.number, field.wire_type) {
            (1, WT_LEN) => name = String::from_utf8_lossy(&field.bytes).into_owned(),
            (2, WT_LEN) => raw_fields.push(field.bytes),
            (3, WT_LEN) => raw_nested.push(field.bytes),
            (4, WT_LEN) => raw_enums.push(field.bytes),
            (8, WT_LEN) => oneofs.push(String::from_utf8_lossy(&field.bytes).into_owned()),
            (9, WT_LEN) => reserved_ranges.push(parse_range(&field.bytes)),
            (10, WT_LEN) => reserved_names.push(String::from_utf8_lossy(&field.bytes).into_owned()),
            _ => {}
        }
    }
    if name.is_empty() {
        return Err(PluginError::new("pbd.message_no_name"));
    }
    let full_name = if scope.is_empty() {
        name.clone()
    } else {
        format!("{scope}.{name}")
    };
    let mut message = MessageDesc {
        name,
        full_name: full_name.clone(),
        fields: Vec::new(),
        oneofs,
        reserved_ranges,
        reserved_names,
        nested: Vec::new(),
        enums: Vec::new(),
    };
    for nested in &raw_nested {
        message
            .nested
            .push(parse_message(nested, &full_name, depth + 1, state)?);
    }
    for enum_data in &raw_enums {
        message
            .enums
            .push(parse_enum(enum_data, &full_name, state)?);
    }
    for raw in &raw_fields {
        message.fields.push(parse_field(raw, &message.oneofs)?);
    }
    Ok(message)
}

fn parse_range(data: &[u8]) -> (i64, i64) {
    let mut start = 0i64;
    let mut end = 0i64;
    let mut reader = Reader::new(data);
    while let Ok(Some(field)) = reader.next_field() {
        match (field.number, field.wire_type) {
            (1, WT_VARINT) => start = field.varint as i64,
            (2, WT_VARINT) => end = field.varint as i64,
            _ => {}
        }
    }
    (start, end)
}

/// 解析 FieldDescriptorProto；oneof_names 用于把 oneof_index 转名称
fn parse_field(data: &[u8], oneof_names: &[String]) -> PluginResult<FieldDesc> {
    let mut field = FieldDesc {
        name: String::new(),
        number: 0,
        label: LABEL_OPTIONAL,
        type_code: 0,
        type_name: String::new(),
        oneof: String::new(),
        proto3_optional: false,
        json_name: String::new(),
    };
    let mut has_number = false;
    let mut oneof_index: Option<usize> = None;
    let mut reader = Reader::new(data);
    while let Some(entry) = reader.next_field()? {
        match (entry.number, entry.wire_type) {
            (1, WT_LEN) => field.name = String::from_utf8_lossy(&entry.bytes).into_owned(),
            (3, WT_VARINT) => {
                field.number = entry.varint as i64;
                has_number = true;
            }
            (4, WT_VARINT) => field.label = entry.varint as i64,
            (5, WT_VARINT) => field.type_code = entry.varint as i64,
            (6, WT_LEN) => field.type_name = String::from_utf8_lossy(&entry.bytes).into_owned(),
            (9, WT_VARINT) => oneof_index = Some(entry.varint as usize),
            (10, WT_LEN) => field.json_name = String::from_utf8_lossy(&entry.bytes).into_owned(),
            (17, WT_VARINT) => field.proto3_optional = entry.varint != 0,
            _ => {}
        }
    }
    if !has_number || field.name.is_empty() {
        return Err(PluginError::new("pbd.field_incomplete").with("name", field.name.as_str()));
    }
    if let Some(name) = oneof_index.and_then(|index| oneof_names.get(index)) {
        field.oneof = name.clone();
    }
    if field.json_name.is_empty() {
        field.json_name = default_json_name(&field.name);
    }
    Ok(field)
}

/// 解析 EnumDescriptorProto
fn parse_enum(data: &[u8], scope: &str, state: &mut Parsing) -> PluginResult<EnumDesc> {
    state.budget()?;
    let mut name = String::new();
    let mut values: Vec<(String, i64)> = Vec::new();
    let mut reserved_ranges = Vec::new();
    let mut reserved_names = Vec::new();
    let mut reader = Reader::new(data);
    while let Some(field) = reader.next_field()? {
        match (field.number, field.wire_type) {
            (1, WT_LEN) => name = String::from_utf8_lossy(&field.bytes).into_owned(),
            (2, WT_LEN) => values.push(parse_enum_value(&field.bytes)?),
            (4, WT_LEN) => reserved_ranges.push(parse_range(&field.bytes)),
            (5, WT_LEN) => reserved_names.push(String::from_utf8_lossy(&field.bytes).into_owned()),
            _ => {}
        }
    }
    if name.is_empty() {
        return Err(PluginError::new("pbd.enum_no_name"));
    }
    let full_name = if scope.is_empty() {
        name.clone()
    } else {
        format!("{scope}.{name}")
    };
    Ok(EnumDesc {
        name,
        full_name,
        values,
        reserved_ranges,
        reserved_names,
    })
}

fn parse_enum_value(data: &[u8]) -> PluginResult<(String, i64)> {
    let mut name = String::new();
    let mut number = 0i64;
    let mut reader = Reader::new(data);
    while let Some(field) = reader.next_field()? {
        match (field.number, field.wire_type) {
            (1, WT_LEN) => name = String::from_utf8_lossy(&field.bytes).into_owned(),
            (2, WT_VARINT) => number = field.varint as i64,
            _ => {}
        }
    }
    if name.is_empty() {
        return Err(PluginError::new("pbd.enum_value_no_name"));
    }
    Ok((name, number))
}

/// 解析 ServiceDescriptorProto
fn parse_service(data: &[u8], package: &str, state: &mut Parsing) -> PluginResult<ServiceDesc> {
    state.budget()?;
    let mut name = String::new();
    let mut methods = Vec::new();
    let mut reader = Reader::new(data);
    while let Some(field) = reader.next_field()? {
        match (field.number, field.wire_type) {
            (1, WT_LEN) => name = String::from_utf8_lossy(&field.bytes).into_owned(),
            (2, WT_LEN) => methods.push(parse_method(&field.bytes)?),
            _ => {}
        }
    }
    if name.is_empty() {
        return Err(PluginError::new("pbd.service_no_name"));
    }
    let full_name = if package.is_empty() {
        name.clone()
    } else {
        format!("{package}.{name}")
    };
    Ok(ServiceDesc {
        name,
        full_name,
        methods,
    })
}

fn parse_method(data: &[u8]) -> PluginResult<MethodDesc> {
    let mut method = MethodDesc {
        name: String::new(),
        input_type: String::new(),
        output_type: String::new(),
        client_streaming: false,
        server_streaming: false,
    };
    let mut reader = Reader::new(data);
    while let Some(field) = reader.next_field()? {
        match (field.number, field.wire_type) {
            (1, WT_LEN) => method.name = String::from_utf8_lossy(&field.bytes).into_owned(),
            (2, WT_LEN) => method.input_type = String::from_utf8_lossy(&field.bytes).into_owned(),
            (3, WT_LEN) => method.output_type = String::from_utf8_lossy(&field.bytes).into_owned(),
            (5, WT_VARINT) => method.client_streaming = field.varint != 0,
            (6, WT_VARINT) => method.server_streaming = field.varint != 0,
            _ => {}
        }
    }
    if method.name.is_empty() {
        return Err(PluginError::new("pbd.method_no_name"));
    }
    Ok(method)
}

/// SourceCodeInfo：location 字段 1，每项 path(1, packed int32)/span(2, packed int32)/leading_comments(3)
fn parse_source_info(data: &[u8], locations: &mut Vec<SourceLocation>) -> PluginResult<()> {
    let mut outer = Reader::new(data);
    while let Some(entry) = outer.next_field()? {
        if entry.number != 1 || entry.wire_type != WT_LEN {
            continue;
        }
        let mut location = SourceLocation {
            path: Vec::new(),
            span: Vec::new(),
            leading_comments: String::new(),
        };
        let mut reader = Reader::new(&entry.bytes);
        while let Some(field) = reader.next_field()? {
            match (field.number, field.wire_type) {
                (1, WT_LEN) => location.path = parse_packed_i32(&field.bytes)?,
                (1, WT_VARINT) => location.path.push(field.varint as i64),
                (2, WT_LEN) => location.span = parse_packed_i32(&field.bytes)?,
                (2, WT_VARINT) => location.span.push(field.varint as i64),
                (3, WT_LEN) => {
                    location.leading_comments = String::from_utf8_lossy(&field.bytes).into_owned()
                }
                _ => {}
            }
        }
        locations.push(location);
    }
    Ok(())
}

fn parse_packed_i32(data: &[u8]) -> PluginResult<Vec<i64>> {
    let mut out = Vec::new();
    let mut reader = Reader::new(data);
    while !reader.at_end() {
        out.push(reader.read_varint()? as i64);
    }
    Ok(out)
}

/// 相对类型名规范化：从字段所在消息作用域逐级向外查找（C++ scoping 规则）。
/// 绝对引用（带前导点）直接去掉前导点保留。
pub fn normalize_types(set: &mut FileSet) {
    let table = set.symbol_table();
    for file in &mut set.files {
        for message in &mut file.messages {
            normalize_message(&table, message);
        }
    }
}

fn normalize_message(table: &BTreeMap<String, String>, message: &mut MessageDesc) {
    for field in &mut message.fields {
        if field.type_name.is_empty() {
            continue;
        }
        field.type_name = resolve_type(table, &message.full_name, &field.type_name);
    }
    for nested in &mut message.nested {
        normalize_message(table, nested);
    }
}

fn resolve_type(table: &BTreeMap<String, String>, scope: &str, raw: &str) -> String {
    let candidate = raw.trim_start_matches('.');
    if raw.starts_with('.') {
        return candidate.to_owned();
    }
    let mut prefix = scope.to_owned();
    loop {
        let attempt = if prefix.is_empty() {
            candidate.to_owned()
        } else {
            format!("{prefix}.{candidate}")
        };
        if table.contains_key(&attempt) {
            return attempt;
        }
        if prefix.is_empty() {
            break;
        }
        prefix = match prefix.rsplit_once('.') {
            Some((parent, _)) => parent.to_owned(),
            None => String::new(),
        };
    }
    candidate.to_owned()
}

/// 依赖检查：依赖完整、无重复符号、类型引用可解析；返回符号表
pub fn validate(set: &FileSet) -> PluginResult<BTreeMap<String, String>> {
    let missing = set.missing_dependencies();
    if !missing.is_empty() {
        return Err(PluginError::new("pbd.missing_dependency").with("files", missing.join(", ")));
    }
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for file in &set.files {
        let prefix = if file.package.is_empty() {
            String::new()
        } else {
            format!("{}.", file.package)
        };
        for message in &file.messages {
            walk_symbols(&mut seen, &file.name, &prefix, message)?;
        }
        for enum_desc in &file.enums {
            insert_symbol(
                &mut seen,
                &format!("{prefix}{}", enum_desc.name),
                &file.name,
            )?;
        }
        for service in &file.services {
            insert_symbol(&mut seen, &format!("{prefix}{}", service.name), &file.name)?;
        }
    }
    let table = set.symbol_table();
    for file in &set.files {
        for message in &file.messages {
            check_message_names(&table, &file.name, message)?;
        }
    }
    Ok(table)
}

fn walk_symbols(
    seen: &mut BTreeMap<String, String>,
    file: &str,
    prefix: &str,
    message: &MessageDesc,
) -> PluginResult<()> {
    let full = format!("{prefix}{}", message.name);
    insert_symbol(seen, &full, file)?;
    let child = format!("{full}.");
    for nested in &message.nested {
        walk_symbols(seen, file, &child, nested)?;
    }
    for enum_desc in &message.enums {
        insert_symbol(seen, &format!("{child}{}", enum_desc.name), file)?;
    }
    Ok(())
}

fn insert_symbol(seen: &mut BTreeMap<String, String>, full: &str, file: &str) -> PluginResult<()> {
    match seen.insert(full.to_owned(), file.to_owned()) {
        Some(previous) if previous != file => {
            Err(errp("pbd.duplicate_symbol", "symbol", full).with("file", file))
        }
        _ => Ok(()),
    }
}

fn check_message_names(
    table: &BTreeMap<String, String>,
    file: &str,
    message: &MessageDesc,
) -> PluginResult<()> {
    for field in &message.fields {
        if (field.type_code == FieldType::Message as i64
            || field.type_code == FieldType::Enum as i64)
            && !table.contains_key(&field.type_name)
        {
            return Err(
                errp("pbd.unknown_type", "type", field.type_name.as_str()).with("file", file)
            );
        }
    }
    for nested in &message.nested {
        check_message_names(table, file, nested)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "descriptor_test.rs"]
mod tests;

#[cfg(test)]
#[path = "testsupport.rs"]
pub mod build;
