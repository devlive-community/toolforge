//! 桌面端与命令行共用的宿主能力：已安装插件的读取、加载与版本校验，命令行入口链接。

pub mod agent;
pub mod cli_links;
pub mod history;
pub mod installed;
pub mod version;

use std::path::PathBuf;

/// 应用标识，与 tauri.conf.json 的 identifier 一致；应用数据目录以它命名
pub const APP_IDENTIFIER: &str = "org.devlive.toolforge";

/// 应用数据目录，与 Tauri 的 `app_data_dir()` 相同；不经过 Tauri 时（命令行）使用
pub fn app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
