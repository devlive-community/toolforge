//! .editorconfig 文件解析（规范 0.17）：整行注释以 # 或 ; 开头（不支持行尾注释）；
//! `[glob]` 开始一个 section；`键 = 值`，键不区分大小写；首个 section 之前只认 root。

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    /// 小写后的键
    pub key: String,
    /// 原始值（去掉首尾空白）
    pub value: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub glob: String,
    pub line: usize,
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// invalidLine / pairOutsideSection / invalidRoot / emptySection / invalidGlob
    pub kind: &'static str,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Config {
    pub root: bool,
    pub sections: Vec<Section>,
    pub problems: Vec<Problem>,
}

pub fn parse(text: &str) -> Config {
    let mut config = Config::default();
    // 去掉 UTF-8 BOM
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if inner.is_empty() {
                config.problems.push(Problem {
                    kind: "emptySection",
                    line,
                    text: trimmed.to_owned(),
                });
            }
            config.sections.push(Section {
                glob: inner.to_owned(),
                line,
                pairs: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = trimmed
            .split_once('=')
            .filter(|(k, _)| !k.trim().is_empty())
        else {
            config.problems.push(Problem {
                kind: "invalidLine",
                line,
                text: trimmed.to_owned(),
            });
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim().to_owned();
        match config.sections.last_mut() {
            Some(section) => section.pairs.push(Pair { key, value, line }),
            None if key == "root" => match value.to_lowercase().as_str() {
                "true" => config.root = true,
                "false" => config.root = false,
                _ => config.problems.push(Problem {
                    kind: "invalidRoot",
                    line,
                    text: trimmed.to_owned(),
                }),
            },
            None => config.problems.push(Problem {
                kind: "pairOutsideSection",
                line,
                text: trimmed.to_owned(),
            }),
        }
    }
    config
}

#[cfg(test)]
#[path = "config_test.rs"]
mod tests;
