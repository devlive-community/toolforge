//! 设置页「AI Agent」与插件详情的接口信息：授权写入 SQLite，`tfp mcp` 每次调用时读取。
//! 授权判断规则见 tf_host::agent。

use serde::Serialize;
use serde_json::{Value, json};
use tauri::State;
use tf_core::store::AGENT_PLUGIN_SCOPE;
use tf_core::{AgentGrant, AgentPath, AppError, AppResult};
use tf_host::agent::{ENABLED_KEY, Mode, Policy};
use tf_host::cli_links;
use tf_plugin_api::Manifest;
use tf_plugin_api::interface::{Channel, Effect, Interface};

use crate::AppState;

const FS_PERMISSION: &str = "fs:user-selected";

fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "read",
        Effect::Write => "write",
        Effect::External => "external",
    }
}

/// 一个函数的接口声明，供插件详情展示
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceInfo {
    pub function: String,
    /// `i18n:` 键，由前端在插件命名空间内翻译
    pub summary: String,
    pub channels: Vec<&'static str>,
    pub effect: &'static str,
    pub uses: Vec<String>,
    pub confirm: bool,
}

pub fn interfaces(manifest: &Manifest) -> Vec<InterfaceInfo> {
    manifest
        .functions
        .iter()
        .filter_map(|(name, spec)| {
            let declaration = spec.declaration();
            let interface = declaration
                .exposed(Channel::Cli)
                .or_else(|| declaration.exposed(Channel::Agent))?;
            let channels = [(Channel::Cli, "cli"), (Channel::Agent, "agent")]
                .into_iter()
                .filter(|(c, _)| declaration.exposed(*c).is_some())
                .map(|(_, n)| n)
                .collect();
            Some(InterfaceInfo {
                function: name.clone(),
                summary: interface.summary.clone(),
                channels,
                effect: effect_name(interface.effect),
                uses: interface.uses.clone(),
                confirm: interface.confirm,
            })
        })
        .collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFunction {
    pub name: String,
    pub summary: String,
    pub effect: &'static str,
    pub uses: Vec<String>,
    /// 实际生效的模式
    pub mode: Mode,
    /// 没有单独设置时的模式
    pub default_mode: Mode,
    /// 用户的设置；None 为按副作用的默认值
    pub custom: Option<Mode>,
    /// 声明要求确认或原生插件用到文件权限：不能设为始终允许
    pub capped: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlugin {
    pub id: String,
    pub sensitive: bool,
    pub enabled: bool,
    /// 原生内置插件，不在 WASM 沙箱中
    pub native: bool,
    pub functions: Vec<AgentFunction>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverview {
    pub enabled: bool,
    /// 客户端启动 MCP 服务的命令（安装包里的 tfp）；开发时未编译 tfp 则为空
    pub command: Option<String>,
    /// 写入 MCP 客户端配置的 JSON
    pub config: Option<String>,
    pub paths: Vec<AgentPath>,
    pub plugins: Vec<AgentPlugin>,
}

fn agent_interface(manifest: &Manifest, function: &str) -> Option<Interface> {
    let declaration = manifest.functions.get(function)?.declaration();
    declaration.exposed(Channel::Agent).cloned()
}

pub fn overview(state: &AppState) -> AppResult<AgentOverview> {
    let policy = Policy::load(&state.store)?;
    let grants = state.store.agent_grants()?;
    let mut plugins: Vec<AgentPlugin> = Vec::new();
    for manifest in state.plugins.manifests() {
        let native = state.builtin.contains(&manifest.id);
        let functions: Vec<AgentFunction> = manifest
            .functions
            .keys()
            .filter_map(|name| {
                let interface = agent_interface(&manifest, name)?;
                let custom = grants
                    .iter()
                    .find(|g| g.plugin_id == manifest.id && g.function == *name)
                    .and_then(|g| Mode::parse(&g.mode));
                let capped = interface.confirm
                    || (native && interface.uses.iter().any(|u| u == FS_PERMISSION));
                let default_mode = if interface.effect == Effect::Read && !capped {
                    Mode::Allow
                } else {
                    Mode::Ask
                };
                Some(AgentFunction {
                    name: name.clone(),
                    summary: interface.summary.clone(),
                    effect: effect_name(interface.effect),
                    mode: policy.function_mode(&manifest.id, name, &interface, native),
                    default_mode,
                    custom,
                    capped,
                    uses: interface.uses,
                })
            })
            .collect();
        if functions.is_empty() {
            continue;
        }
        plugins.push(AgentPlugin {
            enabled: policy.plugin_enabled(&manifest.id),
            id: manifest.id,
            sensitive: manifest.sensitive,
            native,
            functions,
        });
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    let command = std::env::current_exe()
        .ok()
        .map(|exe| cli_links::sidecar_beside(&exe))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned());
    let config = command.as_ref().map(|command| {
        let value =
            json!({ "mcpServers": { "toolforge": { "command": command, "args": ["mcp"] } } });
        serde_json::to_string_pretty(&value).unwrap_or_default()
    });
    Ok(AgentOverview {
        enabled: policy.enabled(),
        command,
        config,
        paths: state.store.agent_paths()?,
        plugins,
    })
}

#[tauri::command]
pub fn plugin_interfaces(
    state: State<'_, AppState>,
    plugin_id: String,
) -> AppResult<Vec<InterfaceInfo>> {
    Ok(interfaces(state.plugins.get(&plugin_id)?.manifest()))
}

#[tauri::command]
pub fn agent_overview(state: State<'_, AppState>) -> AppResult<AgentOverview> {
    overview(&state)
}

#[tauri::command]
pub fn agent_set_enabled(state: State<'_, AppState>, enabled: bool) -> AppResult<AgentOverview> {
    state.store.kv_set(ENABLED_KEY, &Value::Bool(enabled))?;
    overview(&state)
}

#[tauri::command]
pub fn agent_set_plugin(
    state: State<'_, AppState>,
    plugin_id: String,
    enabled: bool,
) -> AppResult<AgentOverview> {
    state.plugins.get(&plugin_id)?;
    if enabled {
        state.store.agent_grant_set(&AgentGrant {
            plugin_id,
            function: AGENT_PLUGIN_SCOPE.to_owned(),
            mode: Mode::Allow.as_str().to_owned(),
            uses: None,
        })?;
    } else {
        state
            .store
            .agent_grant_remove(&plugin_id, AGENT_PLUGIN_SCOPE)?;
    }
    overview(&state)
}

/// mode 为 None 时恢复按副作用的默认值；授权时记下函数当前声明的权限
#[tauri::command]
pub fn agent_set_function(
    state: State<'_, AppState>,
    plugin_id: String,
    function: String,
    mode: Option<String>,
) -> AppResult<AgentOverview> {
    let plugin = state.plugins.get(&plugin_id)?;
    let interface = agent_interface(plugin.manifest(), &function).ok_or_else(|| {
        AppError::new("plugin.function_not_found").with("function", function.as_str())
    })?;
    match mode {
        None => state.store.agent_grant_remove(&plugin_id, &function)?,
        Some(mode) => {
            let mode = Mode::parse(&mode).ok_or_else(|| AppError::new("agent.invalid_mode"))?;
            state.store.agent_grant_set(&AgentGrant {
                plugin_id,
                function,
                mode: mode.as_str().to_owned(),
                uses: Some(interface.uses),
            })?;
        }
    }
    overview(&state)
}

#[tauri::command]
pub fn agent_add_path(
    state: State<'_, AppState>,
    path: String,
    write: bool,
) -> AppResult<AgentOverview> {
    let real = std::path::Path::new(&path).canonicalize()?;
    if !real.is_dir() {
        return Err(AppError::new("fs.not_a_directory").with("path", path));
    }
    state.store.agent_path_set(&AgentPath {
        path: real.to_string_lossy().into_owned(),
        access: if write { "write" } else { "read" }.to_owned(),
    })?;
    overview(&state)
}

#[tauri::command]
pub fn agent_remove_path(state: State<'_, AppState>, path: String) -> AppResult<AgentOverview> {
    state.store.agent_path_remove(&path)?;
    overview(&state)
}

#[cfg(test)]
#[path = "agent_test.rs"]
mod tests;
