//! 兼容性规则引擎：按完整类型名、字段号、枚举数值匹配新旧描述符，
//! 分别报告「旧写→新读」「新写→旧读」的 wire 风险与 ProtoJSON 名称风险。
//! 同一 wire type 不等于值语义兼容；条件性变化单独列出，不伪装成无条件通过。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

use crate::descriptor::{
    self, EnumDesc, FieldDesc, FieldType, FileDesc, FileSet, MessageDesc, ServiceDesc, WireClass,
};

/// 报告条目上限
pub const MAX_FINDINGS: usize = 5000;
pub const RULES_VERSION: &str = "1";

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffArgs {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    /// FileDescriptorSet 的 base64（内联输入）
    pub old_text: Option<String>,
    pub new_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// 旧客户端写 → 新服务端读
    OldWriteNewRead,
    /// 新客户端写 → 旧服务端读
    NewWriteOldRead,
    Both,
    /// ProtoJSON（json_name/枚举名/64 位数字表示）
    ProtoJson,
}

/// 1=breaking 2=conditional 3=info
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Breaking,
    Conditional,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: Severity,
    pub direction: Direction,
    /// 规则标识（前端翻译键）
    pub rule: String,
    /// 完整符号路径，如 pkg.Msg.field
    pub path: String,
    /// 旧描述 / 新描述（类型签名等）
    pub old_desc: String,
    pub new_desc: String,
    /// 源位置（行列，缺源信息时为逻辑路径）
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSummary {
    pub files: usize,
    pub messages: usize,
    pub enums: usize,
    pub services: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedFile {
    pub name: String,
    pub syntax: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub rules_version: String,
    pub old_summary: SetSummary,
    pub new_summary: SetSummary,
    /// proto2 / editions 等不支持的文件：这些文件不参与结论，绝不输出“全部兼容”
    pub unsupported: Vec<UnsupportedFile>,
    pub findings: Vec<Finding>,
    /// 未检查范围说明
    pub notes: Vec<String>,
}

// ---- 输入 ----

pub fn run(args: DiffArgs, ctx: Option<&dyn TaskContext>) -> PluginResult<Report> {
    let old = load_side(&args.old_path, &args.old_text, ctx, "old")?;
    let new = load_side(&args.new_path, &args.new_text, ctx, "new")?;
    Ok(compare(&old, &new))
}

fn load_side(
    path: &Option<String>,
    text: &Option<String>,
    ctx: Option<&dyn TaskContext>,
    side: &str,
) -> PluginResult<FileSet> {
    let data = match (path, text) {
        (Some(path), _) => {
            let ctx = ctx.ok_or_else(|| PluginError::new("pbd.task_required"))?;
            read_file(path, ctx)?
        }
        (None, Some(text)) => {
            let trimmed = text.trim();
            base64::decode(trimmed).map_err(|_| errp("pbd.bad_base64", "side", side))?
        }
        (None, None) => return Err(errp("pbd.missing_input", "side", side)),
    };
    let mut set = descriptor::parse_file_set(&data)?;
    descriptor::normalize_types(&mut set);
    descriptor::validate(&set)?;
    Ok(set)
}

fn read_file(path: &str, ctx: &dyn TaskContext) -> PluginResult<Vec<u8>> {
    use std::io::Read;
    ctx.stage("read");
    let size = ctx
        .file_size(path)
        .map_err(|_| PluginError::new("pbd.file_read"))?;
    if size > descriptor::MAX_BYTES as u64 {
        return Err(
            PluginError::new("pbd.too_large").with("mb", (descriptor::MAX_BYTES >> 20) as i64)
        );
    }
    let mut reader = ctx
        .open_file(path)
        .map_err(|_| PluginError::new("pbd.file_read"))?;
    let mut data = Vec::with_capacity(size as usize);
    let mut chunk = [0u8; 65536];
    loop {
        if ctx.is_cancelled() {
            return Err(PluginError::new("task.cancelled"));
        }
        let read = reader
            .read(&mut chunk)
            .map_err(|_| PluginError::new("pbd.file_read"))?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
        if data.len() > descriptor::MAX_BYTES {
            return Err(
                PluginError::new("pbd.too_large").with("mb", (descriptor::MAX_BYTES >> 20) as i64)
            );
        }
    }
    Ok(data)
}

// ---- 索引 ----

/// 平面索引：完整名 → (文件名, 描述符, 逻辑路径)
struct Index<'a> {
    messages: BTreeMap<&'a str, (&'a str, &'a MessageDesc, Vec<i64>)>,
    enums: BTreeMap<&'a str, (&'a str, &'a EnumDesc)>,
    services: BTreeMap<&'a str, (&'a str, &'a ServiceDesc)>,
}

impl<'a> Index<'a> {
    fn new(set: &'a FileSet) -> Self {
        let mut index = Self {
            messages: BTreeMap::new(),
            enums: BTreeMap::new(),
            services: BTreeMap::new(),
        };
        for file in &set.files {
            // proto2/editions 文件不参与结论，避免给出“兼容”的假象
            if file.unsupported_syntax.is_some() {
                continue;
            }
            for (position, message) in file.messages.iter().enumerate() {
                walk_message(&mut index, &file.name, message, vec![4, position as i64]);
            }
            for enum_desc in &file.enums {
                index
                    .enums
                    .insert(enum_desc.full_name.as_str(), (&file.name, enum_desc));
            }
            for service in &file.services {
                index
                    .services
                    .insert(service.full_name.as_str(), (&file.name, service));
            }
        }
        index
    }
}

fn walk_message<'a>(
    index: &mut Index<'a>,
    file: &'a str,
    message: &'a MessageDesc,
    path: Vec<i64>,
) {
    index
        .messages
        .insert(message.full_name.as_str(), (file, message, path.clone()));
    for (position, nested) in message.nested.iter().enumerate() {
        let mut child = path.clone();
        child.push(3);
        child.push(position as i64);
        walk_message(index, file, nested, child);
    }
    for enum_desc in &message.enums {
        index
            .enums
            .insert(enum_desc.full_name.as_str(), (file, enum_desc));
    }
}

fn summary(set: &FileSet) -> SetSummary {
    let mut counts = SetSummary {
        files: set
            .files
            .iter()
            .filter(|f| f.unsupported_syntax.is_none())
            .count(),
        messages: 0,
        enums: 0,
        services: 0,
    };
    for file in set.files.iter().filter(|f| f.unsupported_syntax.is_none()) {
        counts.services += file.services.len();
        for message in &file.messages {
            count_message(message, &mut counts);
        }
        counts.enums += file.enums.len();
    }
    counts
}

fn count_message(message: &MessageDesc, counts: &mut SetSummary) {
    counts.messages += 1;
    counts.enums += message.enums.len();
    for nested in &message.nested {
        count_message(nested, counts);
    }
}

// ---- 文本渲染 ----

fn type_text(field: &FieldDesc) -> String {
    let scalar = match FieldType::from_code(field.type_code) {
        Some(field_type) => match field_type {
            FieldType::Message | FieldType::Enum | FieldType::Group => field.type_name.clone(),
            other => format!("{other:?}").to_lowercase(),
        },
        None => format!("unknown({})", field.type_code),
    };
    if field.label == descriptor::LABEL_REPEATED {
        format!("repeated {scalar}")
    } else {
        scalar
    }
}

fn presence_text(field: &FieldDesc) -> String {
    if field.proto3_optional {
        "optional".to_owned()
    } else if field.label == descriptor::LABEL_REPEATED {
        "repeated".to_owned()
    } else if field.oneof.is_empty() {
        "implicit".to_owned()
    } else {
        "oneof".to_owned()
    }
}

fn cardinality_text(label: i64) -> &'static str {
    if label == descriptor::LABEL_REPEATED {
        "repeated"
    } else {
        "singular"
    }
}

/// 在文件 source locations 中查找匹配逻辑路径的位置；无源信息时显示逻辑路径
fn locate(
    files: &BTreeMap<&str, &FileDesc>,
    file: &str,
    message_path: &[i64],
    field_index: Option<usize>,
) -> String {
    let mut path = message_path.to_vec();
    if let Some(index) = field_index {
        path.push(2);
        path.push(index as i64);
    }
    if let Some(file_desc) = files.get(file) {
        // 取最长匹配（最精确）的位置
        let found = file_desc
            .locations
            .iter()
            .filter(|location| {
                location.span.len() >= 3
                    && location.path.len() <= path.len()
                    && location.path == path[..location.path.len()]
            })
            .max_by_key(|location| location.path.len());
        if let Some(location) = found {
            return format!("{file}:{}:{}", location.span[0] + 1, location.span[1] + 1);
        }
    }
    format!(
        "{file} path {}",
        path.iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(".")
    )
}

// ---- 主对比 ----

pub fn compare(old_set: &FileSet, new_set: &FileSet) -> Report {
    let mut report = Report {
        rules_version: RULES_VERSION.to_owned(),
        old_summary: summary(old_set),
        new_summary: summary(new_set),
        unsupported: Vec::new(),
        findings: Vec::new(),
        notes: vec![
            "source-compatibility".to_owned(),
            "options-ignored".to_owned(),
            "comments-order-ignored".to_owned(),
        ],
    };
    for file in old_set.files.iter().chain(new_set.files.iter()) {
        if let Some(syntax) = &file.unsupported_syntax
            && !report.unsupported.iter().any(|item| item.name == file.name)
        {
            report.unsupported.push(UnsupportedFile {
                name: file.name.clone(),
                syntax: syntax.clone(),
            });
        }
    }

    let old_index = Index::new(old_set);
    let new_index = Index::new(new_set);
    let old_files: BTreeMap<&str, &FileDesc> = old_set
        .files
        .iter()
        .filter(|f| f.unsupported_syntax.is_none())
        .map(|f| (f.name.as_str(), f))
        .collect();
    let new_files: BTreeMap<&str, &FileDesc> = new_set
        .files
        .iter()
        .filter(|f| f.unsupported_syntax.is_none())
        .map(|f| (f.name.as_str(), f))
        .collect();
    // 位置查询需要文件级索引（含彼此）
    let all_files: BTreeMap<&str, &FileDesc> = old_files
        .iter()
        .chain(new_files.iter())
        .map(|(k, v)| (*k, *v))
        .collect();

    // 文件级：删除 / 新增
    for (name, file) in &old_files {
        if !new_files.contains_key(name) {
            push(
                &mut report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "file.removed".into(),
                    path: (*name).to_owned(),
                    old_desc: format!("{} messages", count_message_tree(file)),
                    new_desc: String::new(),
                    location: (*name).to_owned(),
                },
            );
        }
    }
    for name in new_files.keys() {
        if !old_files.contains_key(name) {
            push(
                &mut report,
                Finding {
                    severity: Severity::Info,
                    direction: Direction::Both,
                    rule: "file.added".into(),
                    path: (*name).to_owned(),
                    old_desc: String::new(),
                    new_desc: String::new(),
                    location: (*name).to_owned(),
                },
            );
        }
    }

    // 消息
    for (full_name, (old_file, old_message, old_path)) in &old_index.messages {
        match new_index.messages.get(full_name) {
            None => push(
                &mut report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "message.removed".into(),
                    path: (*full_name).to_owned(),
                    old_desc: format!("{} fields", old_message.fields.len()),
                    new_desc: String::new(),
                    location: locate(&all_files, old_file, old_path, None),
                },
            ),
            Some((new_file, new_message, new_path)) => compare_message(
                &mut report,
                old_file,
                new_file,
                old_message,
                new_message,
                old_path,
                new_path,
                &all_files,
            ),
        }
    }
    for full_name in new_index.messages.keys() {
        if !old_index.messages.contains_key(full_name) {
            push(
                &mut report,
                Finding {
                    severity: Severity::Info,
                    direction: Direction::Both,
                    rule: "message.added".into(),
                    path: (*full_name).to_owned(),
                    old_desc: String::new(),
                    new_desc: String::new(),
                    location: String::new(),
                },
            );
        }
    }

    // 枚举
    for (full_name, (old_file, old_enum)) in &old_index.enums {
        match new_index.enums.get(full_name) {
            None => push(
                &mut report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "enum.removed".into(),
                    path: (*full_name).to_owned(),
                    old_desc: format!("{} values", old_enum.values.len()),
                    new_desc: String::new(),
                    location: String::new(),
                },
            ),
            Some((new_file, new_enum)) => compare_enum(
                &mut report,
                old_file,
                new_file,
                old_enum,
                new_enum,
                &all_files,
            ),
        }
    }
    for full_name in new_index.enums.keys() {
        if !old_index.enums.contains_key(full_name) {
            push(
                &mut report,
                Finding {
                    severity: Severity::Info,
                    direction: Direction::Both,
                    rule: "enum.added".into(),
                    path: (*full_name).to_owned(),
                    old_desc: String::new(),
                    new_desc: String::new(),
                    location: String::new(),
                },
            );
        }
    }

    // 服务
    for (full_name, (old_file, old_service)) in &old_index.services {
        match new_index.services.get(full_name) {
            None => push(
                &mut report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "service.removed".into(),
                    path: (*full_name).to_owned(),
                    old_desc: format!("{} methods", old_service.methods.len()),
                    new_desc: String::new(),
                    location: String::new(),
                },
            ),
            Some((new_file, new_service)) => compare_service(
                &mut report,
                old_file,
                new_file,
                old_service,
                new_service,
                &all_files,
            ),
        }
    }
    for full_name in new_index.services.keys() {
        if !old_index.services.contains_key(full_name) {
            push(
                &mut report,
                Finding {
                    severity: Severity::Info,
                    direction: Direction::Both,
                    rule: "service.added".into(),
                    path: (*full_name).to_owned(),
                    old_desc: String::new(),
                    new_desc: String::new(),
                    location: String::new(),
                },
            );
        }
    }

    report
        .findings
        .sort_by(|a, b| (&a.severity, &a.path, &a.rule).cmp(&(&b.severity, &b.path, &b.rule)));
    report
}

fn count_message_tree(file: &FileDesc) -> usize {
    fn walk(message: &MessageDesc, count: &mut usize) {
        *count += 1;
        for nested in &message.nested {
            walk(nested, count);
        }
    }
    let mut count = 0;
    for message in &file.messages {
        walk(message, &mut count);
    }
    count
}

fn push(report: &mut Report, finding: Finding) {
    if report.findings.len() < MAX_FINDINGS {
        report.findings.push(finding);
    }
}

/// 字段号是否被 reserved 覆盖
fn reserved_covers(message: &MessageDesc, number: i64, name: &str) -> bool {
    message
        .reserved_ranges
        .iter()
        .any(|(start, end)| number >= *start && number < *end)
        || message
            .reserved_names
            .iter()
            .any(|reserved| reserved == name)
}

#[allow(clippy::too_many_arguments)]
fn compare_message(
    report: &mut Report,
    old_file: &str,
    new_file: &str,
    old_message: &MessageDesc,
    new_message: &MessageDesc,
    old_path: &[i64],
    new_path: &[i64],
    all_files: &BTreeMap<&str, &FileDesc>,
) {
    let old_by_number: BTreeMap<i64, &FieldDesc> =
        old_message.fields.iter().map(|f| (f.number, f)).collect();
    let new_by_number: BTreeMap<i64, &FieldDesc> =
        new_message.fields.iter().map(|f| (f.number, f)).collect();

    // 删除的字段
    for (old_index, old_field) in old_message.fields.iter().enumerate() {
        match new_by_number.get(&old_field.number) {
            None => {
                let covered = reserved_covers(new_message, old_field.number, &old_field.name);
                let severity = if covered {
                    Severity::Info
                } else {
                    Severity::Breaking
                };
                push(
                    report,
                    Finding {
                        severity,
                        direction: Direction::Both,
                        rule: if covered {
                            "field.removed_reserved".into()
                        } else {
                            "field.removed".into()
                        },
                        path: format!("{}.{}", old_message.full_name, old_field.name),
                        old_desc: type_text(old_field),
                        new_desc: if covered {
                            "reserved".to_owned()
                        } else {
                            String::new()
                        },
                        location: locate(all_files, old_file, old_path, Some(old_index)),
                    },
                );
            }
            Some(new_field) => {
                let new_index = new_message
                    .fields
                    .iter()
                    .position(|f| f.number == old_field.number)
                    .unwrap_or_default();
                compare_field(
                    report,
                    new_file,
                    old_message,
                    new_message,
                    old_field,
                    new_field,
                    new_path,
                    new_index,
                    all_files,
                );
            }
        }
    }
    // 新增的字段
    for (new_index, new_field) in new_message.fields.iter().enumerate() {
        if !old_by_number.contains_key(&new_field.number) {
            // 旧 reserved 被新字段占用：明确破坏
            if reserved_covers(old_message, new_field.number, &new_field.name) {
                push(
                    report,
                    Finding {
                        severity: Severity::Breaking,
                        direction: Direction::Both,
                        rule: "reserved.reused".into(),
                        path: format!("{}.{}", new_message.full_name, new_field.name),
                        old_desc: format!("reserved {}", new_field.number),
                        new_desc: type_text(new_field),
                        location: locate(all_files, new_file, new_path, Some(new_index)),
                    },
                );
            } else {
                push(
                    report,
                    Finding {
                        severity: Severity::Info,
                        direction: Direction::Both,
                        rule: "field.added".into(),
                        path: format!("{}.{}", new_message.full_name, new_field.name),
                        old_desc: String::new(),
                        new_desc: type_text(new_field),
                        location: locate(all_files, new_file, new_path, Some(new_index)),
                    },
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_field(
    report: &mut Report,
    new_file: &str,
    old_message: &MessageDesc,
    _new_message: &MessageDesc,
    old_field: &FieldDesc,
    new_field: &FieldDesc,
    new_path: &[i64],
    new_index: usize,
    all_files: &BTreeMap<&str, &FileDesc>,
) {
    let location = locate(all_files, new_file, new_path, Some(new_index));
    let path = format!("{}.{}", old_message.full_name, old_field.name);
    let old_type = type_text(old_field);
    let new_type = type_text(new_field);

    // 类型变化
    if old_field.type_code != new_field.type_code || old_field.type_name != new_field.type_name {
        report_type_change(report, &path, old_field, new_field, &old_type, &new_type);
    }

    // 基数变化
    if old_field.label != new_field.label {
        let old_repeated = old_field.label == descriptor::LABEL_REPEATED;
        let new_repeated = new_field.label == descriptor::LABEL_REPEATED;
        if !old_repeated && new_repeated {
            push(
                report,
                Finding {
                    severity: Severity::Conditional,
                    direction: Direction::Both,
                    rule: "field.cardinality_widened".into(),
                    path: path.clone(),
                    old_desc: cardinality_text(old_field.label).into(),
                    new_desc: cardinality_text(new_field.label).into(),
                    location: location.clone(),
                },
            );
        } else if old_repeated && !new_repeated {
            push(
                report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "field.cardinality_narrowed".into(),
                    path: path.clone(),
                    old_desc: cardinality_text(old_field.label).into(),
                    new_desc: cardinality_text(new_field.label).into(),
                    location: location.clone(),
                },
            );
        } else if old_field.label == descriptor::LABEL_REQUIRED
            || new_field.label == descriptor::LABEL_REQUIRED
        {
            push(
                report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "field.required_changed".into(),
                    path: path.clone(),
                    old_desc: cardinality_text(old_field.label).into(),
                    new_desc: cardinality_text(new_field.label).into(),
                    location: location.clone(),
                },
            );
        }
    }

    // presence 变化（proto3 optional / oneof 归属）
    if old_field.proto3_optional != new_field.proto3_optional {
        push(
            report,
            Finding {
                severity: Severity::Info,
                direction: Direction::Both,
                rule: "field.presence_changed".into(),
                path: path.clone(),
                old_desc: presence_text(old_field),
                new_desc: presence_text(new_field),
                location: location.clone(),
            },
        );
    }
    if old_field.oneof != new_field.oneof {
        push(
            report,
            Finding {
                severity: Severity::Info,
                direction: Direction::Both,
                rule: "field.oneof_changed".into(),
                path: path.clone(),
                old_desc: dash_or(&old_field.oneof),
                new_desc: dash_or(&new_field.oneof),
                location: location.clone(),
            },
        );
    }

    // 改名（同字段号）：二进制兼容，ProtoJSON 名称风险；不能据此断言号码复用
    if old_field.name != new_field.name {
        push(
            report,
            Finding {
                severity: Severity::Conditional,
                direction: Direction::ProtoJson,
                rule: "field.renamed".into(),
                path,
                old_desc: old_field.name.clone(),
                new_desc: new_field.name.clone(),
                location,
            },
        );
    } else if old_field.json_name != new_field.json_name {
        push(
            report,
            Finding {
                severity: Severity::Conditional,
                direction: Direction::ProtoJson,
                rule: "field.json_name_changed".into(),
                path,
                old_desc: old_field.json_name.clone(),
                new_desc: new_field.json_name.clone(),
                location,
            },
        );
    }
}

fn dash_or(value: &str) -> String {
    if value.is_empty() {
        "-".to_owned()
    } else {
        value.to_owned()
    }
}

/// 同字段号的类型变化：wire class 决定破坏级别，class 内部按值语义细分
fn report_type_change(
    report: &mut Report,
    path: &str,
    old_field: &FieldDesc,
    new_field: &FieldDesc,
    old_type: &str,
    new_type: &str,
) {
    let old_code = FieldType::from_code(old_field.type_code);
    let new_code = FieldType::from_code(new_field.type_code);
    let (Some(old_type_code), Some(new_type_code)) = (old_code, new_code) else {
        push(
            report,
            Finding {
                severity: Severity::Conditional,
                direction: Direction::Both,
                rule: "field.type_unknown".into(),
                path: path.to_owned(),
                old_desc: old_type.to_owned(),
                new_desc: new_type.to_owned(),
                location: String::new(),
            },
        );
        return;
    };
    if old_type_code == FieldType::Group || new_type_code == FieldType::Group {
        push(
            report,
            Finding {
                severity: Severity::Breaking,
                direction: Direction::Both,
                rule: "field.group_changed".into(),
                path: path.to_owned(),
                old_desc: old_type.to_owned(),
                new_desc: new_type.to_owned(),
                location: String::new(),
            },
        );
        return;
    }
    let old_class = old_type_code.wire_class();
    let new_class = new_type_code.wire_class();
    if old_class != new_class {
        push(
            report,
            Finding {
                severity: Severity::Breaking,
                direction: Direction::Both,
                rule: "field.wire_type_changed".into(),
                path: path.to_owned(),
                old_desc: old_type.to_owned(),
                new_desc: new_type.to_owned(),
                location: String::new(),
            },
        );
        return;
    }
    match old_class {
        WireClass::Varint => {
            let old_zigzag = matches!(old_type_code, FieldType::Sint32 | FieldType::Sint64);
            let new_zigzag = matches!(new_type_code, FieldType::Sint32 | FieldType::Sint64);
            if old_zigzag != new_zigzag {
                // zigzag 与普通 varint 数值解释不同
                push(
                    report,
                    Finding {
                        severity: Severity::Breaking,
                        direction: Direction::Both,
                        rule: "field.zigzag_changed".into(),
                        path: path.to_owned(),
                        old_desc: old_type.to_owned(),
                        new_desc: new_type.to_owned(),
                        location: String::new(),
                    },
                );
            } else {
                // int32/int64/uint32/uint64/bool/enum 之间：编码一致但值语义（符号/范围）不同
                push(
                    report,
                    Finding {
                        severity: Severity::Conditional,
                        direction: Direction::Both,
                        rule: "field.varint_semantics".into(),
                        path: path.to_owned(),
                        old_desc: old_type.to_owned(),
                        new_desc: new_type.to_owned(),
                        location: String::new(),
                    },
                );
                // 64 位整型 ↔ 其余：ProtoJSON 数字表示（string vs number）变化
                let old_64 = matches!(
                    old_type_code,
                    FieldType::Int64 | FieldType::Uint64 | FieldType::Sint64
                );
                let new_64 = matches!(
                    new_type_code,
                    FieldType::Int64 | FieldType::Uint64 | FieldType::Sint64
                );
                if old_64 != new_64 {
                    push(
                        report,
                        Finding {
                            severity: Severity::Conditional,
                            direction: Direction::ProtoJson,
                            rule: "field.json_number_form".into(),
                            path: path.to_owned(),
                            old_desc: old_type.to_owned(),
                            new_desc: new_type.to_owned(),
                            location: String::new(),
                        },
                    );
                }
            }
        }
        WireClass::Bits64 => {
            let severity = if matches!(old_type_code, FieldType::Double)
                || matches!(new_type_code, FieldType::Double)
            {
                Severity::Breaking
            } else {
                Severity::Conditional
            };
            push(
                report,
                Finding {
                    severity,
                    direction: Direction::Both,
                    rule: if severity == Severity::Breaking {
                        "field.float_reinterpret".into()
                    } else {
                        "field.int64_semantics".into()
                    },
                    path: path.to_owned(),
                    old_desc: old_type.to_owned(),
                    new_desc: new_type.to_owned(),
                    location: String::new(),
                },
            );
        }
        WireClass::Bits32 => {
            let severity = if matches!(old_type_code, FieldType::Float)
                || matches!(new_type_code, FieldType::Float)
            {
                Severity::Breaking
            } else {
                Severity::Conditional
            };
            push(
                report,
                Finding {
                    severity,
                    direction: Direction::Both,
                    rule: if severity == Severity::Breaking {
                        "field.float_reinterpret".into()
                    } else {
                        "field.int32_semantics".into()
                    },
                    path: path.to_owned(),
                    old_desc: old_type.to_owned(),
                    new_desc: new_type.to_owned(),
                    location: String::new(),
                },
            );
        }
        WireClass::LengthDelimited => {
            let old_kind = kind_of(old_type_code);
            let new_kind = kind_of(new_type_code);
            match (old_kind, new_kind) {
                // message ↔ string/bytes：解析多半失败；string ↔ bytes 有 UTF-8 条件
                (Kind::Message, Kind::Scalar) | (Kind::Scalar, Kind::Message) => push(
                    report,
                    Finding {
                        severity: Severity::Conditional,
                        direction: Direction::Both,
                        rule: "field.message_scalar".into(),
                        path: path.to_owned(),
                        old_desc: old_type.to_owned(),
                        new_desc: new_type.to_owned(),
                        location: String::new(),
                    },
                ),
                (Kind::Scalar, Kind::Scalar) => push(
                    report,
                    Finding {
                        severity: Severity::Conditional,
                        direction: Direction::Both,
                        rule: "field.string_bytes".into(),
                        path: path.to_owned(),
                        old_desc: old_type.to_owned(),
                        new_desc: new_type.to_owned(),
                        location: String::new(),
                    },
                ),
                _ => push(
                    report,
                    Finding {
                        // message ↔ message（不同类型）或 enum ↔ enum：无法静态验证结构兼容
                        severity: Severity::Conditional,
                        direction: Direction::Both,
                        rule: "field.message_retyped".into(),
                        path: path.to_owned(),
                        old_desc: old_type.to_owned(),
                        new_desc: new_type.to_owned(),
                        location: String::new(),
                    },
                ),
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Message,
    Scalar,
}

fn kind_of(code: FieldType) -> Kind {
    match code {
        FieldType::Message | FieldType::Group => Kind::Message,
        _ => Kind::Scalar,
    }
}

fn compare_enum(
    report: &mut Report,
    _old_file: &str,
    _new_file: &str,
    old_enum: &EnumDesc,
    new_enum: &EnumDesc,
    _all_files: &BTreeMap<&str, &FileDesc>,
) {
    let old_by_number: BTreeMap<i64, &str> = old_enum
        .values
        .iter()
        .map(|(name, number)| (*number, name.as_str()))
        .collect();
    let new_by_number: BTreeMap<i64, &str> = new_enum
        .values
        .iter()
        .map(|(name, number)| (*number, name.as_str()))
        .collect();
    for (number, old_name) in &old_by_number {
        match new_by_number.get(number) {
            None => {
                let covered = new_enum
                    .reserved_ranges
                    .iter()
                    .any(|(start, end)| number >= start && number < end)
                    || new_enum.reserved_names.iter().any(|name| name == old_name);
                push(
                    report,
                    Finding {
                        severity: if covered {
                            Severity::Info
                        } else {
                            Severity::Conditional
                        },
                        direction: Direction::Both,
                        rule: if covered {
                            "enum.value_removed_reserved".into()
                        } else {
                            "enum.value_removed".into()
                        },
                        path: format!("{}.{}", new_enum.full_name, old_name),
                        old_desc: number.to_string(),
                        new_desc: if covered {
                            "reserved".to_owned()
                        } else {
                            String::new()
                        },
                        location: String::new(),
                    },
                );
            }
            Some(new_name) => {
                if old_name != new_name {
                    push(
                        report,
                        Finding {
                            severity: Severity::Conditional,
                            direction: Direction::ProtoJson,
                            rule: "enum.value_renamed".into(),
                            path: format!("{}.{}", new_enum.full_name, new_name),
                            old_desc: (*old_name).to_owned(),
                            new_desc: (*new_name).to_owned(),
                            location: String::new(),
                        },
                    );
                }
            }
        }
    }
    for (number, new_name) in &new_by_number {
        if old_by_number.contains_key(number) {
            continue;
        }
        let reused = old_enum
            .reserved_ranges
            .iter()
            .any(|(start, end)| number >= start && number < end)
            || old_enum.reserved_names.iter().any(|name| name == new_name);
        push(
            report,
            Finding {
                severity: if reused {
                    Severity::Breaking
                } else {
                    Severity::Info
                },
                direction: Direction::Both,
                rule: if reused {
                    "reserved.reused".into()
                } else {
                    "enum.value_added".into()
                },
                path: format!("{}.{}", new_enum.full_name, new_name),
                old_desc: if reused {
                    format!("reserved {number}")
                } else {
                    String::new()
                },
                new_desc: number.to_string(),
                location: String::new(),
            },
        );
    }
}

fn compare_service(
    report: &mut Report,
    _old_file: &str,
    _new_file: &str,
    old_service: &ServiceDesc,
    new_service: &ServiceDesc,
    _all_files: &BTreeMap<&str, &FileDesc>,
) {
    let old_methods: BTreeMap<&str, &crate::descriptor::MethodDesc> = old_service
        .methods
        .iter()
        .map(|m| (m.name.as_str(), m))
        .collect();
    let new_methods: BTreeMap<&str, &crate::descriptor::MethodDesc> = new_service
        .methods
        .iter()
        .map(|m| (m.name.as_str(), m))
        .collect();
    for (name, old_method) in &old_methods {
        match new_methods.get(name) {
            None => push(
                report,
                Finding {
                    severity: Severity::Breaking,
                    direction: Direction::Both,
                    rule: "method.removed".into(),
                    path: format!("{}/{}", new_service.full_name, name),
                    old_desc: method_text(old_method),
                    new_desc: String::new(),
                    location: String::new(),
                },
            ),
            Some(new_method) => {
                if old_method.input_type != new_method.input_type {
                    push(
                        report,
                        Finding {
                            severity: Severity::Breaking,
                            direction: Direction::Both,
                            rule: "method.input_changed".into(),
                            path: format!("{}/{}", new_service.full_name, name),
                            old_desc: old_method.input_type.clone(),
                            new_desc: new_method.input_type.clone(),
                            location: String::new(),
                        },
                    );
                }
                if old_method.output_type != new_method.output_type {
                    push(
                        report,
                        Finding {
                            severity: Severity::Breaking,
                            direction: Direction::Both,
                            rule: "method.output_changed".into(),
                            path: format!("{}/{}", new_service.full_name, name),
                            old_desc: old_method.output_type.clone(),
                            new_desc: new_method.output_type.clone(),
                            location: String::new(),
                        },
                    );
                }
                if old_method.client_streaming != new_method.client_streaming
                    || old_method.server_streaming != new_method.server_streaming
                {
                    push(
                        report,
                        Finding {
                            severity: Severity::Breaking,
                            direction: Direction::Both,
                            rule: "method.streaming_changed".into(),
                            path: format!("{}/{}", new_service.full_name, name),
                            old_desc: method_text(old_method),
                            new_desc: method_text(new_method),
                            location: String::new(),
                        },
                    );
                }
            }
        }
    }
    for name in new_methods.keys() {
        if !old_methods.contains_key(name) {
            push(
                report,
                Finding {
                    severity: Severity::Info,
                    direction: Direction::Both,
                    rule: "method.added".into(),
                    path: format!("{}/{}", new_service.full_name, name),
                    old_desc: String::new(),
                    new_desc: method_text(new_methods[name]),
                    location: String::new(),
                },
            );
        }
    }
}

fn method_text(method: &crate::descriptor::MethodDesc) -> String {
    let mut text = method.input_type.clone();
    if method.client_streaming {
        text.push_str(" (stream)");
    }
    text.push_str(" → ");
    text.push_str(&method.output_type);
    if method.server_streaming {
        text.push_str(" (stream)");
    }
    text
}

// base64 引用（workspace 无依赖，自带实现）
mod base64 {
    pub fn decode(text: &str) -> Result<Vec<u8>, &'static str> {
        fn value_of(byte: u8) -> Result<u32, &'static str> {
            match byte {
                b'A'..=b'Z' => Ok(u32::from(byte - b'A')),
                b'a'..=b'z' => Ok(u32::from(byte - b'a') + 26),
                b'0'..=b'9' => Ok(u32::from(byte - b'0') + 52),
                b'+' => Ok(62),
                b'/' => Ok(63),
                _ => Err("invalid character"),
            }
        }
        let cleaned: Vec<u8> = text.bytes().filter(|b| !b" \t\r\n".contains(b)).collect();
        let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
        let mut chunk = [0u32; 4];
        let mut filled = 0usize;
        for &byte in &cleaned {
            if byte == b'=' {
                break;
            }
            chunk[filled] = value_of(byte)?;
            filled += 1;
            if filled == 4 {
                out.push(((chunk[0] << 2) | (chunk[1] >> 4)) as u8);
                out.push((((chunk[1] & 0xf) << 4) | (chunk[2] >> 2)) as u8);
                out.push((((chunk[2] & 0x3) << 6) | chunk[3]) as u8);
                filled = 0;
            }
        }
        match filled {
            0 => {}
            2 => out.push(((chunk[0] << 2) | (chunk[1] >> 4)) as u8),
            3 => {
                out.push(((chunk[0] << 2) | (chunk[1] >> 4)) as u8);
                out.push((((chunk[1] & 0xf) << 4) | (chunk[2] >> 2)) as u8);
            }
            _ => return Err("truncated base64"),
        }
        Ok(out)
    }
}

#[cfg(test)]
#[path = "diff_test.rs"]
mod tests;
