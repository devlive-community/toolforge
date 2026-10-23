//! 逐字符分析：码位、名称、类别、文字系统、编码，以及不可见字符、双向控制符、形近字、特殊空格等风险标记。

use serde::{Deserialize, Serialize};
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

/// 最多逐个列出的字符数
pub const MAX_CHARS: usize = 20_000;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Flag {
    /// 零宽、格式控制等看不见的字符
    Invisible,
    /// 改变文字方向的控制符（Trojan Source）
    Bidi,
    /// 与 ASCII 字符外形相同的其他字符
    Confusable,
    /// 普通空格以外的空白
    Space,
    /// 控制字符（换行、制表符除外）
    Control,
    /// 组合用字符（附加在前一个字符上）
    Combining,
    /// 私用区或未分配的码位
    Unassigned,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharInfo {
    /// 字符序号（按 Unicode 标量计）
    pub index: usize,
    /// 在 UTF-16 字符串中的偏移，前端据此定位
    pub offset: usize,
    pub char: String,
    pub code: String,
    pub name: Option<String>,
    pub category: &'static str,
    pub script: String,
    pub utf8: String,
    pub utf16: String,
    pub flags: Vec<Flag>,
    /// 形近的 ASCII 字符
    pub lookalike: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub chars: usize,
    pub graphemes: usize,
    pub utf8_bytes: usize,
    pub utf16_units: usize,
    pub lines: usize,
    pub invisible: usize,
    pub bidi: usize,
    pub confusable: usize,
    pub space: usize,
    pub control: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Normalization {
    pub nfc: bool,
    pub nfd: bool,
    pub nfkc: bool,
    pub nfkd: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub counts: Counts,
    pub chars: Vec<CharInfo>,
    pub truncated: bool,
    /// 混用多种文字系统的单词，如拉丁字母中夹杂西里尔字母
    pub mixed_words: Vec<String>,
    pub normalization: Normalization,
}

pub fn code(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

pub fn name(c: char) -> Option<String> {
    unicode_names2::name(c)
        .map(|n| n.to_string())
        .or_else(|| match c as u32 {
            0x00..=0x1f | 0x7f..=0x9f => Some(control_name(c).to_owned()),
            0x4e00..=0x9fff | 0x3400..=0x4dbf | 0x20000..=0x3134f => {
                Some(format!("CJK UNIFIED IDEOGRAPH-{:X}", c as u32))
            }
            0xac00..=0xd7a3 => Some(format!("HANGUL SYLLABLE {:X}", c as u32)),
            _ => None,
        })
}

/// 控制字符没有正式名称，使用常见别名
fn control_name(c: char) -> &'static str {
    match c {
        '\0' => "NULL",
        '\t' => "CHARACTER TABULATION (TAB)",
        '\n' => "LINE FEED (LF)",
        '\r' => "CARRIAGE RETURN (CR)",
        '\u{1b}' => "ESCAPE",
        '\u{7f}' => "DELETE",
        '\u{85}' => "NEXT LINE (NEL)",
        _ => "CONTROL CHARACTER",
    }
}

pub fn category(c: char) -> &'static str {
    use GeneralCategory::*;
    match get_general_category(c) {
        UppercaseLetter => "Lu",
        LowercaseLetter => "Ll",
        TitlecaseLetter => "Lt",
        ModifierLetter => "Lm",
        OtherLetter => "Lo",
        NonspacingMark => "Mn",
        SpacingMark => "Mc",
        EnclosingMark => "Me",
        DecimalNumber => "Nd",
        LetterNumber => "Nl",
        OtherNumber => "No",
        ConnectorPunctuation => "Pc",
        DashPunctuation => "Pd",
        OpenPunctuation => "Ps",
        ClosePunctuation => "Pe",
        InitialPunctuation => "Pi",
        FinalPunctuation => "Pf",
        OtherPunctuation => "Po",
        MathSymbol => "Sm",
        CurrencySymbol => "Sc",
        ModifierSymbol => "Sk",
        OtherSymbol => "So",
        SpaceSeparator => "Zs",
        LineSeparator => "Zl",
        ParagraphSeparator => "Zp",
        Control => "Cc",
        Format => "Cf",
        Surrogate => "Cs",
        PrivateUse => "Co",
        // 枚举标记为 non_exhaustive
        _ => "Cn",
    }
}

pub fn is_bidi(c: char) -> bool {
    matches!(c as u32, 0x202a..=0x202e | 0x2066..=0x2069 | 0x200e | 0x200f | 0x061c)
}

/// 看不见的字符：格式控制符、变体选择符、标签字符与几个“空白字母”
pub fn is_invisible(c: char) -> bool {
    matches!(get_general_category(c), GeneralCategory::Format)
        || matches!(c as u32,
            0xfe00..=0xfe0f | 0xe0100..=0xe01ef | 0x034f | 0x115f | 0x1160 | 0x3164 | 0xffa0 | 0x2800 | 0x180b..=0x180f)
}

/// 零宽连接符与文本 / emoji 变体选择符
pub fn is_emoji_joiner(c: char) -> bool {
    matches!(c, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}')
}

/// emoji 序列中可能出现在连接符前面的字符：符号、肤色修饰、变体选择符
fn is_emoji_part(c: char) -> bool {
    get_general_category(c) == GeneralCategory::OtherSymbol
        || matches!(c as u32, 0x1f3fb..=0x1f3ff | 0xfe0f | 0x20e3 | 0x1f1e6..=0x1f1ff)
}

fn lookalike(c: char) -> Option<String> {
    if c.is_ascii() {
        return None;
    }
    let skeleton: String =
        unicode_security::confusable_detection::skeleton(&c.to_string()).collect();
    (skeleton != c.to_string()
        && !skeleton.is_empty()
        && skeleton.bytes().all(|b| b.is_ascii_graphic()))
    .then_some(skeleton)
}

pub fn flags(c: char) -> (Vec<Flag>, Option<String>) {
    let mut flags = Vec::new();
    let category = get_general_category(c);
    if is_bidi(c) {
        flags.push(Flag::Bidi);
    }
    // 双向控制符本身也看不见，但单独归类，避免重复计数
    if is_invisible(c) && !is_bidi(c) {
        flags.push(Flag::Invisible);
    }
    if category == GeneralCategory::Control && !matches!(c, '\n' | '\r' | '\t') {
        flags.push(Flag::Control);
    }
    if matches!(
        category,
        GeneralCategory::SpaceSeparator
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    ) && c != ' '
    {
        flags.push(Flag::Space);
    }
    if matches!(
        category,
        GeneralCategory::NonspacingMark | GeneralCategory::EnclosingMark
    ) && !is_invisible(c)
    {
        flags.push(Flag::Combining);
    }
    if matches!(
        category,
        GeneralCategory::Unassigned | GeneralCategory::PrivateUse
    ) {
        flags.push(Flag::Unassigned);
    }
    let lookalike = if flags.is_empty() || flags == [Flag::Combining] {
        lookalike(c)
    } else {
        None
    };
    if lookalike.is_some() {
        flags.push(Flag::Confusable);
    }
    (flags, lookalike)
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn utf8(c: char) -> String {
    let mut buffer = [0u8; 4];
    hex_bytes(c.encode_utf8(&mut buffer).as_bytes())
}

pub fn utf16(c: char) -> String {
    let mut buffer = [0u16; 2];
    c.encode_utf16(&mut buffer)
        .iter()
        .map(|u| format!("{u:04X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 单词中出现了不应混用的文字系统（日文、韩文与汉字混用是正常的）
fn mixed_scripts(word: &str) -> bool {
    let mut scripts: Vec<Script> = word
        .chars()
        .map(|c| c.script())
        .filter(|s| !matches!(s, Script::Common | Script::Inherited | Script::Unknown))
        .collect();
    scripts.sort_by_key(|s| s.full_name());
    scripts.dedup();
    let cjk = [
        Script::Han,
        Script::Hiragana,
        Script::Katakana,
        Script::Hangul,
        Script::Bopomofo,
    ];
    scripts.len() > 1 && !scripts.iter().all(|s| cjk.contains(s))
}

pub fn analyze(text: &str) -> Analysis {
    let mut counts = Counts {
        chars: text.chars().count(),
        graphemes: text.graphemes(true).count(),
        utf8_bytes: text.len(),
        utf16_units: text.encode_utf16().count(),
        lines: if text.is_empty() {
            0
        } else {
            text.lines().count().max(1)
        },
        ..Counts::default()
    };
    let mut chars = Vec::new();
    let mut offset = 0;
    let mut previous: Option<char> = None;
    for (index, c) in text.chars().enumerate() {
        let (mut flags, lookalike) = flags(c);
        // emoji 序列中的零宽连接符与变体选择符是正常组成部分，不需要提示
        if is_emoji_joiner(c) && previous.is_some_and(is_emoji_part) {
            flags.retain(|f| *f != Flag::Invisible);
        }
        previous = Some(c);
        for flag in &flags {
            match flag {
                Flag::Invisible => counts.invisible += 1,
                Flag::Bidi => counts.bidi += 1,
                Flag::Confusable => counts.confusable += 1,
                Flag::Space => counts.space += 1,
                Flag::Control => counts.control += 1,
                _ => {}
            }
        }
        if index < MAX_CHARS {
            chars.push(CharInfo {
                index,
                offset,
                char: c.to_string(),
                code: code(c),
                name: name(c),
                category: category(c),
                script: c.script().full_name().to_owned(),
                utf8: utf8(c),
                utf16: utf16(c),
                flags,
                lookalike,
            });
        }
        offset += c.len_utf16();
    }
    let mut mixed_words: Vec<String> = text
        .split(|c: char| {
            !(c.is_alphanumeric()
                || is_invisible(c)
                || get_general_category(c) == GeneralCategory::NonspacingMark)
        })
        .filter(|w| !w.is_empty() && mixed_scripts(w))
        .map(str::to_owned)
        .collect();
    mixed_words.dedup();
    mixed_words.truncate(50);
    Analysis {
        truncated: counts.chars > MAX_CHARS,
        normalization: Normalization {
            nfc: text.nfc().eq(text.chars()),
            nfd: text.nfd().eq(text.chars()),
            nfkc: text.nfkc().eq(text.chars()),
            nfkd: text.nfkd().eq(text.chars()),
        },
        counts,
        chars,
        mixed_words,
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    #[default]
    None,
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanOptions {
    #[serde(default = "yes")]
    pub remove_invisible: bool,
    #[serde(default = "yes")]
    pub remove_bidi: bool,
    #[serde(default = "yes")]
    pub normalize_spaces: bool,
    #[serde(default)]
    pub replace_confusables: bool,
    #[serde(default)]
    pub remove_controls: bool,
    #[serde(default)]
    pub form: Form,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleaned {
    pub text: String,
    pub removed: usize,
    pub replaced: usize,
}

/// 清理文本：删除不可见字符与双向控制符，把特殊空格换成普通空格，可选替换形近字与规范化
pub fn clean(text: &str, options: &CleanOptions) -> Cleaned {
    let mut out = String::with_capacity(text.len());
    let (mut removed, mut replaced) = (0, 0);
    for c in text.chars() {
        let (flags, lookalike) = flags(c);
        let has = |flag| flags.contains(&flag);
        // ZWJ 用于组合 emoji，保留在 emoji 序列中
        let emoji_joiner = c == '\u{200d}' || (0xfe0e..=0xfe0f).contains(&(c as u32));
        if (options.remove_bidi && has(Flag::Bidi))
            || (options.remove_invisible
                && has(Flag::Invisible)
                && !has(Flag::Bidi)
                && !emoji_joiner)
            || (options.remove_controls && has(Flag::Control))
        {
            removed += 1;
            continue;
        }
        if options.normalize_spaces && has(Flag::Space) {
            out.push(' ');
            replaced += 1;
            continue;
        }
        if options.replace_confusables
            && let Some(lookalike) = lookalike
        {
            out.push_str(&lookalike);
            replaced += 1;
            continue;
        }
        out.push(c);
    }
    let text = match options.form {
        Form::None => out,
        Form::Nfc => out.nfc().collect(),
        Form::Nfd => out.nfd().collect(),
        Form::Nfkc => out.nfkc().collect(),
        Form::Nfkd => out.nfkd().collect(),
    };
    Cleaned {
        text,
        removed,
        replaced,
    }
}

#[cfg(test)]
#[path = "analyze_test.rs"]
mod tests;
