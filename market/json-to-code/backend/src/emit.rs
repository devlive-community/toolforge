//! 各语言的类型定义生成器。

use serde::Deserialize;

use crate::model::{FieldDef, Model, StructDef, Ty, TypeRef, camel, pascal, snake};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Typescript,
    Rust,
    Go,
    Java,
    Kotlin,
    Python,
    Csharp,
}

pub const LANGUAGES: [Language; 7] = [
    Language::Typescript,
    Language::Rust,
    Language::Go,
    Language::Java,
    Language::Kotlin,
    Language::Python,
    Language::Csharp,
];

impl Language {
    pub fn id(self) -> &'static str {
        match self {
            Self::Typescript => "typescript",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Python => "python",
            Self::Csharp => "csharp",
        }
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while",
];
const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "boolean",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "float",
    "for",
    "goto",
    "if",
    "import",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "void",
    "while",
    "record",
];
const KOTLIN_KEYWORDS: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "val",
    "var",
    "when",
    "while",
];
const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "false", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "none", "nonlocal", "not", "or", "pass", "raise", "return", "true", "try", "while",
    "with", "yield",
];

fn escape(name: String, keywords: &[&str]) -> String {
    if keywords.contains(&name.to_lowercase().as_str()) {
        format!("{name}_")
    } else {
        name
    }
}

fn is_ts_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""))
}

pub struct Options {
    pub root: String,
}

pub fn emit(model: &Model, language: Language, options: &Options) -> String {
    let body = match language {
        Language::Typescript => typescript(model, options),
        Language::Rust => rust(model, options),
        Language::Go => go(model, options),
        Language::Java => java(model, options),
        Language::Kotlin => kotlin(model, options),
        Language::Python => python(model, options),
        Language::Csharp => csharp(model, options),
    };
    body.trim_end().to_string() + "\n"
}

/// 根类型不是对象时（数组或基本类型）需要额外输出一个别名
fn root_alias(model: &Model) -> Option<&Ty> {
    (!matches!(model.root.kind, TypeRef::Struct(_))).then_some(&model.root)
}

// ---------------------------------------------------------------- TypeScript

fn ts_type(ty: &Ty) -> String {
    let base = match &ty.kind {
        TypeRef::Bool => "boolean".into(),
        TypeRef::Int | TypeRef::Float => "number".into(),
        TypeRef::Str => "string".into(),
        TypeRef::Any => "unknown".into(),
        TypeRef::Array(inner) => {
            let inner = ts_type(inner);
            if inner.contains(' ') {
                format!("({inner})[]")
            } else {
                format!("{inner}[]")
            }
        }
        TypeRef::Struct(name) => name.clone(),
    };
    if ty.nullable && ty.kind != TypeRef::Any {
        format!("{base} | null")
    } else {
        base
    }
}

fn typescript(model: &Model, options: &Options) -> String {
    let mut out = String::new();
    if let Some(root) = root_alias(model) {
        out += &format!(
            "export type {} = {};\n\n",
            pascal(&options.root),
            ts_type(root)
        );
    }
    for s in &model.structs {
        out += &format!("export interface {} {{\n", s.name);
        for f in &s.fields {
            let key = if is_ts_identifier(&f.key) {
                f.key.clone()
            } else {
                quote(&f.key)
            };
            let optional = if f.optional { "?" } else { "" };
            out += &format!("  {key}{optional}: {};\n", ts_type(&f.ty));
        }
        out += "}\n\n";
    }
    out
}

// ---------------------------------------------------------------- Rust

fn rust_type(ty: &Ty, wrap_option: bool) -> String {
    let base = match &ty.kind {
        TypeRef::Bool => "bool".into(),
        TypeRef::Int => "i64".into(),
        TypeRef::Float => "f64".into(),
        TypeRef::Str => "String".into(),
        TypeRef::Any => "serde_json::Value".into(),
        TypeRef::Array(inner) => format!("Vec<{}>", rust_type(inner, true)),
        TypeRef::Struct(name) => name.clone(),
    };
    if wrap_option && ty.nullable && ty.kind != TypeRef::Any {
        format!("Option<{base}>")
    } else {
        base
    }
}

fn rust(model: &Model, options: &Options) -> String {
    let mut out = String::from("use serde::{Deserialize, Serialize};\n\n");
    if let Some(root) = root_alias(model) {
        out += &format!(
            "pub type {} = {};\n\n",
            pascal(&options.root),
            rust_type(root, true)
        );
    }
    for s in &model.structs {
        out += "#[derive(Debug, Clone, Serialize, Deserialize)]\n";
        out += &format!("pub struct {} {{\n", s.name);
        for f in &s.fields {
            let name = snake(&f.key);
            let (name, raw) = if RUST_KEYWORDS.contains(&name.as_str()) {
                (format!("r#{name}"), name)
            } else {
                (name.clone(), name)
            };
            if raw != f.key {
                out += &format!("    #[serde(rename = {})]\n", quote(&f.key));
            }
            let inner = rust_type(
                &Ty {
                    kind: f.ty.kind.clone(),
                    nullable: false,
                },
                true,
            );
            // serde_json::Value 本身可以表示 null，只有缺失时才需要 Option
            let wrap = f.optional || (f.ty.nullable && f.ty.kind != TypeRef::Any);
            let ty = if wrap {
                format!("Option<{inner}>")
            } else {
                inner
            };
            if f.optional {
                out += "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n";
            }
            out += &format!("    pub {name}: {ty},\n");
        }
        out += "}\n\n";
    }
    out
}

// ---------------------------------------------------------------- Go

fn go_type(ty: &Ty) -> String {
    let base = match &ty.kind {
        TypeRef::Bool => "bool".into(),
        TypeRef::Int => "int64".into(),
        TypeRef::Float => "float64".into(),
        TypeRef::Str => "string".into(),
        TypeRef::Any => "any".into(),
        TypeRef::Array(inner) => format!("[]{}", go_type(inner)),
        TypeRef::Struct(name) => name.clone(),
    };
    // 切片与 any 本身可为 nil，其余可空类型使用指针
    if ty.nullable && !matches!(ty.kind, TypeRef::Any | TypeRef::Array(_)) {
        format!("*{base}")
    } else {
        base
    }
}

/// Go 惯例：常见缩写全大写（ID、URL、HTTP…）
const GO_INITIALISMS: &[&str] = &[
    "Api", "Ascii", "Cpu", "Css", "Dns", "Html", "Http", "Https", "Id", "Ip", "Json", "Sql", "Ssh",
    "Tcp", "Tls", "Ttl", "Udp", "Ui", "Uid", "Uri", "Url", "Utf8", "Uuid", "Xml",
];

fn go_name(key: &str) -> String {
    let name: String = crate::model::words(key)
        .iter()
        .map(|word| {
            let pascal = pascal(word);
            if GO_INITIALISMS.contains(&pascal.as_str()) {
                pascal.to_uppercase()
            } else {
                pascal
            }
        })
        .collect();
    if name.is_empty() {
        "Field".into()
    } else if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("N{name}")
    } else {
        name
    }
}

fn go(model: &Model, options: &Options) -> String {
    let mut out = String::new();
    if let Some(root) = root_alias(model) {
        out += &format!("type {} {}\n\n", pascal(&options.root), go_type(root));
    }
    for s in &model.structs {
        out += &format!("type {} struct {{\n", s.name);
        // 与 gofmt 一致：字段名与类型两列对齐
        let rows: Vec<(String, String, String)> = s
            .fields
            .iter()
            .map(|f| {
                let omit = if f.optional { ",omitempty" } else { "" };
                (
                    go_name(&f.key),
                    go_type(&f.ty),
                    format!("`json:\"{}{omit}\"`", f.key),
                )
            })
            .collect();
        let name_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
        let type_width = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
        for (name, ty, tag) in rows {
            out += &format!("\t{name:name_width$} {ty:type_width$} {tag}\n");
        }
        out += "}\n\n";
    }
    out
}

// ---------------------------------------------------------------- Java

fn java_type(ty: &Ty, boxed: bool) -> String {
    let boxed = boxed || ty.nullable;
    match &ty.kind {
        TypeRef::Bool => if boxed { "Boolean" } else { "boolean" }.into(),
        TypeRef::Int => if boxed { "Long" } else { "long" }.into(),
        TypeRef::Float => if boxed { "Double" } else { "double" }.into(),
        TypeRef::Str => "String".into(),
        TypeRef::Any => "Object".into(),
        TypeRef::Array(inner) => format!("List<{}>", java_type(inner, true)),
        TypeRef::Struct(name) => name.clone(),
    }
}

fn java(model: &Model, options: &Options) -> String {
    let mut out = String::from(
        "import com.fasterxml.jackson.annotation.JsonProperty;\nimport java.util.List;\n\n",
    );
    if let Some(root) = root_alias(model) {
        out += &format!(
            "// {} = {}\n\n",
            pascal(&options.root),
            java_type(root, true)
        );
    }
    for s in &model.structs {
        out += &format!("public record {}(\n", s.name);
        let fields: Vec<String> = s
            .fields
            .iter()
            .map(|f| {
                let name = escape(camel(&f.key), JAVA_KEYWORDS);
                let annotation = if name != f.key {
                    format!("@JsonProperty({}) ", quote(&f.key))
                } else {
                    String::new()
                };
                format!("    {annotation}{} {name}", java_type(&f.ty, f.optional))
            })
            .collect();
        out += &fields.join(",\n");
        out += "\n) {}\n\n";
    }
    out
}

// ---------------------------------------------------------------- Kotlin

fn kotlin_type(ty: &Ty) -> String {
    let base = match &ty.kind {
        TypeRef::Bool => "Boolean".into(),
        TypeRef::Int => "Long".into(),
        TypeRef::Float => "Double".into(),
        TypeRef::Str => "String".into(),
        TypeRef::Any => "JsonElement".into(),
        TypeRef::Array(inner) => format!("List<{}>", kotlin_type(inner)),
        TypeRef::Struct(name) => name.clone(),
    };
    if ty.nullable {
        format!("{base}?")
    } else {
        base
    }
}

fn kotlin(model: &Model, options: &Options) -> String {
    let mut out = String::from(
        "import kotlinx.serialization.SerialName\nimport kotlinx.serialization.Serializable\nimport kotlinx.serialization.json.JsonElement\n\n",
    );
    if let Some(root) = root_alias(model) {
        out += &format!(
            "typealias {} = {}\n\n",
            pascal(&options.root),
            kotlin_type(root)
        );
    }
    for s in &model.structs {
        out += &format!("@Serializable\ndata class {}(\n", s.name);
        let fields: Vec<String> = s
            .fields
            .iter()
            .map(|f| {
                let name = camel(&f.key);
                let name = if KOTLIN_KEYWORDS.contains(&name.as_str()) {
                    format!("`{name}`")
                } else {
                    name
                };
                let annotation = if name.trim_matches('`') != f.key {
                    format!("@SerialName({}) ", quote(&f.key))
                } else {
                    String::new()
                };
                let mut ty = kotlin_type(&f.ty);
                let default = if f.optional {
                    if !ty.ends_with('?') {
                        ty.push('?');
                    }
                    " = null"
                } else {
                    ""
                };
                format!("    {annotation}val {name}: {ty}{default}")
            })
            .collect();
        out += &fields.join(",\n");
        out += ",\n)\n\n";
    }
    out
}

// ---------------------------------------------------------------- Python

fn python_type(ty: &Ty) -> String {
    let base = match &ty.kind {
        TypeRef::Bool => "bool".into(),
        TypeRef::Int => "int".into(),
        TypeRef::Float => "float".into(),
        TypeRef::Str => "str".into(),
        TypeRef::Any => "Any".into(),
        TypeRef::Array(inner) => format!("list[{}]", python_type(inner)),
        TypeRef::Struct(name) => name.clone(),
    };
    if ty.nullable && ty.kind != TypeRef::Any {
        format!("{base} | None")
    } else {
        base
    }
}

fn python(model: &Model, options: &Options) -> String {
    let mut out = String::from(
        "from __future__ import annotations\n\nfrom typing import Any\n\nfrom pydantic import BaseModel, Field\n\n\n",
    );
    // pydantic 需要先定义被引用的类型：子类型在前
    for s in model.structs.iter().rev() {
        out += &format!("class {}(BaseModel):\n", s.name);
        if s.fields.is_empty() {
            out += "    pass\n";
        }
        for f in &s.fields {
            let name = escape(snake(&f.key), PYTHON_KEYWORDS);
            let mut ty = python_type(&f.ty);
            let mut args = Vec::new();
            if f.optional {
                if !ty.ends_with("None") && ty != "Any" {
                    ty = format!("{ty} | None");
                }
                args.push("default=None".to_string());
            }
            if name != f.key {
                args.push(format!("alias={}", quote(&f.key)));
            }
            if args.is_empty() {
                out += &format!("    {name}: {ty}\n");
            } else {
                out += &format!("    {name}: {ty} = Field({})\n", args.join(", "));
            }
        }
        out += "\n\n";
    }
    if let Some(root) = root_alias(model) {
        out += &format!("{} = {}\n", pascal(&options.root), python_type(root));
    }
    out
}

// ---------------------------------------------------------------- C#

fn csharp_type(ty: &Ty) -> String {
    let base = match &ty.kind {
        TypeRef::Bool => "bool".into(),
        TypeRef::Int => "long".into(),
        TypeRef::Float => "double".into(),
        TypeRef::Str => "string".into(),
        TypeRef::Any => "object".into(),
        TypeRef::Array(inner) => format!("List<{}>", csharp_type(inner)),
        TypeRef::Struct(name) => name.clone(),
    };
    if ty.nullable {
        format!("{base}?")
    } else {
        base
    }
}

fn csharp_field(f: &FieldDef, class: &StructDef) -> String {
    let mut name = pascal(&f.key);
    // 属性名不能与类名相同
    if name == class.name {
        name.push('_');
    }
    let mut ty = csharp_type(&f.ty);
    if f.optional && !ty.ends_with('?') {
        ty.push('?');
    }
    let required = if !f.optional && !f.ty.nullable && ty == "string" {
        " = string.Empty;"
    } else if !f.optional
        && !f.ty.nullable
        && matches!(f.ty.kind, TypeRef::Array(_) | TypeRef::Struct(_))
    {
        // 非空的引用类型需要初始值，避免可空性警告 CS8618
        " = new();"
    } else {
        ""
    };
    format!(
        "    [JsonPropertyName({})]\n    public {ty} {name} {{ get; set; }}{required}\n",
        quote(&f.key)
    )
}

fn csharp(model: &Model, options: &Options) -> String {
    let mut out = String::from(
        "using System.Collections.Generic;\nusing System.Text.Json.Serialization;\n\n",
    );
    if let Some(root) = root_alias(model) {
        out += &format!("// {} = {}\n\n", pascal(&options.root), csharp_type(root));
    }
    for s in &model.structs {
        out += &format!("public class {}\n{{\n", s.name);
        let fields: Vec<String> = s.fields.iter().map(|f| csharp_field(f, s)).collect();
        out += &fields.join("\n");
        out += "}\n\n";
    }
    out
}

#[cfg(test)]
#[path = "emit_test.rs"]
mod tests;
