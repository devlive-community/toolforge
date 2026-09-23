//! ToolForge 核心服务：错误码、SQLite 存储。

pub mod error;
pub mod store;

pub use error::{AppError, AppResult};
pub use store::Store;
