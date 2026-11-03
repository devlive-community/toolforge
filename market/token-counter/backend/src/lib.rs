//! Token 计量与成本估算插件后端。
//!
//! 计数用启发式近似（无精确分词器依赖）：拉丁文按词与标点切分、CJK 按字、
//! 数字串按 3 位一组；不同模型给出不同系数并在结果中标注「近似」。误差说明
//! 与单价更新由界面呈现/用户维护，文本不出本机。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TokenCounter {
    manifest: Manifest,
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TokenCounter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "count" => to_value(count(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TokenCounter);

/// 模型的近似系数与上下文窗口；单价在请求里给（由用户维护），Rust 只负责计算。
/// 系数按公开资料的经验值（GPT 系 ~4 字符/token 的启发式），标注 approx。
const MODELS: &[(&str, f64, u64, &str)] = &[
    // (模型 id, tokens-per-CJK-char, 上下文窗口, 说明键)
    ("gpt-4o", 0.75, 128_000, "approxGpt"),
    ("gpt-4o-mini", 0.75, 128_000, "approxGpt"),
    ("gpt-4.1", 0.75, 1_000_000, "approxGpt"),
    ("o3", 0.8, 200_000, "approxGpt"),
    ("claude-sonnet", 0.8, 200_000, "approxClaude"),
    ("claude-haiku", 0.8, 200_000, "approxClaude"),
    ("gemini-pro", 0.85, 1_000_000, "approxGemini"),
    ("deepseek-chat", 0.6, 128_000, "approxGeneric"),
    ("qwen-max", 0.6, 131_072, "approxGeneric"),
    ("generic", 0.7, 128_000, "approxGeneric"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub text: String,
    /// MODELS 中的模型 id；未知模型按 generic 处理
    #[serde(default)]
    pub model: Option<String>,
    /// 输入单价（每百万 token，用户维护）
    #[serde(default)]
    pub input_price: Option<f64>,
    /// 输出单价（每百万 token）
    #[serde(default)]
    pub output_price: Option<f64>,
    /// 预计输出 token 数（成本估算用）
    #[serde(default)]
    pub expected_output: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub model: String,
    /// 说明键（前端翻译，含「近似估算」提示）
    pub method: &'static str,
    pub tokens: u64,
    pub characters: usize,
    /// 无空格字符数（中文场景常看这个）
    pub characters_no_spaces: usize,
    /// 粗略分项：CJK 字符数、拉丁词数、数字组数
    pub cjk_chars: u64,
    pub latin_words: u64,
    pub digit_groups: u64,
    pub context_window: u64,
    /// 窗口占用比例（0–1）
    pub window_usage: f64,
    pub cost: Option<Cost>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cost {
    /// 输入成本（单价 × tokens / 1e6）
    pub input: f64,
    /// 输出成本（按 expectedOutput，缺省按 tokens 同量估算）
    pub output: f64,
    pub total: f64,
}

/// 启发式 token 估算：CJK 每字 ~0.6–0.85 token，拉丁 ~0.25 token/字符（词 + 标点），
/// 数字每 3 位约 1 token。各模型系数略有差异。
fn estimate(text: &str, per_cjk: f64) -> (u64, u64, u64, u64) {
    let mut cjk = 0u64;
    let mut latin_chars = 0u64;
    let mut digits = 0u64;
    let mut in_word = false;
    let mut words = 0u64;
    for ch in text.chars() {
        if is_cjk(ch) {
            cjk += 1;
            in_word = false;
        } else if ch.is_ascii_digit() {
            digits += 1;
            in_word = false;
        } else if ch.is_alphanumeric() {
            latin_chars += 1;
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if !ch.is_whitespace() {
                // 标点与符号按 0.5 token 摊进拉丁部分
                latin_chars += 1;
            }
        }
    }
    // CJK 按系数，拉丁按 ~4 字符/token + 词边界修正，数字 3 位一组
    let tokens = (cjk as f64 * per_cjk)
        + (latin_chars as f64 / 4.0)
        + (words as f64 * 0.06)
        + (digits as f64 / 3.0).ceil();
    (
        tokens.ceil() as u64,
        cjk,
        words,
        (digits as f64 / 3.0).ceil() as u64,
    )
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0x3000..=0x303F | 0xFF00..=0xFFEF)
}

pub fn count(args: Args) -> PluginResult<Output> {
    if args.text.is_empty() {
        return Err(PluginError::new("token.empty_input"));
    }
    let key = args.model.as_deref().unwrap_or("generic");
    let found = MODELS.iter().find(|(id, ..)| *id == key);
    // 未知模型按 generic 系数处理但保留用户输入的名称
    let (per_cjk, window, method) = match found {
        Some((_, cjk, win, method)) => (*cjk, *win, *method),
        None => (0.7, 128_000, "approxGeneric"),
    };
    let (tokens, cjk, words, digit_groups) = estimate(&args.text, per_cjk);
    let usage = tokens as f64 / window as f64;
    let cost = args.input_price.map(|input_price| {
        let output_price = args.output_price.unwrap_or(input_price);
        let output_tokens = args.expected_output.unwrap_or(tokens) as f64;
        let input = input_price * tokens as f64 / 1_000_000.0;
        let output = output_price * output_tokens / 1_000_000.0;
        Cost {
            input,
            output,
            total: input + output,
        }
    });
    Ok(Output {
        model: key.to_owned(),
        method,
        tokens,
        characters: args.text.chars().count(),
        characters_no_spaces: args.text.chars().filter(|c| !c.is_whitespace()).count(),
        cjk_chars: cjk,
        latin_words: words,
        digit_groups,
        context_window: window,
        window_usage: usage,
        cost,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
