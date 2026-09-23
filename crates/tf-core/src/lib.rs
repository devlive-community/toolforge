//! ToolForge 核心服务：错误码、SQLite 存储、插件注册表、耗时任务。

pub mod error;
pub mod registry;
pub mod store;
pub mod task;

pub use error::{AppError, AppResult};
pub use registry::PluginRegistry;
pub use store::{Store, TaskRecord};
pub use task::{EventSink, LogLine, TaskEvent, TaskManager, TaskRunner, TaskStatus};
