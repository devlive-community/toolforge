//! 打字练习与闯关后端：逐字对照、错误尝试计数、准确率与速度。
//! 个人纪录与关卡进度由前端通过 usePluginState 持久化到 ToolForge SQLite，
//! 这里只做无状态计算；每次调用都把完整累计状态回传，宿主不保存任何会话。

mod drill;
mod fingerprint;
mod record;
mod score;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TypingPractice {
    manifest: Manifest,
}

impl Default for TypingPractice {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

/// 输入文本超过上限时多给的余量（允许少量打过头的字符，再多就是异常输入）
const TYPED_SLACK: usize = 200;

/// `score` 的入参。接口声明要求扁平标量（CLI/Agent 友好）；界面调用时 `session`
/// 可以直接传对象，CLI/Agent 则以 JSON 字符串传入，两者都由这里解析。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScoreArgs {
    target: String,
    /// 上一次返回的累计状态：对象（界面）或 JSON 字符串（命令行/Agent），缺省视为新局
    #[serde(default)]
    session: Option<Value>,
    /// 练习区当前全部文本
    typed: String,
    /// 上一次已提交的文本，缺省为空（新局）；新增尝试由两者的差异在这里计算
    #[serde(default)]
    prev: String,
    /// 是否由粘贴或拖放产生
    #[serde(default)]
    pasted: bool,
    /// 有效输入时长（毫秒），由前端扣除暂停时间后得到
    #[serde(default)]
    elapsed_ms: u64,
}

fn invalid_session(e: serde_json::Error) -> PluginError {
    PluginError::new("plugin.invalid_args").with("detail", e.to_string())
}

/// 把对象或 JSON 字符串形式的参数解析成结构；空值得到默认值
fn parse_loose<T: DeserializeOwned + Default>(raw: Option<Value>) -> PluginResult<T> {
    match raw {
        None | Some(Value::Null) => Ok(T::default()),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(T::default()),
        Some(Value::String(text)) => serde_json::from_str(&text).map_err(invalid_session),
        Some(value) => serde_json::from_value(value).map_err(invalid_session),
    }
}

/// 文本长度检查（按可见字符计）
fn check_len(text: &str, max: usize) -> PluginResult<usize> {
    let n = score::graphemes(text).len();
    if n > max {
        return Err(PluginError::new("typing.text_too_long").with("max", max.to_string()));
    }
    Ok(n)
}

/// 重练入参：易错字符、期望长度与随机种子（缺省时用固定种子，结果可复现）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DrillArgs {
    chars: String,
    #[serde(default = "default_drill_len")]
    len: usize,
    #[serde(default)]
    seed: u64,
}

fn default_drill_len() -> usize {
    200
}

/// 准备入参：练习文本
#[derive(Debug, Deserialize)]
struct PrepareArgs {
    text: String,
}

/// 准备一段练习文本：检查长度，切分可见字符供界面逐字显示，并给出内容指纹（纪录按指纹分桶）
fn prepare_call(args: Value) -> PluginResult<Value> {
    let args: PrepareArgs = parse_args(args)?;
    check_len(&args.text, score::MAX_TEXT)?;
    let graphemes = score::graphemes(&args.text);
    to_value(serde_json::json!({
        "fingerprint": fingerprint::fingerprint(&graphemes.concat()),
        "total": graphemes.len(),
        "graphemes": graphemes,
        "passAccuracy": score::PASS_ACCURACY,
        "maxText": score::MAX_TEXT,
    }))
}

fn drill_call(args: Value) -> PluginResult<Value> {
    let args: DrillArgs = parse_args(args)?;
    let text = drill::build(&args.chars, args.len, args.seed);
    to_value(serde_json::json!({ "text": text, "length": score::graphemes(&text).len() }))
}

fn score_call(args: Value) -> PluginResult<Value> {
    let args: ScoreArgs = parse_args(args)?;
    let total = check_len(&args.target, score::MAX_TEXT)?;
    check_len(&args.typed, total + TYPED_SLACK)?;
    check_len(&args.prev, total + TYPED_SLACK)?;
    let session: score::Session = parse_loose(args.session)?;
    let snapshot = score::Snapshot {
        prev: args.prev,
        typed: args.typed,
        pasted: args.pasted,
    };
    let (session, score) = score::apply(&args.target, &session, &snapshot, args.elapsed_ms);
    to_value(serde_json::json!({ "session": session, "score": score }))
}

/// 结算入参：已有纪录、本局纪录桶、是否内置关卡、本局成绩与完成时间
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettleArgs {
    #[serde(default)]
    bests: Option<Value>,
    bucket: String,
    #[serde(default)]
    stage: bool,
    round: record::Round,
    at: String,
}

fn settle_call(args: Value) -> PluginResult<Value> {
    let args: SettleArgs = parse_args(args)?;
    let bests: BTreeMap<String, record::Best> = parse_loose(args.bests)?;
    let outcome = record::settle(bests, &args.bucket, args.stage, &args.round, &args.at);
    to_value(outcome)
}

/// 后端分发的全部函数；宿主只允许调用 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 4] = ["score", "drill", "prepare", "settle"];

impl ToolPlugin for TypingPractice {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "score" => score_call(args),
            "drill" => drill_call(args),
            "prepare" => prepare_call(args),
            "settle" => settle_call(args),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TypingPractice);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
