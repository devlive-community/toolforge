//! ToolForge 核心服务：错误码、SQLite 存储、插件注册表。

pub mod error;
pub mod registry;
pub mod store;

pub use error::{AppError, AppResult};
pub use registry::PluginRegistry;
pub use store::Store;
