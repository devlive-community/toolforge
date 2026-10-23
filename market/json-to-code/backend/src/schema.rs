//! 从 JSON 样例推断结构：数组元素合并、缺失字段标记为可选、null 标记为可空。

use indexmap::IndexMap;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// 尚无样本（例如空数组的元素）
    Unknown,
    Null,
    Bool,
    Int,
    Float,
    Str,
    Array(Box<Typed>),
    Object(Obj),
    /// 多种类型混合
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Typed {
    pub shape: Shape,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Obj {
    pub fields: IndexMap<String, Field>,
    /// 合并了多少个对象样本
    pub samples: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub typed: Typed,
    /// 出现该字段的样本数
    pub seen: usize,
}

impl Typed {
    fn of(shape: Shape) -> Self {
        let nullable = shape == Shape::Null;
        Self { shape, nullable }
    }
}

pub fn infer(value: &Value) -> Typed {
    Typed::of(match value {
        Value::Null => Shape::Null,
        Value::Bool(_) => Shape::Bool,
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                Shape::Int
            } else {
                Shape::Float
            }
        }
        Value::String(_) => Shape::Str,
        Value::Array(items) => {
            let element = items
                .iter()
                .map(infer)
                .fold(Typed::of(Shape::Unknown), merge);
            Shape::Array(Box::new(element))
        }
        Value::Object(map) => Shape::Object(Obj {
            fields: map
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        Field {
                            typed: infer(value),
                            seen: 1,
                        },
                    )
                })
                .collect(),
            samples: 1,
        }),
    })
}

fn merge_obj(mut a: Obj, b: Obj) -> Obj {
    for (key, field) in b.fields {
        match a.fields.get_mut(&key) {
            Some(existing) => {
                let typed = std::mem::replace(&mut existing.typed, Typed::of(Shape::Unknown));
                existing.typed = merge(typed, field.typed);
                existing.seen += field.seen;
            }
            None => {
                a.fields.insert(key, field);
            }
        }
    }
    a.samples += b.samples;
    a
}

pub fn merge(a: Typed, b: Typed) -> Typed {
    let nullable = a.nullable || b.nullable;
    let shape = match (a.shape, b.shape) {
        (Shape::Unknown, x) | (x, Shape::Unknown) => x,
        (Shape::Null, x) | (x, Shape::Null) => x,
        (Shape::Int, Shape::Float) | (Shape::Float, Shape::Int) => Shape::Float,
        (Shape::Array(x), Shape::Array(y)) => Shape::Array(Box::new(merge(*x, *y))),
        (Shape::Object(x), Shape::Object(y)) => Shape::Object(merge_obj(x, y)),
        (x, y) if x == y => x,
        _ => Shape::Any,
    };
    // 合并后仍只有 null 时保持 Null，由生成器输出「任意类型且可空」
    Typed { shape, nullable }
}

#[cfg(test)]
#[path = "schema_test.rs"]
mod tests;
