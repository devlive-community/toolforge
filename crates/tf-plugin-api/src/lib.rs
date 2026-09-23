//! 宿主与插件之间的契约。
//!
//! 所有插件（包括内置插件）都只通过 [`ToolPlugin`] 与宿主交互；宿主不感知具体工具。
//! 目前内置插件以原生 crate 形式注册，后续 WASM 插件会实现同一个 trait 的适配层，
//! 因此宿主代码无需改动。

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 结构化错误：只携带错误码与参数，文案由前端按语言翻译。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginError {
    pub code: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
}

impl PluginError {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            params: Map::new(),
        }
    }

    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)?;
        if !self.params.is_empty() {
            write!(f, " {}", Value::Object(self.params.clone()))?;
        }
        Ok(())
    }
}

impl std::error::Error for PluginError {}

pub type PluginResult<T> = Result<T, PluginError>;

/// 函数声明：宿主据此决定是否以任务方式运行。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSpec {
    #[serde(default)]
    pub task: bool,
}

/// 插件清单，与插件目录中的 manifest.json 一一对应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub version: String,
    /// 形如 `i18n:name`，由前端在插件命名空间内翻译
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 图标底色 token：violet / blue / green / orange / pink / cyan
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub functions: IndexMap<String, FunctionSpec>,
    #[serde(default)]
    pub sensitive: bool,
}

impl Manifest {
    /// 解析内置插件的 manifest.json，格式错误属于开发期错误，直接 panic。
    pub fn from_static(source: &str) -> Self {
        serde_json::from_str(source).expect("invalid plugin manifest")
    }
}

/// 任务日志级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// 耗时任务的运行上下文，由宿主提供。
///
/// 日志只携带错误码风格的 `code` 与参数，文案由前端按语言翻译；
/// 文件只能通过宿主打开，便于后续在沙箱中做权限控制。
pub trait TaskContext: Send + Sync {
    fn log(&self, level: LogLevel, code: &str, params: Value);
    fn progress(&self, done: u64, total: u64);
    fn stage(&self, code: &str);
    fn is_cancelled(&self) -> bool;
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn std::io::Read + Send>>;
    fn file_size(&self, path: &str) -> PluginResult<u64>;
}

/// 插件后端需要实现的接口。
pub trait ToolPlugin: Send + Sync {
    fn manifest(&self) -> &Manifest;

    /// 调用插件函数，参数与返回值均为 JSON。
    fn call(&self, function: &str, args: Value) -> PluginResult<Value>;

    /// 以任务方式运行 manifest 中声明为 `task` 的函数；默认退化为普通调用。
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let _ = ctx;
        self.call(function, args)
    }
}

/// 任务被用户取消时返回的错误
pub fn cancelled() -> PluginError {
    PluginError::new("task.cancelled")
}

/// 把 JSON 参数反序列化为强类型结构。
pub fn parse_args<T: DeserializeOwned>(args: Value) -> PluginResult<T> {
    serde_json::from_value(args)
        .map_err(|e| PluginError::new("plugin.invalid_args").with("detail", e.to_string()))
}

/// 把返回值序列化为 JSON。
pub fn to_value<T: Serialize>(value: T) -> PluginResult<Value> {
    serde_json::to_value(value)
        .map_err(|e| PluginError::new("plugin.serialize_failed").with("detail", e.to_string()))
}

/// 未知函数的统一错误。
pub fn unknown_function(function: &str) -> PluginError {
    PluginError::new("plugin.function_not_found").with("function", function)
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
