//! 重命名规则：每条规则把文件名（主名 + 扩展名）变换为新的文件名，按顺序依次应用。

use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

/// 规则作用的部分
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    /// 主名（不含扩展名）
    #[default]
    Stem,
    Ext,
    /// 完整文件名
    Full,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    #[default]
    Start,
    End,
    /// 从开头数第 index 个字符之前
    Index,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaseMode {
    Lower,
    Upper,
    Title,
    Sentence,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExtMode {
    Lower,
    Upper,
    Set,
    Remove,
}

fn one() -> i64 {
    1
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Rule {
    #[serde(rename_all = "camelCase")]
    Replace {
        find: String,
        #[serde(default)]
        replace: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        part: Part,
    },
    #[serde(rename_all = "camelCase")]
    Insert {
        text: String,
        #[serde(default)]
        place: Place,
        #[serde(default)]
        index: usize,
        #[serde(default)]
        part: Part,
    },
    #[serde(rename_all = "camelCase")]
    Remove {
        #[serde(default)]
        place: Place,
        /// place 为 index 时的起始位置
        #[serde(default)]
        index: usize,
        count: usize,
        #[serde(default)]
        part: Part,
    },
    #[serde(rename_all = "camelCase")]
    Case {
        mode: CaseMode,
        #[serde(default)]
        part: Part,
    },
    #[serde(rename_all = "camelCase")]
    Number {
        #[serde(default = "one")]
        start: i64,
        #[serde(default = "one")]
        step: i64,
        #[serde(default)]
        pad: usize,
        #[serde(default)]
        place: Place,
        #[serde(default)]
        separator: String,
    },
    /// 用模板生成主名，支持 {name} {ext} {n} {n:3} {parent} {date} {date:%Y%m%d} {exif} {exif:…}
    #[serde(rename_all = "camelCase")]
    Template { pattern: String },
    #[serde(rename_all = "camelCase")]
    Extension {
        mode: ExtMode,
        #[serde(default)]
        value: String,
    },
    #[serde(rename_all = "camelCase")]
    Clean {
        /// 连续空白合并为一个空格
        #[serde(default = "yes")]
        collapse_spaces: bool,
        /// 空格替换为该字符串（为空时不替换）
        #[serde(default)]
        spaces_to: String,
        #[serde(default = "yes")]
        trim: bool,
        /// 去掉 Windows 不允许的字符 <>:"/\|?*
        #[serde(default = "yes")]
        remove_illegal: bool,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(flatten)]
    pub rule: Rule,
}

/// 文件名的两部分；以点开头且没有其他点的名字（如 .gitignore）没有扩展名
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub stem: String,
    pub ext: String,
}

impl Name {
    pub fn parse(full: &str) -> Self {
        match full.rfind('.') {
            // 以点结尾（如 "archive."）时整个名字都是主名
            Some(dot) if dot > 0 && dot + 1 < full.len() => Self {
                stem: full[..dot].to_owned(),
                ext: full[dot + 1..].to_owned(),
            },
            _ => Self {
                stem: full.to_owned(),
                ext: String::new(),
            },
        }
    }

    pub fn full(&self) -> String {
        if self.ext.is_empty() {
            self.stem.clone()
        } else {
            format!("{}.{}", self.stem, self.ext)
        }
    }

    fn apply(&mut self, part: Part, f: impl FnOnce(&str) -> String) {
        match part {
            Part::Stem => self.stem = f(&self.stem),
            Part::Ext => self.ext = f(&self.ext),
            Part::Full => *self = Self::parse(&f(&self.full())),
        }
    }
}

/// 生成模板与编号所需的文件信息
pub trait Context {
    /// 排序后的序号，从 0 开始
    fn index(&self) -> usize;
    fn original(&self) -> &Name;
    fn parent(&self) -> &str;
    fn modified(&self) -> Option<jiff::Zoned>;
    /// EXIF 拍摄时间，没有时返回 None
    fn captured(&self) -> Option<jiff::Zoned>;
}

/// 编译后的规则：正则只编译一次
pub enum Compiled {
    Replace {
        pattern: Regex,
        replace: String,
        part: Part,
    },
    Other(Rule),
}

pub fn compile(steps: &[Step]) -> PluginResult<Vec<Compiled>> {
    steps
        .iter()
        .filter(|s| s.enabled)
        .map(|step| match &step.rule {
            Rule::Replace {
                find,
                replace,
                regex,
                case_sensitive,
                part,
            } => {
                let source = if *regex {
                    find.clone()
                } else {
                    regex::escape(find)
                };
                let pattern = RegexBuilder::new(&source)
                    .case_insensitive(!case_sensitive)
                    .build()
                    .map_err(|e| {
                        PluginError::new("rename.invalid_regex").with("detail", e.to_string())
                    })?;
                // 普通文本替换时 $ 不作为分组引用
                let replace = if *regex {
                    replace.clone()
                } else {
                    replace.replace('$', "$$")
                };
                Ok(Compiled::Replace {
                    pattern,
                    replace,
                    part: *part,
                })
            }
            Rule::Template { pattern } => {
                validate_template(pattern)?;
                Ok(Compiled::Other(step.rule.clone()))
            }
            other => Ok(Compiled::Other(other.clone())),
        })
        .collect()
}

fn char_index(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(i, _)| i)
}

fn insert_at(text: &str, insert: &str, place: Place, index: usize) -> String {
    match place {
        Place::Start => format!("{insert}{text}"),
        Place::End => format!("{text}{insert}"),
        Place::Index => {
            let at = char_index(text, index);
            format!("{}{insert}{}", &text[..at], &text[at..])
        }
    }
}

fn remove(text: &str, place: Place, index: usize, count: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let (from, to) = match place {
        Place::Start => (0, count.min(len)),
        Place::End => (len.saturating_sub(count), len),
        Place::Index => (index.min(len), (index + count).min(len)),
    };
    chars[..from].iter().chain(&chars[to..]).collect()
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

fn change_case(text: &str, mode: CaseMode) -> String {
    match mode {
        CaseMode::Lower => text.to_lowercase(),
        CaseMode::Upper => text.to_uppercase(),
        CaseMode::Sentence => capitalize(text),
        CaseMode::Title => {
            let mut out = String::with_capacity(text.len());
            let mut word = String::new();
            for c in text.chars() {
                if c.is_whitespace() || c == '_' || c == '-' || c == '.' {
                    out.push_str(&capitalize(&word));
                    word.clear();
                    out.push(c);
                } else {
                    word.push(c);
                }
            }
            out.push_str(&capitalize(&word));
            out
        }
    }
}

const ILLEGAL: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

fn clean(text: &str, collapse: bool, spaces_to: &str, trim: bool, remove_illegal: bool) -> String {
    let mut out: String = if remove_illegal {
        text.chars()
            .filter(|c| !ILLEGAL.contains(c) && !c.is_control())
            .collect()
    } else {
        text.to_owned()
    };
    if collapse {
        let mut collapsed = String::with_capacity(out.len());
        let mut previous_space = false;
        for c in out.chars() {
            if c.is_whitespace() {
                if !previous_space {
                    collapsed.push(' ');
                }
                previous_space = true;
            } else {
                collapsed.push(c);
                previous_space = false;
            }
        }
        out = collapsed;
    }
    if trim {
        out = out.trim().to_owned();
    }
    if !spaces_to.is_empty() {
        out = out.replace(' ', spaces_to);
    }
    out
}

fn format_number(value: i64, pad: usize) -> String {
    if value < 0 {
        format!("-{:0pad$}", value.unsigned_abs())
    } else {
        format!("{value:0pad$}")
    }
}

/// 模板中的占位符，返回 (名称, 参数)
fn tokens(pattern: &str) -> Vec<(usize, usize, &str, Option<&str>)> {
    let mut out = Vec::new();
    let mut rest = 0;
    while let Some(open) = pattern[rest..].find('{') {
        let open = rest + open;
        let Some(close) = pattern[open..].find('}') else {
            break;
        };
        let close = open + close;
        let inner = &pattern[open + 1..close];
        let (name, arg) = match inner.split_once(':') {
            Some((name, arg)) => (name, Some(arg)),
            None => (inner, None),
        };
        out.push((open, close + 1, name, arg));
        rest = close + 1;
    }
    out
}

fn validate_template(pattern: &str) -> PluginResult<()> {
    for (_, _, name, arg) in tokens(pattern) {
        match name {
            "name" | "ext" | "parent" => {}
            "n" => {
                if let Some(arg) = arg
                    && arg.parse::<usize>().is_err()
                {
                    return Err(PluginError::new("rename.invalid_token")
                        .with("token", format!("{{{name}:{arg}}}")));
                }
            }
            "date" | "exif" => {
                if let Some(arg) = arg
                    && jiff::fmt::strtime::format(arg, &jiff::Zoned::now()).is_err()
                {
                    return Err(PluginError::new("rename.invalid_token")
                        .with("token", format!("{{{name}:{arg}}}")));
                }
            }
            other => {
                return Err(
                    PluginError::new("rename.unknown_token").with("token", format!("{{{other}}}"))
                );
            }
        }
    }
    Ok(())
}

fn render_template(pattern: &str, ctx: &dyn Context) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (start, end, name, arg) in tokens(pattern) {
        out.push_str(&pattern[last..start]);
        let value = match name {
            "name" => ctx.original().stem.clone(),
            "ext" => ctx.original().ext.clone(),
            "parent" => ctx.parent().to_owned(),
            "n" => format_number(
                ctx.index() as i64 + 1,
                arg.and_then(|a| a.parse().ok()).unwrap_or(0),
            ),
            "date" | "exif" => {
                let time = if name == "exif" {
                    ctx.captured().or_else(|| ctx.modified())
                } else {
                    ctx.modified()
                };
                time.and_then(|t| jiff::fmt::strtime::format(arg.unwrap_or("%Y-%m-%d"), &t).ok())
                    .unwrap_or_default()
            }
            _ => String::new(),
        };
        out.push_str(&value);
        last = end;
    }
    out.push_str(&pattern[last..]);
    out
}

/// 依次应用全部规则
pub fn apply(rules: &[Compiled], ctx: &dyn Context) -> Name {
    let mut name = ctx.original().clone();
    for rule in rules {
        match rule {
            Compiled::Replace {
                pattern,
                replace,
                part,
            } => {
                name.apply(*part, |t| {
                    pattern.replace_all(t, replace.as_str()).into_owned()
                });
            }
            Compiled::Other(rule) => match rule {
                Rule::Insert {
                    text,
                    place,
                    index,
                    part,
                } => name.apply(*part, |t| insert_at(t, text, *place, *index)),
                Rule::Remove {
                    place,
                    index,
                    count,
                    part,
                } => name.apply(*part, |t| remove(t, *place, *index, *count)),
                Rule::Case { mode, part } => name.apply(*part, |t| change_case(t, *mode)),
                Rule::Number {
                    start,
                    step,
                    pad,
                    place,
                    separator,
                } => {
                    let value = start.saturating_add(step.saturating_mul(ctx.index() as i64));
                    let number = format_number(value, *pad);
                    name.stem = match place {
                        Place::End => format!("{}{separator}{number}", name.stem),
                        _ => format!("{number}{separator}{}", name.stem),
                    };
                }
                Rule::Template { pattern } => name.stem = render_template(pattern, ctx),
                Rule::Extension { mode, value } => {
                    name.ext = match mode {
                        ExtMode::Lower => name.ext.to_lowercase(),
                        ExtMode::Upper => name.ext.to_uppercase(),
                        ExtMode::Set => value.trim_start_matches('.').to_owned(),
                        ExtMode::Remove => String::new(),
                    }
                }
                Rule::Clean {
                    collapse_spaces,
                    spaces_to,
                    trim,
                    remove_illegal,
                } => name.apply(Part::Stem, |t| {
                    clean(t, *collapse_spaces, spaces_to, *trim, *remove_illegal)
                }),
                Rule::Replace { .. } => {}
            },
        }
    }
    name
}

#[cfg(test)]
#[path = "rules_test.rs"]
mod tests;
