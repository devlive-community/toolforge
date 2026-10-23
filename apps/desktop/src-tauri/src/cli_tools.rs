//! 设置页「命令行工具」：在 `~/.toolforge/bin` 生成 `tfp` 与 `tfp-<插件>`，指向安装包里的 tfp。
//! 只为声明了 CLI 接口的插件生成命令；安装或卸载插件后自动刷新已生成的命令。

use serde::Serialize;
use tauri::State;
use tf_core::{AppError, AppResult, PluginRegistry};
use tf_host::cli_links;
use tf_plugin_api::interface::Channel;

use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    /// 安装包里有 tfp（开发时需要先编译 toolforge-cli）
    pub available: bool,
    pub installed: bool,
    pub dir: Option<String>,
    pub commands: Vec<String>,
    /// 把目录加入 PATH 的命令
    pub path_hint: Option<String>,
}

/// 声明了 CLI 接口的已加载插件名
pub fn cli_plugins(registry: &PluginRegistry) -> Vec<String> {
    let mut out: Vec<String> = registry
        .manifests()
        .into_iter()
        .filter(|m| {
            m.functions
                .values()
                .any(|spec| spec.declaration().exposed(Channel::Cli).is_some())
        })
        .map(|m| m.id.rsplit('.').next().unwrap_or(&m.id).to_owned())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn sidecar() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(cli_links::sidecar_beside(&exe)).filter(|p| p.exists())
}

fn status() -> CliStatus {
    let dir = cli_links::bin_dir();
    CliStatus {
        available: sidecar().is_some(),
        installed: dir.as_deref().is_some_and(cli_links::installed),
        commands: dir.as_deref().map(cli_links::commands).unwrap_or_default(),
        path_hint: dir.as_deref().map(cli_links::path_hint),
        dir: dir.map(|d| d.to_string_lossy().into_owned()),
    }
}

fn write(registry: &PluginRegistry) -> AppResult<()> {
    let sidecar = sidecar().ok_or_else(|| AppError::new("cli.unavailable"))?;
    let dir = cli_links::bin_dir().ok_or_else(|| AppError::new("cli.unavailable"))?;
    cli_links::write(&dir, &sidecar, &cli_plugins(registry))?;
    Ok(())
}

/// 插件变化后刷新已生成的命令；没有生成过就什么也不做
pub fn refresh(registry: &PluginRegistry) {
    if cli_links::bin_dir().is_some_and(|dir| cli_links::installed(&dir))
        && let Err(err) = write(registry)
    {
        log::warn!("refreshing command line links failed: {}", err.code);
    }
}

#[tauri::command]
pub fn cli_status() -> CliStatus {
    status()
}

#[tauri::command]
pub fn cli_install(state: State<'_, AppState>) -> AppResult<CliStatus> {
    write(&state.plugins)?;
    Ok(status())
}

#[tauri::command]
pub fn cli_uninstall() -> AppResult<CliStatus> {
    if let Some(dir) = cli_links::bin_dir() {
        cli_links::remove(&dir)?;
    }
    Ok(status())
}

#[cfg(test)]
#[path = "cli_tools_test.rs"]
mod tests;
