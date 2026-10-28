//! 打字练习与闯关后端：逐字对照、错误尝试计数、准确率与速度。
//! 个人纪录与关卡进度由前端通过 usePluginState 持久化到 ToolForge SQLite，
//! 这里只做无状态计算；每次调用都把完整累计状态回传，宿主不保存任何会话。

mod score;

use serde::Deserialize;
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
    /// 本次新增字符数
    #[serde(default)]
    added: u32,
    /// 本次新增字符中与目标位置不符的个数
    #[serde(default)]
    wrong_added: u32,
    /// 是否由粘贴产生
    #[serde(default)]
    pasted: bool,
    /// 有效输入时长（毫秒），由前端扣除暂停时间后得到
    #[serde(default)]
    elapsed_ms: u64,
}

fn invalid_session(e: serde_json::Error) -> PluginError {
    PluginError::new("plugin.invalid_args").with("detail", e.to_string())
}

fn parse_session(raw: Option<Value>) -> PluginResult<score::Session> {
    match raw {
        None | Some(Value::Null) => Ok(score::Session::default()),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(score::Session::default()),
        Some(Value::String(text)) => serde_json::from_str(&text).map_err(invalid_session),
        Some(value) => serde_json::from_value(value).map_err(invalid_session),
    }
}

fn score_call(args: Value) -> PluginResult<Value> {
    let args: ScoreArgs = parse_args(args)?;
    let session = parse_session(args.session)?;
    let snapshot = score::Snapshot {
        typed: args.typed,
        added: args.added,
        wrong_added: args.wrong_added,
        pasted: args.pasted,
    };
    let (session, score) = score::apply(&args.target, &session, &snapshot, args.elapsed_ms);
    to_value(serde_json::json!({ "session": session, "score": score }))
}

impl ToolPlugin for TypingPractice {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "score" => score_call(args),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TypingPractice);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
