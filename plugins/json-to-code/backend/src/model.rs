//! 把推断出的结构整理成具名类型，供各语言生成器使用。

use std::collections::HashSet;

use crate::schema::{Shape, Typed};

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Bool,
    Int,
    Float,
    Str,
    Any,
    Array(Box<Ty>),
    Struct(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ty {
    pub kind: TypeRef,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    /// JSON 中的原始键
    pub key: String,
    pub ty: Ty,
    /// 部分样本中缺失
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub root: Ty,
    /// 根类型在前，其余按首次出现的顺序
    pub structs: Vec<StructDef>,
}

/// 按单词边界拆分：非字母数字字符、小写→大写、字母↔数字
pub fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some(&prev) = current.chars().last().as_ref() {
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_uppercase() && c.is_uppercase() && next_lower)
                || (prev.is_ascii_digit() != c.is_ascii_digit());
            if boundary {
                words.push(std::mem::take(&mut current));
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn capitalize(word: &str) -> String {
    let lower = word.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn pascal(text: &str) -> String {
    let out: String = words(text).iter().map(|w| capitalize(w)).collect();
    if out.is_empty() {
        "Field".into()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("N{out}")
    } else {
        out
    }
}

pub fn camel(text: &str) -> String {
    let words = words(text);
    let Some((first, rest)) = words.split_first() else {
        return "field".into();
    };
    let out: String = std::iter::once(first.to_lowercase())
        .chain(rest.iter().map(|w| capitalize(w)))
        .collect();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("n{out}")
    } else {
        out
    }
}

pub fn snake(text: &str) -> String {
    let out = words(text)
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_");
    if out.is_empty() {
        "field".into()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("n{out}")
    } else {
        out
    }
}

/// 简单的英文单数化，用于数组元素的类型名
pub fn singular(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.ends_with("ies") && name.len() > 3 {
        format!("{}y", &name[..name.len() - 3])
    } else if lower.ends_with("sses")
        || lower.ends_with("xes")
        || lower.ends_with("ches")
        || lower.ends_with("shes")
    {
        name[..name.len() - 2].to_string()
    } else if lower.ends_with('s') && !lower.ends_with("ss") && name.len() > 1 {
        name[..name.len() - 1].to_string()
    } else {
        format!("{name}Item")
    }
}

struct Builder {
    structs: Vec<StructDef>,
    used: HashSet<String>,
}

impl Builder {
    fn unique(&mut self, base: &str) -> String {
        let mut name = base.to_string();
        let mut n = 2;
        while self.used.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        self.used.insert(name.clone());
        name
    }

    fn ty(&mut self, typed: &Typed, name_hint: &str) -> Ty {
        let kind = match &typed.shape {
            Shape::Bool => TypeRef::Bool,
            Shape::Int => TypeRef::Int,
            Shape::Float => TypeRef::Float,
            Shape::Str => TypeRef::Str,
            Shape::Unknown | Shape::Null | Shape::Any => TypeRef::Any,
            Shape::Array(element) => {
                TypeRef::Array(Box::new(self.ty(element, &singular(name_hint))))
            }
            Shape::Object(obj) => {
                let name = self.unique(&pascal(name_hint));
                // 先占位保证父类型排在子类型前面
                let index = self.structs.len();
                self.structs.push(StructDef {
                    name: name.clone(),
                    fields: Vec::new(),
                });
                let fields = obj
                    .fields
                    .iter()
                    .map(|(key, field)| FieldDef {
                        key: key.clone(),
                        ty: self.ty(&field.typed, key),
                        optional: field.seen < obj.samples,
                    })
                    .collect();
                self.structs[index].fields = fields;
                TypeRef::Struct(name)
            }
        };
        Ty {
            kind,
            nullable: typed.nullable,
        }
    }
}

pub fn build(typed: &Typed, root_name: &str) -> Model {
    let mut builder = Builder {
        structs: Vec::new(),
        used: HashSet::new(),
    };
    let root = builder.ty(typed, root_name);
    Model {
        root,
        structs: builder.structs,
    }
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
