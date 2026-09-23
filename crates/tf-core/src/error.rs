use serde::Serialize;
use serde_json::{Map, Value};
use tf_plugin_api::PluginError;

/// 返回给前端的错误：只有错误码和参数，文案由前端 i18n 翻译。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppError {
    pub code: String,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
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

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl std::error::Error for AppError {}

impl From<PluginError> for AppError {
    fn from(err: PluginError) -> Self {
        Self {
            code: err.code,
            params: err.params,
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::new("store.failed").with("detail", err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        let code = match err.kind() {
            std::io::ErrorKind::NotFound => "fs.not_found",
            std::io::ErrorKind::PermissionDenied => "fs.permission_denied",
            _ => "fs.io",
        };
        AppError::new(code).with("detail", err.to_string())
    }
}
