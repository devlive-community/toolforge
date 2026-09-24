//! ToolForge 核心服务：错误码、SQLite 存储、插件注册表、耗时任务、Markdown 解析。

pub mod error;
pub mod markdown;
pub mod registry;
pub mod resources;
pub mod store;
pub mod task;

pub use error::{AppError, AppResult};
pub use registry::PluginRegistry;
pub use resources::{ResourceStatus, Resources};
pub use store::{Store, TaskRecord};
pub use task::{EventSink, LogLine, TaskEvent, TaskManager, TaskRunner, TaskStatus};
