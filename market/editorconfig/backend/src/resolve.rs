//! 计算目标文件的生效规则。
//!
//! 规范：从目标文件所在目录逐级向上读取 .editorconfig，遇到 root = true 的文件后停止；
//! 越近的文件优先，同一文件中靠后的 section 优先；值为 unset 时删除该属性。
//! 这里只在用户选择的项目目录内读取；到项目根仍未遇到 root = true 时标记为上下文不完整。
//!
//! 规范要求的推导：indent_style = tab 且未设 indent_size 时 indent_size = tab；
//! indent_size 为数字且未设 tab_width 时 tab_width = indent_size；indent_size = tab 且设了 tab_width 时 indent_size = tab_width。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::config::{self, Problem};
use crate::glob::Glob;

pub const SPEC_VERSION: &str = "0.17.2";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInput {
    /// 所在目录，相对项目根，以 / 分隔，根目录为空
    pub dir: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 目标文件，相对项目根
    pub targets: Vec<String>,
    /// 读到的 .editorconfig（不存在的目录不必传）
    pub configs: Vec<ConfigInput>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// .editorconfig 所在目录（相对项目根）
    pub dir: String,
    pub line: usize,
    pub section: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub value: String,
    pub source: Source,
    /// 这一步是 unset
    pub unset: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Property {
    pub key: String,
    /// 最终值；被 unset 删除时为 None
    pub value: Option<String>,
    /// standard / extension / derived
    pub kind: &'static str,
    /// 标准属性的值不在规范允许的范围内（原文保留）
    pub invalid: bool,
    pub source: Option<Source>,
    /// 按应用顺序的全部赋值（最后一个生效）
    pub trace: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SectionHit {
    pub dir: String,
    pub line: usize,
    pub glob: String,
    pub matched: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileProblem {
    pub dir: String,
    #[serde(flatten)]
    pub problem: Problem,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetResult {
    pub path: String,
    /// 读取顺序（由远到近）中参与的 .editorconfig 目录
    pub files: Vec<String>,
    /// 停在哪个目录的 root = true；None 表示到项目根仍未遇到
    pub root_dir: Option<String>,
    /// 项目根之上可能还有配置未读取
    pub incomplete: bool,
    pub sections: Vec<SectionHit>,
    pub properties: Vec<Property>,
    /// 规范标准属性中没有设置的
    pub unset_standard: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub spec_version: &'static str,
    pub targets: Vec<TargetResult>,
    pub problems: Vec<FileProblem>,
}

/// 规范中的标准属性及其合法值检查
pub const STANDARD: &[&str] = &[
    "indent_style",
    "indent_size",
    "tab_width",
    "end_of_line",
    "charset",
    "trim_trailing_whitespace",
    "insert_final_newline",
];

fn positive(value: &str) -> bool {
    value.parse::<u32>().is_ok_and(|n| n > 0)
}

fn valid(key: &str, value: &str) -> bool {
    match key {
        "indent_style" => matches!(value, "tab" | "space"),
        "indent_size" => value == "tab" || positive(value),
        "tab_width" => positive(value),
        "end_of_line" => matches!(value, "lf" | "cr" | "crlf"),
        "charset" => matches!(
            value,
            "latin1" | "utf-8" | "utf-8-bom" | "utf-16be" | "utf-16le"
        ),
        "trim_trailing_whitespace" | "insert_final_newline" => matches!(value, "true" | "false"),
        _ => true,
    }
}

/// 规范化相对路径：统一为 /，去掉 . 段；含 .. 或为绝对路径时拒绝
pub fn normalize(path: &str) -> PluginResult<String> {
    let unified = path.trim().replace('\\', "/");
    if unified.starts_with('/') || unified.get(1..2) == Some(":") {
        return Err(PluginError::new("ec.absolute_path").with("path", path));
    }
    let mut parts = Vec::new();
    for segment in unified.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(PluginError::new("ec.outside_root").with("path", path)),
            s => parts.push(s),
        }
    }
    if parts.is_empty() {
        return Err(PluginError::new("ec.empty_path"));
    }
    Ok(parts.join("/"))
}

/// 目标文件所在目录到项目根的各级目录（由近到远），根目录为空字符串
pub fn ancestors(target: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut current = target;
    while let Some(slash) = current.rfind('/') {
        current = &current[..slash];
        dirs.push(current.to_owned());
    }
    dirs.push(String::new());
    dirs
}

/// 目标相对 .editorconfig 所在目录的路径
fn relative_to(target: &str, dir: &str) -> String {
    if dir.is_empty() {
        target.to_owned()
    } else {
        target[dir.len() + 1..].to_owned()
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.targets.is_empty() {
        return Err(PluginError::new("ec.no_targets"));
    }
    let mut parsed: BTreeMap<String, config::Config> = BTreeMap::new();
    for input in &args.configs {
        let dir = if input.dir.trim().is_empty() {
            String::new()
        } else {
            normalize(&input.dir)?
        };
        parsed.insert(dir, config::parse(&input.text));
    }
    let mut problems = Vec::new();
    // glob 编译一次
    let mut globs: BTreeMap<(String, usize), Option<Glob>> = BTreeMap::new();
    for (dir, cfg) in &parsed {
        for problem in &cfg.problems {
            problems.push(FileProblem {
                dir: dir.clone(),
                problem: problem.clone(),
            });
        }
        for section in &cfg.sections {
            let glob = Glob::new(&section.glob);
            if glob.is_none() {
                problems.push(FileProblem {
                    dir: dir.clone(),
                    problem: Problem {
                        kind: "invalidGlob",
                        line: section.line,
                        text: section.glob.clone(),
                    },
                });
            }
            globs.insert((dir.clone(), section.line), glob);
        }
    }

    let mut targets = Vec::new();
    for raw in &args.targets {
        let path = normalize(raw)?;
        // 由近到远收集，遇到 root = true 停止
        let mut chain: Vec<&String> = Vec::new();
        let mut root_dir = None;
        for dir in ancestors(&path) {
            if let Some((key, cfg)) = parsed.get_key_value(&dir) {
                chain.push(key);
                if cfg.root {
                    root_dir = Some(dir);
                    break;
                }
            }
        }
        chain.reverse();

        let mut sections = Vec::new();
        let mut props: BTreeMap<String, Vec<Step>> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        for dir in &chain {
            let cfg = &parsed[*dir];
            let relative = relative_to(&path, dir);
            for section in &cfg.sections {
                let matched = globs
                    .get(&((*dir).clone(), section.line))
                    .and_then(Option::as_ref)
                    .is_some_and(|g| g.matches(&relative));
                sections.push(SectionHit {
                    dir: (*dir).clone(),
                    line: section.line,
                    glob: section.glob.clone(),
                    matched,
                });
                if !matched {
                    continue;
                }
                for pair in &section.pairs {
                    let known = STANDARD.contains(&pair.key.as_str());
                    // 标准属性的值不区分大小写
                    let value = if known {
                        pair.value.to_lowercase()
                    } else {
                        pair.value.clone()
                    };
                    let unset = value.eq_ignore_ascii_case("unset");
                    if !props.contains_key(&pair.key) {
                        order.push(pair.key.clone());
                    }
                    props.entry(pair.key.clone()).or_default().push(Step {
                        value,
                        source: Source {
                            dir: (*dir).clone(),
                            line: pair.line,
                            section: section.glob.clone(),
                        },
                        unset,
                    });
                }
            }
        }

        let mut properties: Vec<Property> = order
            .iter()
            .map(|key| {
                let trace = props.remove(key).unwrap_or_default();
                let last = trace.last().cloned();
                let (value, source) = match last {
                    Some(step) if !step.unset => (Some(step.value), Some(step.source)),
                    _ => (None, None),
                };
                let standard = STANDARD.contains(&key.as_str());
                Property {
                    invalid: standard && value.as_deref().is_some_and(|v| !valid(key, v)),
                    key: key.clone(),
                    value,
                    kind: if standard { "standard" } else { "extension" },
                    source,
                    trace,
                }
            })
            .collect();
        derive(&mut properties);

        let unset_standard = STANDARD
            .iter()
            .copied()
            .filter(|key| {
                !properties
                    .iter()
                    .any(|p| p.key == *key && p.value.is_some())
            })
            .collect();
        targets.push(TargetResult {
            path,
            files: chain.into_iter().cloned().collect(),
            incomplete: root_dir.is_none(),
            root_dir,
            sections,
            properties,
            unset_standard,
        });
    }
    Ok(Output {
        spec_version: SPEC_VERSION,
        targets,
        problems,
    })
}

fn value_of<'a>(props: &'a [Property], key: &str) -> Option<&'a str> {
    props
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| p.value.as_deref())
}

fn set_derived(props: &mut Vec<Property>, key: &str, value: String) {
    match props.iter_mut().find(|p| p.key == key) {
        Some(existing) => {
            existing.value = Some(value);
            existing.kind = "derived";
            existing.source = None;
            existing.invalid = false;
        }
        None => props.push(Property {
            key: key.to_owned(),
            value: Some(value),
            kind: "derived",
            invalid: false,
            source: None,
            trace: Vec::new(),
        }),
    }
}

/// 规范要求的属性推导
fn derive(props: &mut Vec<Property>) {
    let style = value_of(props, "indent_style").map(str::to_owned);
    let size = value_of(props, "indent_size").map(str::to_owned);
    let tab_width = value_of(props, "tab_width").map(str::to_owned);
    if style.as_deref() == Some("tab") && size.is_none() {
        set_derived(props, "indent_size", "tab".to_owned());
    }
    if let Some(size) = &size
        && positive(size)
        && tab_width.is_none()
    {
        set_derived(props, "tab_width", size.clone());
    }
    if size.as_deref() == Some("tab")
        && let Some(width) = tab_width
    {
        set_derived(props, "indent_size", width);
    }
}

#[cfg(test)]
#[path = "resolve_test.rs"]
mod tests;
