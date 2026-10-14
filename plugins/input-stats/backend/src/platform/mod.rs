//! 各系统的输入采集、前台应用与显示器信息。
//!
//! 每个平台模块提供同样的函数：
//! - `support()`：当前环境能否采集，不能时返回原因码
//! - `permission()` / `request_permission()`：系统授权（只有 macOS 需要）
//! - `start(sink)`：启动采集线程，同步返回是否成功，返回值用于停止
//! - `foreground_app()`、`displays()`：由聚合线程每秒 / 每隔几秒调用

use std::sync::mpsc::Sender;

use crate::stats::Event;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
pub use windows::*;

/// 停止采集线程；调用后等待线程退出
pub type Stopper = Box<dyn FnOnce() + Send>;

/// 采集线程把事件发往聚合线程
pub type Sink = Sender<Event>;

/// 系统授权状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Granted,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Denied,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    NotRequired,
}

impl Permission {
    pub fn name(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::NotRequired => "not_required",
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod unsupported {
    use super::*;
    use crate::stats::{App, Display};
    use tf_plugin_api::{PluginError, PluginResult};

    pub fn support() -> Result<(), &'static str> {
        Err("unsupported")
    }

    pub fn permission() -> Permission {
        Permission::NotRequired
    }

    pub fn request_permission() {}

    pub fn start(_: Sink) -> PluginResult<Stopper> {
        Err(PluginError::new("input_stats.unsupported"))
    }

    pub fn foreground_app() -> Option<App> {
        None
    }

    pub fn displays() -> Vec<Display> {
        Vec::new()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub use unsupported::*;
