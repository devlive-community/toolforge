use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase, ToTrainCase,
    ToUpperCamelCase,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Args {
    input: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Converted {
    pub style: &'static str,
    pub output: String,
}

pub const STYLES: &[&str] = &[
    "camel", "pascal", "snake", "kebab", "constant", "train", "dot", "path", "title", "sentence",
    "lower", "upper", "swap",
];

/// 句子格式：首字母大写，其余小写
fn sentence(line: &str) -> String {
    let lower = line.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn swap(line: &str) -> String {
    line.chars()
        .flat_map(|c| -> Box<dyn Iterator<Item = char>> {
            if c.is_uppercase() {
                Box::new(c.to_lowercase())
            } else if c.is_lowercase() {
                Box::new(c.to_uppercase())
            } else {
                Box::new(std::iter::once(c))
            }
        })
        .collect()
}

pub fn convert(line: &str, style: &str) -> String {
    match style {
        "camel" => line.to_lower_camel_case(),
        "pascal" => line.to_upper_camel_case(),
        "snake" => line.to_snake_case(),
        "kebab" => line.to_kebab_case(),
        "constant" => line.to_shouty_snake_case(),
        "train" => line.to_train_case(),
        "dot" => line.to_snake_case().replace('_', "."),
        "path" => line.to_snake_case().replace('_', "/"),
        "title" => line.to_title_case(),
        "sentence" => sentence(line),
        "lower" => line.to_lowercase(),
        "upper" => line.to_uppercase(),
        "swap" => swap(line),
        _ => line.to_owned(),
    }
}

/// 按行转换全部风格，保留换行结构
pub fn all(args: Args) -> Vec<Converted> {
    STYLES
        .iter()
        .map(|style| Converted {
            style,
            output: args
                .input
                .split('\n')
                .map(|line| convert(line.trim_end_matches('\r'), style))
                .collect::<Vec<_>>()
                .join("\n"),
        })
        .collect()
}

#[cfg(test)]
#[path = "case_test.rs"]
mod tests;
